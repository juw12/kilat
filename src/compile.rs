//! Bytecode compiler: typed IR -> register bytecode.
//!
//! Every function gets a window of up to 256 registers. Arguments sit in
//! consecutive registers starting at `a`; the callee's window starts there, so
//! its register 0 is the caller's `a`, which is also where the result lands.

use crate::builtins::B;
use crate::tir::*;
use crate::types::{Diag, Pos};
use std::collections::HashMap;

#[rustfmt::skip]
#[derive(Clone, Copy, Debug)]
pub enum Op {
    Move { a: u8, b: u8 }, LoadK { a: u8, k: u32 }, LoadInt { a: u8, v: i32 }, LoadBool { a: u8, v: bool },
    LoadUnit { a: u8 }, LoadTag { a: u8, tag: u16 }, LoadFn { a: u8, f: u32 },
    GetGlobal { a: u8, g: u32 }, SetGlobal { a: u8, g: u32 },
    AddI { a: u8, b: u8, c: u8 }, SubI { a: u8, b: u8, c: u8 }, MulI { a: u8, b: u8, c: u8 },
    DivI { a: u8, b: u8, c: u8 }, ModI { a: u8, b: u8, c: u8 }, PowI { a: u8, b: u8, c: u8 },
    NegI { a: u8, b: u8 }, AddII { a: u8, b: u8, imm: i32 }, MulII { a: u8, b: u8, imm: i32 },
    /// Floor modulo and floor division by a literal power of two: `b & imm` and `b >> imm`.
    AndII { a: u8, b: u8, imm: i32 }, ShrII { a: u8, b: u8, imm: i32 },
    AddF { a: u8, b: u8, c: u8 }, SubF { a: u8, b: u8, c: u8 }, MulF { a: u8, b: u8, c: u8 },
    DivF { a: u8, b: u8, c: u8 }, ModF { a: u8, b: u8, c: u8 }, PowF { a: u8, b: u8, c: u8 },
    PowFI { a: u8, b: u8, c: u8 }, NegF { a: u8, b: u8 }, IntToFloat { a: u8, b: u8 }, Sqrt { a: u8, b: u8 },
    EqI { a: u8, b: u8, c: u8 }, NeI { a: u8, b: u8, c: u8 }, LtI { a: u8, b: u8, c: u8 }, LeI { a: u8, b: u8, c: u8 },
    EqF { a: u8, b: u8, c: u8 }, NeF { a: u8, b: u8, c: u8 }, LtF { a: u8, b: u8, c: u8 }, LeF { a: u8, b: u8, c: u8 },
    EqV { a: u8, b: u8, c: u8 }, NeV { a: u8, b: u8, c: u8 }, LtS { a: u8, b: u8, c: u8 }, LeS { a: u8, b: u8, c: u8 },
    Not { a: u8, b: u8 },
    Jmp { to: u32 }, Loop { to: u32 }, JmpT { a: u8, to: u32 }, JmpF { a: u8, to: u32 },
    JeqI { a: u8, b: u8, to: u32 }, JneI { a: u8, b: u8, to: u32 }, JltI { a: u8, b: u8, to: u32 }, JleI { a: u8, b: u8, to: u32 },
    JeqII { a: u8, imm: i16, to: u32 }, JneII { a: u8, imm: i16, to: u32 }, JltII { a: u8, imm: i16, to: u32 },
    JleII { a: u8, imm: i16, to: u32 }, JgtII { a: u8, imm: i16, to: u32 }, JgeII { a: u8, imm: i16, to: u32 },
    JltF { a: u8, b: u8, to: u32 }, JleF { a: u8, b: u8, to: u32 }, JnltF { a: u8, b: u8, to: u32 },
    JnleF { a: u8, b: u8, to: u32 }, JeqF { a: u8, b: u8, to: u32 }, JneF { a: u8, b: u8, to: u32 },
    JTag { a: u8, tag: u16, to: u32 }, JnTag { a: u8, tag: u16, to: u32 },
    /// `a += 1; if a < b jump`
    ForI { a: u8, b: u8, to: u32 },
    /// `if a < b { a += 1; jump }`
    ForIE { a: u8, b: u8, to: u32 },
    /// list in `a`, index in `a+1`, element in `a+2`: `if idx < len { elem = list[idx]; idx += 1; jump }`
    ForList { a: u8, to: u32 },
    Call { a: u8, to: u32 }, CallC { a: u8, n: u8 }, CallB { a: u8, n: u8, id: u16 },
    Ret { a: u8 }, RetUnit, Closure { a: u8, n: u8, f: u16 },
    NewList { a: u8, n: u8 }, NewStruct { a: u8, n: u8, ty: u16 }, NewEnum { a: u8, n: u8, variant: u16 }, NewMap { a: u8, n: u8 },
    GetField { a: u8, b: u8, idx: u16 }, SetField { a: u8, b: u8, idx: u16 },
    ListGet { a: u8, b: u8, c: u8 }, ListSet { a: u8, b: u8, c: u8 }, MapGet { a: u8, b: u8, c: u8 }, MapSet { a: u8, b: u8, c: u8 },
    StrIndex { a: u8, b: u8, c: u8 }, Push { a: u8, b: u8 }, Len { a: u8, b: u8 }, Concat { a: u8, b: u8, c: u8 }, StrCat { a: u8, n: u8 },
    Host { a: u8 }, Trap { code: u8 },
}

