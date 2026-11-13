//! Showing values as text.
//!
//! A number with a unit is one float in SI base units at run time; its unit exists only in the
//! static type. So wherever a value turns into text (a `{...}` placeholder, `print`, `str`, the
//! last line of `kilat eval`) the checker must know which unit to print. When the program does
//! not name one, the checker refuses and suggests the unit the program itself used.
//!
//! The suggestions come from a small memory of the units written near each name. That memory
//! only affects how helpful a hint is: every suggested unit is checked against the real type.

use crate::ast::*;
use crate::check::*;
use crate::parser::parse_unit_str;
use crate::tir::*;
use crate::types::*;

const MAX_UNITS: usize = 8;
const SI_NOTE: &str = " (numbers with a unit are shown in SI base units)";

#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) enum ShowKind {
    Interp,
    Print,
    Str,
    Last,
}

/// One place where a value turns into text.
#[derive(Clone, Copy)]
pub(crate) struct Shown<'a> {
    pub expr: &'a Expr,
    pub kind: ShowKind,
    /// The literal text right after a placeholder.
    pub after: Option<&'a str>,
}

#[derive(Clone)]
pub(crate) struct ShowSite {
    kind: ShowKind,
    /// The expression as the program wrote it, or `value` when it is too long to repeat.
    name: String,
    /// `name`, in parentheses when it needs them before `in` or `.`.
    atom: String,
    named: bool,
    before: String,
    digits: String,
    units: Vec<String>,
    after: Vec<String>,
}

/// A show whose type was not known yet; judged again once checking is done.
pub(crate) struct LateShow {
    ty: Ty,
    pos: Pos,
    site: ShowSite,
    owner: Option<u32>,
}

/// One use of a generic function, kept so a unit cannot slip into text through a type parameter.
pub(crate) struct GenericUse {
    caller: Option<u32>,
    callee: u32,
    vars: Vec<Ty>,
    pos: Pos,
    name: String,
    units: Vec<String>,
}

#[derive(Default)]
struct Scan {
    quantity: Option<Dim>,
    params: Vec<u32>,
    has_var: bool,
}

fn push_unit(out: &mut Vec<String>, unit: &str) {
    if !unit.is_empty() && out.len() < MAX_UNITS && !out.iter().any(|u| u == unit) {
        out.push(unit.to_string());
    }
}

/// A unit the way a program writes it: `N*m`, `kg/m^3`, `1/s`.
pub(crate) fn unit_text(factors: &[(String, i32)]) -> String {
    let mut text = String::new();
    for (name, exp) in factors.iter().filter(|(_, exp)| *exp > 0) {
        if !text.is_empty() {
            text.push('*');
        }
        text.push_str(name);
        if *exp != 1 {
            text.push_str(&format!("^{}", exp));
        }
    }
    if text.is_empty() {
        text.push('1');
    }
    for (name, exp) in factors.iter().filter(|(_, exp)| *exp < 0) {
        text.push('/');
        text.push_str(name);
        if *exp != -1 {
            text.push_str(&format!("^{}", -exp));
        }
    }
    text
}

fn needs_parens(e: &Expr) -> bool {
    matches!(e.kind, ExprKind::Binary(..) | ExprKind::Neg(_) | ExprKind::UnitLit(..))
}

fn level(op: &BinOp) -> Option<(&'static str, u8)> {
    match op {
        BinOp::Add => Some(("+", 1)),
        BinOp::Sub => Some(("-", 1)),
        BinOp::Mul => Some(("*", 2)),
        BinOp::Div => Some(("/", 2)),
        _ => None,
    }
}

