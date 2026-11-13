//! The native part of the standard library: everything the bytecode calls that
//! is not a plain instruction, plus display, deep equality, number formatting and JSON.
//!
//! Rules for every function here: nothing existing is changed before the function
//! is sure to succeed (the VM may collect garbage and run the instruction again),
//! and long operations report their cost in `core.work`.

use crate::builtins::{ALL, B, BUILTINS};
use crate::compile::Program;
use crate::heap::{bad, err, Heap, KMap, Obj, Trap, Value, NONE};
use crate::vm::{seed_rng, Core};
use std::fmt::Write;

const MAX_DEPTH: u32 = 200;
const MAX_STEPS: u64 = 100_000_000;
/// Added to a failed `assert_eq` whose two sides print the same.
const ROUNDING_NOTE: &str =
    " (the two differ only beyond the digits shown; for calculated numbers use `assert(approx(a, b, tolerance))`)";

// Variant ids of the prelude enums (the checker verifies their order).
const T_NONE: u32 = 0;
const T_SOME: u32 = 1;
const T_OK: u32 = 2;
const T_ERR: u32 = 3;
const J_NULL: u32 = 4;
const J_BOOL: u32 = 5;
const J_NUM: u32 = 6;
const J_STR: u32 = 7;
const J_LIST: u32 = 8;
const J_OBJ: u32 = 9;

// ---------------------------------------------------------------- small helpers

#[cold]
fn overflow() -> Trap {
    Trap::Error("integer overflow".to_string())
}

fn int(v: Value) -> Result<i64, Trap> {
    match v {
        Value::Int(i) => Ok(i),
        _ => Err(bad()),
    }
}

fn float(v: Value) -> Result<f64, Trap> {
    match v {
        Value::Float(f) => Ok(f),
        Value::Int(i) => Ok(i as f64),
        _ => Err(bad()),
    }
}

fn fl(x: f64) -> Result<Value, Trap> {
    Ok(Value::Float(x))
}

/// The numbers of a list of Int or Float, for `mean`, `median` and `stddev`.
fn numbers(heap: &Heap, list: Value, name: &str, least: usize) -> Result<Vec<f64>, Trap> {
    let items = heap.list_of(list)?;
    if items.len() < least {
        let need = if least == 1 { "at least one value" } else { "at least two values" };
        let got = if items.is_empty() { "an empty list".to_string() } else { format!("a list of {}", items.len()) };
        return err(format!("{} needs {}, got {}", name, need, got));
    }
    items.iter().map(|v| float(*v)).collect()
}

/// A sum that carries a running correction (Neumaier), so small terms next to large ones are not lost.
fn careful_sum(values: impl Iterator<Item = f64>) -> f64 {
    let (mut total, mut lost) = (0.0f64, 0.0f64);
    for x in values {
        let next = total + x;
        lost += if total.abs() >= x.abs() { (total - next) + x } else { (x - next) + total };
        total = next;
    }
    if total.is_finite() {
        total + lost
    } else {
        total
    }
}

fn mean_of(xs: &[f64]) -> f64 {
    let n = xs.len() as f64;
    let mean = careful_sum(xs.iter().copied()) / n;
    if mean.is_finite() {
        return mean;
    }
    // The plain sum went past the largest float; dividing first keeps it in range.
    careful_sum(xs.iter().map(|x| x / n))
}

fn option(heap: &mut Heap, v: Option<Value>) -> Result<Value, Trap> {
    match v {
        Some(v) => heap.alloc(Obj::Enum(T_SOME, Box::new([v]))),
        None => Ok(Value::Tag(T_NONE)),
    }
}

/// Bytes that still fit under the memory limit.
fn room(heap: &Heap) -> usize {
    heap.limit.saturating_sub(heap.bytes)
}

/// A count given by the program: negative means zero, absurdly large means out of memory.
fn count(n: i64) -> Result<usize, Trap> {
    if n <= 0 {
        return Ok(0);
    }
    usize::try_from(n).map_err(|_| Trap::NeedGc)
}

/// A list index that may be negative (counting from the end).
fn index(i: i64, n: usize) -> Option<usize> {
    let j = if i < 0 { i.checked_add(n as i64)? } else { i };
    if j < 0 || j >= n as i64 {
        return None;
    }
    Some(j as usize)
}

/// A slice bound, clamped to `0..=n`.
fn bound(i: i64, n: usize) -> usize {
    let j = if i < 0 { i.saturating_add(n as i64) } else { i };
    j.clamp(0, n as i64) as usize
}

/// Byte offset of the character with index `chars`.
fn byte_at(s: &str, chars: usize) -> usize {
    if s.is_ascii() {
        return chars.min(s.len());
    }
    s.char_indices().nth(chars).map_or(s.len(), |(i, _)| i)
}

pub fn div_floor(x: i64, y: i64) -> Result<i64, Trap> {
    if y == 0 {
        return err("division by zero");
    }
    let q = x.checked_div(y).ok_or_else(overflow)?;
    if x % y != 0 && ((x < 0) != (y < 0)) {
        return Ok(q - 1);
    }
    Ok(q)
}

pub fn mod_floor(x: i64, y: i64) -> Result<i64, Trap> {
    if y == 0 {
        return err("modulo by zero");
    }
    if y == -1 {
        return Ok(0);
    }
    let r = x % y;
    if r != 0 && ((r < 0) != (y < 0)) {
        return Ok(r + y);
    }
    Ok(r)
}

pub fn ipow(x: i64, n: i64) -> Result<i64, Trap> {
    if n < 0 {
        return err("an Int cannot be raised to a negative power (use a Float base, e.g. 2.0 ^ -1)");
    }
    match u32::try_from(n) {
        Ok(n) => x.checked_pow(n).ok_or_else(overflow),
        Err(_) => match x {
            0 | 1 => Ok(x),
            -1 => Ok(if n % 2 == 0 { 1 } else { -1 }),
            _ => Err(overflow()),
        },
    }
}

// ---------------------------------------------------------------- numbers as text

/// The default text of a float: 12 significant digits, always with a decimal point.
pub fn fmt_float(x: f64, out: &mut String) {
    if x.is_nan() {
        out.push_str("nan");
        return;
    }
    if x.is_infinite() {
        out.push_str(if x > 0.0 { "inf" } else { "-inf" });
        return;
    }
    if x == 0.0 {
        out.push_str("0.0");
        return;
    }
    let sci = format!("{:.11e}", x);
    let (mantissa, exponent) = sci.split_once('e').unwrap_or((&sci, "0"));
    let exp: i32 = exponent.parse().unwrap_or(0);
    let all: String = mantissa.chars().filter(|c| c.is_ascii_digit()).collect();
    let trimmed = all.trim_end_matches('0');
    let digits = if trimmed.is_empty() { "0" } else { trimmed };
    if x < 0.0 {
        out.push('-');
    }
    if !(-5..16).contains(&exp) {
        out.push_str(&digits[..1]);
        if digits.len() > 1 {
            out.push('.');
            out.push_str(&digits[1..]);
        }
        let _ = write!(out, "e{}", exp);
    } else if exp < 0 {
        out.push_str("0.");
        for _ in 0..(-exp - 1) {
            out.push('0');
        }
        out.push_str(digits);
    } else {
        let whole = exp as usize + 1;
        if digits.len() <= whole {
            out.push_str(digits);
            for _ in digits.len()..whole {
                out.push('0');
            }
            out.push_str(".0");
        } else {
            out.push_str(&digits[..whole]);
            out.push('.');
            out.push_str(&digits[whole..]);
        }
    }
}

