//! Type and unit checker: syntax tree -> typed IR.
//! This file holds the tables, type machinery, declarations and statements;
//! `check_expr.rs` holds expressions, calls and patterns.

use crate::ast::*;
use crate::builtins::BUILTINS;
use crate::parser::parse_sig;
use crate::tir::*;
use crate::types::*;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

pub(crate) const V_SOME: u32 = 1;
pub(crate) const V_OK: u32 = 2;
pub(crate) const E_OPTION: u32 = 0;
pub(crate) const E_RESULT: u32 = 1;
const MAX_ERRORS: usize = 20;
pub(crate) const MAX_DEPTH: u32 = 600;

#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) enum Origin {
    Prelude,
    User,
    Lambda,
    Test,
    Main,
}

pub(crate) struct FnInfo {
    pub name: String,
    pub tparams: Vec<String>,
    pub bounds: Vec<Option<Bound>>,
    pub params: Vec<Ty>,
    pub ret: Option<Ty>,
    #[allow(dead_code)]
    pub origin: Origin,
    pub decl: Option<Rc<FnDecl>>,
    /// 0 = body not checked, 1 = being checked, 2 = done.
    pub state: u8,
    pub body: Option<TExpr>,
    pub nslots: u32,
    pub captures: Vec<u32>,
    pub pos: Pos,
}

pub(crate) struct StructInfo {
    pub name: String,
    pub tparams: Vec<String>,
    pub fields: Vec<(String, Ty)>,
}

pub(crate) struct EnumInfo {
    pub name: String,
    pub tparams: Vec<String>,
    pub variants: Vec<u32>,
}

pub(crate) struct VariantInfo {
    pub name: String,
    pub enum_id: u32,
    pub fields: Vec<Ty>,
}

pub(crate) struct TopInfo {
    pub name: String,
    pub ty: Ty,
    pub mutable: bool,
    pub captured: bool,
    pub konst: Option<TKind>,
    pub pos: Pos,
    pub prelude: bool,
}

#[derive(Clone, Copy, PartialEq)]
pub(crate) enum Bound {
    Num,
    Ord,
    Key,
}

pub(crate) struct BSig {
    pub ntp: usize,
    pub bounds: Vec<Option<Bound>>,
    pub params: Vec<Ty>,
    pub ret: Ty,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) enum Head {
    Any,
    Unit,
    Bool,
    Int,
    Num,
    Str,
    List,
    Map,
    Tuple,
    Fn,
    Struct(u32),
    Enum(u32),
}

#[derive(Clone, Copy)]
pub(crate) enum Cand {
    Fn(u32),
    Builtin(u16),
}

#[derive(Clone, Copy)]
pub(crate) struct CandInfo {
    pub cand: Cand,
    pub arity: usize,
    pub head: Head,
    /// 2 = user, 1 = prelude, 0 = builtin.
    pub rank: u8,
}

#[derive(Clone)]
pub(crate) enum Expect {
    Discard,
    Tail,
    Infer,
    Has(Ty),
}

pub(crate) struct Local {
    pub name: String,
    pub slot: u32,
    pub ty: Ty,
    pub mutable: bool,
    pub captured: bool,
}

#[derive(Clone, Copy, PartialEq)]
pub(crate) enum CtxKind {
    Main,
    Fn,
    Lambda,
    Test,
}

pub(crate) struct FnCtx {
    pub kind: CtxKind,
    pub scopes: Vec<Vec<Local>>,
    pub nslots: u32,
    pub ret: Ty,
    pub loop_depth: u32,
    /// (slot in the enclosing function, slot in this lambda)
    pub captures: Vec<(u32, u32)>,
}

pub(crate) enum Found {
    Local(u32, Ty, bool, bool),
    Top(u32),
    None,
}

pub(crate) enum Decl {
    Local(u32),
    Top(u32),
}

pub struct Checker {
    pub(crate) errs: Vec<Diag>,
    pub(crate) units: HashMap<String, UnitDef>,
    pub(crate) bases: Vec<String>,
    pub(crate) fns: Vec<FnInfo>,
    pub(crate) structs: Vec<StructInfo>,
    pub(crate) struct_names: HashMap<String, u32>,
    pub(crate) enums: Vec<EnumInfo>,
    pub(crate) enum_names: HashMap<String, u32>,
    pub(crate) variants: Vec<VariantInfo>,
    pub(crate) variant_names: HashMap<String, u32>,
    pub(crate) tops: Vec<TopInfo>,
    pub(crate) top_names: HashMap<String, u32>,
    pub(crate) future_tops: HashMap<String, Pos>,
    pub(crate) fn_names: HashMap<String, Vec<CandInfo>>,
    pub(crate) bsigs: Vec<BSig>,
    pub(crate) subst: Vec<Option<Ty>>,
    pub(crate) trail: Vec<u32>,
    pub(crate) ctxs: Vec<FnCtx>,
    pub(crate) tscope: Vec<String>,
    /// Bounds of the type parameters in `tscope` (empty when there are none).
    pub(crate) tbounds: Vec<Option<Bound>>,
    pub(crate) assigns: u32,
    pub(crate) depth: u32,
    pub(crate) tests: Vec<(String, u32)>,
}

pub(crate) fn te(kind: TKind, pos: Pos) -> TExpr {
    TExpr { kind, pos }
}

pub(crate) fn bx(e: TExpr) -> Box<TExpr> {
    Box::new(e)
}

pub(crate) fn contains_param(t: &Ty, k: u32) -> bool {
    match t {
        Ty::Param(i) => *i == k,
        Ty::List(x) => contains_param(x, k),
        Ty::Map(a, b) => contains_param(a, k) || contains_param(b, k),
        Ty::Tuple(xs) | Ty::Struct(_, xs) | Ty::Enum(_, xs) => xs.iter().any(|x| contains_param(x, k)),
        Ty::Fn(ps, r) => ps.iter().any(|x| contains_param(x, k)) || contains_param(r, k),
        _ => false,
    }
}

pub(crate) fn head_sig(t: &Ty) -> Head {
    match t {
        Ty::Unit => Head::Unit,
        Ty::Bool => Head::Bool,
        Ty::Int => Head::Int,
        Ty::Num(_) => Head::Num,
        Ty::Str => Head::Str,
        Ty::List(_) => Head::List,
        Ty::Map(_, _) => Head::Map,
        Ty::Tuple(_) => Head::Tuple,
        Ty::Fn(_, _) => Head::Fn,
        Ty::Struct(id, _) => Head::Struct(*id),
        Ty::Enum(id, _) => Head::Enum(*id),
        _ => Head::Any,
    }
}

fn is_literal(e: &Expr) -> bool {
    match &e.kind {
        ExprKind::Int(_) | ExprKind::Float(_) | ExprKind::Bool(_) | ExprKind::Str(_) | ExprKind::UnitLit(_, _) => true,
        ExprKind::Neg(inner) => matches!(
            inner.kind,
            ExprKind::Int(_) | ExprKind::Float(_) | ExprKind::UnitLit(_, _)
        ),
        _ => false,
    }
}

fn stmt_pos(s: &Stmt) -> Pos {
    match s {
        Stmt::Let { pos, .. } | Stmt::Assign { pos, .. } | Stmt::While { pos, .. } | Stmt::For { pos, .. } => *pos,
        Stmt::Expr(e) => e.pos,
        Stmt::Return(_, p) | Stmt::Break(p) | Stmt::Continue(p) => *p,
    }
}