/// A short expression written back as source text, for use inside a hint.
fn expr_text(e: &Expr, depth: u32) -> Option<String> {
    if depth > 6 {
        return None;
    }
    let part = |inner: &Expr| -> Option<String> {
        let text = expr_text(inner, depth + 1)?;
        Some(if needs_parens(inner) { format!("({})", text) } else { text })
    };
    let arguments = |args: &[Arg]| -> Option<String> {
        if args.len() > 3 {
            return None;
        }
        let mut parts = Vec::new();
        for arg in args {
            let value = expr_text(&arg.value, depth + 1)?;
            parts.push(match &arg.name {
                Some(name) => format!("{}: {}", name, value),
                None => value,
            });
        }
        Some(parts.join(", "))
    };
    let text = match &e.kind {
        ExprKind::Ident(name) => name.clone(),
        ExprKind::Int(n) => n.to_string(),
        ExprKind::Float(x) => format!("{:?}", x),
        ExprKind::UnitLit(x, unit) => format!("{} {}", x, unit_text(&unit.factors)),
        ExprKind::Field(inner, name) => format!("{}.{}", part(inner)?, name),
        ExprKind::TupleIndex(inner, index) => format!("{}.{}", part(inner)?, index),
        ExprKind::Index(inner, index) => format!("{}[{}]", part(inner)?, expr_text(index, depth + 1)?),
        ExprKind::Call(callee, args) => format!("{}({})", part(callee)?, arguments(args)?),
        ExprKind::Method(recv, name, args) => format!("{}.{}({})", part(recv)?, name, arguments(args)?),
        ExprKind::Neg(inner) => format!("-{}", part(inner)?),
        ExprKind::Binary(op, left, right) => {
            let (symbol, own) = level(op)?;
            let side = |inner: &Expr, right_side: bool| -> Option<String> {
                let text = expr_text(inner, depth + 1)?;
                let wrap = match &inner.kind {
                    ExprKind::Binary(inner_op, _, _) => {
                        let (_, theirs) = level(inner_op)?;
                        theirs < own || (right_side && theirs == own)
                    }
                    _ => false,
                };
                Some(if wrap { format!("({})", text) } else { text })
            };
            format!("{} {} {}", side(left, false)?, symbol, side(right, true)?)
        }
        _ => return None,
    };
    if text.chars().count() > 40 {
        None
    } else {
        Some(text)
    }
}

/// The words right after a placeholder that could be meant as its unit: `{force} N` gives `N`.
fn words_after(text: &str) -> Vec<String> {
    let text = text.strip_prefix(' ').unwrap_or(text);
    let mut out = Vec::new();
    let run: String = text
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || matches!(c, '/' | '*' | '^'))
        .collect();
    if !text[run.len()..].starts_with('_') {
        push_unit(&mut out, run.trim_end_matches(|c: char| !c.is_ascii_alphanumeric()));
    }
    let word: String = text.chars().take_while(|c| c.is_ascii_alphabetic()).collect();
    let joined = matches!(text[word.len()..].chars().next(), Some(c) if c.is_ascii_alphanumeric() || c == '_');
    if !joined {
        push_unit(&mut out, &word);
    }
    out
}

fn pat_names<'a>(pat: &'a Pat, out: &mut Vec<&'a str>) {
    match pat {
        Pat::Bind(name, _) => out.push(name),
        Pat::Variant(_, parts, _) | Pat::Tuple(parts, _) => {
            for inner in parts {
                pat_names(inner, out);
            }
        }
        _ => {}
    }
}

impl Checker {
    // ---- units as text

    /// Like `unit_def`, for a unit written as text, without reporting anything.
    pub(crate) fn quiet_unit(&self, text: &str) -> Option<(Dim, f64, f64)> {
        let unit = parse_unit_str(text, Pos::default()).ok()?;
        let mut dim = Dim::none();
        let (mut num, mut den) = (1.0f64, 1.0f64);
        for (name, exp) in &unit.factors {
            let def = self.units.get(name)?;
            dim = def.dim.pow(*exp, 1).and_then(|d| dim.mul(&d))?;
            let k = exp.unsigned_abs().min(1000) as i32;
            let (n, d) = (def.num.powi(k), def.den.powi(k));
            if *exp >= 0 {
                num *= n;
                den *= d;
            } else {
                num *= d;
                den *= n;
            }
        }
        Some((dim, num, den))
    }

    fn unit_fits(&self, text: &str, dim: &Dim) -> bool {
        matches!(self.quiet_unit(text), Some((found, _, _)) if found == *dim)
    }