const _: () = assert!(std::mem::size_of::<Op>() == 8);

/// `ty` of a `NewStruct` that builds a tuple.
pub const TUPLE: u16 = u16::MAX;

#[derive(Clone, Debug)]
pub enum Const {
    Int(i64),
    Float(f64),
    Str(String),
}

#[derive(Clone, Debug)]
pub struct FnMeta {
    pub entry: u32,
    pub name: String,
    pub nparams: u32,
}

#[derive(Clone, Debug)]
pub struct Program {
    pub code: Vec<Op>,
    /// `Pos::pack()` of the source position of every op.
    pub lines: Vec<u32>,
    pub consts: Vec<Const>,
    pub fns: Vec<FnMeta>,
    pub nglobals: u32,
    pub global_names: Vec<String>,
    pub structs: Vec<(String, Vec<String>)>,
    pub variants: Vec<(String, u32)>,
    pub main: u32,
    pub tests: Vec<(String, u32)>,
    pub files: Vec<String>,
}

#[derive(Hash, PartialEq, Eq)]
enum ConstKey {
    Int(i64),
    Float(u64),
    Str(String),
}

struct LoopCtx {
    /// Start of the condition for a `while` loop; `None` for `for` loops.
    head: Option<u32>,
    breaks: Vec<usize>,
    continues: Vec<usize>,
}

#[derive(Clone, Copy)]
enum Mode {
    Value(u8, bool),
    Discard,
    Tail,
}

const NO_REG: u8 = 255;
const MAX_REG: u32 = 250;

struct Compiler<'a> {
    ck: &'a Checked,
    code: Vec<Op>,
    lines: Vec<u32>,
    consts: Vec<Const>,
    const_ids: HashMap<ConstKey, u32>,
    /// Register of a top-level variable that lives in a register of main.
    top_reg: Vec<u8>,
    slot_reg: Vec<u8>,
    /// Next free register.
    top: u32,
    loops: Vec<LoopCtx>,
    in_main: bool,
    ret_unit: bool,
    pos: u32,
    err: Option<Diag>,
    fn_name: String,
    fn_pos: Pos,
    calls: Vec<usize>,
}

pub fn compile(ck: &Checked, files: Vec<String>) -> Result<Program, Diag> {
    let limit = u16::MAX as usize - 1;
    if ck.fns.len() > limit || ck.variants.len() > limit || ck.structs.len() > limit {
        return Err(Diag::new(Pos::default(), "the program is too large to compile"));
    }
    let mut c = Compiler {
        ck,
        code: Vec::new(),
        lines: Vec::new(),
        consts: Vec::new(),
        const_ids: HashMap::new(),
        top_reg: vec![NO_REG; ck.tops.len()],
        slot_reg: Vec::new(),
        top: 0,
        loops: Vec::new(),
        in_main: false,
        ret_unit: true,
        pos: 0,
        err: None,
        fn_name: String::new(),
        fn_pos: Pos::default(),
        calls: Vec::new(),
    };
    let mut fns = Vec::with_capacity(ck.fns.len());
    for (id, f) in ck.fns.iter().enumerate() {
        let entry = c.code.len() as u32;
        c.function(f, id as u32 == ck.main);
        fns.push(FnMeta { entry, name: f.name.clone(), nparams: f.nparams });
    }
    for at in std::mem::take(&mut c.calls) {
        if let Op::Call { to, .. } = &mut c.code[at] {
            *to = fns[*to as usize].entry;
        }
    }
    if let Some(d) = c.err {
        return Err(d);
    }
    Ok(Program {
        code: c.code,
        lines: c.lines,
        consts: c.consts,
        fns,
        nglobals: ck.tops.len() as u32,
        global_names: ck.tops.iter().map(|t| t.0.clone()).collect(),
        structs: ck.structs.clone(),
        variants: ck.variants.clone(),
        main: ck.main,
        tests: ck.tests.clone(),
        files,
    })
}

fn lit(e: &TExpr) -> Option<i64> {
    match e.kind {
        TKind::Int(v) => Some(v),
        _ => None,
    }
}

fn lit16(e: &TExpr) -> Option<i16> {
    lit(e).and_then(|v| i16::try_from(v).ok())
}

fn lit32(e: &TExpr) -> Option<i32> {
    lit(e).and_then(|v| i32::try_from(v).ok())
}

impl<'a> Compiler<'a> {
    fn emit(&mut self, op: Op) -> usize {
        self.code.push(op);
        self.lines.push(self.pos);
        self.code.len() - 1
    }

    fn here(&self) -> u32 {
        self.code.len() as u32
    }

    fn set_pos(&mut self, pos: Pos) {
        if pos.line != 0 {
            self.pos = pos.pack();
        }
    }

    #[rustfmt::skip]
    fn patch(&mut self, at: usize, target: u32) {
        use Op::*;
        match &mut self.code[at] {
            Jmp { to } | Loop { to } | JmpT { to, .. } | JmpF { to, .. }
            | JeqI { to, .. } | JneI { to, .. } | JltI { to, .. } | JleI { to, .. }
            | JeqII { to, .. } | JneII { to, .. } | JltII { to, .. } | JleII { to, .. } | JgtII { to, .. } | JgeII { to, .. }
            | JltF { to, .. } | JleF { to, .. } | JnltF { to, .. } | JnleF { to, .. } | JeqF { to, .. } | JneF { to, .. }
            | JTag { to, .. } | JnTag { to, .. } | ForI { to, .. } | ForIE { to, .. } | ForList { to, .. } => *to = target,
            _ => {}
        }
    }