impl Checker {
    fn new() -> Checker {
        Checker {
            errs: Vec::new(),
            units: builtin_units(),
            bases: BASE_NAMES.iter().map(|s| s.to_string()).collect(),
            fns: Vec::new(),
            structs: Vec::new(),
            struct_names: HashMap::new(),
            enums: Vec::new(),
            enum_names: HashMap::new(),
            variants: Vec::new(),
            variant_names: HashMap::new(),
            tops: Vec::new(),
            top_names: HashMap::new(),
            future_tops: HashMap::new(),
            fn_names: HashMap::new(),
            bsigs: Vec::new(),
            subst: Vec::new(),
            trail: Vec::new(),
            ctxs: Vec::new(),
            tscope: Vec::new(),
            tbounds: Vec::new(),
            assigns: 0,
            depth: 0,
            tests: Vec::new(),
        }
    }

    // ---- errors

    pub(crate) fn err(&mut self, pos: Pos, msg: impl Into<String>) {
        if self.errs.len() < MAX_ERRORS {
            self.errs.push(Diag::new(pos, msg));
        }
    }

    pub(crate) fn err_hint(&mut self, pos: Pos, msg: impl Into<String>, hint: impl Into<String>) {
        if self.errs.len() < MAX_ERRORS {
            self.errs.push(Diag::new(pos, msg).with_hint(hint));
        }
    }

    pub(crate) fn bad(&self, pos: Pos) -> (TExpr, Ty) {
        (te(TKind::Unit, pos), Ty::Error)
    }

    // ---- type machinery

    pub(crate) fn fresh(&mut self) -> Ty {
        self.subst.push(None);
        Ty::Var(self.subst.len() as u32 - 1)
    }

    pub(crate) fn resolve(&self, t: &Ty) -> Ty {
        let mut cur = t;
        loop {
            match cur {
                Ty::Var(v) => match &self.subst[*v as usize] {
                    Some(next) => cur = next,
                    None => return cur.clone(),
                },
                _ => return cur.clone(),
            }
        }
    }

    pub(crate) fn zonk(&self, t: &Ty) -> Ty {
        match self.resolve(t) {
            Ty::List(x) => Ty::List(Box::new(self.zonk(&x))),
            Ty::Map(k, v) => Ty::Map(Box::new(self.zonk(&k)), Box::new(self.zonk(&v))),
            Ty::Tuple(xs) => Ty::Tuple(xs.iter().map(|x| self.zonk(x)).collect()),
            Ty::Fn(ps, r) => Ty::Fn(ps.iter().map(|x| self.zonk(x)).collect(), Box::new(self.zonk(&r))),
            Ty::Struct(id, xs) => Ty::Struct(id, xs.iter().map(|x| self.zonk(x)).collect()),
            Ty::Enum(id, xs) => Ty::Enum(id, xs.iter().map(|x| self.zonk(x)).collect()),
            other => other,
        }
    }

    fn has_var(t: &Ty) -> bool {
        match t {
            Ty::Var(_) => true,
            Ty::List(x) => Self::has_var(x),
            Ty::Map(a, b) => Self::has_var(a) || Self::has_var(b),
            Ty::Tuple(xs) | Ty::Struct(_, xs) | Ty::Enum(_, xs) => xs.iter().any(Self::has_var),
            Ty::Fn(ps, r) => ps.iter().any(Self::has_var) || Self::has_var(r),
            _ => false,
        }
    }

    fn occurs(&self, v: u32, t: &Ty) -> bool {
        match self.resolve(t) {
            Ty::Var(x) => x == v,
            Ty::List(x) => self.occurs(v, &x),
            Ty::Map(a, b) => self.occurs(v, &a) || self.occurs(v, &b),
            Ty::Tuple(xs) | Ty::Struct(_, xs) | Ty::Enum(_, xs) => xs.iter().any(|x| self.occurs(v, x)),
            Ty::Fn(ps, r) => ps.iter().any(|x| self.occurs(v, x)) || self.occurs(v, &r),
            _ => false,
        }
    }

    pub(crate) fn unify(&mut self, a: &Ty, b: &Ty) -> bool {
        let mark = self.trail.len();
        if self.unify_in(a, b) {
            return true;
        }
        while self.trail.len() > mark {
            let v = self.trail.pop().unwrap();
            self.subst[v as usize] = None;
        }
        false
    }

    fn bind(&mut self, v: u32, t: &Ty) -> bool {
        if self.occurs(v, t) {
            return false;
        }
        self.subst[v as usize] = Some(t.clone());
        self.trail.push(v);
        true
    }

    fn unify_all(&mut self, xs: &[Ty], ys: &[Ty]) -> bool {
        xs.len() == ys.len() && xs.iter().zip(ys).all(|(x, y)| self.unify_in(x, y))
    }

    fn unify_in(&mut self, a: &Ty, b: &Ty) -> bool {
        let a = self.resolve(a);
        let b = self.resolve(b);
        match (&a, &b) {
            (Ty::Error, _) | (_, Ty::Error) | (Ty::Never, _) | (_, Ty::Never) => true,
            (Ty::Var(x), Ty::Var(y)) if x == y => true,
            (Ty::Var(x), _) => self.bind(*x, &b),
            (_, Ty::Var(y)) => self.bind(*y, &a),
            (Ty::Unit, Ty::Unit) | (Ty::Bool, Ty::Bool) | (Ty::Int, Ty::Int) | (Ty::Str, Ty::Str) => true,
            (Ty::Num(d1), Ty::Num(d2)) => d1 == d2,
            (Ty::List(x), Ty::List(y)) => self.unify_in(x, y),
            (Ty::Map(k1, v1), Ty::Map(k2, v2)) => self.unify_in(k1, k2) && self.unify_in(v1, v2),
            (Ty::Tuple(xs), Ty::Tuple(ys)) => self.unify_all(xs, ys),
            (Ty::Fn(p1, r1), Ty::Fn(p2, r2)) => self.unify_all(p1, p2) && self.unify_in(r1, r2),
            (Ty::Struct(i, xs), Ty::Struct(j, ys)) => i == j && self.unify_all(xs, ys),
            (Ty::Enum(i, xs), Ty::Enum(j, ys)) => i == j && self.unify_all(xs, ys),
            (Ty::Param(i), Ty::Param(j)) => i == j,
            _ => false,
        }
    }

    pub(crate) fn inst(t: &Ty, vars: &[Ty]) -> Ty {
        match t {
            Ty::Param(i) => vars.get(*i as usize).cloned().unwrap_or(Ty::Error),
            Ty::List(x) => Ty::List(Box::new(Self::inst(x, vars))),
            Ty::Map(k, v) => Ty::Map(Box::new(Self::inst(k, vars)), Box::new(Self::inst(v, vars))),
            Ty::Tuple(xs) => Ty::Tuple(xs.iter().map(|x| Self::inst(x, vars)).collect()),
            Ty::Fn(ps, r) => Ty::Fn(ps.iter().map(|x| Self::inst(x, vars)).collect(), Box::new(Self::inst(r, vars))),
            Ty::Struct(id, xs) => Ty::Struct(*id, xs.iter().map(|x| Self::inst(x, vars)).collect()),
            Ty::Enum(id, xs) => Ty::Enum(*id, xs.iter().map(|x| Self::inst(x, vars)).collect()),
            other => other.clone(),
        }
    }

    pub(crate) fn fresh_vars(&mut self, n: usize) -> Vec<Ty> {
        (0..n).map(|_| self.fresh()).collect()
    }