    /// Units a value of this dimension can be shown in, the ones the program wrote first.
    /// The second result says how many of them came from the program.
    fn unit_choices(&self, dim: &Dim, wanted: &[String]) -> (Vec<String>, usize) {
        let mut out: Vec<String> = Vec::new();
        for unit in wanted {
            if self.unit_fits(unit, dim) {
                push_unit(&mut out, unit);
            }
        }
        // The single units inside what the program wrote. `rpm` and `Hz` share a dimension but
        // not a scale, so a unit of that dimension is only ever offered exactly as written.
        let per_second = Dim::of(&[(2, -1)]);
        let mut simple: Vec<String> = Vec::new();
        for unit in wanted {
            if let Ok(parsed) = parse_unit_str(unit, Pos::default()) {
                for (name, _) in &parsed.factors {
                    let usable = match self.units.get(name) {
                        Some(def) => !def.dim.is_none() && def.dim != per_second,
                        None => false,
                    };
                    if usable {
                        push_unit(&mut simple, name);
                    }
                }
            }
        }
        for name in &simple {
            if self.unit_fits(name, dim) {
                push_unit(&mut out, name);
            }
        }
        if out.is_empty() {
            if let Some(found) = self.compose_unit(&simple, dim) {
                out.push(found);
            }
        }
        let from_program = out.len();
        let mut declared: Vec<&String> = self.units.keys().filter(|name| !self.unit_order.contains(name)).collect();
        declared.sort();
        for name in declared {
            if self.unit_fits(name, dim) {
                push_unit(&mut out, name);
            }
        }
        let si_name = dim_name(dim, &self.bases);
        if self.unit_fits(&si_name, dim) {
            push_unit(&mut out, &si_name);
        }
        for name in &self.unit_order {
            if name != "rpm" && self.unit_fits(name, dim) {
                push_unit(&mut out, name);
            }
        }
        (out, from_program)
    }

    /// The first product or quotient of the program's own units that has this dimension.
    fn compose_unit(&self, simple: &[String], dim: &Dim) -> Option<String> {
        for a in simple {
            for text in [format!("{}^2", a), format!("{}^3", a)] {
                if self.unit_fits(&text, dim) {
                    return Some(text);
                }
            }
        }
        for a in simple {
            for b in simple {
                if a == b {
                    continue;
                }
                let tries = [
                    format!("{}*{}", a, b),
                    format!("{}/{}", a, b),
                    format!("{}/{}^2", a, b),
                    format!("{}/{}^3", a, b),
                ];
                for text in tries {
                    if self.unit_fits(&text, dim) {
                        return Some(text);
                    }
                }
            }
        }
        None
    }

    /// The unit to suggest, and a note with other units when the program gave no clue.
    fn pick_unit(&self, dim: &Dim, wanted: &[String]) -> Option<(String, String)> {
        let (choices, from_program) = self.unit_choices(dim, wanted);
        let first = choices.first()?.clone();
        let mut note = String::new();
        if from_program == 0 && choices.len() > 1 {
            let others: Vec<String> = choices[1..].iter().take(3).map(|unit| format!("`{}`", unit)).collect();
            note = format!(" (or another unit of the same kind: {})", others.join(", "));
        }
        Some((first, note))
    }

    // ---- which units the program wrote near a name

    fn recall(&self, key: &str, out: &mut Vec<String>) {
        let found = match self.unit_local.get(key) {
            Some(units) => Some(units),
            None => self.unit_seen.get(key),
        };
        if let Some(units) = found {
            for unit in units {
                push_unit(out, unit);
            }
        }
    }

    fn remember(&mut self, key: &str, mut units: Vec<String>) {
        units.truncate(6);
        if self.cur_fn.is_some() {
            self.unit_local.insert(key.to_string(), units);
        } else {
            self.unit_seen.insert(key.to_string(), units);
        }
    }

    fn note_global(&mut self, key: String, units: Vec<String>) {
        if units.is_empty() {
            return;
        }
        let slot = self.unit_seen.entry(key).or_default();
        for unit in &units {
            if slot.len() < 6 {
                push_unit(slot, unit);
            }
        }
    }

    fn units_near(&self, ty: Option<&TypeExpr>, init: Option<&Expr>) -> Vec<String> {
        let mut units = Vec::new();
        if let Some(t) = ty {
            self.units_in_type(t, &mut units);
        }
        if let Some(e) = init {
            self.units_in(e, &mut units, 0);
        }
        units
    }