fn show_float(x: f64) -> String {
    let mut s = String::new();
    fmt_float(x, &mut s);
    s
}

/// `x` with exactly `n` decimals. The shortest decimal form of `x` is rounded half
/// away from zero, so 2.675 gives 2.68 and 1.005 gives 1.01, as on a calculator.
pub fn fixed(x: f64, n: usize) -> String {
    if x.is_nan() {
        return "nan".to_string();
    }
    if x.is_infinite() {
        return if x > 0.0 { "inf" } else { "-inf" }.to_string();
    }
    let text = format!("{}", x.abs());
    let (whole, frac) = text.split_once('.').unwrap_or((&text, ""));
    let frac = frac.as_bytes();
    let mut digits: Vec<u8> = Vec::with_capacity(whole.len() + n + 1);
    digits.extend_from_slice(whole.as_bytes());
    let mut point = digits.len();
    for i in 0..n {
        digits.push(*frac.get(i).unwrap_or(&b'0'));
    }
    if frac.len() > n && frac[n] >= b'5' {
        let mut i = digits.len();
        loop {
            if i == 0 {
                digits.insert(0, b'1');
                point += 1;
                break;
            }
            i -= 1;
            if digits[i] == b'9' {
                digits[i] = b'0';
            } else {
                digits[i] += 1;
                break;
            }
        }
    }
    let mut out = String::with_capacity(digits.len() + 2);
    if x < 0.0 && digits.iter().any(|d| *d != b'0') {
        out.push('-');
    }
    for (i, d) in digits.iter().enumerate() {
        if i == point {
            out.push('.');
        }
        out.push(*d as char);
    }
    out
}

fn round_to(x: f64, n: i64) -> f64 {
    if !x.is_finite() || n > 320 {
        return x;
    }
    if n < 0 {
        let p = 10f64.powi((-n).min(400) as i32);
        return (x / p).round() * p;
    }
    fixed(x, n as usize).parse().unwrap_or(x)
}

fn to_int(x: f64) -> Result<Value, Trap> {
    if !x.is_finite() {
        return err(format!("{} cannot be turned into an Int", show_float(x)));
    }
    if x >= 9_223_372_036_854_775_808.0 || x < -9_223_372_036_854_775_808.0 {
        return err(format!("{} is too large for an Int", show_float(x)));
    }
    Ok(Value::Int(x as i64))
}

// ---------------------------------------------------------------- display

struct Show<'a> {
    heap: &'a Heap,
    prog: &'a Program,
    out: String,
    cap: usize,
}