    fn patch_all(&mut self, list: Vec<usize>) {
        let target = self.here();
        for at in list {
            self.patch(at, target);
        }
    }

    fn fail(&mut self, msg: String, hint: &str) {
        if self.err.is_none() {
            self.err = Some(Diag::new(self.fn_pos, msg).with_hint(hint));
        }
    }

    fn alloc(&mut self) -> u8 {
        if self.top >= MAX_REG {
            let msg = format!("`{}` is too complex to compile", self.fn_name);
            self.fail(msg, "split it into smaller functions");
            return 0;
        }
        self.top += 1;
        (self.top - 1) as u8
    }

    fn konst(&mut self, key: ConstKey) -> u32 {
        if let Some(&id) = self.const_ids.get(&key) {
            return id;
        }
        let id = self.consts.len() as u32;
        self.consts.push(match &key {
            ConstKey::Int(v) => Const::Int(*v),
            ConstKey::Float(bits) => Const::Float(f64::from_bits(*bits)),
            ConstKey::Str(s) => Const::Str(s.clone()),
        });
        self.const_ids.insert(key, id);
        id
    }

    fn load_int(&mut self, dst: u8, v: i64) {
        match i32::try_from(v) {
            Ok(v) => self.emit(Op::LoadInt { a: dst, v }),
            Err(_) => {
                let k = self.konst(ConstKey::Int(v));
                self.emit(Op::LoadK { a: dst, k })
            }
        };
    }

    fn slot(&self, slot: u32) -> u8 {
        self.slot_reg.get(slot as usize).copied().unwrap_or(0)
    }

    fn bind(&mut self, slot: u32, reg: u8) {
        if let Some(r) = self.slot_reg.get_mut(slot as usize) {
            *r = reg;
        }
    }

    /// Register of a top-level variable kept in a register of main, if any.
    fn top_in_reg(&self, id: u32) -> Option<u8> {
        let r = self.top_reg[id as usize];
        if self.in_main && r != NO_REG {
            Some(r)
        } else {
            None
        }
    }

    fn function(&mut self, f: &TFn, is_main: bool) {
        self.fn_name = f.name.clone();
        self.fn_pos = f.pos;
        self.in_main = is_main;
        self.ret_unit = f.ret_unit;
        self.loops.clear();
        self.slot_reg = vec![0; f.nslots as usize];
        let fixed = f.nparams as usize + f.captures.len();
        if fixed > 200 {
            let msg = format!("`{}` has too many parameters or captured variables", f.name);
            self.fail(msg, "pass a struct or a list instead");
        }
        for i in 0..f.nparams {
            self.bind(i, i as u8);
        }
        for (i, &slot) in f.captures.iter().enumerate() {
            self.bind(slot, (f.nparams as usize + i) as u8);
        }
        self.top = fixed.min(200) as u32;
        self.set_pos(f.pos);
        if f.ret_unit {
            self.discard(&f.body);
            self.emit(Op::RetUnit);
        } else {
            self.tail(&f.body);
        }
    }

    /// Evaluate `items` into consecutive registers. Returns the first register and the saved `top`.
    fn seq_regs(&mut self, items: &[&TExpr], dst: u8, fresh: bool) -> (u8, u32) {
        let saved = self.top;
        let base = if fresh && dst as u32 + 1 == self.top { dst } else { self.alloc() };
        for (i, item) in items.iter().enumerate() {
            let r = if i == 0 { base } else { self.alloc() };
            self.expr_to(item, r, true);
            self.top = (base as u32 + i as u32 + 1).max(saved);
        }
        (base, saved)
    }

    fn done(&mut self, base: u8, dst: u8, saved: u32) {
        if base != dst {
            self.emit(Op::Move { a: dst, b: base });
        }
        self.top = saved;
    }

    /// Evaluate into some register: a variable's own register or a new temporary.
    fn any(&mut self, e: &TExpr) -> u8 {
        match &e.kind {
            TKind::Local(s) => return self.slot(*s),
            TKind::Top(id) => {
                if let Some(r) = self.top_in_reg(*id) {
                    return r;
                }
            }
            _ => {}
        }
        let r = self.alloc();
        self.expr_to(e, r, true);
        r
    }