    pub(crate) fn note_unit(&mut self, name: &str, ty: Option<&TypeExpr>, init: Option<&Expr>) {
        let units = self.units_near(ty, init);
        self.remember(name, units);
    }

    /// The program showed this name in `unit`, so later hints for the same name offer that unit first.
    pub(crate) fn note_shown_unit(&mut self, e: &Expr, unit: &str) {
        let key = match &e.kind {
            ExprKind::Ident(name) => name.clone(),
            ExprKind::Field(_, name) => format!(".{}", name),
            _ => return,
        };
        let mut units = vec![unit.to_string()];
        self.recall(&key, &mut units);
        self.remember(&key, units);
    }

    pub(crate) fn note_pat(&mut self, pat: &Pat, ty: Option<&TypeExpr>, init: Option<&Expr>) {
        let mut names = Vec::new();
        pat_names(pat, &mut names);
        if names.is_empty() {
            return;
        }
        let units = self.units_near(ty, init);
        for name in names {
            self.remember(name, units.clone());
        }
    }

    pub(crate) fn note_field(&mut self, name: &str, ty: &TypeExpr) {
        let units = self.units_near(Some(ty), None);
        self.note_global(format!(".{}", name), units);
    }

    /// The unit in a declared return type. Type parameters are in scope so `K` is not read as kelvin.
    pub(crate) fn note_fn_ret(&mut self, d: &FnDecl) {
        if let Some(ret) = &d.ret {
            let saved = std::mem::replace(&mut self.tscope, d.tparams.clone());
            let units = self.units_near(Some(ret), None);
            self.tscope = saved;
            self.note_global(format!("{}()", d.name), units);
        }
    }

    /// The unit of a function without a declared return type, from the last expression of its body.
    pub(crate) fn note_fn_tail(&mut self, d: &FnDecl) {
        if let (None, Some(tail)) = (&d.ret, &d.body.tail) {
            let units = self.units_near(None, Some(tail));
            self.note_global(format!("{}()", d.name), units);
        }
    }

    /// Parameters of lambdas passed in a call take the units of the values they are applied to.
    pub(crate) fn note_lambdas(&mut self, recv: Option<&Expr>, args: &[Arg]) {
        if !args.iter().any(|arg| matches!(arg.value.kind, ExprKind::Lambda(..))) {
            return;
        }
        let mut around = Vec::new();
        if let Some(r) = recv {
            self.units_in(r, &mut around, 0);
        }
        for arg in args {
            if !matches!(arg.value.kind, ExprKind::Lambda(..)) {
                self.units_in(&arg.value, &mut around, 0);
            }
        }
        for arg in args {
            if let ExprKind::Lambda(params, _) = &arg.value.kind {
                for (name, ty, _) in params {
                    let mut units = self.units_near(ty.as_ref(), None);
                    if units.is_empty() {
                        units = around.clone();
                    }
                    self.remember(name, units);
                }
            }
        }
    }

    fn units_in(&self, e: &Expr, out: &mut Vec<String>, depth: u32) {
        if depth > 12 || out.len() >= MAX_UNITS {
            return;
        }
        let next = depth + 1;
        match &e.kind {
            ExprKind::UnitLit(_, unit) => push_unit(out, &unit_text(&unit.factors)),
            ExprKind::Ident(name) => self.recall(name, out),
            ExprKind::Field(inner, name) => {
                self.recall(&format!(".{}", name), out);
                self.units_in(inner, out, next);
            }
            ExprKind::Call(callee, args) => {
                if let ExprKind::Ident(name) = &callee.kind {
                    self.recall(&format!("{}()", name), out);
                }
                for arg in args.iter().take(4) {
                    self.units_in(&arg.value, out, next);
                }
            }
            ExprKind::Method(recv, name, args) => {
                self.recall(&format!("{}()", name), out);
                self.units_in(recv, out, next);
                for arg in args.iter().take(4) {
                    self.units_in(&arg.value, out, next);
                }
            }
            ExprKind::Neg(inner) | ExprKind::Try(inner) | ExprKind::TupleIndex(inner, _) | ExprKind::Index(inner, _) => {
                self.units_in(inner, out, next)
            }
            ExprKind::Binary(_, left, right) | ExprKind::Coalesce(left, right) => {
                self.units_in(left, out, next);
                self.units_in(right, out, next);
            }
            ExprKind::List(items) | ExprKind::Tuple(items) => {
                for item in items.iter().take(4) {
                    self.units_in(item, out, next);
                }
            }
            ExprKind::Map(pairs) => {
                for (_, value) in pairs.iter().take(4) {
                    self.units_in(value, out, next);
                }
            }
            ExprKind::If(_, then, other) => {
                if let Some(tail) = &then.tail {
                    self.units_in(tail, out, next);
                }
                if let Some(inner) = other {
                    self.units_in(inner, out, next);
                }
            }
            ExprKind::Block(block) => {
                if let Some(tail) = &block.tail {
                    self.units_in(tail, out, next);
                }
            }
            _ => {}
        }
    }