impl<'a> Show<'a> {
    fn new(core: &'a Core) -> Show<'a> {
        Show { heap: &core.heap, prog: &core.prog, out: String::new(), cap: room(&core.heap) }
    }

    fn func(&mut self, f: u32) {
        match self.prog.fns.get(f as usize) {
            Some(m) if !m.name.starts_with('<') => {
                let _ = write!(self.out, "<fn {}>", m.name);
            }
            _ => self.out.push_str("<fn>"),
        }
    }

    fn variant(&mut self, tag: u32) {
        match self.prog.variants.get(tag as usize) {
            Some(v) => self.out.push_str(&v.0),
            None => self.out.push('?'),
        }
    }

    fn quote(&mut self, s: &str) {
        self.out.push('"');
        for c in s.chars() {
            match c {
                '"' => self.out.push_str("\\\""),
                '\\' => self.out.push_str("\\\\"),
                '\n' => self.out.push_str("\\n"),
                '\r' => self.out.push_str("\\r"),
                '\t' => self.out.push_str("\\t"),
                _ => self.out.push(c),
            }
        }
        self.out.push('"');
    }

    fn seq(&mut self, items: &[Value], depth: u32) -> Result<(), Trap> {
        for (i, v) in items.iter().enumerate() {
            if i > 0 {
                self.out.push_str(", ");
            }
            self.value(*v, depth + 1, true)?;
        }
        Ok(())
    }

    /// `quoted`: show text in quotes (used inside lists, structs and so on).
    fn value(&mut self, v: Value, depth: u32, quoted: bool) -> Result<(), Trap> {
        if self.out.len() > self.cap {
            return Err(Trap::NeedGc);
        }
        match v {
            Value::Unit => self.out.push_str("()"),
            Value::Bool(b) => self.out.push_str(if b { "true" } else { "false" }),
            Value::Int(i) => push_int(&mut self.out, i),
            Value::Float(f) => fmt_float(f, &mut self.out),
            Value::Fn(f) => self.func(f),
            Value::Tag(t) => self.variant(t),
            Value::Uninit => self.out.push_str("<uninit>"),
            Value::Obj(i) => {
                let heap = self.heap;
                let prog = self.prog;
                let Some(obj) = heap.objs.get(i as usize) else { return Err(bad()) };
                if depth > MAX_DEPTH {
                    return err("the value is nested too deeply to display");
                }
                match obj {
                    Obj::Str(s) => {
                        if self.out.len().saturating_add(s.len()) > self.cap {
                            return Err(Trap::NeedGc);
                        }
                        if quoted {
                            self.quote(s);
                        } else {
                            self.out.push_str(s);
                        }
                    }
                    Obj::List(items) => {
                        self.out.push('[');
                        self.seq(items, depth)?;
                        self.out.push(']');
                    }
                    Obj::Map(m) => {
                        if m.len() == 0 {
                            self.out.push_str("[:]");
                        } else {
                            self.out.push('[');
                            for (n, (k, v)) in m.iter().enumerate() {
                                if n > 0 {
                                    self.out.push_str(", ");
                                }
                                self.value(k, depth + 1, true)?;
                                self.out.push_str(": ");
                                self.value(v, depth + 1, true)?;
                            }
                            self.out.push(']');
                        }
                    }
                    Obj::Struct(ty, fields) => {
                        if *ty == NONE {
                            self.out.push('(');
                            self.seq(fields, depth)?;
                            self.out.push(')');
                        } else {
                            let known = prog.structs.get(*ty as usize);
                            self.out.push_str(known.map_or("?", |s| s.0.as_str()));
                            self.out.push('(');
                            for (n, v) in fields.iter().enumerate() {
                                if n > 0 {
                                    self.out.push_str(", ");
                                }
                                if let Some(name) = known.and_then(|s| s.1.get(n)) {
                                    self.out.push_str(name);
                                    self.out.push_str(": ");
                                }
                                self.value(*v, depth + 1, true)?;
                            }
                            self.out.push(')');
                        }
                    }
                    Obj::Enum(tag, fields) => {
                        self.variant(*tag);
                        if !fields.is_empty() {
                            self.out.push('(');
                            self.seq(fields, depth)?;
                            self.out.push(')');
                        }
                    }
                    Obj::Closure(f, _) => self.func(*f),
                    Obj::Free(_) => return Err(bad()),
                }
            }
        }
        Ok(())
    }
}

/// How many characters `i` takes in decimal.
fn int_len(i: i64) -> usize {
    let mut n = i.unsigned_abs();
    let mut len = if i < 0 { 2 } else { 1 };
    while n >= 10 {
        n /= 10;
        len += 1;
    }
    len
}

/// Appends `i` in decimal (much cheaper than the general formatter).
fn push_int(out: &mut String, i: i64) {
    let mut digits = [0u8; 20];
    let mut at = digits.len();
    let mut n = i.unsigned_abs();
    loop {
        at -= 1;
        digits[at] = b'0' + (n % 10) as u8;
        n /= 10;
        if n == 0 {
            break;
        }
    }
    if i < 0 {
        at -= 1;
        digits[at] = b'-';
    }
    for d in &digits[at..] {
        out.push(*d as char);
    }
}

/// The total length when every part is text or a whole number.
fn plain_len(heap: &Heap, parts: &[Value]) -> Option<usize> {
    let mut total = 0usize;
    for p in parts {
        let n = match *p {
            Value::Int(i) => int_len(i),
            Value::Obj(_) => heap.str_of(*p).ok()?.len(),
            _ => return None,
        };
        total = total.saturating_add(n);
    }
    Some(total)
}

fn display(core: &Core, v: Value, quoted: bool) -> Result<String, Trap> {
    let mut w = Show::new(core);
    w.value(v, 0, quoted)?;
    Ok(w.out)
}

/// String interpolation: text parts as they are, everything else as displayed.
pub fn str_cat(core: &mut Core, parts: &[Value]) -> Result<Value, Trap> {
    if let [one] = parts {
        if core.heap.str_of(*one).is_ok() {
            return Ok(*one);
        }
    }
    if let Some(total) = plain_len(&core.heap, parts) {
        // Only text and whole numbers: build it in one buffer of exactly the right size.
        core.heap.check(total)?;
        let mut text = String::with_capacity(total);
        for p in parts {
            match *p {
                Value::Int(i) => push_int(&mut text, i),
                _ => text.push_str(core.heap.str_of(*p)?),
            }
        }
        core.work += (total / 64) as u64;
        return core.heap.str(text);
    }
    let text = {
        let mut w = Show::new(core);
        for p in parts {
            w.value(*p, 0, false)?;
        }
        w.out
    };
    core.work += (text.len() / 64) as u64;
    core.heap.str(text)
}

fn fmt(core: &mut Core, v: Value, width: i64, prec: i64, align: i64) -> Result<Value, Trap> {
    let numeric = matches!(v, Value::Int(_) | Value::Float(_));
    let mut text = if prec >= 0 {
        let p = prec.min(100) as usize;
        match v {
            Value::Int(i) => fixed(i as f64, p),
            Value::Float(f) => fixed(f, p),
            _ if core.heap.str_of(v).is_ok() => core.heap.str_of(v)?.chars().take(p).collect(),
            _ => display(core, v, false)?,
        }
    } else {
        display(core, v, false)?
    };
    let width = width.clamp(0, 10_000) as usize;
    let n = text.chars().count();
    if n < width {
        let pad = " ".repeat(width - n);
        let right = align == 2 || (align == 0 && numeric);
        text = if right { pad + &text } else { text + &pad };
    }
    core.work += (text.len() / 64) as u64;
    core.heap.str(text)
}

// ---------------------------------------------------------------- equality and order

/// Deep equality.
pub fn equal(core: &mut Core, a: Value, b: Value) -> Result<bool, Trap> {
    let mut steps = 0u64;
    let result = eq(&core.heap, a, b, 0, &mut steps);
    core.work += steps / 8;
    result
}

fn eq_all(heap: &Heap, p: &[Value], q: &[Value], depth: u32, steps: &mut u64) -> Result<bool, Trap> {
    if p.len() != q.len() {
        return Ok(false);
    }
    for (x, y) in p.iter().zip(q) {
        if !eq(heap, *x, *y, depth + 1, steps)? {
            return Ok(false);
        }
    }
    Ok(true)
}

fn eq(heap: &Heap, a: Value, b: Value, depth: u32, steps: &mut u64) -> Result<bool, Trap> {
    *steps += 1;
    if *steps > MAX_STEPS {
        return err("the values are too large to compare");
    }
    match (a, b) {
        (Value::Obj(i), Value::Obj(j)) => {
            if i == j {
                return Ok(true);
            }
            if depth > MAX_DEPTH {
                return err("the values are nested too deeply to compare");
            }
            let (Some(x), Some(y)) = (heap.objs.get(i as usize), heap.objs.get(j as usize)) else { return Err(bad()) };
            match (x, y) {
                (Obj::Str(p), Obj::Str(q)) => {
                    *steps += (p.len() / 16) as u64;
                    Ok(p == q)
                }
                (Obj::List(p), Obj::List(q)) => eq_all(heap, p, q, depth, steps),
                (Obj::Struct(t, p), Obj::Struct(u, q)) | (Obj::Enum(t, p), Obj::Enum(u, q)) => {
                    Ok(t == u && eq_all(heap, p, q, depth, steps)?)
                }
                (Obj::Map(p), Obj::Map(q)) => {
                    if p.len() != q.len() {
                        return Ok(false);
                    }
                    for (k, v) in p.iter() {
                        match q.get(&heap.objs, k, heap.hash_key(k)?) {
                            Some(w) => {
                                if !eq(heap, v, w, depth + 1, steps)? {
                                    return Ok(false);
                                }
                            }
                            None => return Ok(false),
                        }
                    }
                    Ok(true)
                }
                _ => Ok(false),
            }
        }
        (Value::Int(x), Value::Float(y)) | (Value::Float(y), Value::Int(x)) => Ok(x as f64 == y),
        _ => Ok(a == b),
    }
}

fn less(heap: &Heap, a: Value, b: Value) -> Result<bool, Trap> {
    match (a, b) {
        (Value::Int(x), Value::Int(y)) => Ok(x < y),
        (Value::Obj(_), Value::Obj(_)) => Ok(heap.str_of(a)? < heap.str_of(b)?),
        _ => Ok(float(a)? < float(b)?),
    }
}

fn number(v: Value) -> f64 {
    match v {
        Value::Int(i) => i as f64,
        Value::Float(f) => f,
        _ => 0.0,
    }
}

fn all_ints(items: &[Value]) -> bool {
    items.iter().all(|v| matches!(v, Value::Int(_)))
}

fn all_numbers(items: &[Value]) -> bool {
    items.iter().all(|v| matches!(v, Value::Int(_) | Value::Float(_)))
}

fn texts<'a>(heap: &'a Heap, items: &[Value]) -> Result<Vec<&'a str>, Trap> {
    let mut out = Vec::with_capacity(items.len());
    for v in items {
        match heap.str_of(*v) {
            Ok(s) => out.push(s),
            Err(_) => return err("only numbers and text can be put in order"),
        }
    }
    Ok(out)
}

/// The kinds are checked first: Rust's sort may panic on an inconsistent order.
fn sort_values(heap: &Heap, items: &mut [Value]) -> Result<(), Trap> {
    if all_ints(items) {
        let mut keys: Vec<i64> = items.iter().map(|v| int(*v).unwrap_or(0)).collect();
        keys.sort_unstable();
        for (slot, k) in items.iter_mut().zip(keys) {
            *slot = Value::Int(k);
        }
    } else if all_numbers(items) {
        items.sort_by(|a, b| number(*a).total_cmp(&number(*b)));
    } else {
        let names = texts(heap, items)?;
        let mut order: Vec<u32> = (0..items.len() as u32).collect();
        order.sort_by(|a, b| names[*a as usize].cmp(names[*b as usize]));
        let sorted: Vec<Value> = order.iter().map(|i| items[*i as usize]).collect();
        items.copy_from_slice(&sorted);
    }
    Ok(())
}

/// The order of indices that sorts `keys` (stable).
fn sort_order(heap: &Heap, keys: &[Value]) -> Result<Vec<u32>, Trap> {
    let mut order: Vec<u32> = (0..keys.len() as u32).collect();
    if all_ints(keys) {
        order.sort_by_key(|i| int(keys[*i as usize]).unwrap_or(0));
    } else if all_numbers(keys) {
        order.sort_by(|a, b| number(keys[*a as usize]).total_cmp(&number(keys[*b as usize])));
    } else {
        let names = texts(heap, keys)?;
        order.sort_by(|a, b| names[*a as usize].cmp(names[*b as usize]));
    }
    Ok(order)
}