    /// Evaluate `e` into `dst`. `fresh` means no live variable is stored in `dst`.
    fn expr_to(&mut self, e: &TExpr, dst: u8, fresh: bool) {
        self.set_pos(e.pos);
        let saved = self.top;
        match &e.kind {
            TKind::Unit => {
                self.emit(Op::LoadUnit { a: dst });
            }
            TKind::Bool(v) => {
                self.emit(Op::LoadBool { a: dst, v: *v });
            }
            TKind::Int(v) => self.load_int(dst, *v),
            TKind::Float(v) => {
                let k = self.konst(ConstKey::Float(v.to_bits()));
                self.emit(Op::LoadK { a: dst, k });
            }
            TKind::Str(s) => {
                let k = self.konst(ConstKey::Str(s.clone()));
                self.emit(Op::LoadK { a: dst, k });
            }
            TKind::Local(s) => {
                let r = self.slot(*s);
                if r != dst {
                    self.emit(Op::Move { a: dst, b: r });
                }
            }
            TKind::Top(id) => match self.top_in_reg(*id) {
                Some(r) if r == dst => {}
                Some(r) => {
                    self.emit(Op::Move { a: dst, b: r });
                }
                None => {
                    self.emit(Op::GetGlobal { a: dst, g: *id });
                }
            },
            TKind::FnRef(f) => {
                self.emit(Op::LoadFn { a: dst, f: *f });
            }
            TKind::Tag(v) => {
                self.emit(Op::LoadTag { a: dst, tag: *v as u16 });
            }
            TKind::Bin(op, l, r) => self.bin(*op, l, r, dst),
            TKind::Un(op, x) => {
                let b = self.any(x);
                let a = dst;
                self.emit(match op {
                    Un1::NegI => Op::NegI { a, b },
                    Un1::NegF => Op::NegF { a, b },
                    Un1::Not => Op::Not { a, b },
                    Un1::IntToFloat => Op::IntToFloat { a, b },
                });
            }
            TKind::Copy(x) => self.expr_to(x, dst, fresh),
            TKind::And(l, r) | TKind::Or(l, r) => {
                let t = if fresh { dst } else { self.alloc() };
                self.expr_to(l, t, true);
                let is_and = matches!(e.kind, TKind::And(..));
                let j = self.emit(if is_and { Op::JmpF { a: t, to: 0 } } else { Op::JmpT { a: t, to: 0 } });
                self.expr_to(r, t, true);
                self.patch_all(vec![j]);
                if t != dst {
                    self.emit(Op::Move { a: dst, b: t });
                }
            }
            TKind::Call(f, args) => {
                let items: Vec<&TExpr> = args.iter().collect();
                let (base, saved) = self.seq_regs(&items, dst, fresh);
                let at = self.emit(Op::Call { a: base, to: *f });
                self.calls.push(at);
                self.done(base, dst, saved);
            }
            TKind::CallClosure(callee, args) => {
                let mut items: Vec<&TExpr> = vec![&**callee];
                items.extend(args.iter());
                let (base, saved) = self.seq_regs(&items, dst, fresh);
                self.emit(Op::CallC { a: base, n: args.len() as u8 });
                self.emit(Op::Move { a: dst, b: base.wrapping_add(1) });
                self.top = saved;
            }
            TKind::CallBuiltin(id, args) => self.builtin(*id, args, dst, fresh),
            TKind::Host(name, payload) => {
                let (base, saved) = self.seq_regs(&[&**name, &**payload], dst, fresh);
                self.emit(Op::Host { a: base });
                self.done(base, dst, saved);
            }
            TKind::NewList(items) if items.len() > 24 => {
                let t = self.alloc();
                self.emit(Op::NewList { a: t, n: 0 });
                for item in items {
                    let mark = self.top;
                    let r = self.any(item);
                    self.emit(Op::Push { a: t, b: r });
                    self.top = mark;
                }
                self.emit(Op::Move { a: dst, b: t });
            }
            TKind::NewList(items) => {
                let refs: Vec<&TExpr> = items.iter().collect();
                let (base, saved) = self.seq_regs(&refs, dst, fresh);
                self.emit(Op::NewList { a: base, n: items.len() as u8 });
                self.done(base, dst, saved);
            }
            TKind::NewMap(pairs) if pairs.len() > 8 => {
                let t = self.alloc();
                self.emit(Op::NewMap { a: t, n: 0 });
                for (k, v) in pairs {
                    let mark = self.top;
                    let b = self.any(k);
                    let c = self.any(v);
                    self.emit(Op::MapSet { a: t, b, c });
                    self.top = mark;
                }
                self.emit(Op::Move { a: dst, b: t });
            }
            TKind::NewMap(pairs) => {
                let mut refs: Vec<&TExpr> = Vec::new();
                for (k, v) in pairs {
                    refs.push(k);
                    refs.push(v);
                }
                let (base, saved) = self.seq_regs(&refs, dst, fresh);
                self.emit(Op::NewMap { a: base, n: pairs.len() as u8 });
                self.done(base, dst, saved);
            }
            TKind::NewTuple(items) | TKind::NewStruct(_, items) => {
                let ty = match &e.kind {
                    TKind::NewStruct(id, _) => *id as u16,
                    _ => TUPLE,
                };
                let refs: Vec<&TExpr> = items.iter().collect();
                let (base, saved) = self.seq_regs(&refs, dst, fresh);
                self.emit(Op::NewStruct { a: base, n: items.len() as u8, ty });
                self.done(base, dst, saved);
            }
            TKind::NewEnum(v, items) if items.is_empty() => {
                self.emit(Op::LoadTag { a: dst, tag: *v as u16 });
            }
            TKind::NewEnum(v, items) => {
                let refs: Vec<&TExpr> = items.iter().collect();
                let (base, saved) = self.seq_regs(&refs, dst, fresh);
                self.emit(Op::NewEnum { a: base, n: items.len() as u8, variant: *v as u16 });
                self.done(base, dst, saved);
            }
            TKind::Field(obj, idx) => {
                let b = self.any(obj);
                self.emit(Op::GetField { a: dst, b, idx: *idx as u16 });
            }
            TKind::IndexList(l, i) | TKind::IndexMap(l, i) | TKind::IndexStr(l, i) => {
                let b = self.any(l);
                let c = self.any(i);
                let a = dst;
                self.emit(match &e.kind {
                    TKind::IndexList(..) => Op::ListGet { a, b, c },
                    TKind::IndexMap(..) => Op::MapGet { a, b, c },
                    _ => Op::StrIndex { a, b, c },
                });
            }
            TKind::Lambda(f, slots) if slots.is_empty() => {
                self.emit(Op::LoadFn { a: dst, f: *f });
            }
            TKind::Lambda(f, slots) => {
                let base = if fresh && dst as u32 + 1 == self.top { dst } else { self.alloc() };
                for (i, &s) in slots.iter().enumerate() {
                    let r = if i == 0 { base } else { self.alloc() };
                    let from = self.slot(s);
                    self.emit(Op::Move { a: r, b: from });
                }
                self.emit(Op::Closure { a: base, n: slots.len() as u8, f: *f as u16 });
                if base != dst {
                    self.emit(Op::Move { a: dst, b: base });
                }
            }
            TKind::If(c, t, Some(els), false) => {
                let no = self.cond(c, false);
                self.expr_to(t, dst, fresh);
                self.top = saved;
                let j = self.emit(Op::Jmp { to: 0 });
                self.patch_all(no);
                self.expr_to(els, dst, fresh);
                self.patch_all(vec![j]);
            }
            TKind::Match(s, arms, false) => self.match_(s, arms, Mode::Value(dst, fresh)),
            TKind::Seq(stmts, tail) => {
                for s in stmts {
                    self.discard(s);
                }
                match tail {
                    Some(t) => self.expr_to(t, dst, fresh),
                    None => {
                        self.emit(Op::LoadUnit { a: dst });
                    }
                }
            }
            TKind::StrCat(parts) => {
                let refs: Vec<&TExpr> = parts.iter().map(plain_fmt).collect();
                let (base, saved) = self.seq_regs(&refs, dst, fresh);
                self.emit(Op::StrCat { a: base, n: parts.len() as u8 });
                self.done(base, dst, saved);
            }
            TKind::Try(x, v) => {
                let s = self.any(x);
                let ok = self.emit(Op::JTag { a: s, tag: *v as u16, to: 0 });
                self.emit(Op::Ret { a: s });
                self.patch_all(vec![ok]);
                self.emit(Op::GetField { a: dst, b: s, idx: 0 });
            }
            TKind::Coalesce(x, y, v) => {
                let s = self.any(x);
                let no = self.emit(Op::JnTag { a: s, tag: *v as u16, to: 0 });
                self.emit(Op::GetField { a: dst, b: s, idx: 0 });
                self.top = saved;
                let j = self.emit(Op::Jmp { to: 0 });
                self.patch_all(vec![no]);
                self.expr_to(y, dst, fresh);
                self.patch_all(vec![j]);
            }
            // Statements (and unit-valued `if` / `match`) used where a value is expected.
            _ => {
                self.discard(e);
                self.emit(Op::LoadUnit { a: dst });
                return;
            }
        }
        self.top = saved;
    }