    fn units_in_type(&self, t: &TypeExpr, out: &mut Vec<String>) {
        match t {
            TypeExpr::Unit(unit) => push_unit(out, &unit_text(&unit.factors)),
            TypeExpr::Name(name, args, _) => {
                let is_type = self.tscope.contains(name)
                    || self.struct_names.contains_key(name)
                    || self.enum_names.contains_key(name);
                if args.is_empty() && !is_type && self.units.contains_key(name) {
                    push_unit(out, name);
                }
                for arg in args {
                    self.units_in_type(arg, out);
                }
            }
            TypeExpr::Opt(inner, _) => self.units_in_type(inner, out),
            TypeExpr::Tuple(parts, _) => {
                for inner in parts {
                    self.units_in_type(inner, out);
                }
            }
            TypeExpr::Fn(..) => {}
        }
    }

    // ---- what a type holds

    fn scan(&self, t: &Ty) -> Scan {
        let mut out = Scan::default();
        self.scan_ty(t, false, &mut Vec::new(), &mut out);
        out
    }

    /// Find the first number with a unit, and the type parameters and unknowns that could become one.
    /// Inside a declaration (`in_decl`) a type parameter stands for a type argument already scanned.
    fn scan_ty(&self, t: &Ty, in_decl: bool, seen: &mut Vec<(bool, u32)>, out: &mut Scan) {
        match self.resolve(t) {
            Ty::Num(dim) => {
                if !dim.is_none() && out.quantity.is_none() {
                    out.quantity = Some(dim);
                }
            }
            Ty::List(inner) => self.scan_ty(&inner, in_decl, seen, out),
            Ty::Map(key, value) => {
                self.scan_ty(&key, in_decl, seen, out);
                self.scan_ty(&value, in_decl, seen, out);
            }
            Ty::Tuple(parts) => {
                for inner in &parts {
                    self.scan_ty(inner, in_decl, seen, out);
                }
            }
            Ty::Struct(id, targs) => {
                for inner in &targs {
                    self.scan_ty(inner, in_decl, seen, out);
                }
                if !seen.contains(&(false, id)) {
                    seen.push((false, id));
                    for (_, field) in &self.structs[id as usize].fields {
                        self.scan_ty(field, true, seen, out);
                    }
                }
            }
            Ty::Enum(id, targs) => {
                for inner in &targs {
                    self.scan_ty(inner, in_decl, seen, out);
                }
                if !seen.contains(&(true, id)) {
                    seen.push((true, id));
                    for variant in &self.enums[id as usize].variants {
                        for field in &self.variants[*variant as usize].fields {
                            self.scan_ty(field, true, seen, out);
                        }
                    }
                }
            }
            Ty::Param(k) => {
                if !in_decl && !out.params.contains(&k) {
                    out.params.push(k);
                }
            }
            Ty::Var(_) => out.has_var = true,
            _ => {}
        }
    }

    fn is_shown(&self, fid: u32, k: usize) -> bool {
        self.fns[fid as usize].shown.get(k).copied().unwrap_or(false)
    }

    fn mark_shown(&mut self, fid: u32, k: usize) {
        let shown = &mut self.fns[fid as usize].shown;
        if shown.len() <= k {
            shown.resize(k + 1, false);
        }
        shown[k] = true;
    }

    // ---- the rule