// ---------------------------------------------------------------- instructions that need help

pub fn len(core: &mut Core, v: Value) -> Result<i64, Trap> {
    let Value::Obj(i) = v else { return Err(bad()) };
    match core.heap.objs.get(i as usize) {
        Some(Obj::List(items)) => Ok(items.len() as i64),
        Some(Obj::Str(s)) => {
            core.work += (s.len() / 64) as u64;
            Ok(s.chars().count() as i64)
        }
        Some(Obj::Map(m)) => Ok(m.len() as i64),
        _ => Err(bad()),
    }
}

pub fn concat(core: &mut Core, a: Value, b: Value) -> Result<Value, Trap> {
    let (Value::Obj(i), Value::Obj(j)) = (a, b) else { return Err(bad()) };
    let heap = &core.heap;
    match (heap.objs.get(i as usize), heap.objs.get(j as usize)) {
        (Some(Obj::Str(x)), Some(Obj::Str(y))) => {
            let n = x.len() + y.len();
            heap.check(24 + n)?;
            let mut out = String::with_capacity(n);
            out.push_str(x);
            out.push_str(y);
            core.work += (n / 64) as u64;
            core.heap.str(out)
        }
        (Some(Obj::List(x)), Some(Obj::List(y))) => {
            let n = x.len() + y.len();
            heap.check(40 + 16 * n)?;
            let mut out = Vec::with_capacity(n);
            out.extend_from_slice(x);
            out.extend_from_slice(y);
            core.work += (n / 16) as u64;
            core.heap.list(out)
        }
        _ => Err(bad()),
    }
}

/// `s[i]`: a one-character text.
pub fn str_index(core: &mut Core, s: Value, i: i64) -> Result<Value, Trap> {
    let text = core.heap.str_of(s)?;
    core.work += (text.len() / 256) as u64;
    let (found, n) = if text.is_ascii() {
        (index(i, text.len()).map(|j| text.as_bytes()[j] as char), text.len())
    } else {
        let n = text.chars().count();
        (index(i, n).and_then(|j| text.chars().nth(j)), n)
    };
    match found {
        Some(c) => core.heap.char_str(c),
        None => err(format!("index {} is out of range for a text of length {}", i, n)),
    }
}

pub fn missing_key(core: &Core, key: Value) -> Trap {
    let shown = display(core, key, true).unwrap_or_else(|_| "?".to_string());
    Trap::Error(format!("the key {} is not in the map (use get or get_or for keys that may be missing)", shown))
}

// ---------------------------------------------------------------- random numbers

fn next_u64(s: &mut [u64; 4]) -> u64 {
    let result = s[1].wrapping_mul(5).rotate_left(7).wrapping_mul(9);
    let t = s[1] << 17;
    s[2] ^= s[0];
    s[3] ^= s[1];
    s[1] ^= s[2];
    s[0] ^= s[3];
    s[2] ^= t;
    s[3] = s[3].rotate_left(45);
    result
}

fn rand_int(rng: &mut [u64; 4], lo: i64, hi: i64) -> Result<i64, Trap> {
    if lo > hi {
        return err(format!("rand_int: the range {}..={} is empty", lo, hi));
    }
    let span = (hi as i128 - lo as i128 + 1) as u128;
    if span > u64::MAX as u128 {
        return Ok(next_u64(rng) as i64);
    }
    let span = span as u64;
    let limit = u64::MAX - (u64::MAX - span + 1) % span;
    loop {
        let r = next_u64(rng);
        if r <= limit {
            return Ok(lo.wrapping_add((r % span) as i64));
        }
    }
}

// ---------------------------------------------------------------- lists

fn strings(heap: &mut Heap, parts: Vec<String>) -> Result<Value, Trap> {
    let mut items = Vec::with_capacity(parts.len());
    for p in parts {
        items.push(heap.str(p)?);
    }
    heap.list(items)
}

fn range(core: &mut Core, from: i64, to: i64, step: i64) -> Result<Value, Trap> {
    if step == 0 {
        return err("range: the step is zero");
    }
    let span = if step > 0 { to as i128 - from as i128 } else { from as i128 - to as i128 };
    let size = step.unsigned_abs() as i128;
    let n = if span <= 0 { 0 } else { (span + size - 1) / size };
    let n = usize::try_from(n).map_err(|_| Trap::NeedGc)?;
    core.heap.check(n.saturating_mul(16).saturating_add(40))?;
    core.work += (n / 8) as u64;
    let mut items = Vec::with_capacity(n);
    let mut x = from;
    for _ in 0..n {
        items.push(Value::Int(x));
        x = x.wrapping_add(step);
    }
    core.heap.list(items)
}

// ---------------------------------------------------------------- JSON

enum JErr {
    Trap(Trap),
    Syntax(usize, &'static str),
}

impl From<Trap> for JErr {
    fn from(t: Trap) -> JErr {
        JErr::Trap(t)
    }
}

struct JsonIn<'a> {
    text: &'a str,
    src: &'a [u8],
    pos: usize,
}

impl<'a> JsonIn<'a> {
    fn peek(&self) -> Option<u8> {
        self.src.get(self.pos).copied()
    }

    fn fail<T>(&self, msg: &'static str) -> Result<T, JErr> {
        Err(JErr::Syntax(self.pos, msg))
    }

    fn ws(&mut self) {
        while let Some(b' ' | b'\t' | b'\n' | b'\r') = self.peek() {
            self.pos += 1;
        }
    }

    fn digits(&mut self) {
        while let Some(b'0'..=b'9') = self.peek() {
            self.pos += 1;
        }
    }

    fn word(&mut self, word: &'static str) -> Result<(), JErr> {
        if self.src[self.pos..].starts_with(word.as_bytes()) {
            self.pos += word.len();
            return Ok(());
        }
        self.fail("unknown word (expected true, false or null)")
    }

    fn number(&mut self) -> Result<f64, JErr> {
        let start = self.pos;
        if self.peek() == Some(b'-') {
            self.pos += 1;
        }
        match self.peek() {
            Some(b'0') => self.pos += 1,
            Some(b'1'..=b'9') => self.digits(),
            _ => return self.fail("expected a digit"),
        }
        if self.peek() == Some(b'.') {
            self.pos += 1;
            if !matches!(self.peek(), Some(b'0'..=b'9')) {
                return self.fail("expected a digit after the decimal point");
            }
            self.digits();
        }
        if let Some(b'e' | b'E') = self.peek() {
            self.pos += 1;
            if let Some(b'+' | b'-') = self.peek() {
                self.pos += 1;
            }
            if !matches!(self.peek(), Some(b'0'..=b'9')) {
                return self.fail("expected a digit in the exponent");
            }
            self.digits();
        }
        match self.text[start..self.pos].parse::<f64>() {
            Ok(x) if x.is_finite() => Ok(x),
            _ => Err(JErr::Syntax(start, "the number is too large")),
        }
    }

    fn hex4(&mut self) -> Result<u32, JErr> {
        let mut v = 0u32;
        for _ in 0..4 {
            let Some(d) = self.peek().and_then(|b| (b as char).to_digit(16)) else {
                return self.fail("expected four hex digits after \\u");
            };
            v = v * 16 + d;
            self.pos += 1;
        }
        Ok(v)
    }