    fn builtin(&mut self, id: u16, args: &[TExpr], dst: u8, fresh: bool) {
        let one = |op: fn(u8, u8) -> Op, c: &mut Self| {
            let b = c.any(&args[0]);
            c.emit(op(dst, b));
        };
        if args.len() == 1 {
            if id == B::Sqrt as u16 {
                return one(|a, b| Op::Sqrt { a, b }, self);
            }
            if id == B::LenL as u16 || id == B::LenS as u16 || id == B::LenM as u16 {
                return one(|a, b| Op::Len { a, b }, self);
            }
            if id == B::ToFloat as u16 {
                return one(|a, b| Op::IntToFloat { a, b }, self);
            }
        }
        let refs: Vec<&TExpr> = args.iter().collect();
        let (base, saved) = self.seq_regs(&refs, dst, fresh);
        self.emit(Op::CallB { a: base, n: args.len() as u8, id });
        self.done(base, dst, saved);
    }

    fn bin(&mut self, op: Bin2, l: &TExpr, r: &TExpr, dst: u8) {
        use Bin2::*;
        let a = dst;
        match op {
            AddI | MulI => {
                let (x, k) = match (lit32(l), lit32(r)) {
                    (_, Some(k)) => (l, Some(k)),
                    (Some(k), None) => (r, Some(k)),
                    _ => (l, None),
                };
                if let Some(imm) = k {
                    let b = self.any(x);
                    self.emit(if op == AddI { Op::AddII { a, b, imm } } else { Op::MulII { a, b, imm } });
                    return;
                }
            }
            SubI => {
                if let Some(imm) = lit32(r).and_then(|k| k.checked_neg()) {
                    let b = self.any(l);
                    self.emit(Op::AddII { a, b, imm });
                    return;
                }
            }
            // For a positive power of two, floor modulo is a mask and floor division is an
            // arithmetic shift; both are exact for negative numbers too and cannot overflow.
            ModI | DivI => {
                if let Some(k) = lit32(r).filter(|k| *k > 0 && k & (k - 1) == 0) {
                    let b = self.any(l);
                    self.emit(if op == ModI {
                        Op::AndII { a, b, imm: k - 1 }
                    } else {
                        Op::ShrII { a, b, imm: k.trailing_zeros() as i32 }
                    });
                    return;
                }
            }
            _ => {}
        }
        let b = self.any(l);
        let c = self.any(r);
        self.emit(match op {
            AddI => Op::AddI { a, b, c },
            SubI => Op::SubI { a, b, c },
            MulI => Op::MulI { a, b, c },
            DivI => Op::DivI { a, b, c },
            ModI => Op::ModI { a, b, c },
            PowI => Op::PowI { a, b, c },
            AddF => Op::AddF { a, b, c },
            SubF => Op::SubF { a, b, c },
            MulF => Op::MulF { a, b, c },
            DivF => Op::DivF { a, b, c },
            ModF => Op::ModF { a, b, c },
            PowF => Op::PowF { a, b, c },
            PowFI => Op::PowFI { a, b, c },
            EqI => Op::EqI { a, b, c },
            NeI => Op::NeI { a, b, c },
            LtI => Op::LtI { a, b, c },
            LeI => Op::LeI { a, b, c },
            GtI => Op::LtI { a, b: c, c: b },
            GeI => Op::LeI { a, b: c, c: b },
            EqF => Op::EqF { a, b, c },
            NeF => Op::NeF { a, b, c },
            LtF => Op::LtF { a, b, c },
            LeF => Op::LeF { a, b, c },
            GtF => Op::LtF { a, b: c, c: b },
            GeF => Op::LeF { a, b: c, c: b },
            EqV => Op::EqV { a, b, c },
            NeV => Op::NeV { a, b, c },
            LtS => Op::LtS { a, b, c },
            LeS => Op::LeS { a, b, c },
            GtS => Op::LtS { a, b: c, c: b },
            GeS => Op::LeS { a, b: c, c: b },
            Concat => Op::Concat { a, b, c },
        });
    }