    /// A value of type `ty` turns into text here without a named unit.
    pub(crate) fn check_shown(&mut self, ty: &Ty, spec: Option<&str>, at: Shown) {
        let ty = self.zonk(ty);
        let scan = self.scan(&ty);
        if scan.quantity.is_none() && scan.params.is_empty() && !scan.has_var {
            return;
        }
        let site = self.show_site(at, spec);
        let (pos, owner) = (at.expr.pos, self.cur_fn);
        if scan.quantity.is_none() && scan.has_var {
            self.late_shows.push(LateShow { ty: ty.clone(), pos, site: site.clone(), owner });
        }
        self.judge_shown(&ty, pos, &site, owner, &scan);
    }

    fn judge_shown(&mut self, ty: &Ty, pos: Pos, site: &ShowSite, owner: Option<u32>, scan: &Scan) {
        if let Some(dim) = &scan.quantity {
            self.refuse_shown(ty, dim, pos, site);
            return;
        }
        if let Some(fid) = owner {
            for k in &scan.params {
                self.mark_shown(fid, *k as usize);
            }
        }
    }

    /// Called once every body is checked: judge the shows whose type was unknown, then make sure
    /// no generic function was handed a value with a unit that it turns into text.
    pub(crate) fn finish_shows(&mut self) {
        for late in std::mem::take(&mut self.late_shows) {
            let ty = self.zonk(&late.ty);
            let scan = self.scan(&ty);
            self.judge_shown(&ty, late.pos, &late.site, late.owner, &scan);
        }
        let uses = std::mem::take(&mut self.generic_uses);
        if uses.is_empty() || !self.fns.iter().any(|f| f.shown.contains(&true)) {
            return;
        }
        let scans: Vec<Vec<Scan>> = uses
            .iter()
            .map(|used| used.vars.iter().map(|var| self.scan(&self.zonk(var))).collect())
            .collect();
        // A function that passes its own type parameter to a function that shows it, shows it too.
        let mut changed = true;
        while changed {
            changed = false;
            for (used, scan) in uses.iter().zip(&scans) {
                let caller = match used.caller {
                    Some(fid) => fid,
                    None => continue,
                };
                for (k, arg) in scan.iter().enumerate() {
                    if !self.is_shown(used.callee, k) {
                        continue;
                    }
                    for j in &arg.params {
                        if !self.is_shown(caller, *j as usize) {
                            self.mark_shown(caller, *j as usize);
                            changed = true;
                        }
                    }
                }
            }
        }
        for (used, scan) in uses.iter().zip(&scans) {
            let hit = scan.iter().enumerate().find(|(k, arg)| arg.quantity.is_some() && self.is_shown(used.callee, *k));
            let dim = match hit {
                Some((_, arg)) => arg.quantity.clone().unwrap_or_default(),
                None => continue,
            };
            let msg = format!("`{}` turns this value into text, but the value has a unit", used.name);
            let hint = match self.pick_unit(&dim, &used.units) {
                Some((unit, _)) => format!(
                    "name the unit before the call: pass a text such as `\"{{value:{0}}}\"`, or a plain number such as `value in {0}`",
                    unit
                ),
                None => "name the unit before the call: pass a text that shows the number with its unit".to_string(),
            };
            self.err_hint(used.pos, msg, hint);
        }
    }

    /// Remember a call of a generic function with the type variables that stand for its parameters.
    pub(crate) fn note_generic_use(&mut self, callee: u32, vars: &[Ty], pos: Pos, name: &str, argv: &[&Expr]) {
        if self.fns[callee as usize].origin == Origin::Prelude {
            return;
        }
        let mut units = Vec::new();
        for arg in argv.iter().take(4) {
            self.units_in(arg, &mut units, 0);
        }
        self.generic_uses.push(GenericUse {
            caller: self.cur_fn,
            callee,
            vars: vars.to_vec(),
            pos,
            name: name.to_string(),
            units,
        });
    }