    fn string(&mut self) -> Result<String, JErr> {
        self.pos += 1;
        let mut out = String::new();
        loop {
            let start = self.pos;
            while let Some(b) = self.peek() {
                if b == b'"' || b == b'\\' || b < 0x20 {
                    break;
                }
                self.pos += 1;
            }
            out.push_str(&self.text[start..self.pos]);
            match self.peek() {
                None => return self.fail("the text is not closed with a quote"),
                Some(b'"') => {
                    self.pos += 1;
                    return Ok(out);
                }
                Some(b'\\') => {
                    self.pos += 1;
                    let c = match self.peek() {
                        Some(b'"') => '"',
                        Some(b'\\') => '\\',
                        Some(b'/') => '/',
                        Some(b'b') => '\u{8}',
                        Some(b'f') => '\u{c}',
                        Some(b'n') => '\n',
                        Some(b'r') => '\r',
                        Some(b't') => '\t',
                        Some(b'u') => {
                            self.pos += 1;
                            let mut code = self.hex4()?;
                            if (0xD800..0xDC00).contains(&code) && self.src[self.pos..].starts_with(b"\\u") {
                                let saved = self.pos;
                                self.pos += 2;
                                let low = self.hex4()?;
                                if (0xDC00..0xE000).contains(&low) {
                                    code = 0x10000 + ((code - 0xD800) << 10) + (low - 0xDC00);
                                } else {
                                    self.pos = saved;
                                }
                            }
                            out.push(char::from_u32(code).unwrap_or('\u{FFFD}'));
                            continue;
                        }
                        _ => return self.fail("unknown escape after \\"),
                    };
                    self.pos += 1;
                    out.push(c);
                }
                Some(_) => return self.fail("a line break or control character inside text"),
            }
        }
    }

    fn value(&mut self, heap: &mut Heap, depth: u32) -> Result<Value, JErr> {
        if depth > MAX_DEPTH {
            return self.fail("nested too deeply");
        }
        self.ws();
        let inner = match self.peek() {
            None => return self.fail("unexpected end of the text"),
            Some(b'n') => {
                self.word("null")?;
                return Ok(Value::Tag(J_NULL));
            }
            Some(b't') => {
                self.word("true")?;
                (J_BOOL, Value::Bool(true))
            }
            Some(b'f') => {
                self.word("false")?;
                (J_BOOL, Value::Bool(false))
            }
            Some(b'"') => {
                let s = self.string()?;
                (J_STR, heap.str(s)?)
            }
            Some(b'-' | b'0'..=b'9') => (J_NUM, Value::Float(self.number()?)),
            Some(b'[') => {
                self.pos += 1;
                let mut items = Vec::new();
                self.ws();
                if self.peek() == Some(b']') {
                    self.pos += 1;
                } else {
                    loop {
                        items.push(self.value(heap, depth + 1)?);
                        self.ws();
                        match self.peek() {
                            Some(b',') => self.pos += 1,
                            Some(b']') => {
                                self.pos += 1;
                                break;
                            }
                            _ => return self.fail("expected `,` or `]`"),
                        }
                    }
                }
                (J_LIST, heap.list(items)?)
            }
            Some(b'{') => {
                self.pos += 1;
                let map = heap.alloc(Obj::Map(Box::new(KMap::new())))?;
                self.ws();
                if self.peek() == Some(b'}') {
                    self.pos += 1;
                } else {
                    loop {
                        self.ws();
                        if self.peek() != Some(b'"') {
                            return self.fail("expected a key in double quotes");
                        }
                        let key = self.string()?;
                        self.ws();
                        if self.peek() != Some(b':') {
                            return self.fail("expected `:` after the key");
                        }
                        self.pos += 1;
                        let v = self.value(heap, depth + 1)?;
                        let k = heap.str(key)?;
                        heap.map_set(map, k, v)?;
                        self.ws();
                        match self.peek() {
                            Some(b',') => self.pos += 1,
                            Some(b'}') => {
                                self.pos += 1;
                                break;
                            }
                            _ => return self.fail("expected `,` or `}`"),
                        }
                    }
                }
                (J_OBJ, map)
            }
            Some(_) => return self.fail("unexpected character"),
        };
        Ok(heap.alloc(Obj::Enum(inner.0, Box::new([inner.1])))?)
    }
}

fn json_parse(heap: &mut Heap, text: &str) -> Result<Value, Trap> {
    let mut p = JsonIn { text, src: text.as_bytes(), pos: 0 };
    let mut parsed = p.value(heap, 0);
    if parsed.is_ok() {
        p.ws();
        if p.pos < p.src.len() {
            parsed = Err(JErr::Syntax(p.pos, "unexpected text after the value"));
        }
    }
    match parsed {
        Ok(v) => heap.alloc(Obj::Enum(T_OK, Box::new([v]))),
        Err(JErr::Trap(t)) => Err(t),
        Err(JErr::Syntax(pos, msg)) => {
            let before = &p.src[..pos.min(p.src.len())];
            let line = before.iter().filter(|b| **b == b'\n').count() + 1;
            let start = before.iter().rposition(|b| *b == b'\n').map_or(0, |i| i + 1);
            let col = before[start..].iter().filter(|b| (**b & 0xC0) != 0x80).count() + 1;
            let s = heap.str(format!("invalid JSON at line {}, column {}: {}", line, col, msg))?;
            heap.alloc(Obj::Enum(T_ERR, Box::new([s])))
        }
    }
}

fn json_quote(s: &str, out: &mut String) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

fn json_num(x: f64, out: &mut String) {
    if !x.is_finite() {
        out.push_str("null");
        return;
    }
    let size = x.abs();
    if size != 0.0 && !(1e-7..1e17).contains(&size) {
        let _ = write!(out, "{:e}", x);
    } else {
        let _ = write!(out, "{}", x);
    }
}

struct JsonOut<'a> {
    heap: &'a Heap,
    out: String,
    cap: usize,
    pretty: bool,
}

impl<'a> JsonOut<'a> {
    fn newline(&mut self, indent: usize) {
        if self.pretty {
            self.out.push('\n');
            for _ in 0..indent {
                self.out.push_str("  ");
            }
        }
    }

    fn write(&mut self, v: Value, depth: u32) -> Result<(), Trap> {
        if self.out.len() > self.cap {
            return Err(Trap::NeedGc);
        }
        if depth > MAX_DEPTH {
            return err("the JSON value is nested too deeply");
        }
        let indent = depth as usize;
        let heap = self.heap;
        let i = match v {
            Value::Tag(J_NULL) => {
                self.out.push_str("null");
                return Ok(());
            }
            Value::Obj(i) => i,
            _ => return Err(bad()),
        };
        let Some(Obj::Enum(tag, fields)) = heap.objs.get(i as usize) else { return Err(bad()) };
        let Some(&inner) = fields.first() else { return Err(bad()) };
        match *tag {
            J_BOOL => self.out.push_str(if inner == Value::Bool(true) { "true" } else { "false" }),
            J_NUM => json_num(float(inner)?, &mut self.out),
            J_STR => {
                let s = heap.str_of(inner)?;
                if self.out.len().saturating_add(s.len()) > self.cap {
                    return Err(Trap::NeedGc);
                }
                json_quote(s, &mut self.out);
            }
            J_LIST => {
                let items = heap.list_of(inner)?;
                if items.is_empty() {
                    self.out.push_str("[]");
                    return Ok(());
                }
                self.out.push('[');
                for (n, item) in items.iter().enumerate() {
                    if n > 0 {
                        self.out.push(',');
                    }
                    self.newline(indent + 1);
                    self.write(*item, depth + 1)?;
                }
                self.newline(indent);
                self.out.push(']');
            }
            J_OBJ => {
                let map = heap.map_of(inner)?;
                if map.len() == 0 {
                    self.out.push_str("{}");
                    return Ok(());
                }
                self.out.push('{');
                for (n, (k, item)) in map.iter().enumerate() {
                    if n > 0 {
                        self.out.push(',');
                    }
                    self.newline(indent + 1);
                    match k {
                        Value::Int(key) => {
                            let _ = write!(self.out, "\"{}\"", key);
                        }
                        _ => json_quote(heap.str_of(k)?, &mut self.out),
                    }
                    self.out.push_str(if self.pretty { ": " } else { ":" });
                    self.write(item, depth + 1)?;
                }
                self.newline(indent);
                self.out.push('}');
            }
            _ => return Err(bad()),
        }
        Ok(())
    }
}