    pub(crate) fn show(&self, t: &Ty) -> String {
        let list = |xs: &[Ty]| xs.iter().map(|x| self.show(x)).collect::<Vec<_>>().join(", ");
        match self.resolve(t) {
            Ty::Unit => "()".to_string(),
            Ty::Bool => "Bool".to_string(),
            Ty::Int => "Int".to_string(),
            Ty::Num(d) => dim_name(&d, &self.bases),
            Ty::Str => "Str".to_string(),
            Ty::List(x) => format!("List[{}]", self.show(&x)),
            Ty::Map(k, v) => format!("Map[{}, {}]", self.show(&k), self.show(&v)),
            Ty::Tuple(xs) => format!("({})", list(&xs)),
            Ty::Fn(ps, r) => format!("fn({}) -> {}", list(&ps), self.show(&r)),
            Ty::Struct(id, xs) => {
                let name = self.structs[id as usize].name.clone();
                if xs.is_empty() {
                    name
                } else {
                    format!("{}[{}]", name, list(&xs))
                }
            }
            Ty::Enum(id, xs) => {
                if id == E_OPTION && xs.len() == 1 {
                    return format!("{}?", self.show(&xs[0]));
                }
                let name = self.enums[id as usize].name.clone();
                if xs.is_empty() {
                    name
                } else {
                    format!("{}[{}]", name, list(&xs))
                }
            }
            Ty::Param(i) => self.tscope.get(i as usize).cloned().unwrap_or_else(|| "T".to_string()),
            Ty::Var(_) => "_".to_string(),
            Ty::Never => "Never".to_string(),
            Ty::Error => "?".to_string(),
        }
    }

    pub(crate) fn to_float(v: TExpr) -> TExpr {
        let pos = v.pos;
        match v.kind {
            TKind::Int(i) => te(TKind::Float(i as f64), pos),
            _ => te(TKind::Un(Un1::IntToFloat, bx(v)), pos),
        }
    }

    /// Make `v` fit the wanted type, or report a mismatch.
    pub(crate) fn coerce(&mut self, v: TExpr, actual: &Ty, want: &Ty, pos: Pos, what: &str) -> TExpr {
        if self.unify(actual, want) {
            return v;
        }
        let (a, w) = (self.resolve(actual), self.resolve(want));
        if a == Ty::Int && w.is_float() {
            return Self::to_float(v);
        }
        if let (Ty::Fn(p1, _), Ty::Fn(p2, r2)) = (&a, &w) {
            if self.resolve(r2) == Ty::Unit && p1.len() == p2.len() {
                let mut ok = true;
                for (x, y) in p1.iter().zip(p2.iter()) {
                    ok = ok && self.unify(x, y);
                }
                if ok {
                    return v;
                }
            }
        }
        let mut msg = format!("{}: expected {}, found {}", what, self.show(&w), self.show(&a));
        let hint: Option<String> = match (&a, &w) {
            (Ty::Int, Ty::Num(_)) => Some("add a unit, e.g. `5 kg`".to_string()),
            (Ty::Num(d1), Ty::Num(d2)) => {
                if d1.is_none() {
                    Some("add a unit, e.g. `5.0 kg`".to_string())
                } else if d2.is_none() {
                    Some("convert it to a plain number first, e.g. `x in mm`".to_string())
                } else {
                    msg = format!("unit mismatch in {}: expected {}, found {}", what, self.show(&w), self.show(&a));
                    None
                }
            }
            (Ty::Num(d), Ty::Int) if d.is_none() => {
                Some("use `round(x)`, `floor(x)` or `int(x)` to get an Int, or start the variable as `0.0`".to_string())
            }
            (Ty::Enum(E_OPTION, _), _) => {
                Some("this value may be missing: use `?? default`, `match` or `.unwrap()`".to_string())
            }
            (Ty::Enum(E_RESULT, _), _) => Some("this can fail: use `?? default`, `match` or `.unwrap()`".to_string()),
            (_, Ty::Enum(E_OPTION, _)) => Some("wrap it: `Some(...)`".to_string()),
            (Ty::Unit, _) => Some("there is no value here: make every path end with a value or a `return`".to_string()),
            (_, Ty::Str) => Some("turn it into text with `str(x)` or `\"{x}\"`".to_string()),
            _ => None,
        };
        match hint {
            Some(h) => self.err_hint(pos, msg, h),
            None => self.err(pos, msg),
        }
        te(TKind::Unit, pos)
    }

    pub(crate) fn expect_ty(&mut self, e: &Expr, want: &Ty, what: &str) -> TExpr {
        let (v, vty) = self.expr(e, &Expect::Has(want.clone()));
        self.coerce(v, &vty, want, e.pos, what)
    }

    // ---- type and unit expressions

    pub(crate) fn ty_expr(&mut self, t: &TypeExpr) -> Ty {
        match t {
            TypeExpr::Name(name, args, pos) => {
                let targs: Vec<Ty> = args.iter().map(|a| self.ty_expr(a)).collect();
                let n = targs.len();
                match name.as_str() {
                    "Int" if n == 0 => return Ty::Int,
                    "Float" if n == 0 => return Ty::float(),
                    "Bool" if n == 0 => return Ty::Bool,
                    "Str" if n == 0 => return Ty::Str,
                    "Never" if n == 0 => return Ty::Never,
                    "List" => {
                        if n == 1 {
                            return Ty::List(Box::new(targs[0].clone()));
                        }
                        self.err(*pos, "`List` needs one type, e.g. `List[Int]`");
                        return Ty::Error;
                    }
                    "Map" => {
                        if n == 2 {
                            return Ty::Map(Box::new(targs[0].clone()), Box::new(targs[1].clone()));
                        }
                        self.err(*pos, "`Map` needs two types, e.g. `Map[Str, Int]`");
                        return Ty::Error;
                    }
                    _ => {}
                }
                if let Some(i) = self.tscope.iter().position(|x| x == name) {
                    return Ty::Param(i as u32);
                }
                if let Some(&id) = self.struct_names.get(name) {
                    let want = self.structs[id as usize].tparams.len();
                    if want != n {
                        self.err(*pos, format!("`{}` needs {} type argument(s), found {}", name, want, n));
                        return Ty::Error;
                    }
                    return Ty::Struct(id, targs);
                }
                if let Some(&id) = self.enum_names.get(name) {
                    let want = self.enums[id as usize].tparams.len();
                    if want != n {
                        self.err(*pos, format!("`{}` needs {} type argument(s), found {}", name, want, n));
                        return Ty::Error;
                    }
                    return Ty::Enum(id, targs);
                }
                if n == 0 {
                    if let Some(u) = self.units.get(name) {
                        return Ty::Num(u.dim.clone());
                    }
                }
                let basics = ["Int", "Float", "Bool", "Str", "List", "Map"];
                let near = closest(
                    name,
                    basics
                        .iter()
                        .copied()
                        .chain(self.struct_names.keys().map(|s| s.as_str()))
                        .chain(self.enum_names.keys().map(|s| s.as_str()))
                        .chain(self.units.keys().map(|s| s.as_str())),
                );
                match near {
                    Some(c) => self.err_hint(*pos, format!("unknown type `{}`", name), format!("did you mean `{}`?", c)),
                    None => self.err(*pos, format!("unknown type `{}`", name)),
                }
                Ty::Error
            }
            TypeExpr::Unit(u) => match self.unit_def(u, false) {
                Some((dim, _, _)) => Ty::Num(dim),
                None => Ty::Error,
            },
            TypeExpr::Tuple(items, _) => {
                if items.is_empty() {
                    Ty::Unit
                } else {
                    Ty::Tuple(items.iter().map(|x| self.ty_expr(x)).collect())
                }
            }
            TypeExpr::Fn(params, ret, _) => {
                let ps = params.iter().map(|x| self.ty_expr(x)).collect();
                Ty::Fn(ps, Box::new(self.ty_expr(ret)))
            }
            TypeExpr::Opt(inner, _) => Ty::Enum(E_OPTION, vec![self.ty_expr(inner)]),
        }
    }