    fn show_site(&self, at: Shown, spec: Option<&str>) -> ShowSite {
        let (name, atom, named) = match expr_text(at.expr, 0) {
            Some(text) => {
                let atom = if needs_parens(at.expr) { format!("({})", text) } else { text.clone() };
                (text, atom, true)
            }
            None => ("value".to_string(), "value".to_string(), false),
        };
        let spec = spec.unwrap_or("").trim();
        let lead = spec.chars().take_while(|c| matches!(c, '<' | '>') || c.is_ascii_digit()).count();
        let digits = match spec.rfind('.') {
            Some(at_dot) => spec[at_dot..].to_string(),
            None => String::new(),
        };
        let mut units = Vec::new();
        self.units_in(at.expr, &mut units, 0);
        let after = match (at.kind, at.after) {
            (ShowKind::Interp, Some(text)) => words_after(text),
            _ => Vec::new(),
        };
        ShowSite { kind: at.kind, name, atom, named, before: spec[..lead].to_string(), digits, units, after }
    }

    /// `{force:N} N` would print the unit twice.
    pub(crate) fn second_unit_after(&mut self, unit: &str, after: &str, pos: Pos) {
        if words_after(after).first().map(|word| word.as_str()) == Some(unit) {
            let msg = format!(
                "this placeholder already prints `{0}`, so the `{0}` after the braces would be a second unit",
                unit
            );
            self.err_hint(pos, msg, format!("delete the `{}` after the braces", unit));
        }
    }

    /// Extra arguments for `assert_eq`, so a failure reads `2.4 mm is not equal to 2.5 mm`.
    pub(crate) fn assert_eq_unit(&self, left: &Ty, argv: &[&Expr]) -> Vec<TExpr> {
        let ty = self.zonk(left);
        let pos = argv[0].pos;
        let scan = self.scan(&ty);
        let dim = match (&ty, &scan.quantity) {
            (Ty::Num(dim), Some(_)) => dim.clone(),
            (_, Some(_)) => return vec![te(TKind::Str(SI_NOTE.to_string()), pos)],
            _ => return Vec::new(),
        };
        let mut wanted = Vec::new();
        for arg in argv.iter().take(2) {
            self.units_in(arg, &mut wanted, 0);
        }
        let (choices, _) = self.unit_choices(&dim, &wanted);
        let found = choices.first().and_then(|unit| self.quiet_unit(unit).map(|(_, num, den)| (unit, num, den)));
        match found {
            Some((unit, num, den)) => vec![
                te(TKind::Float(den), pos),
                te(TKind::Float(num), pos),
                te(TKind::Str(format!(" {}", unit)), pos),
            ],
            None => vec![te(TKind::Str(SI_NOTE.to_string()), pos)],
        }
    }