    /// Compile a condition. The returned jumps are taken when `e == sense`.
    fn cond(&mut self, e: &TExpr, sense: bool) -> Vec<usize> {
        use Bin2::*;
        self.set_pos(e.pos);
        let saved = self.top;
        let to = 0;
        let out = match &e.kind {
            TKind::Bool(b) if *b == sense => vec![self.emit(Op::Jmp { to })],
            TKind::Bool(_) => Vec::new(),
            TKind::Un(Un1::Not, x) => self.cond(x, !sense),
            TKind::And(l, r) | TKind::Or(l, r) => {
                let is_and = matches!(e.kind, TKind::And(..));
                if is_and == sense {
                    // Both sides must agree with `sense`... only when the left one does not settle it.
                    let skip = self.cond(l, !sense);
                    let jumps = self.cond(r, sense);
                    self.patch_all(skip);
                    jumps
                } else {
                    let mut jumps = self.cond(l, sense);
                    jumps.extend(self.cond(r, sense));
                    jumps
                }
            }
            TKind::Bin(op @ (EqI | NeI | LtI | LeI | GtI | GeI), l, r) => {
                let (op, x, k) = match (lit16(l), lit16(r)) {
                    (_, Some(k)) => (*op, l, Some(k)),
                    (Some(k), None) => {
                        let mirrored = match op {
                            LtI => GtI,
                            LeI => GeI,
                            GtI => LtI,
                            GeI => LeI,
                            other => *other,
                        };
                        (mirrored, r, Some(k))
                    }
                    _ => (*op, l, None),
                };
                let ins = if let Some(imm) = k {
                    let a = self.any(x);
                    match (op, sense) {
                        (LtI, true) | (GeI, false) => Op::JltII { a, imm, to },
                        (LeI, true) | (GtI, false) => Op::JleII { a, imm, to },
                        (GtI, true) | (LeI, false) => Op::JgtII { a, imm, to },
                        (GeI, true) | (LtI, false) => Op::JgeII { a, imm, to },
                        (EqI, true) | (NeI, false) => Op::JeqII { a, imm, to },
                        _ => Op::JneII { a, imm, to },
                    }
                } else {
                    let a = self.any(l);
                    let b = self.any(r);
                    match (op, sense) {
                        (LtI, true) | (GeI, false) => Op::JltI { a, b, to },
                        (LeI, true) | (GtI, false) => Op::JleI { a, b, to },
                        (GtI, true) | (LeI, false) => Op::JltI { a: b, b: a, to },
                        (GeI, true) | (LtI, false) => Op::JleI { a: b, b: a, to },
                        (EqI, true) | (NeI, false) => Op::JeqI { a, b, to },
                        _ => Op::JneI { a, b, to },
                    }
                };
                vec![self.emit(ins)]
            }
            TKind::Bin(op @ (EqF | NeF | LtF | LeF | GtF | GeF), l, r) => {
                let a = self.any(l);
                let b = self.any(r);
                let ins = match (op, sense) {
                    (LtF, true) => Op::JltF { a, b, to },
                    (LtF, false) => Op::JnltF { a, b, to },
                    (LeF, true) => Op::JleF { a, b, to },
                    (LeF, false) => Op::JnleF { a, b, to },
                    (GtF, true) => Op::JltF { a: b, b: a, to },
                    (GtF, false) => Op::JnltF { a: b, b: a, to },
                    (GeF, true) => Op::JleF { a: b, b: a, to },
                    (GeF, false) => Op::JnleF { a: b, b: a, to },
                    (EqF, true) | (NeF, false) => Op::JeqF { a, b, to },
                    _ => Op::JneF { a, b, to },
                };
                vec![self.emit(ins)]
            }
            _ => {
                let a = self.any(e);
                vec![self.emit(if sense { Op::JmpT { a, to } } else { Op::JmpF { a, to } })]
            }
        };
        self.top = saved;
        out
    }