    /// Resolve a unit expression to (dimension, numerator, denominator).
    pub(crate) fn unit_def(&mut self, u: &UnitExpr, after_in: bool) -> Option<(Dim, f64, f64)> {
        let mut dim = Dim::none();
        let (mut num, mut den) = (1.0f64, 1.0f64);
        for (name, exp) in &u.factors {
            let def = match self.units.get(name) {
                Some(d) => d.clone(),
                None => {
                    self.unknown_unit(name, u.pos, after_in);
                    return None;
                }
            };
            let combined = def.dim.pow(*exp, 1).and_then(|d| dim.mul(&d));
            dim = match combined {
                Some(d) => d,
                None => {
                    self.err(u.pos, "this unit has an exponent that is too large");
                    return None;
                }
            };
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

    fn unknown_unit(&mut self, name: &str, pos: Pos, after_in: bool) {
        let msg = format!("unknown unit `{}`", name);
        let var_ty = match self.lookup(name) {
            Found::Local(_, ty, _, _) => Some(ty),
            Found::Top(id) => Some(self.tops[id as usize].ty.clone()),
            Found::None => None,
        };
        if let Some(ty) = var_ty {
            if after_in && matches!(self.resolve(&ty), Ty::List(_)) {
                self.err_hint(pos, msg, format!("to test membership write `{}.contains(x)`", name));
            } else {
                self.err_hint(pos, msg, format!("`{0}` is a variable; to multiply write `3 * {0}`", name));
            }
            return;
        }
        match closest(name, self.units.keys().map(|s| s.as_str())) {
            Some(c) => self.err_hint(pos, msg, format!("did you mean `{}`?", c)),
            None => self.err_hint(pos, msg, format!("declare it first with `unit {}`", name)),
        }
    }

    // ---- scopes

    pub(crate) fn cx(&mut self) -> &mut FnCtx {
        self.ctxs.last_mut().unwrap()
    }

    pub(crate) fn new_slot(&mut self) -> u32 {
        let c = self.cx();
        c.nslots += 1;
        c.nslots - 1
    }

    pub(crate) fn push_scope(&mut self) {
        self.cx().scopes.push(Vec::new());
    }

    pub(crate) fn pop_scope(&mut self) {
        self.cx().scopes.pop();
    }

    pub(crate) fn in_main_proper(&self) -> bool {
        self.ctxs.len() == 1 && self.ctxs[0].kind == CtxKind::Main
    }

    pub(crate) fn lookup(&mut self, name: &str) -> Found {
        if !self.ctxs.is_empty() {
            let ci = self.ctxs.len() - 1;
            if let Some(f) = self.lookup_ctx(ci, name) {
                return f;
            }
        }
        match self.top_names.get(name) {
            Some(&id) => Found::Top(id),
            None => Found::None,
        }
    }

    fn lookup_ctx(&mut self, ci: usize, name: &str) -> Option<Found> {
        for scope in self.ctxs[ci].scopes.iter().rev() {
            for l in scope.iter().rev() {
                if l.name == name {
                    return Some(Found::Local(l.slot, l.ty.clone(), l.mutable, l.captured));
                }
            }
        }
        if self.ctxs[ci].kind == CtxKind::Lambda && ci > 0 {
            if let Some(Found::Local(outer, ty, _, _)) = self.lookup_ctx(ci - 1, name) {
                let c = &mut self.ctxs[ci];
                let inner = c.nslots;
                c.nslots += 1;
                c.captures.push((outer, inner));
                c.scopes[0].push(Local {
                    name: name.to_string(),
                    slot: inner,
                    ty: ty.clone(),
                    mutable: false,
                    captured: true,
                });
                return Some(Found::Local(inner, ty, false, true));
            }
        }
        None
    }

    pub(crate) fn declare(&mut self, name: &str, ty: Ty, mutable: bool, pos: Pos) -> Decl {
        if self.in_main_proper() && self.ctxs[0].scopes.len() == 1 {
            if let Some(&old) = self.top_names.get(name) {
                let o = &self.tops[old as usize];
                if !o.prelude {
                    let line = o.pos.line;
                    self.err_hint(
                        pos,
                        format!("`{}` is already defined on line {}", name, line),
                        "pick another name, or assign with `=` if it was declared with `var`",
                    );
                }
            }
            let id = self.tops.len() as u32;
            self.tops.push(TopInfo {
                name: name.to_string(),
                ty,
                mutable,
                captured: false,
                konst: None,
                pos,
                prelude: pos.file == 0,
            });
            self.top_names.insert(name.to_string(), id);
            self.future_tops.remove(name);
            return Decl::Top(id);
        }
        let dup = name != "_" && self.cx().scopes.last().unwrap().iter().any(|l| l.name == name);
        if dup {
            self.err_hint(
                pos,
                format!("`{}` is already defined in this block", name),
                "pick another name, or assign with `=` if it was declared with `var`",
            );
        }
        let slot = self.new_slot();
        self.cx().scopes.last_mut().unwrap().push(Local {
            name: name.to_string(),
            slot,
            ty,
            mutable,
            captured: false,
        });
        Decl::Local(slot)
    }

    fn declare_slot(&mut self, name: &str, ty: Ty, pos: Pos) -> u32 {
        match self.declare(name, ty, false, pos) {
            Decl::Local(s) => s,
            Decl::Top(_) => 0,
        }
    }

    // ---- declarations

    fn new_fn(&mut self, name: &str, origin: Origin, pos: Pos) -> u32 {
        self.fns.push(FnInfo {
            name: name.to_string(),
            tparams: Vec::new(),
            bounds: Vec::new(),
            params: Vec::new(),
            ret: None,
            origin,
            decl: None,
            state: 0,
            body: None,
            nslots: 0,
            captures: Vec::new(),
            pos,
        });
        self.fns.len() as u32 - 1
    }

    pub(crate) fn new_lambda(&mut self, pos: Pos) -> u32 {
        self.new_fn("<lambda>", Origin::Lambda, pos)
    }

    fn add_cand(&mut self, name: &str, c: CandInfo, pos: Pos) {
        let list = self.fn_names.entry(name.to_string()).or_default();
        let same = |o: &CandInfo| o.arity == c.arity && o.head == c.head;
        let dup = list.iter().any(|o| o.rank == c.rank && same(o));
        list.retain(|o| !(o.rank < c.rank && same(o)));
        list.push(c);
        if dup && c.rank == 2 {
            self.err_hint(
                pos,
                format!("function `{}` is already defined for this first parameter type", name),
                "give it another name, or a different first parameter type",
            );
        }
    }

    fn declare_units(&mut self, items: &[Item]) {
        for item in items {
            if let Item::Unit(u) = item {
                if self.units.contains_key(&u.name) {
                    self.err(u.pos, format!("unit `{}` already exists", u.name));
                    continue;
                }
                let def = match &u.def {
                    None => {
                        self.bases.push(u.name.clone());
                        UnitDef {
                            dim: Dim::base(self.bases.len() as u16 - 1),
                            num: 1.0,
                            den: 1.0,
                        }
                    }
                    Some((value, ue)) => match self.unit_def(ue, false) {
                        Some((dim, num, den)) => UnitDef {
                            dim,
                            num: value * num,
                            den,
                        },
                        None => continue,
                    },
                };
                self.units.insert(u.name.clone(), def);
            }
        }
    }

    fn declare_types(&mut self, items: &[Item]) {
        for item in items {
            let (name, pos) = match item {
                Item::Struct(s) => (&s.name, s.pos),
                Item::Enum(e) => (&e.name, e.pos),
                _ => continue,
            };
            let taken = self.struct_names.contains_key(name)
                || self.enum_names.contains_key(name)
                || ["Int", "Float", "Bool", "Str", "List", "Map", "Never"].contains(&name.as_str());
            if taken {
                self.err(pos, format!("type `{}` is already defined", name));
                continue;
            }
            match item {
                Item::Struct(s) => {
                    self.struct_names.insert(s.name.clone(), self.structs.len() as u32);
                    self.structs.push(StructInfo {
                        name: s.name.clone(),
                        tparams: s.tparams.clone(),
                        fields: Vec::new(),
                    });
                }
                Item::Enum(e) => {
                    self.enum_names.insert(e.name.clone(), self.enums.len() as u32);
                    self.enums.push(EnumInfo {
                        name: e.name.clone(),
                        tparams: e.tparams.clone(),
                        variants: Vec::new(),
                    });
                }
                _ => {}
            }
        }
        for item in items {
            match item {
                Item::Struct(s) => {
                    let id = match self.struct_names.get(&s.name) {
                        Some(&id) => id,
                        None => continue,
                    };
                    self.tscope = s.tparams.clone();
                    let mut fields: Vec<(String, Ty)> = Vec::new();
                    for (fname, fty, fpos) in &s.fields {
                        if fields.iter().any(|(n, _)| n == fname) {
                            self.err(*fpos, format!("field `{}` is listed twice", fname));
                        }
                        let ty = self.ty_expr(fty);
                        fields.push((fname.clone(), ty));
                    }
                    self.structs[id as usize].fields = fields;
                }
                Item::Enum(e) => {
                    let id = match self.enum_names.get(&e.name) {
                        Some(&id) => id,
                        None => continue,
                    };
                    self.tscope = e.tparams.clone();
                    for (vname, vfields, vpos) in &e.variants {
                        if self.variant_names.contains_key(vname) || self.struct_names.contains_key(vname) {
                            self.err_hint(
                                *vpos,
                                format!("the name `{}` is already used", vname),
                                "variant names must be unique in the whole program",
                            );
                            continue;
                        }
                        let fields: Vec<Ty> = vfields.iter().map(|t| self.ty_expr(t)).collect();
                        let vid = self.variants.len() as u32;
                        self.variants.push(VariantInfo {
                            name: vname.clone(),
                            enum_id: id,
                            fields,
                        });
                        self.variant_names.insert(vname.clone(), vid);
                        self.enums[id as usize].variants.push(vid);
                    }
                }
                _ => {}
            }
        }
        self.tscope = Vec::new();
    }

    fn declare_builtins(&mut self) {
        for (i, (name, sig)) in BUILTINS.iter().enumerate() {
            let (tps, params, ret) = match parse_sig(sig) {
                Ok(s) => s,
                Err(d) => {
                    self.errs.push(Diag::new(d.pos, format!("internal: bad signature for `{}`: {}", name, d.msg)));
                    continue;
                }
            };
            self.tscope = tps.iter().map(|(n, _)| n.clone()).collect();
            let bounds = tps
                .iter()
                .map(|(_, b)| match b.as_deref() {
                    Some("Num") => Some(Bound::Num),
                    Some("Ord") => Some(Bound::Ord),
                    Some("Key") => Some(Bound::Key),
                    _ => None,
                })
                .collect();
            let ptys: Vec<Ty> = params.iter().map(|p| self.ty_expr(p)).collect();
            let rty = self.ty_expr(&ret);
            let head = ptys.first().map(head_sig).unwrap_or(Head::Unit);
            let arity = ptys.len();
            self.bsigs.push(BSig {
                ntp: tps.len(),
                bounds,
                params: ptys,
                ret: rty,
            });
            let cand = CandInfo {
                cand: Cand::Builtin(i as u16),
                arity,
                head,
                rank: 0,
            };
            self.add_cand(name, cand, Pos::default());
        }
        self.tscope = Vec::new();
    }

    fn declare_fns(&mut self, items: &[Item]) {
        for item in items {
            if let Item::Fn(d) = item {
                if self.struct_names.contains_key(&d.name) || self.variant_names.contains_key(&d.name) {
                    self.err(d.pos, format!("the name `{}` is already used by a type", d.name));
                    continue;
                }
                self.tscope = d.tparams.clone();
                let params: Vec<Ty> = d.params.iter().map(|(_, t, _)| self.ty_expr(t)).collect();
                let ret = d.ret.as_ref().map(|t| self.ty_expr(t));
                let origin = if d.pos.file == 0 { Origin::Prelude } else { Origin::User };
                let fid = self.new_fn(&d.name, origin, d.pos);
                let head = params.first().map(head_sig).unwrap_or(Head::Unit);
                let arity = params.len();
                let mut bounds = Vec::with_capacity(d.tbounds.len());
                for b in &d.tbounds {
                    bounds.push(match b {
                        None => None,
                        Some((n, bpos)) => match n.as_str() {
                            "Num" => Some(Bound::Num),
                            "Ord" => Some(Bound::Ord),
                            "Key" => Some(Bound::Key),
                            _ => {
                                self.err_hint(
                                    *bpos,
                                    format!("unknown bound `{}`", n),
                                    "the bounds are `Num` (numbers), `Ord` (numbers or text) and `Key` (Int or Str)",
                                );
                                None
                            }
                        },
                    });
                }
                let f = &mut self.fns[fid as usize];
                f.tparams = d.tparams.clone();
                f.bounds = bounds;
                f.params = params;
                f.ret = ret;
                f.decl = Some(d.clone());
                let cand = CandInfo {
                    cand: Cand::Fn(fid),
                    arity,
                    head,
                    rank: if origin == Origin::User { 2 } else { 1 },
                };
                self.add_cand(&d.name, cand, d.pos);
            }
        }
        self.tscope = Vec::new();
    }

    /// Top-level `let NAME = literal` constants are known everywhere, in any order.
    fn hoist_consts(&mut self, items: &[Item]) -> HashSet<(u16, u32, u16)> {
        let mut hoisted = HashSet::new();
        self.ctxs.push(FnCtx {
            kind: CtxKind::Fn,
            scopes: vec![Vec::new()],
            nslots: 0,
            ret: Ty::Unit,
            loop_depth: 0,
            captures: Vec::new(),
        });
        for item in items {
            if let Item::Stmt(Stmt::Let {
                pat: Pat::Bind(name, npos),
                ty: None,
                init,
                mutable,
                pos,
            }) = item
            {
                if !*mutable && is_literal(init) {
                    let (v, vty) = self.expr(init, &Expect::Infer);
                    if let Some(&old) = self.top_names.get(name) {
                        if !self.tops[old as usize].prelude {
                            self.err(*npos, format!("`{}` is already defined", name));
                        }
                    }
                    self.top_names.insert(name.clone(), self.tops.len() as u32);
                    self.tops.push(TopInfo {
                        name: name.clone(),
                        ty: vty,
                        mutable: false,
                        captured: false,
                        konst: Some(v.kind),
                        pos: *npos,
                        prelude: npos.file == 0,
                    });
                    hoisted.insert((pos.file, pos.line, pos.col));
                } else {
                    self.future_tops.insert(name.clone(), *npos);
                }
            }
        }
        self.ctxs.pop();
        hoisted
    }

    fn run(&mut self, items: &[Item], print_last: bool) {
        self.declare_units(items);
        self.declare_types(items);
        let fixed = [("None", 0u32), ("Some", 1), ("Ok", 2), ("Err", 3), ("JNull", 4), ("JObj", 9)];
        for (name, id) in fixed {
            if self.variant_names.get(name) != Some(&id) {
                self.err(Pos::default(), "internal: the prelude enums are missing or out of order");
                return;
            }
        }
        self.declare_builtins();
        self.declare_fns(items);
        let hoisted = self.hoist_consts(items);

        // The main body: top-level statements in order.
        let main = self.new_fn("<main>", Origin::Main, Pos::default());
        self.fns[main as usize].ret = Some(Ty::Unit);
        self.fns[main as usize].state = 2;
        self.ctxs.push(FnCtx {
            kind: CtxKind::Main,
            scopes: vec![Vec::new()],
            nslots: 0,
            ret: Ty::Unit,
            loop_depth: 0,
            captures: Vec::new(),
        });
        let last_main = items
            .iter()
            .rposition(|it| matches!(it, Item::Stmt(s) if stmt_pos(s).file == 1));
        let mut out: Vec<TExpr> = Vec::new();
        for (i, item) in items.iter().enumerate() {
            if let Item::Stmt(s) = item {
                if let Stmt::Let { pos, .. } = s {
                    if hoisted.contains(&(pos.file, pos.line, pos.col)) {
                        continue;
                    }
                }
                match s {
                    Stmt::Expr(e) if print_last && Some(i) == last_main => {
                        let (v, vty) = self.expr(e, &Expect::Infer);
                        if matches!(self.resolve(&vty), Ty::Unit | Ty::Never | Ty::Error) {
                            out.push(v);
                        } else {
                            let mut parts = Vec::new();
                            self.display(v, &vty, None, e.pos, &mut parts);
                            let s = self.strcat(parts, e.pos);
                            out.push(te(TKind::CallBuiltin(crate::builtins::B::Print as u16, vec![s]), e.pos));
                        }
                    }
                    _ => {
                        self.stmt(s, &mut out);
                    }
                }
            }
        }
        let ctx = self.ctxs.pop().unwrap();
        let f = &mut self.fns[main as usize];
        f.body = Some(te(TKind::Seq(out, None), Pos::default()));
        f.nslots = ctx.nslots;

        // Function bodies that were not needed while checking the main body.
        let mut fid = 0;
        while fid < self.fns.len() {
            if self.fns[fid].decl.is_some() && self.fns[fid].state == 0 {
                self.check_fn_body(fid as u32);
            }
            fid += 1;
        }

        for item in items {
            if let Item::Test(name, body, pos) = item {
                if pos.file != 1 {
                    continue;
                }
                let tid = self.new_fn(&format!("test \"{}\"", name), Origin::Test, *pos);
                self.ctxs.push(FnCtx {
                    kind: CtxKind::Test,
                    scopes: vec![Vec::new()],
                    nslots: 0,
                    ret: Ty::Unit,
                    loop_depth: 0,
                    captures: Vec::new(),
                });
                let (b, _) = self.block(body, &Expect::Discard);
                let ctx = self.ctxs.pop().unwrap();
                let f = &mut self.fns[tid as usize];
                f.ret = Some(Ty::Unit);
                f.state = 2;
                f.body = Some(b);
                f.nslots = ctx.nslots;
                self.tests.push((name.clone(), tid));
            }
        }
    }

    pub(crate) fn check_fn_body(&mut self, fid: u32) {
        let decl = match self.fns[fid as usize].decl.clone() {
            Some(d) => d,
            None => return,
        };
        self.fns[fid as usize].state = 1;
        let saved_ctxs = std::mem::take(&mut self.ctxs);
        let saved_tscope = std::mem::replace(&mut self.tscope, decl.tparams.clone());
        let fn_bounds = self.fns[fid as usize].bounds.clone();
        let saved_tbounds = std::mem::replace(&mut self.tbounds, fn_bounds);
        let annotated = self.fns[fid as usize].ret.clone();
        let ret = match &annotated {
            Some(t) => t.clone(),
            None => self.fresh(),
        };
        let mut scope = Vec::new();
        for (i, (name, _, _)) in decl.params.iter().enumerate() {
            scope.push(Local {
                name: name.clone(),
                slot: i as u32,
                ty: self.fns[fid as usize].params[i].clone(),
                mutable: false,
                captured: false,
            });
        }
        self.ctxs.push(FnCtx {
            kind: CtxKind::Fn,
            scopes: vec![scope],
            nslots: decl.params.len() as u32,
            ret: ret.clone(),
            loop_depth: 0,
            captures: Vec::new(),
        });
        let end_pos = decl.body.tail.as_ref().map(|t| t.pos).unwrap_or(decl.pos);
        let body = if annotated.is_some() {
            if self.resolve(&ret) == Ty::Unit {
                self.block(&decl.body, &Expect::Discard).0
            } else {
                let (b, bty) = self.block(&decl.body, &Expect::Has(ret.clone()));
                self.coerce(b, &bty, &ret, end_pos, "return value")
            }
        } else {
            let (b, bty) = self.block(&decl.body, &Expect::Tail);
            if self.unify(&bty, &ret) {
                b
            } else if self.resolve(&bty) == Ty::Int && self.resolve(&ret).is_float() {
                Self::to_float(b)
            } else {
                let msg = format!(
                    "`{}` returns {} with `return` but ends with {}",
                    decl.name,
                    self.show(&ret),
                    self.show(&bty)
                );
                self.err_hint(end_pos, msg, "make every path return the same type, or add a return type");
                b
            }
        };
        let ctx = self.ctxs.pop().unwrap();
        self.ctxs = saved_ctxs;
        let mut final_ret = self.zonk(&ret);
        if matches!(final_ret, Ty::Var(_)) {
            final_ret = Ty::Never;
        } else if Self::has_var(&final_ret) {
            let msg = format!("cannot infer the full return type of `{}`", decl.name);
            self.err_hint(decl.pos, msg, "add a return type, e.g. `-> List[Int]`");
            final_ret = Ty::Error;
        }
        self.tscope = saved_tscope;
        self.tbounds = saved_tbounds;
        let f = &mut self.fns[fid as usize];
        f.ret = Some(final_ret);
        f.state = 2;
        f.body = Some(body);
        f.nslots = ctx.nslots;
    }

    /// Return type of a function, checking its body first if it has no annotation.
    pub(crate) fn fn_ret(&mut self, fid: u32, pos: Pos) -> Ty {
        if let Some(t) = &self.fns[fid as usize].ret {
            return t.clone();
        }
        let name = self.fns[fid as usize].name.clone();
        if self.fns[fid as usize].state == 1 {
            self.err_hint(
                pos,
                format!("recursive function `{}` needs a return type", name),
                format!("write it as `fn {}(...) -> Int {{ ... }}`", name),
            );
            return Ty::Error;
        }
        if self.depth > MAX_DEPTH / 2 {
            self.err_hint(pos, "functions call each other too deeply to infer", "add return types to them");
            return Ty::Error;
        }
        self.depth += 40;
        self.check_fn_body(fid);
        self.depth -= 40;
        self.fns[fid as usize].ret.clone().unwrap_or(Ty::Error)
    }

    // ---- statements

    pub(crate) fn block(&mut self, b: &Block, ex: &Expect) -> (TExpr, Ty) {
        self.push_scope();
        let mut out = Vec::new();
        let mut diverges = false;
        for s in &b.stmts {
            diverges = self.stmt(s, &mut out);
        }
        let (tail, mut ty) = match &b.tail {
            Some(t) => {
                let (v, vty) = match ex {
                    Expect::Discard => self.discard_expr(t),
                    _ => self.expr(t, ex),
                };
                (Some(bx(v)), vty)
            }
            None => (None, if diverges { Ty::Never } else { Ty::Unit }),
        };
        self.pop_scope();
        if let (Expect::Has(want), None, false) = (ex, &b.tail, diverges) {
            if !matches!(self.resolve(want), Ty::Unit | Ty::Var(_) | Ty::Error) {
                let msg = format!("this block must give a value of type {}, but it ends with a statement", self.show(want));
                self.err_hint(b.pos, msg, "the last expression of a block is its value");
                ty = Ty::Error;
            }
        }
        (te(TKind::Seq(out, tail), b.pos), ty)
    }

    /// An expression whose value is thrown away.
    pub(crate) fn discard_expr(&mut self, e: &Expr) -> (TExpr, Ty) {
        if let ExprKind::Binary(BinOp::Eq, _, _) = &e.kind {
            self.err_hint(e.pos, "this comparison does nothing", "to assign use `=`");
        }
        let (v, vty) = self.expr(e, &Expect::Discard);
        match self.resolve(&vty) {
            Ty::Enum(E_RESULT, _) => {
                self.err_hint(
                    e.pos,
                    "unused Result: this can fail and the failure is ignored",
                    "handle it with `match`, `?? default`, `.unwrap()`, or `let _ = ...`",
                );
                (v, Ty::Unit)
            }
            Ty::Never => (v, Ty::Never),
            _ => (v, Ty::Unit),
        }
    }

    fn bind_pat(&mut self, pat: &Pat, v: TExpr, ty: &Ty, mutable: bool, out: &mut Vec<TExpr>) {
        let pos = v.pos;
        match pat {
            Pat::Wild(_) => out.push(v),
            Pat::Bind(name, p) => {
                if self.variant_names.contains_key(name) {
                    self.err_hint(*p, format!("`{}` is an enum variant, not a variable name", name), "use `match` to test a value");
                    return;
                }
                match self.declare(name, ty.clone(), mutable, *p) {
                    Decl::Local(slot) => out.push(te(TKind::Let(slot, bx(v)), pos)),
                    Decl::Top(id) => out.push(te(TKind::LetTop(id, bx(v)), pos)),
                }
            }
            Pat::Tuple(subs, p) => {
                let elems = self.fresh_vars(subs.len());
                if !self.unify(ty, &Ty::Tuple(elems.clone())) {
                    let msg = format!("cannot unpack {} into {} names", self.show(ty), subs.len());
                    self.err(*p, msg);
                    return;
                }
                let h = self.new_slot();
                out.push(te(TKind::Let(h, bx(v)), pos));
                for (i, sub) in subs.iter().enumerate() {
                    let field = te(TKind::Field(bx(te(TKind::Local(h), pos)), i as u32), pos);
                    self.bind_pat(sub, field, &elems[i], mutable, out);
                }
            }
            other => {
                self.err_hint(other.pos(), "this pattern can fail to match", "use `match` or `if let` instead");
            }
        }
    }

    /// Keep a sub-expression in a hidden slot so it is evaluated only once.
    fn stabilise(&mut self, v: TExpr, out: &mut Vec<TExpr>) -> TExpr {
        match v.kind {
            TKind::Local(_) | TKind::Top(_) | TKind::Int(_) | TKind::Float(_) | TKind::Str(_) => v,
            _ => {
                let pos = v.pos;
                let h = self.new_slot();
                out.push(te(TKind::Let(h, bx(v)), pos));
                te(TKind::Local(h), pos)
            }
        }
    }

    fn assign_value(&mut self, op: &Option<BinOp>, cur: TExpr, cur_ty: &Ty, value: &Expr, pos: Pos) -> TExpr {
        match op {
            None => self.expect_ty(value, cur_ty, "assigned value"),
            Some(op) => {
                let (r, rty) = self.expr(value, &Expect::Infer);
                let (v, vty) = self.bin_typed(*op, cur, cur_ty, r, &rty, pos);
                self.coerce(v, &vty, cur_ty, pos, "assigned value")
            }
        }
    }

    fn assign(&mut self, target: &Expr, op: &Option<BinOp>, value: &Expr, pos: Pos, out: &mut Vec<TExpr>) {
        match &target.kind {
            ExprKind::Ident(name) => match self.lookup(name) {
                Found::Local(slot, ty, mutable, captured) => {
                    if captured {
                        self.err_hint(
                            target.pos,
                            format!("cannot assign to `{}` inside a closure", name),
                            "closures copy the variables they use; keep changing state in a list or a top-level `var`",
                        );
                    } else if !mutable {
                        self.err_hint(
                            target.pos,
                            format!("cannot assign to `{}`: it is not a `var`", name),
                            format!("declare it with `var {} = ...`", name),
                        );
                    }
                    let v = self.assign_value(op, te(TKind::Local(slot), pos), &ty, value, pos);
                    self.assigns += 1;
                    out.push(te(TKind::SetLocal(slot, bx(v)), pos));
                }
                Found::Top(id) => {
                    if !self.tops[id as usize].mutable {
                        self.err_hint(
                            target.pos,
                            format!("cannot assign to `{}`: it is not a `var`", name),
                            format!("declare it with `var {} = ...`", name),
                        );
                    }
                    let (cur, ty) = self.top_ref(id, pos);
                    let v = self.assign_value(op, cur, &ty, value, pos);
                    self.assigns += 1;
                    out.push(te(TKind::SetTop(id, bx(v)), pos));
                }
                Found::None => {
                    self.err_hint(
                        target.pos,
                        format!("unknown variable `{}`", name),
                        format!("create it first with `var {} = ...`", name),
                    );
                }
            },
            ExprKind::Index(obj, idx) => {
                let (o, oty) = self.expr(obj, &Expect::Infer);
                let (key_ty, val_ty, is_map) = match self.resolve(&oty) {
                    Ty::List(t) => (Ty::Int, *t, false),
                    Ty::Map(k, v) => (*k, *v, true),
                    Ty::Str => {
                        self.err_hint(target.pos, "text cannot be changed in place", "build a new Str instead");
                        return;
                    }
                    Ty::Error => return,
                    other => {
                        let msg = format!("cannot index into {}", self.show(&other));
                        self.err(target.pos, msg);
                        return;
                    }
                };
                let i = self.expect_ty(idx, &key_ty, "index");
                let (o, i) = if op.is_some() {
                    (self.stabilise(o, out), self.stabilise(i, out))
                } else {
                    (o, i)
                };
                let cur = if is_map {
                    te(TKind::IndexMap(bx(o.clone()), bx(i.clone())), pos)
                } else {
                    te(TKind::IndexList(bx(o.clone()), bx(i.clone())), pos)
                };
                let v = self.assign_value(op, cur, &val_ty, value, pos);
                let kind = if is_map {
                    TKind::SetIndexMap(bx(o), bx(i), bx(v))
                } else {
                    TKind::SetIndexList(bx(o), bx(i), bx(v))
                };
                out.push(te(kind, pos));
            }
            ExprKind::Field(obj, fname) => {
                let (o, oty) = self.expr(obj, &Expect::Infer);
                let (idx, fty) = match self.field_of(&oty, fname, target.pos) {
                    Some(x) => x,
                    None => return,
                };
                let o = if op.is_some() { self.stabilise(o, out) } else { o };
                let cur = te(TKind::Field(bx(o.clone()), idx), pos);
                let v = self.assign_value(op, cur, &fty, value, pos);
                out.push(te(TKind::SetField(bx(o), idx, bx(v)), pos));
            }
            _ => self.err_hint(
                target.pos,
                "cannot assign to this",
                "only variables, `list[i]`, `map[key]` and `value.field` can be assigned",
            ),
        }
    }

    fn loop_body(&mut self, body: &Block, mut pre: Vec<TExpr>) -> TExpr {
        self.cx().loop_depth += 1;
        let (b, _) = self.block(body, &Expect::Discard);
        self.cx().loop_depth -= 1;
        if pre.is_empty() {
            b
        } else {
            let pos = b.pos;
            pre.push(b);
            te(TKind::Seq(pre, None), pos)
        }
    }

    /// Check one statement; returns true when control never continues past it.
    pub(crate) fn stmt(&mut self, s: &Stmt, out: &mut Vec<TExpr>) -> bool {
        match s {
            Stmt::Let {
                pat,
                ty,
                init,
                mutable,
                ..
            } => {
                let (v, vty) = match ty {
                    Some(t) => {
                        let want = self.ty_expr(t);
                        let v = self.expect_ty(init, &want, "the value");
                        (v, want)
                    }
                    None => {
                        let (v, vty) = self.expr(init, &Expect::Infer);
                        if self.resolve(&vty) == Ty::Unit {
                            self.err(init.pos, "this expression gives no value to store");
                            (v, Ty::Error)
                        } else {
                            (v, vty)
                        }
                    }
                };
                let never = matches!(self.resolve(&vty), Ty::Never);
                self.bind_pat(pat, v, &vty, *mutable, out);
                never
            }
            Stmt::Assign {
                target,
                op,
                value,
                pos,
            } => {
                self.assign(target, op, value, *pos, out);
                false
            }
            Stmt::Expr(e) => {
                let (v, vty) = self.discard_expr(e);
                out.push(v);
                matches!(vty, Ty::Never)
            }
            Stmt::While { cond, body, pos } => {
                let c = self.expect_ty(cond, &Ty::Bool, "loop condition");
                let b = self.loop_body(body, Vec::new());
                out.push(te(TKind::While(bx(c), bx(b)), *pos));
                false
            }
            Stmt::For {
                pat,
                iter,
                range_end,
                body,
                pos,
            } => {
                self.push_scope();
                let mut pre: Vec<TExpr> = Vec::new();
                let node = match range_end {
                    Some((end, inclusive)) => {
                        let start = self.expect_ty(iter, &Ty::Int, "range start");
                        let end = self.expect_ty(end, &Ty::Int, "range end");
                        let slot = match pat {
                            Pat::Bind(name, p) => self.declare_slot(name, Ty::Int, *p),
                            Pat::Wild(_) => self.new_slot(),
                            other => {
                                self.err_hint(other.pos(), "a range loop needs one plain name", "write `for i in 0..n`");
                                self.new_slot()
                            }
                        };
                        let b = self.loop_body(body, pre);
                        TKind::ForRange {
                            slot,
                            start: bx(start),
                            end: bx(end),
                            inclusive: *inclusive,
                            body: bx(b),
                        }
                    }
                    None => {
                        let (l, lty) = self.expr(iter, &Expect::Infer);
                        let elem = match self.resolve(&lty) {
                            Ty::List(t) => *t,
                            Ty::Var(_) => {
                                let t = self.fresh();
                                self.unify(&lty, &Ty::List(Box::new(t.clone())));
                                t
                            }
                            Ty::Map(_, _) => {
                                self.err_hint(iter.pos, "cannot loop over a Map directly", "loop over `m.keys()`, `m.values()` or `m.items()`");
                                Ty::Error
                            }
                            Ty::Str => {
                                self.err_hint(iter.pos, "cannot loop over a Str directly", "loop over `s.chars()` or `s.lines()`");
                                Ty::Error
                            }
                            Ty::Int => {
                                self.err_hint(iter.pos, "cannot loop over an Int", "write `for i in 0..n`");
                                Ty::Error
                            }
                            Ty::Error | Ty::Never => Ty::Error,
                            other => {
                                let msg = format!("cannot loop over {}", self.show(&other));
                                self.err(iter.pos, msg);
                                Ty::Error
                            }
                        };
                        let slot = match pat {
                            Pat::Bind(name, p) => self.declare_slot(name, elem, *p),
                            Pat::Wild(_) => self.new_slot(),
                            Pat::Tuple(_, _) => {
                                let h = self.new_slot();
                                self.bind_pat(pat, te(TKind::Local(h), *pos), &elem, false, &mut pre);
                                h
                            }
                            other => {
                                self.err_hint(other.pos(), "this pattern can fail to match", "use a plain name or a tuple like `(a, b)`");
                                self.new_slot()
                            }
                        };
                        let b = self.loop_body(body, pre);
                        TKind::ForList {
                            slot,
                            list: bx(l),
                            body: bx(b),
                        }
                    }
                };
                self.pop_scope();
                out.push(te(node, *pos));
                false
            }
            Stmt::Return(value, pos) => {
                let kind = self.cx().kind;
                if kind == CtxKind::Main {
                    self.err(*pos, "`return` can only be used inside a function");
                    return true;
                }
                let ret = self.cx().ret.clone();
                let v = match value {
                    Some(e) => Some(bx(self.expect_ty(e, &ret, "return value"))),
                    None => {
                        if !self.unify(&ret, &Ty::Unit) {
                            let msg = format!("this function must return {}", self.show(&ret));
                            self.err(*pos, msg);
                        }
                        None
                    }
                };
                out.push(te(TKind::Return(v), *pos));
                true
            }
            Stmt::Break(pos) | Stmt::Continue(pos) => {
                if self.cx().loop_depth == 0 {
                    self.err(*pos, "`break` and `continue` can only be used inside a loop");
                }
                let kind = if matches!(s, Stmt::Break(_)) {
                    TKind::Break
                } else {
                    TKind::Continue
                };
                out.push(te(kind, *pos));
                true
            }
        }
    }

    fn finish(self) -> Checked {
        let ret_unit: Vec<bool> = self
            .fns
            .iter()
            .map(|f| match &f.ret {
                Some(t) => matches!(self.zonk(t), Ty::Unit | Ty::Never | Ty::Error),
                None => true,
            })
            .collect();
        let fns = self
            .fns
            .into_iter()
            .zip(ret_unit)
            .map(|(f, ret_unit)| TFn {
                name: f.name,
                nparams: f.params.len() as u32,
                captures: f.captures,
                nslots: f.nslots,
                body: f.body.unwrap_or(te(TKind::Unit, f.pos)),
                ret_unit,
                pos: f.pos,
            })
            .collect::<Vec<_>>();
        let main = fns.iter().position(|f| f.name == "<main>").unwrap_or(0) as u32;
        Checked {
            fns,
            structs: self
                .structs
                .into_iter()
                .map(|s| (s.name, s.fields.into_iter().map(|(n, _)| n).collect()))
                .collect(),
            variants: self.variants.into_iter().map(|v| (v.name, v.fields.len() as u32)).collect(),
            tops: self.tops.into_iter().map(|t| (t.name, t.captured)).collect(),
            main,
            tests: self.tests,
        }
    }
}

/// Check a whole program (prelude, imports and main file items together).
pub fn check(items: &[Item], print_last: bool) -> Result<Checked, Vec<Diag>> {
    let mut c = Checker::new();
    c.run(items, print_last);
    if c.errs.is_empty() {
        Ok(c.finish())
    } else {
        Err(c.errs)
    }
}
