//! The bytecode interpreter. It can pause (out of fuel for this slice, waiting
//! for the host) and be resumed, and it has no access to anything outside itself.

use crate::compile::{Const, Op, Program, TUPLE};
use crate::heap::{bad, Heap, KMap, Obj, Seed, Trap, Value, NONE};
use crate::natives;
use std::rc::Rc;

const MAX_STACK: usize = 1 << 20;
const MAX_FRAMES: usize = 250_000;
const FLUSH_AT: usize = 1 << 16;

#[derive(Clone, Copy)]
struct Frame {
    ret_pc: u32,
    base: u32,
}

/// The part of the machine that native functions may touch.
pub struct Core {
    pub heap: Heap,
    /// Printed text that the embedder has not taken yet.
    pub out: String,
    pub out_total: u64,
    pub out_limit: u64,
    /// Extra fuel charged by native functions for long operations.
    pub work: u64,
    pub rng: [u64; 4],
    pub prog: Rc<Program>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Status {
    Done,
    /// Call `run` again.
    More,
    /// Waiting for `resume` with the answer to `host_name` / `host_payload`.
    Host,
    Error,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ErrKind {
    None = 0,
    Compile = 1,
    Runtime = 2,
    Fuel = 3,
    Memory = 4,
}

enum Exit {
    Done,
    Yield,
    Host,
    Flush,
    Collect,
}

pub struct Vm {
    prog: Rc<Program>,
    stack: Vec<Value>,
    frames: Vec<Frame>,
    globals: Vec<Value>,
    consts: Vec<Value>,
    pc: usize,
    base: usize,
    pub core: Core,
    pub fuel_used: u64,
    pub fuel_limit: u64,
    gc_retry: bool,
    host_reg: usize,
    pub host_name: String,
    pub host_payload: String,
    status: Status,
    started: bool,
    pub error: String,
    pub error_kind: ErrKind,
}

fn splitmix(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E3779B97F4A7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
    z ^ (z >> 31)
}

pub fn seed_rng(seed: u64) -> [u64; 4] {
    let mut s = seed;
    [splitmix(&mut s), splitmix(&mut s), splitmix(&mut s), splitmix(&mut s)]
}

#[cold]
fn overflow() -> Trap {
    Trap::Error("integer overflow".to_string())
}

fn tag_of(heap: &Heap, v: Value) -> u32 {
    match v {
        Value::Tag(t) => t,
        Value::Obj(i) => match &heap.objs[i as usize] {
            Obj::Enum(t, _) => *t,
            _ => NONE,
        },
        _ => NONE,
    }
}

impl Vm {
    /// A machine ready to run the top-level code of `prog`.
    pub fn new(prog: Rc<Program>, seed: u64, mem_limit: usize, fuel_limit: u64) -> Vm {
        let mut s = seed;
        let mut heap = Heap::new(mem_limit, Seed(splitmix(&mut s), splitmix(&mut s)));
        let mut consts = Vec::with_capacity(prog.consts.len());
        for c in &prog.consts {
            consts.push(match c {
                Const::Int(v) => Value::Int(*v),
                Const::Float(v) => Value::Float(*v),
                Const::Str(text) => {
                    heap.objs.push(Obj::Str(text.as_str().into()));
                    Value::Obj((heap.objs.len() - 1) as u32)
                }
            });
        }
        heap.freeze();
        let core = Core {
            heap,
            out: String::new(),
            out_total: 0,
            out_limit: u64::MAX,
            work: 0,
            rng: seed_rng(splitmix(&mut s)),
            prog: prog.clone(),
        };
        let mut vm = Vm {
            globals: vec![Value::Uninit; prog.nglobals as usize],
            stack: vec![Value::Unit; 1024],
            frames: Vec::new(),
            consts,
            pc: 0,
            base: 0,
            core,
            fuel_used: 0,
            fuel_limit,
            gc_retry: false,
            host_reg: 0,
            host_name: String::new(),
            host_payload: String::new(),
            status: Status::More,
            started: false,
            error: String::new(),
            error_kind: ErrKind::None,
            prog,
        };
        let main = vm.prog.main;
        vm.start(main);
        vm
    }

    /// Start a function without arguments (the top-level code, or a test after it).
    pub fn start(&mut self, f: u32) {
        self.frames.clear();
        self.base = if self.started { 256 } else { 0 };
        self.started = true;
        if self.stack.len() < self.base + 256 {
            self.stack.resize(self.base + 256, Value::Unit);
        }
        self.pc = self.prog.fns[f as usize].entry as usize;
        self.status = Status::More;
    }

    pub fn status(&self) -> Status {
        self.status
    }

    /// Run for at most `slice` units of fuel.
    pub fn run(&mut self, slice: u64) -> Status {
        if self.status != Status::More {
            return self.status;
        }
        let left = self.fuel_limit.saturating_sub(self.fuel_used);
        let mut budget = slice.min(left).min(1 << 60) as i64;
        loop {
            let before = budget;
            let result = self.exec(&mut budget);
            self.fuel_used = self.fuel_used.saturating_add((before - budget).max(0) as u64);
            match result {
                Ok(Exit::Done) => self.status = Status::Done,
                Ok(Exit::Host) => self.status = Status::Host,
                Ok(Exit::Flush) => {}
                Ok(Exit::Yield) => {
                    if self.fuel_used >= self.fuel_limit {
                        self.fail(ErrKind::Fuel, "the fuel limit was reached (the program ran too long)".to_string());
                    }
                }
                Ok(Exit::Collect) => {
                    self.collect();
                    continue;
                }
                Err(Trap::NeedGc) => {
                    if self.gc_retry {
                        let mb = self.core.heap.limit >> 20;
                        self.fail(ErrKind::Memory, format!("out of memory (the limit is {mb} MB)"));
                    } else {
                        // Retry the instruction after a collection.
                        self.pc -= 1;
                        self.collect();
                        self.gc_retry = true;
                        continue;
                    }
                }
                Err(Trap::Error(msg)) => self.fail(ErrKind::Runtime, msg),
            }
            return self.status;
        }
    }

    /// Answer a host call: the pending `host(...)` expression becomes `Ok(text)` or `Err(text)`.
    pub fn resume(&mut self, ok: bool, text: &str) {
        if self.status != Status::Host {
            return;
        }
        let variant = if ok { 2 } else { 3 };
        for attempt in 0..2 {
            let heap = &mut self.core.heap;
            let made = heap.str(text).and_then(|s| heap.alloc(Obj::Enum(variant, Box::new([s]))));
            match made {
                Ok(v) => {
                    self.stack[self.host_reg] = v;
                    self.status = Status::More;
                    return;
                }
                Err(_) if attempt == 0 => self.collect(),
                Err(_) => {}
            }
        }
        let mb = self.core.heap.limit >> 20;
        self.fail(ErrKind::Memory, format!("out of memory (the limit is {mb} MB)"));
    }

    fn collect(&mut self) {
        let top = (self.base + 256).min(self.stack.len());
        self.core.heap.collect(&self.stack[..top], &self.globals);
    }

    fn fail(&mut self, kind: ErrKind, msg: String) {
        let mut text = format!("runtime error: {msg}");
        let mut pcs = vec![self.pc.saturating_sub(1)];
        pcs.extend(self.frames.iter().rev().map(|f| (f.ret_pc as usize).saturating_sub(1)));
        for (i, &pc) in pcs.iter().enumerate() {
            if i == 12 {
                text.push_str(&format!("\n  ... and {} more calls", pcs.len() - i));
                break;
            }
            let packed = self.prog.lines.get(pc).copied().unwrap_or(0);
            let file = self.prog.files.get((packed >> 20) as usize).map(|s| s.as_str()).unwrap_or("?");
            let f = self.prog.fns.partition_point(|f| f.entry as usize <= pc).saturating_sub(1);
            let name = self.prog.fns.get(f).map(|f| f.name.as_str()).unwrap_or("?");
            text.push_str(&format!("\n  at {}:{}", file, packed & 0xFFFFF));
            if name != "<main>" {
                text.push_str(&format!(" in `{name}`"));
            }
        }
        self.error = text;
        self.error_kind = kind;
        self.status = Status::Error;
    }

    fn exec(&mut self, budget: &mut i64) -> Result<Exit, Trap> {
        let prog = self.prog.clone();
        let code: &[Op] = &prog.code;
        let mut pc = self.pc;
        let mut base = self.base;
        let mut fuel = *budget;
        let Vm { stack, frames, globals, consts, core, gc_retry, host_reg, host_name, host_payload, .. } = self;

        macro_rules! window {
            () => {
                match <&mut [Value; 256]>::try_from(&mut stack[base..base + 256]) {
                    Ok(w) => w,
                    Err(_) => unreachable!(),
                }
            };
        }
        let mut regs = window!();

        let exit = 'run: loop {
            macro_rules! r {
                ($x:expr) => {
                    regs[$x as usize]
                };
            }
            macro_rules! fail {
                ($($t:tt)*) => {
                    break 'run Err(Trap::Error(format!($($t)*)))
                };
            }
            macro_rules! tri {
                ($e:expr) => {
                    match $e {
                        Ok(v) => v,
                        Err(t) => break 'run Err(t),
                    }
                };
            }
            macro_rules! int {
                ($x:expr) => {
                    match r!($x) {
                        Value::Int(v) => v,
                        _ => break 'run Err(bad()),
                    }
                };
            }
            macro_rules! flt {
                ($x:expr) => {
                    match r!($x) {
                        Value::Float(v) => v,
                        _ => break 'run Err(bad()),
                    }
                };
            }
            macro_rules! obj {
                ($x:expr) => {
                    match r!($x) {
                        Value::Obj(v) => v as usize,
                        _ => break 'run Err(bad()),
                    }
                };
            }
            macro_rules! set_int {
                ($a:expr, $v:expr) => {
                    match $v {
                        Some(v) => r!($a) = Value::Int(v),
                        None => break 'run Err(overflow()),
                    }
                };
            }
            macro_rules! tick {
                () => {
                    fuel -= 1;
                    if fuel <= 0 {
                        break 'run Ok(Exit::Yield);
                    }
                };
            }
            // Charge the fuel that a native function reported.
            macro_rules! work {
                () => {
                    if core.work != 0 {
                        fuel -= core.work.min(1 << 40) as i64;
                        core.work = 0;
                    }
                };
            }
            // After every instruction that allocated: a collection may be due.
            macro_rules! allocated {
                () => {
                    *gc_retry = false;
                    if core.heap.bytes > core.heap.threshold {
                        break 'run Ok(Exit::Collect);
                    }
                };
            }
            macro_rules! slice {
                ($a:expr, $n:expr) => {
                    match regs.get($a as usize..$a as usize + $n as usize) {
                        Some(s) => s,
                        None => break 'run Err(bad()),
                    }
                };
            }
            // Enter a function whose window starts at `$nb`.
            macro_rules! enter {
                ($nb:expr) => {
                    let nb: usize = $nb;
                    if frames.len() >= MAX_FRAMES || nb + 256 > MAX_STACK {
                        fail!("stack overflow (the recursion is too deep)");
                    }
                    if nb + 256 > stack.len() {
                        let n = (stack.len() * 2).clamp(nb + 256, MAX_STACK);
                        stack.resize(n, Value::Unit);
                    }
                    frames.push(Frame { ret_pc: pc as u32, base: base as u32 });
                    base = nb;
                    regs = window!();
                };
            }
            macro_rules! leave {
                ($v:expr) => {
                    regs[0] = $v;
                    match frames.pop() {
                        Some(f) => {
                            base = f.base as usize;
                            pc = f.ret_pc as usize;
                            regs = window!();
                        }
                        None => break 'run Ok(Exit::Done),
                    }
                };
            }

            let op = code[pc];
            pc += 1;
            match op {
                Op::Move { a, b } => r!(a) = r!(b),
                Op::LoadK { a, k } => r!(a) = consts[k as usize],
                Op::LoadInt { a, v } => r!(a) = Value::Int(v as i64),
                Op::LoadBool { a, v } => r!(a) = Value::Bool(v),
                Op::LoadUnit { a } => r!(a) = Value::Unit,
                Op::LoadTag { a, tag } => r!(a) = Value::Tag(tag as u32),
                Op::LoadFn { a, f } => r!(a) = Value::Fn(f),
                Op::GetGlobal { a, g } => {
                    let v = globals[g as usize];
                    if matches!(v, Value::Uninit) {
                        fail!("`{}` is used before it is defined", prog.global_names[g as usize]);
                    }
                    r!(a) = v;
                }
                Op::SetGlobal { a, g } => globals[g as usize] = r!(a),

                Op::AddI { a, b, c } => set_int!(a, int!(b).checked_add(int!(c))),
                Op::SubI { a, b, c } => set_int!(a, int!(b).checked_sub(int!(c))),
                Op::MulI { a, b, c } => set_int!(a, int!(b).checked_mul(int!(c))),
                Op::DivI { a, b, c } => r!(a) = Value::Int(tri!(natives::div_floor(int!(b), int!(c)))),
                Op::ModI { a, b, c } => r!(a) = Value::Int(tri!(natives::mod_floor(int!(b), int!(c)))),
                Op::PowI { a, b, c } => r!(a) = Value::Int(tri!(natives::ipow(int!(b), int!(c)))),
                Op::NegI { a, b } => set_int!(a, int!(b).checked_neg()),
                Op::AddII { a, b, imm } => set_int!(a, int!(b).checked_add(imm as i64)),
                Op::MulII { a, b, imm } => set_int!(a, int!(b).checked_mul(imm as i64)),

                Op::AddF { a, b, c } => r!(a) = Value::Float(flt!(b) + flt!(c)),
                Op::SubF { a, b, c } => r!(a) = Value::Float(flt!(b) - flt!(c)),
                Op::MulF { a, b, c } => r!(a) = Value::Float(flt!(b) * flt!(c)),
                Op::DivF { a, b, c } => {
                    let y = flt!(c);
                    if y == 0.0 {
                        fail!("division by zero");
                    }
                    r!(a) = Value::Float(flt!(b) / y);
                }
                Op::ModF { a, b, c } => {
                    let (x, y) = (flt!(b), flt!(c));
                    if y == 0.0 {
                        fail!("modulo by zero");
                    }
                    let m = x % y;
                    r!(a) = Value::Float(if m != 0.0 && ((m < 0.0) != (y < 0.0)) { m + y } else { m });
                }
                Op::PowF { a, b, c } => r!(a) = Value::Float(flt!(b).powf(flt!(c))),
                Op::PowFI { a, b, c } => {
                    let (x, n) = (flt!(b), int!(c));
                    r!(a) = Value::Float(match i32::try_from(n) {
                        Ok(n) => x.powi(n),
                        Err(_) => x.powf(n as f64),
                    });
                }
                Op::NegF { a, b } => r!(a) = Value::Float(-flt!(b)),
                Op::IntToFloat { a, b } => r!(a) = Value::Float(int!(b) as f64),
                Op::Sqrt { a, b } => {
                    let x = flt!(b);
                    if x < 0.0 {
                        fail!("sqrt needs a number that is not negative");
                    }
                    r!(a) = Value::Float(x.sqrt());
                }

                Op::EqI { a, b, c } => r!(a) = Value::Bool(int!(b) == int!(c)),
                Op::NeI { a, b, c } => r!(a) = Value::Bool(int!(b) != int!(c)),
                Op::LtI { a, b, c } => r!(a) = Value::Bool(int!(b) < int!(c)),
                Op::LeI { a, b, c } => r!(a) = Value::Bool(int!(b) <= int!(c)),
                Op::EqF { a, b, c } => r!(a) = Value::Bool(flt!(b) == flt!(c)),
                Op::NeF { a, b, c } => r!(a) = Value::Bool(flt!(b) != flt!(c)),
                Op::LtF { a, b, c } => r!(a) = Value::Bool(flt!(b) < flt!(c)),
                Op::LeF { a, b, c } => r!(a) = Value::Bool(flt!(b) <= flt!(c)),
                Op::EqV { a, b, c } => {
                    r!(a) = Value::Bool(tri!(natives::equal(core, r!(b), r!(c))));
                    work!();
                }
                Op::NeV { a, b, c } => {
                    r!(a) = Value::Bool(!tri!(natives::equal(core, r!(b), r!(c))));
                    work!();
                }
                Op::LtS { a, b, c } => {
                    let less = tri!(core.heap.str_of(r!(b))) < tri!(core.heap.str_of(r!(c)));
                    r!(a) = Value::Bool(less);
                }
                Op::LeS { a, b, c } => {
                    let le = tri!(core.heap.str_of(r!(b))) <= tri!(core.heap.str_of(r!(c)));
                    r!(a) = Value::Bool(le);
                }
                Op::Not { a, b } => r!(a) = Value::Bool(!matches!(r!(b), Value::Bool(true))),

                Op::Jmp { to } => pc = to as usize,
                Op::Loop { to } => {
                    pc = to as usize;
                    tick!();
                }
                Op::JmpT { a, to } => {
                    if matches!(r!(a), Value::Bool(true)) {
                        pc = to as usize;
                    }
                }
                Op::JmpF { a, to } => {
                    if !matches!(r!(a), Value::Bool(true)) {
                        pc = to as usize;
                    }
                }
                Op::JeqI { a, b, to } => {
                    if int!(a) == int!(b) {
                        pc = to as usize;
                    }
                }
                Op::JneI { a, b, to } => {
                    if int!(a) != int!(b) {
                        pc = to as usize;
                    }
                }
                Op::JltI { a, b, to } => {
                    if int!(a) < int!(b) {
                        pc = to as usize;
                    }
                }
                Op::JleI { a, b, to } => {
                    if int!(a) <= int!(b) {
                        pc = to as usize;
                    }
                }
                Op::JeqII { a, imm, to } => {
                    if int!(a) == imm as i64 {
                        pc = to as usize;
                    }
                }
                Op::JneII { a, imm, to } => {
                    if int!(a) != imm as i64 {
                        pc = to as usize;
                    }
                }
                Op::JltII { a, imm, to } => {
                    if int!(a) < imm as i64 {
                        pc = to as usize;
                    }
                }
                Op::JleII { a, imm, to } => {
                    if int!(a) <= imm as i64 {
                        pc = to as usize;
                    }
                }
                Op::JgtII { a, imm, to } => {
                    if int!(a) > imm as i64 {
                        pc = to as usize;
                    }
                }
                Op::JgeII { a, imm, to } => {
                    if int!(a) >= imm as i64 {
                        pc = to as usize;
                    }
                }
                Op::JltF { a, b, to } => {
                    if flt!(a) < flt!(b) {
                        pc = to as usize;
                    }
                }
                Op::JleF { a, b, to } => {
                    if flt!(a) <= flt!(b) {
                        pc = to as usize;
                    }
                }
                Op::JnltF { a, b, to } => {
                    if !(flt!(a) < flt!(b)) {
                        pc = to as usize;
                    }
                }
                Op::JnleF { a, b, to } => {
                    if !(flt!(a) <= flt!(b)) {
                        pc = to as usize;
                    }
                }
                Op::JeqF { a, b, to } => {
                    if flt!(a) == flt!(b) {
                        pc = to as usize;
                    }
                }
                Op::JneF { a, b, to } => {
                    if flt!(a) != flt!(b) {
                        pc = to as usize;
                    }
                }
                Op::JTag { a, tag, to } => {
                    if tag_of(&core.heap, r!(a)) == tag as u32 {
                        pc = to as usize;
                    }
                }
                Op::JnTag { a, tag, to } => {
                    if tag_of(&core.heap, r!(a)) != tag as u32 {
                        pc = to as usize;
                    }
                }

                Op::ForI { a, b, to } => {
                    let i = int!(a).wrapping_add(1);
                    r!(a) = Value::Int(i);
                    if i < int!(b) {
                        pc = to as usize;
                        tick!();
                    }
                }
                Op::ForIE { a, b, to } => {
                    let i = int!(a);
                    if i < int!(b) {
                        r!(a) = Value::Int(i + 1);
                        pc = to as usize;
                        tick!();
                    }
                }
                Op::ForList { a, to } => {
                    let list = obj!(a);
                    let i = int!(a.wrapping_add(1)) as usize;
                    let Obj::List(items) = &core.heap.objs[list] else { break 'run Err(bad()) };
                    if i < items.len() {
                        r!(a.wrapping_add(2)) = items[i];
                        r!(a.wrapping_add(1)) = Value::Int(i as i64 + 1);
                        pc = to as usize;
                        tick!();
                    }
                }

                Op::Call { a, to } => {
                    enter!(base + a as usize);
                    pc = to as usize;
                    tick!();
                }
                Op::CallC { a, n } => {
                    let callee = r!(a);
                    let f = match callee {
                        Value::Fn(f) => f,
                        Value::Obj(i) => match &core.heap.objs[i as usize] {
                            Obj::Closure(f, _) => *f,
                            _ => break 'run Err(bad()),
                        },
                        _ => break 'run Err(bad()),
                    };
                    enter!(base + a as usize + 1);
                    if let Value::Obj(i) = callee {
                        if let Obj::Closure(_, captured) = &core.heap.objs[i as usize] {
                            for (j, v) in captured.iter().enumerate() {
                                regs[(n as usize + j) & 255] = *v;
                            }
                        }
                    }
                    pc = prog.fns[f as usize].entry as usize;
                    tick!();
                }
                Op::CallB { a, n, id } => {
                    let v = tri!(natives::call(core, id, slice!(a, n)));
                    r!(a) = v;
                    work!();
                    allocated!();
                    if core.out.len() > FLUSH_AT {
                        break 'run Ok(Exit::Flush);
                    }
                    if fuel <= 0 {
                        break 'run Ok(Exit::Yield);
                    }
                }
                Op::Ret { a } => {
                    let v = r!(a);
                    leave!(v);
                }
                Op::RetUnit => {
                    leave!(Value::Unit);
                }
                Op::Closure { a, n, f } => {
                    let captured: Box<[Value]> = slice!(a, n).into();
                    r!(a) = tri!(core.heap.alloc(Obj::Closure(f as u32, captured)));
                    allocated!();
                }

                Op::NewList { a, n } => {
                    let items = slice!(a, n).to_vec();
                    r!(a) = tri!(core.heap.alloc(Obj::List(items)));
                    allocated!();
                }
                Op::NewStruct { a, n, ty } => {
                    let fields: Box<[Value]> = slice!(a, n).into();
                    let ty = if ty == TUPLE { NONE } else { ty as u32 };
                    r!(a) = tri!(core.heap.alloc(Obj::Struct(ty, fields)));
                    allocated!();
                }
                Op::NewEnum { a, n, variant } => {
                    let fields: Box<[Value]> = slice!(a, n).into();
                    r!(a) = tri!(core.heap.alloc(Obj::Enum(variant as u32, fields)));
                    allocated!();
                }
                Op::NewMap { a, n } => {
                    let mut map = KMap::new(core.heap.seed);
                    for pair in slice!(a, 2 * n as usize).chunks_exact(2) {
                        let key = tri!(core.heap.key(pair[0]));
                        map.set(key, pair[0], pair[1]);
                    }
                    r!(a) = tri!(core.heap.alloc(Obj::Map(Box::new(map))));
                    allocated!();
                }
                Op::GetField { a, b, idx } => {
                    let fields = match &core.heap.objs[obj!(b)] {
                        Obj::Struct(_, f) | Obj::Enum(_, f) => f,
                        _ => break 'run Err(bad()),
                    };
                    match fields.get(idx as usize) {
                        Some(v) => r!(a) = *v,
                        None => break 'run Err(bad()),
                    }
                }
                Op::SetField { a, b, idx } => {
                    let v = r!(b);
                    match &mut core.heap.objs[obj!(a)] {
                        Obj::Struct(_, f) if (idx as usize) < f.len() => f[idx as usize] = v,
                        _ => break 'run Err(bad()),
                    }
                }
                Op::ListGet { a, b, c } => {
                    let i = int!(c);
                    let Obj::List(items) = &core.heap.objs[obj!(b)] else { break 'run Err(bad()) };
                    let n = items.len();
                    let j = if i < 0 { i.wrapping_add(n as i64) } else { i };
                    if j < 0 || j as usize >= n {
                        fail!("index {i} is out of range for a list of length {n}");
                    }
                    r!(a) = items[j as usize];
                }
                Op::ListSet { a, b, c } => {
                    let (i, v) = (int!(b), r!(c));
                    let Obj::List(items) = &mut core.heap.objs[obj!(a)] else { break 'run Err(bad()) };
                    let n = items.len();
                    let j = if i < 0 { i.wrapping_add(n as i64) } else { i };
                    if j < 0 || j as usize >= n {
                        fail!("index {i} is out of range for a list of length {n}");
                    }
                    items[j as usize] = v;
                }
                Op::MapGet { a, b, c } => match tri!(core.heap.map_get(r!(b), r!(c))) {
                    Some(v) => r!(a) = v,
                    None => break 'run Err(natives::missing_key(core, r!(c))),
                },
                Op::MapSet { a, b, c } => {
                    tri!(core.heap.map_set(r!(a), r!(b), r!(c)));
                    allocated!();
                }
                Op::StrIndex { a, b, c } => {
                    r!(a) = tri!(natives::str_index(core, r!(b), int!(c)));
                    work!();
                    allocated!();
                }
                Op::Push { a, b } => {
                    let v = r!(b);
                    tri!(core.heap.charge(16));
                    tri!(core.heap.list_mut(r!(a))).push(v);
                    allocated!();
                }
                Op::Len { a, b } => {
                    r!(a) = Value::Int(tri!(natives::len(core, r!(b))));
                    work!();
                }
                Op::Concat { a, b, c } => {
                    r!(a) = tri!(natives::concat(core, r!(b), r!(c)));
                    work!();
                    allocated!();
                }
                Op::StrCat { a, n } => {
                    let v = tri!(natives::str_cat(core, slice!(a, n)));
                    r!(a) = v;
                    work!();
                    allocated!();
                }
                Op::Host { a } => {
                    *host_name = tri!(core.heap.str_of(r!(a))).to_string();
                    *host_payload = tri!(core.heap.str_of(r!(a.wrapping_add(1)))).to_string();
                    *host_reg = base + a as usize;
                    break 'run Ok(Exit::Host);
                }
                Op::Trap { .. } => fail!("no arm of the `match` fits the value"),
            }
        };
        self.pc = pc;
        self.base = base;
        *budget = fuel;
        exit
    }
}