    /// Compile for effect only.
    fn discard(&mut self, e: &TExpr) {
        self.set_pos(e.pos);
        let saved = self.top;
        match &e.kind {
            TKind::Unit | TKind::Bool(_) | TKind::Int(_) | TKind::Float(_) | TKind::Str(_) => {}
            TKind::Local(_) | TKind::FnRef(_) | TKind::Tag(_) => {}
            TKind::Let(slot, v) => {
                let r = self.alloc();
                self.expr_to(v, r, true);
                self.bind(*slot, r);
                return;
            }
            TKind::LetTop(id, v) => {
                if self.in_main && !self.ck.tops[*id as usize].1 && self.top < 200 {
                    let r = self.alloc();
                    self.expr_to(v, r, true);
                    self.top_reg[*id as usize] = r;
                    return;
                }
                let r = self.any(v);
                self.emit(Op::SetGlobal { a: r, g: *id });
            }
            TKind::SetLocal(slot, v) => {
                let r = self.slot(*slot);
                self.expr_to(v, r, false);
            }
            TKind::SetTop(id, v) => match self.top_in_reg(*id) {
                Some(r) => self.expr_to(v, r, false),
                None => {
                    let r = self.any(v);
                    self.emit(Op::SetGlobal { a: r, g: *id });
                }
            },
            TKind::SetIndexList(l, i, v) | TKind::SetIndexMap(l, i, v) => {
                let a = self.any(l);
                let b = self.any(i);
                let c = self.any(v);
                let is_list = matches!(e.kind, TKind::SetIndexList(..));
                self.emit(if is_list { Op::ListSet { a, b, c } } else { Op::MapSet { a, b, c } });
            }
            TKind::SetField(obj, idx, v) => {
                let a = self.any(obj);
                let b = self.any(v);
                self.emit(Op::SetField { a, b, idx: *idx as u16 });
            }
            TKind::CallBuiltin(id, args) if *id == B::Push as u16 && args.len() == 2 => {
                let a = self.any(&args[0]);
                let b = self.any(&args[1]);
                self.emit(Op::Push { a, b });
            }
            TKind::While(c, body) => {
                let head = self.here();
                let exits = if matches!(c.kind, TKind::Bool(true)) { Vec::new() } else { self.cond(c, false) };
                self.loops.push(LoopCtx { head: Some(head), breaks: Vec::new(), continues: Vec::new() });
                self.discard(body);
                self.top = saved;
                self.emit(Op::Loop { to: head });
                let ctx = self.loops.pop().unwrap();
                self.patch_all(exits);
                self.patch_all(ctx.breaks);
            }
            TKind::ForRange { slot, start, end, inclusive, body } => {
                let i = self.alloc();
                self.expr_to(start, i, true);
                let n = self.alloc();
                self.expr_to(end, n, true);
                self.bind(*slot, i);
                let skip = self.emit(if *inclusive { Op::JltI { a: n, b: i, to: 0 } } else { Op::JleI { a: n, b: i, to: 0 } });
                let start_pc = self.here();
                self.loops.push(LoopCtx { head: None, breaks: Vec::new(), continues: Vec::new() });
                self.discard(body);
                let ctx = self.loops.pop().unwrap();
                self.patch_all(ctx.continues);
                self.emit(if *inclusive { Op::ForIE { a: i, b: n, to: start_pc } } else { Op::ForI { a: i, b: n, to: start_pc } });
                self.patch_all(vec![skip]);
                self.patch_all(ctx.breaks);
            }
            TKind::ForList { slot, list, body } => {
                let l = self.alloc();
                self.expr_to(list, l, true);
                let idx = self.alloc();
                self.emit(Op::LoadInt { a: idx, v: 0 });
                let elem = self.alloc();
                self.bind(*slot, elem);
                let enter = self.emit(Op::Jmp { to: 0 });
                let start_pc = self.here();
                self.loops.push(LoopCtx { head: None, breaks: Vec::new(), continues: Vec::new() });
                self.discard(body);
                let ctx = self.loops.pop().unwrap();
                self.patch_all(ctx.continues);
                self.patch_all(vec![enter]);
                self.emit(Op::ForList { a: l, to: start_pc });
                self.patch_all(ctx.breaks);
            }
            TKind::Return(v) => match v {
                Some(v) if !self.ret_unit => self.tail(v),
                Some(v) => {
                    self.discard(v);
                    self.emit(Op::RetUnit);
                }
                None => {
                    self.emit(Op::RetUnit);
                }
            },
            TKind::Break | TKind::Continue => {
                let head = self.loops.last().map(|l| l.head);
                match (head, &e.kind) {
                    (None, _) => self.fail("internal: `break` or `continue` outside a loop".to_string(), "please report this"),
                    (Some(Some(pc)), TKind::Continue) => {
                        self.emit(Op::Loop { to: pc });
                    }
                    (Some(_), kind) => {
                        let j = self.emit(Op::Jmp { to: 0 });
                        let ctx = self.loops.last_mut().unwrap();
                        if matches!(kind, TKind::Break) {
                            ctx.breaks.push(j);
                        } else {
                            ctx.continues.push(j);
                        }
                    }
                }
            }
            TKind::If(c, t, els, _) => {
                let no = self.cond(c, false);
                self.discard(t);
                self.top = saved;
                match els {
                    Some(els) => {
                        let j = self.emit(Op::Jmp { to: 0 });
                        self.patch_all(no);
                        self.discard(els);
                        self.patch_all(vec![j]);
                    }
                    None => self.patch_all(no),
                }
            }
            TKind::Match(s, arms, _) => self.match_(s, arms, Mode::Discard),
            TKind::Seq(stmts, tail) => {
                for s in stmts {
                    self.discard(s);
                }
                if let Some(t) = tail {
                    self.discard(t);
                }
            }
            TKind::And(l, r) | TKind::Or(l, r) => {
                let skip = self.cond(l, matches!(e.kind, TKind::Or(..)));
                self.discard(r);
                self.patch_all(skip);
            }
            _ => {
                self.any(e);
            }
        }
        self.top = saved;
    }