// ---------------------------------------------------------------- the builtin table

/// Builtins that do not look at the whole of their arguments.
fn constant_time(b: B) -> bool {
    matches!(
        b,
        B::LenL
            | B::Push
            | B::Pop
            | B::First
            | B::Last
            | B::ClearL
            | B::IsEmptyL
            | B::IsEmptyS
            | B::CharCode
            | B::LenM
            | B::GetM
            | B::GetOr
            | B::Has
            | B::RemoveM
            | B::ClearM
            | B::IsEmptyM
    )
}

pub fn call(core: &mut Core, id: u16, args: &[Value]) -> Result<Value, Trap> {
    let Some(&b) = ALL.get(id as usize) else { return Err(bad()) };
    let name = BUILTINS[id as usize].0;
    let arg = |i: usize| args.get(i).copied().ok_or_else(bad);
    let num = |i: usize| float(args.get(i).copied().ok_or_else(bad)?);
    if !constant_time(b) {
        for v in args {
            if let Value::Obj(i) = v {
                if let Some(obj) = core.heap.objs.get(*i as usize) {
                    core.work += (obj.size() / 64) as u64;
                }
            }
        }
    }
    match b {
        // ---- math
        B::Abs => match arg(0)? {
            Value::Int(i) => Ok(Value::Int(i.checked_abs().ok_or_else(overflow)?)),
            Value::Float(f) => fl(f.abs()),
            _ => Err(bad()),
        },
        B::Min2 => {
            let (x, y) = (arg(0)?, arg(1)?);
            Ok(if less(&core.heap, y, x)? { y } else { x })
        }
        B::Max2 => {
            let (x, y) = (arg(0)?, arg(1)?);
            Ok(if less(&core.heap, x, y)? { y } else { x })
        }
        B::MinL | B::MaxL => {
            let heap = &core.heap;
            let items = heap.list_of(arg(0)?)?;
            let Some(&first) = items.first() else { return err(format!("{} of an empty list", name)) };
            let mut best = first;
            for &v in &items[1..] {
                let better = if b == B::MinL { less(heap, v, best)? } else { less(heap, best, v)? };
                if better {
                    best = v;
                }
            }
            Ok(best)
        }
        B::Clamp => {
            let (x, lo, hi) = (arg(0)?, arg(1)?, arg(2)?);
            let heap = &core.heap;
            if less(heap, hi, lo)? {
                return err("clamp: the lower limit is above the upper limit");
            }
            Ok(if less(heap, x, lo)? {
                lo
            } else if less(heap, hi, x)? {
                hi
            } else {
                x
            })
        }
        B::Floor => to_int(num(0)?.floor()),
        B::Ceil => to_int(num(0)?.ceil()),
        B::Round => to_int(num(0)?.round()),
        B::Trunc | B::ToInt => to_int(num(0)?.trunc()),
        B::RoundTo => fl(round_to(num(0)?, int(arg(1)?)?)),
        B::Sin => fl(num(0)?.sin()),
        B::Cos => fl(num(0)?.cos()),
        B::Tan => fl(num(0)?.tan()),
        B::Atan => fl(num(0)?.atan()),
        B::Exp => fl(num(0)?.exp()),
        B::Asin | B::Acos => {
            let x = num(0)?;
            if !(-1.0..=1.0).contains(&x) {
                return err(format!("{} needs a value from -1 to 1, got {}", name, show_float(x)));
            }
            fl(if b == B::Asin { x.asin() } else { x.acos() })
        }
        B::Ln | B::Log10 | B::Log2 => {
            let x = num(0)?;
            if x <= 0.0 {
                return err(format!("{} needs a positive number, got {}", name, show_float(x)));
            }
            fl(match b {
                B::Ln => x.ln(),
                B::Log10 => x.log10(),
                _ => x.log2(),
            })
        }
        B::Atan2 => fl(num(0)?.atan2(num(1)?)),
        B::ToFloat => fl(num(0)?),
        B::IsNan => Ok(Value::Bool(num(0)?.is_nan())),
        B::Approx => Ok(Value::Bool((num(0)? - num(1)?).abs() <= num(2)?.abs())),
        B::Rand => fl((next_u64(&mut core.rng) >> 11) as f64 / 9_007_199_254_740_992.0),
        B::RandInt => Ok(Value::Int(rand_int(&mut core.rng, int(arg(0)?)?, int(arg(1)?)?)?)),
        B::RandSeed => {
            core.rng = seed_rng(int(arg(0)?)? as u64);
            Ok(Value::Unit)
        }

        // ---- strings
        B::LenS => Ok(Value::Int(core.heap.str_of(arg(0)?)?.chars().count() as i64)),
        B::Upper => {
            let s = core.heap.str_of(arg(0)?)?.to_uppercase();
            core.heap.str(s)
        }
        B::Lower => {
            let s = core.heap.str_of(arg(0)?)?.to_lowercase();
            core.heap.str(s)
        }
        B::Trim => {
            let s = core.heap.str_of(arg(0)?)?.trim().to_string();
            core.heap.str(s)
        }
        B::Split => {
            let heap = &core.heap;
            let (s, sep) = (heap.str_of(arg(0)?)?, heap.str_of(arg(1)?)?);
            if sep.is_empty() {
                return err("split: the separator is empty (use chars to get single characters)");
            }
            // A one-byte separator is always ASCII, and searching for a byte is much faster.
            let byte = if sep.len() == 1 { Some(sep.as_bytes()[0]) } else { None };
            let n = match byte {
                Some(b) => s.as_bytes().iter().filter(|&&x| x == b).count() + 1,
                None => s.matches(sep).count() + 1,
            };
            heap.check(n.saturating_mul(40).saturating_add(s.len()).saturating_add(40))?;
            let mut parts: Vec<Box<str>> = Vec::with_capacity(n);
            match byte {
                Some(b) => parts.extend(s.split(b as char).map(|p| Box::<str>::from(p))),
                None => parts.extend(s.split(sep).map(|p| Box::<str>::from(p))),
            }
            core.heap.reserve(parts.len() + 1);
            let mut items = Vec::with_capacity(parts.len());
            for p in parts {
                items.push(core.heap.str(p)?);
            }
            core.heap.list(items)
        }
        B::Lines => {
            let heap = &core.heap;
            let s = heap.str_of(arg(0)?)?;
            let n = s.matches('\n').count() + 1;
            heap.check(n.saturating_mul(40).saturating_add(s.len()).saturating_add(40))?;
            let parts: Vec<String> = s.lines().map(str::to_string).collect();
            strings(&mut core.heap, parts)
        }
        B::Chars => {
            let s = core.heap.str_of(arg(0)?)?.to_string();
            let n = s.chars().count();
            core.heap.check(n.saturating_mul(16).saturating_add((s.len() - n).saturating_mul(28)).saturating_add(40))?;
            let mut items = Vec::with_capacity(n);
            for c in s.chars() {
                items.push(core.heap.char_str(c)?);
            }
            core.heap.list(items)
        }
        B::Join => {
            let heap = &core.heap;
            let items = heap.list_of(arg(0)?)?;
            let sep = heap.str_of(arg(1)?)?;
            let mut total = sep.len().saturating_mul(items.len().saturating_sub(1));
            for v in items {
                total = total.saturating_add(heap.str_of(*v)?.len());
            }
            heap.check(total.saturating_add(24))?;
            let mut out = String::with_capacity(total);
            for (i, v) in items.iter().enumerate() {
                if i > 0 {
                    out.push_str(sep);
                }
                out.push_str(heap.str_of(*v)?);
            }
            core.work += (total / 64) as u64;
            core.heap.str(out)
        }
        B::ContainsS => {
            let heap = &core.heap;
            Ok(Value::Bool(heap.str_of(arg(0)?)?.contains(heap.str_of(arg(1)?)?)))
        }
        B::StartsWith => {
            let heap = &core.heap;
            Ok(Value::Bool(heap.str_of(arg(0)?)?.starts_with(heap.str_of(arg(1)?)?)))
        }
        B::EndsWith => {
            let heap = &core.heap;
            Ok(Value::Bool(heap.str_of(arg(0)?)?.ends_with(heap.str_of(arg(1)?)?)))
        }
        B::Replace => {
            let heap = &core.heap;
            let (s, from, to) = (heap.str_of(arg(0)?)?, heap.str_of(arg(1)?)?, heap.str_of(arg(2)?)?);
            if from.is_empty() {
                return err("replace: the text to look for is empty");
            }
            let found = s.matches(from).count();
            let size = (s.len() - found * from.len()).saturating_add(found.saturating_mul(to.len()));
            heap.check(size.saturating_add(24))?;
            let out = s.replace(from, to);
            core.heap.str(out)
        }
        B::FindS => {
            let heap = &core.heap;
            let (s, pat) = (heap.str_of(arg(0)?)?, heap.str_of(arg(1)?)?);
            let found = s.find(pat).map(|i| Value::Int(s[..i].chars().count() as i64));
            option(&mut core.heap, found)
        }
        B::SliceS => {
            let s = core.heap.str_of(arg(0)?)?;
            let n = s.chars().count();
            let (from, to) = (bound(int(arg(1)?)?, n), bound(int(arg(2)?)?, n));
            let part = if from >= to { String::new() } else { s[byte_at(s, from)..byte_at(s, to)].to_string() };
            core.heap.str(part)
        }
        B::Repeat => {
            let s = core.heap.str_of(arg(0)?)?;
            let n = count(int(arg(1)?)?)?;
            let size = s.len().checked_mul(n).ok_or(Trap::NeedGc)?;
            core.heap.check(size.saturating_add(24))?;
            let out = s.repeat(n);
            core.work += (size / 64) as u64;
            core.heap.str(out)
        }
        B::ToIntS => {
            let v = core.heap.str_of(arg(0)?)?.trim().parse::<i64>().ok().map(Value::Int);
            option(&mut core.heap, v)
        }
        B::ToFloatS => {
            let parsed = core.heap.str_of(arg(0)?)?.trim().parse::<f64>().ok();
            let v = parsed.filter(|x| x.is_finite()).map(Value::Float);
            option(&mut core.heap, v)
        }
        B::PadLeft | B::PadRight => {
            let s = core.heap.str_of(arg(0)?)?;
            let pad = count(int(arg(1)?)?)?.saturating_sub(s.chars().count());
            core.heap.check(s.len().saturating_add(pad).saturating_add(24))?;
            let spaces = " ".repeat(pad);
            let out = if b == B::PadLeft { spaces + s } else { s.to_string() + &spaces };
            core.work += (pad / 64) as u64;
            core.heap.str(out)
        }
        B::CharCode => match core.heap.str_of(arg(0)?)?.chars().next() {
            Some(c) => Ok(Value::Int(c as i64)),
            None => err("char_code: the text is empty"),
        },
        B::FromCharCode => {
            let n = int(arg(0)?)?;
            match u32::try_from(n).ok().and_then(char::from_u32) {
                Some(c) => core.heap.char_str(c),
                None => err(format!("from_char_code: {} is not a character code", n)),
            }
        }
        B::IsEmptyS => Ok(Value::Bool(core.heap.str_of(arg(0)?)?.is_empty())),

        // ---- lists
        B::LenL => Ok(Value::Int(core.heap.list_of(arg(0)?)?.len() as i64)),
        B::Push => {
            let v = arg(1)?;
            core.heap.list_of(arg(0)?)?;
            core.heap.charge(16)?;
            core.heap.list_mut(arg(0)?)?.push(v);
            Ok(Value::Unit)
        }
        B::Pop => {
            core.heap.check(48)?;
            let v = core.heap.list_mut(arg(0)?)?.pop();
            option(&mut core.heap, v)
        }
        B::Insert => {
            let (i, v) = (int(arg(1)?)?, arg(2)?);
            let n = core.heap.list_of(arg(0)?)?.len();
            let j = if i < 0 { i.saturating_add(n as i64) } else { i };
            if j < 0 || j > n as i64 {
                return err(format!("insert: index {} is out of range for a list of length {}", i, n));
            }
            core.heap.charge(16)?;
            core.heap.list_mut(arg(0)?)?.insert(j as usize, v);
            Ok(Value::Unit)
        }
        B::RemoveAt => {
            let i = int(arg(1)?)?;
            let items = core.heap.list_mut(arg(0)?)?;
            match index(i, items.len()) {
                Some(j) => Ok(items.remove(j)),
                None => err(format!("remove_at: index {} is out of range for a list of length {}", i, items.len())),
            }
        }
        B::First => {
            let v = core.heap.list_of(arg(0)?)?.first().copied();
            option(&mut core.heap, v)
        }
        B::Last => {
            let v = core.heap.list_of(arg(0)?)?.last().copied();
            option(&mut core.heap, v)
        }
        B::ContainsL | B::IndexOf => {
            let needle = arg(1)?;
            let mut steps = 0u64;
            let mut found = None;
            let heap = &core.heap;
            for (i, v) in heap.list_of(arg(0)?)?.iter().enumerate() {
                if eq(heap, *v, needle, 0, &mut steps)? {
                    found = Some(Value::Int(i as i64));
                    break;
                }
            }
            core.work += steps / 8;
            if b == B::ContainsL {
                return Ok(Value::Bool(found.is_some()));
            }
            option(&mut core.heap, found)
        }
        B::SliceL => {
            let items = core.heap.list_of(arg(0)?)?;
            let n = items.len();
            let (from, to) = (bound(int(arg(1)?)?, n), bound(int(arg(2)?)?, n));
            let part = if from >= to { Vec::new() } else { items[from..to].to_vec() };
            core.heap.list(part)
        }
        B::Reverse => {
            let mut items = core.heap.list_of(arg(0)?)?.clone();
            items.reverse();
            core.heap.list(items)
        }
        B::Sort => {
            let mut items = core.heap.list_of(arg(0)?)?.clone();
            let n = items.len() as u64;
            core.work += n * (64 - n.leading_zeros() as u64) / 4;
            sort_values(&core.heap, &mut items)?;
            core.heap.list(items)
        }
        B::ConcatL => concat(core, arg(0)?, arg(1)?),
        B::Range1 => range(core, 0, int(arg(0)?)?, 1),
        B::Range2 => range(core, int(arg(0)?)?, int(arg(1)?)?, 1),
        B::Range3 => range(core, int(arg(0)?)?, int(arg(1)?)?, int(arg(2)?)?),
        B::Fill => {
            let n = int(arg(0)?)?;
            if n < 0 {
                return err(format!("fill: the count is negative ({})", n));
            }
            let n = count(n)?;
            core.heap.check(n.saturating_mul(16).saturating_add(40))?;
            core.work += (n / 8) as u64;
            core.heap.list(vec![arg(1)?; n])
        }
        B::ClearL => {
            core.heap.list_mut(arg(0)?)?.clear();
            Ok(Value::Unit)
        }
        B::CopyL => {
            let items = core.heap.list_of(arg(0)?)?.clone();
            core.heap.list(items)
        }
        B::IsEmptyL => Ok(Value::Bool(core.heap.list_of(arg(0)?)?.is_empty())),
        B::Extend => {
            let extra = core.heap.list_of(arg(1)?)?.clone();
            core.heap.list_of(arg(0)?)?;
            core.heap.charge(16 * extra.len())?;
            core.heap.list_mut(arg(0)?)?.extend(extra);
            Ok(Value::Unit)
        }
        B::SortKeys => {
            let heap = &core.heap;
            let (items, keys) = (heap.list_of(arg(0)?)?, heap.list_of(arg(1)?)?);
            if items.len() != keys.len() {
                return err("sort_by: the list changed while it was being sorted");
            }
            let order = sort_order(heap, keys)?;
            let sorted: Vec<Value> = order.iter().map(|i| items[*i as usize]).collect();
            let n = sorted.len() as u64;
            core.work += n * (64 - n.leading_zeros() as u64) / 4;
            core.heap.list(sorted)
        }

        // ---- maps
        B::LenM => Ok(Value::Int(core.heap.map_of(arg(0)?)?.len() as i64)),
        B::GetM => {
            let v = core.heap.map_get(arg(0)?, arg(1)?)?;
            option(&mut core.heap, v)
        }
        B::GetOr => Ok(core.heap.map_get(arg(0)?, arg(1)?)?.unwrap_or(arg(2)?)),
        B::Has => Ok(Value::Bool(core.heap.map_get(arg(0)?, arg(1)?)?.is_some())),
        B::RemoveM => {
            core.heap.check(48)?;
            let v = core.heap.map_remove(arg(0)?, arg(1)?)?;
            option(&mut core.heap, v)
        }
        B::Keys => {
            let keys: Vec<Value> = core.heap.map_of(arg(0)?)?.iter().map(|e| e.0).collect();
            core.heap.list(keys)
        }
        B::Values => {
            let values: Vec<Value> = core.heap.map_of(arg(0)?)?.iter().map(|e| e.1).collect();
            core.heap.list(values)
        }
        B::Items => {
            let pairs: Vec<(Value, Value)> = core.heap.map_of(arg(0)?)?.iter().collect();
            core.heap.check(pairs.len().saturating_mul(80).saturating_add(40))?;
            let mut items = Vec::with_capacity(pairs.len());
            for (k, v) in pairs {
                items.push(core.heap.alloc(Obj::Struct(NONE, Box::new([k, v])))?);
            }
            core.heap.list(items)
        }
        B::ClearM => {
            core.heap.map_mut(arg(0)?)?.clear();
            Ok(Value::Unit)
        }
        B::IsEmptyM => Ok(Value::Bool(core.heap.map_of(arg(0)?)?.len() == 0)),
        B::CopyM => {
            let heap = &core.heap;
            let map = heap.map_of(arg(0)?)?;
            heap.check(map.size())?;
            let copy = map.clone();
            core.heap.alloc(Obj::Map(Box::new(copy)))
        }

        // ---- json
        B::JsonParse => {
            let text = core.heap.str_of(arg(0)?)?.to_string();
            core.work += (text.len() / 8) as u64;
            json_parse(&mut core.heap, &text)
        }
        B::JsonStr | B::JsonPretty => {
            let text = {
                let heap = &core.heap;
                let mut w = JsonOut { heap, out: String::new(), cap: room(heap), pretty: b == B::JsonPretty };
                w.write(arg(0)?, 0)?;
                w.out
            };
            core.work += (text.len() / 16) as u64;
            core.heap.str(text)
        }

        // ---- internal
        B::Print => {
            let s = core.heap.str_of(arg(0)?)?;
            let n = s.len() as u64 + 1;
            if core.out_total.saturating_add(n) > core.out_limit {
                return err("the output limit was reached");
            }
            core.out.push_str(s);
            core.out.push('\n');
            core.out_total += n;
            Ok(Value::Unit)
        }
        B::Fmt => fmt(core, arg(0)?, int(arg(1)?)?, int(arg(2)?)?, int(arg(3)?)?),
        B::Panic => Err(Trap::Error(core.heap.str_of(arg(0)?)?.to_string())),
        B::Assert => {
            if arg(0)? == Value::Bool(true) {
                return Ok(Value::Unit);
            }
            let msg = core.heap.str_of(arg(1)?)?;
            if msg.is_empty() {
                return err("assertion failed");
            }
            err(format!("assertion failed: {}", msg))
        }
        B::AssertEq => {
            let (x, y) = (arg(0)?, arg(1)?);
            if equal(core, x, y)? {
                return Ok(Value::Unit);
            }
            // The checker adds (divide, multiply, unit text) for two numbers with a unit,
            // or one note when the values hold such numbers.
            if args.len() == 5 {
                let (scale_up, scale_down) = (num(2)?, num(3)?);
                let unit = core.heap.str_of(arg(4)?)?.to_string();
                let left = show_float(num(0)? * scale_up / scale_down);
                let right = show_float(num(1)? * scale_up / scale_down);
                let rounding = if left == right { ROUNDING_NOTE } else { "" };
                return err(format!("assertion failed: {left}{unit} is not equal to {right}{unit}{rounding}"));
            }
            let note = if args.len() == 3 { core.heap.str_of(arg(2)?)?.to_string() } else { String::new() };
            let (left, right) = (display(core, x, true)?, display(core, y, true)?);
            let rounding = if left == right { ROUNDING_NOTE } else { "" };
            err(format!("assertion failed: {left} is not equal to {right}{note}{rounding}"))
        }
        B::Sqrt => {
            let x = num(0)?;
            if x < 0.0 {
                return err(format!("sqrt needs a number that is not negative, got {}", show_float(x)));
            }
            fl(x.sqrt())
        }
        B::SumI => {
            let mut total = 0i64;
            for v in core.heap.list_of(arg(0)?)? {
                total = total.checked_add(int(*v)?).ok_or_else(overflow)?;
            }
            Ok(Value::Int(total))
        }
        B::SumF => {
            let mut total = 0.0;
            for v in core.heap.list_of(arg(0)?)? {
                total += float(*v)?;
            }
            fl(total)
        }
        B::Mean => fl(mean_of(&numbers(&core.heap, arg(0)?, "mean", 1)?)),
        B::Median => {
            let mut xs = numbers(&core.heap, arg(0)?, "median", 1)?;
            xs.sort_by(|a, b| a.total_cmp(b));
            let mid = xs.len() / 2;
            if xs.len() % 2 == 1 {
                fl(xs[mid])
            } else {
                fl(xs[mid - 1] / 2.0 + xs[mid] / 2.0)
            }
        }
        B::Stddev => {
            // The sample form: the squared distances from the mean are divided by n - 1.
            let xs = numbers(&core.heap, arg(0)?, "stddev", 2)?;
            let mean = mean_of(&xs);
            let squares = careful_sum(xs.iter().map(|x| (x - mean) * (x - mean)));
            fl((squares / (xs.len() - 1) as f64).sqrt())
        }
    }
}