    fn refuse_shown(&mut self, ty: &Ty, dim: &Dim, pos: Pos, site: &ShowSite) {
        let mut wanted: Vec<String> = site.after.iter().filter(|word| self.unit_fits(word, dim)).cloned().collect();
        let after_fit = wanted.first().cloned();
        wanted.extend(site.units.iter().cloned());
        let (unit, others) = match self.pick_unit(dim, &wanted) {
            Some(found) => found,
            None => {
                let msg = format!(
                    "this value has a unit that cannot be written in a format (`{}`)",
                    dim_name(dim, &self.bases)
                );
                let hint = "finish the calculation so the unit is a whole one, or divide by a value of the same unit to get a plain number";
                self.err_hint(pos, msg, hint);
                return;
            }
        };
        let (msg, hint) = match ty {
            Ty::Num(_) => {
                let mut hint = match site.kind {
                    ShowKind::Interp => format!("write `{{{}:{}{}{}}}`", site.name, site.before, unit, site.digits),
                    ShowKind::Print => format!("put it in a text with its unit: `\"{{{}:{}}}\"`", site.name, unit),
                    ShowKind::Str => {
                        format!("write `\"{{{0}:{1}}}\"` in place of `str({0})`", site.name, unit)
                    }
                    ShowKind::Last => format!(
                        "end with `{} in {}` for the plain number, or show it as a text: `\"{{{}:{}}}\"`",
                        site.atom, unit, site.name, unit
                    ),
                };
                if site.kind == ShowKind::Interp && after_fit.as_deref() == Some(unit.as_str()) {
                    hint.push_str(&format!(" and delete the `{}` after the braces", unit));
                } else {
                    hint.push_str(&others);
                }
                if !site.named {
                    hint.push_str("; `value` stands for the expression shown");
                }
                ("this value has a unit, so say which unit to show it in".to_string(), hint)
            }
            Ty::List(inner) => {
                let direct = matches!(self.resolve(inner), Ty::Num(_));
                let hint = if direct && site.named {
                    format!(
                        "for the plain numbers write `{0}.map(|x| x in {1})`, or show them one by one: `for x in {0} {{ print(\"{{x:{1}}}\") }}`",
                        site.atom, unit
                    )
                } else {
                    format!("show each number on its own with its unit, e.g. `\"{{x:{}}}\"` inside a loop", unit)
                };
                ("this list holds values with a unit, so say which unit to show them in".to_string(), hint)
            }
            Ty::Map(_, value) => {
                let direct = matches!(self.resolve(value), Ty::Num(_));
                let hint = if direct && site.named {
                    format!(
                        "show them one by one: `for (key, value) in {}.items() {{ print(\"{{key}}: {{value:{}}}\") }}`",
                        site.atom, unit
                    )
                } else {
                    format!(
                        "show them one by one in a loop over `.items()`, each number with its unit, e.g. `\"{{value:{}}}\"`",
                        unit
                    )
                };
                ("this map holds values with a unit, so say which unit to show them in".to_string(), hint)
            }
            Ty::Tuple(parts) => {
                let mut hint = "take the parts out first, e.g. `let (a, b) = ...`, then show each number with its unit".to_string();
                if site.named {
                    for (index, inner) in parts.iter().enumerate() {
                        if let Ty::Num(part_dim) = self.resolve(inner) {
                            if let Some((part_unit, _)) = self.pick_unit(&part_dim, &wanted) {
                                hint = format!(
                                    "show the parts one by one, e.g. `\"{{{}.{}:{}}}\"`",
                                    site.atom, index, part_unit
                                );
                                break;
                            }
                        }
                    }
                }
                ("this tuple holds a value with a unit, so say which unit to show it in".to_string(), hint)
            }
            Ty::Struct(id, targs) => self.refuse_struct(*id, targs, site, &unit),
            Ty::Enum(id, targs) if *id == E_OPTION => {
                let direct = matches!(targs.first().map(|inner| self.resolve(inner)), Some(Ty::Num(_)));
                let hint = if direct && site.named {
                    format!(
                        "take the number out first, e.g. `let value = {0} ?? 0 {1}`, then show `\"{{value:{1}}}\"` (or use `match`)",
                        site.atom, unit
                    )
                } else {
                    "take the value out with `match` or `??` first, then show its numbers with their units".to_string()
                };
                ("this optional value has a unit, so say which unit to show it in".to_string(), hint)
            }
            Ty::Enum(id, _) => (
                format!(
                    "this `{}` holds a value with a unit, so say which unit to show it in",
                    self.enums[*id as usize].name
                ),
                "take the number out with `match` first, then show it with its unit".to_string(),
            ),
            _ => (
                "this value holds a number with a unit, so say which unit to show it in".to_string(),
                "show each number on its own with its unit".to_string(),
            ),
        };
        self.err_hint(pos, msg, hint);
    }

    fn refuse_struct(&self, id: u32, targs: &[Ty], site: &ShowSite, unit: &str) -> (String, String) {
        let info = &self.structs[id as usize];
        for (field, declared) in &info.fields {
            let field_ty = Self::inst(declared, targs);
            let field_dim = match self.scan(&field_ty).quantity {
                Some(dim) => dim,
                None => continue,
            };
            let msg = format!(
                "this `{}` holds a value with a unit (field `{}`), so say which unit to show it in",
                info.name, field
            );
            let direct = matches!(self.resolve(&field_ty), Ty::Num(_));
            if !direct {
                return (msg, format!("show the field `{}` on its own, with the unit of its numbers", field));
            }
            let mut wanted = Vec::new();
            self.recall(&format!(".{}", field), &mut wanted);
            wanted.extend(site.units.iter().cloned());
            let field_unit = match self.pick_unit(&field_dim, &wanted) {
                Some((found, _)) => found,
                None => unit.to_string(),
            };
            let owner = if site.named { site.atom.as_str() } else { "value" };
            return (msg, format!("show the fields one by one, e.g. `\"{{{}.{}:{}}}\"`", owner, field, field_unit));
        }
        (
            format!("this `{}` holds a value with a unit, so say which unit to show it in", info.name),
            "show its numbers one by one, each with its unit".to_string(),
        )
    }
}