    /// Compile `e` in return position of a function that returns a value.
    fn tail(&mut self, e: &TExpr) {
        self.set_pos(e.pos);
        let saved = self.top;
        match &e.kind {
            TKind::If(c, t, Some(els), false) => {
                let no = self.cond(c, false);
                self.tail(t);
                self.top = saved;
                self.patch_all(no);
                self.tail(els);
            }
            TKind::Seq(stmts, Some(t)) => {
                for s in stmts {
                    self.discard(s);
                }
                self.tail(t);
            }
            TKind::Match(s, arms, false) => self.match_(s, arms, Mode::Tail),
            TKind::Return(Some(v)) => self.tail(v),
            _ => {
                let r = self.any(e);
                self.emit(Op::Ret { a: r });
            }
        }
        self.top = saved;
    }

    fn match_(&mut self, scrut: &TExpr, arms: &[TArm], mode: Mode) {
        let saved = self.top;
        let owned = !matches!(scrut.kind, TKind::Local(_) | TKind::Top(_));
        let s = self.any(scrut);
        let mut ends = Vec::new();
        let mut closed = false;
        for arm in arms {
            let arm_top = self.top;
            let mut fails = Vec::new();
            self.pattern(&arm.pat, s, owned, &mut fails);
            if let Some(g) = &arm.guard {
                fails.extend(self.cond(g, false));
            }
            match mode {
                Mode::Value(dst, fresh) => self.expr_to(&arm.body, dst, fresh),
                Mode::Discard => self.discard(&arm.body),
                Mode::Tail => self.tail(&arm.body),
            }
            self.top = arm_top;
            if fails.is_empty() {
                closed = true;
                break;
            }
            if !matches!(mode, Mode::Tail) {
                ends.push(self.emit(Op::Jmp { to: 0 }));
            }
            self.patch_all(fails);
        }
        if !closed {
            self.emit(Op::Trap { code: 0 });
        }
        self.patch_all(ends);
        self.top = saved;
    }

    /// Test `pat` against register `r`; jumps pushed to `fails` are taken when it does not match.
    fn pattern(&mut self, pat: &TPat, r: u8, owned: bool, fails: &mut Vec<usize>) {
        match pat {
            TPat::Wild => {}
            TPat::Bind(slot) => {
                if owned {
                    self.bind(*slot, r);
                } else {
                    let n = self.alloc();
                    self.emit(Op::Move { a: n, b: r });
                    self.bind(*slot, n);
                }
            }
            TPat::Int(v) => match i16::try_from(*v) {
                Ok(imm) => fails.push(self.emit(Op::JneII { a: r, imm, to: 0 })),
                Err(_) => {
                    let t = self.alloc();
                    self.load_int(t, *v);
                    fails.push(self.emit(Op::JneI { a: r, b: t, to: 0 }));
                }
            },
            TPat::Bool(b) => {
                fails.push(self.emit(if *b { Op::JmpF { a: r, to: 0 } } else { Op::JmpT { a: r, to: 0 } }));
            }
            TPat::Str(s) => {
                let t = self.alloc();
                let k = self.konst(ConstKey::Str(s.clone()));
                self.emit(Op::LoadK { a: t, k });
                self.emit(Op::EqV { a: t, b: r, c: t });
                fails.push(self.emit(Op::JmpF { a: t, to: 0 }));
            }
            TPat::Variant(_, subs) | TPat::Tuple(subs) => {
                if let TPat::Variant(v, _) = pat {
                    fails.push(self.emit(Op::JnTag { a: r, tag: *v as u16, to: 0 }));
                }
                for (i, sub) in subs.iter().enumerate() {
                    if matches!(sub, TPat::Wild) {
                        continue;
                    }
                    let t = self.alloc();
                    self.emit(Op::GetField { a: t, b: r, idx: i as u16 });
                    self.pattern(sub, t, true, fails);
                }
            }
        }
    }
}

/// `"{x}"` with no format spec: the VM's `StrCat` formats the value itself.
fn plain_fmt(e: &TExpr) -> &TExpr {
    if let TKind::CallBuiltin(id, args) = &e.kind {
        if *id == B::Fmt as u16 && args.len() == 4 {
            let plain = lit(&args[1]) == Some(-1) && lit(&args[2]) == Some(-1) && lit(&args[3]) == Some(0);
            if plain {
                return &args[0];
            }
        }
    }
    e
}
