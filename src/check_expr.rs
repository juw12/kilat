//! Type and unit checker, part 2: expressions, calls and patterns.

use crate::ast::*;
use crate::builtins::B;
use crate::check::*;
use crate::parser::parse_unit_str;
use crate::show::*;
use crate::tir::*;
use crate::types::*;

const SPECIAL: [&str; 11] =
    ["print", "str", "assert", "assert_eq", "sqrt", "sum", "mean", "median", "stddev", "panic", "host"];
const FORMAT_HELP: &str = "examples: `{x:.2}` (2 decimals), `{x:8}` (width 8), `{p:MPa.1}` (shown in MPa)";

fn count(n: usize, word: &str) -> String {
    if n == 1 {
        format!("1 {}", word)
    } else {
        format!("{} {}s", n, word)
    }
}

/// Names people bring from other languages, mapped to the Kilat spelling.
fn alias_hint(name: &str) -> Option<&'static str> {
    Some(match name {
        "append" | "add" | "push_back" => "push",
        "println" | "printf" | "echo" | "puts" => "print",
        "length" | "size" => "len",
        "toString" | "to_string" | "to_str" | "string" => "str",
        "substr" | "substring" => "slice",
        "strip" => "trim",
        "toUpperCase" | "to_upper" | "uppercase" => "upper",
        "toLowerCase" | "to_lower" | "lowercase" => "lower",
        "indexOf" | "index" => "index_of",
        "startswith" | "startsWith" => "starts_with",
        "endswith" | "endsWith" => "ends_with",
        "parse_int" | "parseInt" => "to_int",
        "parse_float" | "parseFloat" => "to_float",
        "forEach" | "for_each" => "each",
        "fold" => "reduce",
        "includes" => "contains",
        "delete" | "del" => "remove",
        "sorted" => "sort",
        "reversed" => "reverse",
        "log" => "ln",
        _ => return None,
    })
}

fn ident_hint(name: &str) -> Option<&'static str> {
    Some(match name {
        "True" => "write `true`",
        "False" => "write `false`",
        "null" | "nil" | "undefined" | "NULL" => "there is no null: use `None` and `Some(x)`",
        "math" | "Math" => "math functions are built in: write `sin(x)`, `sqrt(x)`, `pi`",
        "self" | "this" => "there are no classes: pass the value as the first parameter",
        "PI" => "write `pi`",
        _ => return None,
    })
}

/// What the next item of a list or map literal should look like.
fn hint_ex(want: &Option<Ty>, got: &[(TExpr, Ty, Pos)]) -> Expect {
    match (want, got.first()) {
        (Some(w), _) => Expect::Has(w.clone()),
        (None, Some((_, t, _))) => Expect::Has(t.clone()),
        (None, None) => Expect::Infer,
    }
}

fn bin(k: Bin2, l: TExpr, r: TExpr, pos: Pos) -> TExpr {
    te(TKind::Bin(k, bx(l), bx(r)), pos)
}

fn fmt_call(v: TExpr, width: i64, prec: i64, align: i64, pos: Pos) -> TExpr {
    let k = |n: i64| te(TKind::Int(n), pos);
    te(TKind::CallBuiltin(B::Fmt as u16, vec![v, k(width), k(prec), k(align)]), pos)
}

/// `v * mul / div`, skipping factors of one and folding literals.
fn scale(v: TExpr, mul: f64, div: f64) -> TExpr {
    let pos = v.pos;
    if let TKind::Float(f) = v.kind {
        return te(TKind::Float(f * mul / div), pos);
    }
    let mut v = v;
    if mul != 1.0 {
        v = bin(Bin2::MulF, v, te(TKind::Float(mul), pos), pos);
    }
    if div != 1.0 {
        v = bin(Bin2::DivF, v, te(TKind::Float(div), pos), pos);
    }
    v
}

impl Checker {
    pub(crate) fn expr(&mut self, e: &Expr, ex: &Expect) -> (TExpr, Ty) {
        self.depth += 1;
        if self.depth > MAX_DEPTH {
            self.depth -= 1;
            self.err(e.pos, "this code is nested too deeply");
            return self.bad(e.pos);
        }
        let r = self.expr_inner(e, ex);
        self.depth -= 1;
        r
    }

    fn expr_inner(&mut self, e: &Expr, ex: &Expect) -> (TExpr, Ty) {
        let pos = e.pos;
        match &e.kind {
            ExprKind::Int(i) => {
                if let Expect::Has(t) = ex {
                    if self.resolve(t).is_float() {
                        return (te(TKind::Float(*i as f64), pos), Ty::float());
                    }
                }
                (te(TKind::Int(*i), pos), Ty::Int)
            }
            ExprKind::Float(f) => (te(TKind::Float(*f), pos), Ty::float()),
            ExprKind::Bool(b) => (te(TKind::Bool(*b), pos), Ty::Bool),
            ExprKind::Str(s) => (te(TKind::Str(s.clone()), pos), Ty::Str),
            ExprKind::Interp(parts) => {
                let mut out = Vec::new();
                for (i, part) in parts.iter().enumerate() {
                    match part {
                        InterpPart::Lit(s) => out.push(te(TKind::Str(s.clone()), pos)),
                        InterpPart::Expr(inner, spec) => {
                            let (v, vty) = self.expr(inner, &Expect::Infer);
                            let after = match parts.get(i + 1) {
                                Some(InterpPart::Lit(text)) => Some(text.as_str()),
                                _ => None,
                            };
                            let at = Shown { expr: &**inner, kind: ShowKind::Interp, after };
                            self.display(v, &vty, spec.as_deref(), at, &mut out);
                        }
                    }
                }
                (self.strcat(out, pos), Ty::Str)
            }
            ExprKind::UnitLit(v, u) => match self.unit_def(u, false) {
                Some((dim, num, den)) => (te(TKind::Float(v * num / den), pos), Ty::Num(dim)),
                None => self.bad(pos),
            },
            ExprKind::Ident(name) => self.ident(name, pos),
            ExprKind::List(items) => self.list_lit(items, ex, pos),
            ExprKind::Map(pairs) => self.map_lit(pairs, ex, pos),
            ExprKind::Tuple(items) => self.tuple_lit(items, ex, pos),
            ExprKind::Neg(inner) => self.neg(inner, ex, pos),
            ExprKind::Not(inner) => {
                let v = self.expect_ty(inner, &Ty::Bool, "the value after `not`");
                (te(TKind::Un(Un1::Not, bx(v)), pos), Ty::Bool)
            }
            ExprKind::And(a, b) => {
                let l = self.expect_ty(a, &Ty::Bool, "left side of `and`");
                let r = self.expect_ty(b, &Ty::Bool, "right side of `and`");
                (te(TKind::And(bx(l), bx(r)), pos), Ty::Bool)
            }
            ExprKind::Or(a, b) => {
                let l = self.expect_ty(a, &Ty::Bool, "left side of `or`");
                let r = self.expect_ty(b, &Ty::Bool, "right side of `or`");
                (te(TKind::Or(bx(l), bx(r)), pos), Ty::Bool)
            }
            ExprKind::Binary(op, l, r) => {
                let product = matches!(op, BinOp::Mul | BinOp::Div);
                if product && self.product_child != e as *const Expr as usize {
                    self.rpm_twice(e);
                }
                if product {
                    self.product_child = &**l as *const Expr as usize;
                }
                let (lv, lt) = self.expr(l, &Expect::Infer);
                let before = self.assigns;
                if product {
                    self.product_child = &**r as *const Expr as usize;
                }
                let (rv, rt) = self.expr(r, &Expect::Infer);
                let lv = self.keep(lv, before);
                self.bin_typed(*op, lv, &lt, rv, &rt, pos)
            }
            ExprKind::Coalesce(a, b) => self.coalesce(a, b, pos),
            ExprKind::Convert(inner, u) => self.convert(inner, u, pos),
            ExprKind::Call(callee, args) => match &callee.kind {
                ExprKind::Ident(name) => {
                    self.note_lambdas(None, args);
                    self.call_name(name, None, args, pos, ex)
                }
                _ => self.call_value(callee, args, pos),
            },
            ExprKind::Method(recv, name, args) => {
                self.note_lambdas(Some(&**recv), args);
                self.call_name(name, Some(&**recv), args, pos, ex)
            }
            ExprKind::Field(obj, name) => {
                let (o, ot) = self.expr(obj, &Expect::Infer);
                match self.field_of(&ot, name, pos) {
                    Some((idx, fty)) => (te(TKind::Field(bx(o), idx), pos), fty),
                    None => self.bad(pos),
                }
            }
            ExprKind::TupleIndex(obj, i) => {
                let (o, ot) = self.expr(obj, &Expect::Infer);
                match self.resolve(&ot) {
                    Ty::Tuple(ts) => match ts.get(*i as usize) {
                        Some(t) => (te(TKind::Field(bx(o), *i), pos), t.clone()),
                        None => {
                            let msg = format!("this tuple has {}, so `.{}` does not exist", count(ts.len(), "item"), i);
                            self.err(pos, msg);
                            self.bad(pos)
                        }
                    },
                    Ty::Error => self.bad(pos),
                    other => {
                        let msg = format!("`.{}` works on tuples, but this is {}", i, self.show(&other));
                        self.err(pos, msg);
                        self.bad(pos)
                    }
                }
            }
            ExprKind::Index(obj, idx) => self.index(obj, idx, pos),
            ExprKind::Try(inner) => self.try_expr(inner, pos),
            ExprKind::Lambda(params, body) => self.lambda(params, body, ex, pos),
            ExprKind::If(c, then, els) => self.if_expr(c, then, els.as_deref(), ex, pos),
            ExprKind::Match(scrut, arms) => self.match_expr(scrut, arms, ex, pos),
            ExprKind::Block(b) => self.block(b, ex),
        }
    }

    /// A variable read on the left must be copied if the right side reassigns variables.
    fn keep(&self, v: TExpr, assigns_before: u32) -> TExpr {
        if self.assigns != assigns_before && matches!(v.kind, TKind::Local(_) | TKind::Top(_)) {
            let pos = v.pos;
            te(TKind::Copy(bx(v)), pos)
        } else {
            v
        }
    }

    // ---- names

    fn ident(&mut self, name: &str, pos: Pos) -> (TExpr, Ty) {
        match self.lookup(name) {
            Found::Local(slot, ty, _, _) => return (te(TKind::Local(slot), pos), ty),
            Found::Top(id) => return self.top_ref(id, pos),
            Found::None => {}
        }
        if let Some(&vid) = self.variant_names.get(name) {
            let (eid, nfields) = {
                let v = &self.variants[vid as usize];
                (v.enum_id, v.fields.len())
            };
            if nfields > 0 {
                let msg = format!("`{}` holds {}", name, count(nfields, "value"));
                self.err_hint(pos, msg, format!("write `{}(...)`", name));
                return self.bad(pos);
            }
            let n = self.enums[eid as usize].tparams.len();
            let targs = self.fresh_vars(n);
            return (te(TKind::Tag(vid), pos), Ty::Enum(eid, targs));
        }
        if let Some(cands) = self.fn_names.get(name).cloned() {
            if cands.len() == 1 {
                if let Cand::Fn(fid) = cands[0].cand {
                    let ret = self.fn_ret(fid, pos);
                    let n = self.fns[fid as usize].tparams.len();
                    let vars = self.fresh_vars(n);
                    if n > 0 {
                        self.note_generic_use(fid, &vars, pos, name, &[]);
                    }
                    let params: Vec<Ty> = self.fns[fid as usize].params.iter().map(|p| Self::inst(p, &vars)).collect();
                    let ret = Self::inst(&ret, &vars);
                    return (te(TKind::FnRef(fid), pos), Ty::Fn(params, Box::new(ret)));
                }
            }
            let msg = format!("`{}` cannot be used as a value here", name);
            self.err_hint(pos, msg, format!("wrap it in a lambda, e.g. `|x| {}(x)`", name));
            return self.bad(pos);
        }
        if SPECIAL.contains(&name) {
            let msg = format!("`{}` cannot be used as a value here", name);
            self.err_hint(pos, msg, format!("wrap it in a lambda, e.g. `|x| {}(x)`", name));
            return self.bad(pos);
        }
        if self.struct_names.contains_key(name) {
            let msg = format!("`{}` is a struct, not a value", name);
            self.err_hint(pos, msg, format!("build one with `{}(...)`", name));
            return self.bad(pos);
        }
        if let Some(p) = self.future_tops.get(name) {
            let msg = format!("`{}` is used before it is defined (line {})", name, p.line);
            self.err_hint(
                pos,
                msg,
                "define it above the code that uses it; if a function uses it, give that function a return type",
            );
            return self.bad(pos);
        }
        let msg = format!("unknown name `{}`", name);
        if let Some(h) = ident_hint(name) {
            self.err_hint(pos, msg, h);
            return self.bad(pos);
        }
        let mut names: Vec<String> = Vec::new();
        if let Some(c) = self.ctxs.last() {
            for scope in &c.scopes {
                for l in scope {
                    names.push(l.name.clone());
                }
            }
        }
        names.extend(self.top_names.keys().cloned());
        names.extend(self.fn_names.keys().cloned());
        names.extend(self.variant_names.keys().cloned());
        match closest(name, names.iter().map(|s| s.as_str())) {
            Some(c) => self.err_hint(pos, msg, format!("did you mean `{}`?", c)),
            None => self.err_hint(pos, msg, format!("create it first with `let {} = ...`", name)),
        }
        self.bad(pos)
    }

    pub(crate) fn top_ref(&mut self, id: u32, pos: Pos) -> (TExpr, Ty) {
        let ty = self.tops[id as usize].ty.clone();
        if let Some(k) = &self.tops[id as usize].konst {
            return (te(k.clone(), pos), ty);
        }
        if !self.in_main_proper() {
            self.tops[id as usize].captured = true;
        }
        (te(TKind::Top(id), pos), ty)
    }

    // ---- literals

    /// Give a group of values one common type; a mix of Int and Float becomes Float.
    fn same_type(&mut self, items: Vec<(TExpr, Ty, Pos)>, want: Option<Ty>, what: &str) -> (Vec<TExpr>, Ty) {
        let ty = match want {
            Some(w) => w,
            None => {
                let any_float = items.iter().any(|(_, t, _)| self.resolve(t).is_float());
                let first = items
                    .iter()
                    .map(|(_, t, _)| self.resolve(t))
                    .find(|t| !matches!(t, Ty::Never | Ty::Error));
                match first {
                    Some(Ty::Int) if any_float => Ty::float(),
                    Some(t) => t,
                    None => self.fresh(),
                }
            }
        };
        let mut out = Vec::with_capacity(items.len());
        for (v, t, p) in items {
            out.push(self.coerce(v, &t, &ty, p, what));
        }
        (out, ty)
    }

    /// An expected type that is still unknown gives no guidance.
    fn known(&self, t: Option<Ty>) -> Option<Ty> {
        match t {
            Some(t) if matches!(self.resolve(&t), Ty::Var(_)) => None,
            other => other,
        }
    }

    fn list_lit(&mut self, items: &[Expr], ex: &Expect, pos: Pos) -> (TExpr, Ty) {
        let want: Option<Ty> = match ex {
            Expect::Has(t) => match self.resolve(t) {
                Ty::List(el) => Some(*el),
                _ => None,
            },
            _ => None,
        };
        let want = self.known(want);
        let mut got: Vec<(TExpr, Ty, Pos)> = Vec::with_capacity(items.len());
        for it in items {
            let hint = hint_ex(&want, &got);
            let (v, t) = self.expr(it, &hint);
            if self.resolve(&t) == Ty::Unit {
                self.err(it.pos, "this expression gives no value to put in the list");
                got.push((v, Ty::Error, it.pos));
            } else {
                got.push((v, t, it.pos));
            }
        }
        let (vals, el) = self.same_type(got, want, "list item");
        (te(TKind::NewList(vals), pos), Ty::List(Box::new(el)))
    }

    fn map_lit(&mut self, pairs: &[(Expr, Expr)], ex: &Expect, pos: Pos) -> (TExpr, Ty) {
        let (want_k, want_v) = match ex {
            Expect::Has(t) => match self.resolve(t) {
                Ty::Map(k, v) => (Some(*k), Some(*v)),
                _ => (None, None),
            },
            _ => (None, None),
        };
        let want_k = self.known(want_k);
        let want_v = self.known(want_v);
        let mut keys: Vec<(TExpr, Ty, Pos)> = Vec::with_capacity(pairs.len());
        let mut vals: Vec<(TExpr, Ty, Pos)> = Vec::with_capacity(pairs.len());
        for (k, v) in pairs {
            let kh = hint_ex(&want_k, &keys);
            let (kv, kt) = self.expr(k, &kh);
            keys.push((kv, kt, k.pos));
            let vh = hint_ex(&want_v, &vals);
            let (vv, vt) = self.expr(v, &vh);
            if self.resolve(&vt) == Ty::Unit {
                self.err(v.pos, "this expression gives no value to put in the map");
                vals.push((vv, Ty::Error, v.pos));
            } else {
                vals.push((vv, vt, v.pos));
            }
        }
        let (ks, kty) = self.same_type(keys, want_k, "map key");
        let (vs, vty) = self.same_type(vals, want_v, "map value");
        match self.resolve(&kty) {
            Ty::Int | Ty::Str | Ty::Var(_) | Ty::Param(_) | Ty::Error => {}
            other => {
                let msg = format!("map keys must be Int or Str, found {}", self.show(&other));
                self.err_hint(pos, msg, "turn the key into text, e.g. `\"{x}\"`");
            }
        }
        let entries: Vec<(TExpr, TExpr)> = ks.into_iter().zip(vs).collect();
        (te(TKind::NewMap(entries), pos), Ty::Map(Box::new(kty), Box::new(vty)))
    }

    fn tuple_lit(&mut self, items: &[Expr], ex: &Expect, pos: Pos) -> (TExpr, Ty) {
        if items.is_empty() {
            return (te(TKind::Unit, pos), Ty::Unit);
        }
        let want: Option<Vec<Ty>> = match ex {
            Expect::Has(t) => match self.resolve(t) {
                Ty::Tuple(ts) if ts.len() == items.len() => Some(ts),
                _ => None,
            },
            _ => None,
        };
        let mut vals = Vec::with_capacity(items.len());
        let mut tys = Vec::with_capacity(items.len());
        for (i, it) in items.iter().enumerate() {
            match &want {
                Some(ts) => {
                    vals.push(self.expect_ty(it, &ts[i], "tuple item"));
                    tys.push(ts[i].clone());
                }
                None => {
                    let (v, t) = self.expr(it, &Expect::Infer);
                    if self.resolve(&t) == Ty::Unit {
                        self.err(it.pos, "this expression gives no value to put in the tuple");
                        tys.push(Ty::Error);
                    } else {
                        tys.push(t);
                    }
                    vals.push(v);
                }
            }
        }
        (te(TKind::NewTuple(vals), pos), Ty::Tuple(tys))
    }

    // ---- operators

    fn neg(&mut self, inner: &Expr, ex: &Expect, pos: Pos) -> (TExpr, Ty) {
        let sub = match ex {
            Expect::Has(t) => Expect::Has(t.clone()),
            _ => Expect::Infer,
        };
        let (v, t) = self.expr(inner, &sub);
        match self.resolve(&t) {
            Ty::Int => match v.kind {
                TKind::Int(i) => (te(TKind::Int(i.wrapping_neg()), pos), Ty::Int),
                _ => (te(TKind::Un(Un1::NegI, bx(v)), pos), Ty::Int),
            },
            Ty::Num(d) => match v.kind {
                TKind::Float(f) => (te(TKind::Float(-f), pos), Ty::Num(d)),
                _ => (te(TKind::Un(Un1::NegF, bx(v)), pos), Ty::Num(d)),
            },
            Ty::Never => (v, Ty::Never),
            Ty::Error => self.bad(pos),
            Ty::Var(_) => {
                self.err_hint(pos, "cannot infer the type of this value", "add a type annotation, e.g. `|x: Float| ...`");
                self.bad(pos)
            }
            other => {
                let msg = format!("cannot negate {}", self.show(&other));
                self.err(pos, msg);
                self.bad(pos)
            }
        }
    }

    fn unit_mismatch(&mut self, verb: &str, a: &Ty, b: &Ty, pos: Pos) -> (TExpr, Ty) {
        let msg = format!("unit mismatch: cannot {} {} and {}", verb, self.show(a), self.show(b));
        let plain = |t: &Ty| matches!(t, Ty::Int) || t.is_float();
        if plain(a) != plain(b) {
            let unit = if plain(a) { self.show(b) } else { self.show(a) };
            self.err_hint(
                pos,
                msg,
                format!("one side is a plain number: add a unit, e.g. `5 {0}`, or convert the other side with `x in {0}`", unit),
            );
        } else {
            self.err(pos, msg);
        }
        self.bad(pos)
    }

    fn dim_overflow(&mut self, pos: Pos) -> (TExpr, Ty) {
        self.err(pos, "the unit of this result has an exponent that is too large");
        self.bad(pos)
    }

    pub(crate) fn bin_typed(&mut self, op: BinOp, l: TExpr, lt: &Ty, r: TExpr, rt: &Ty, pos: Pos) -> (TExpr, Ty) {
        use BinOp::*;
        let mut a = self.resolve(lt);
        let mut b = self.resolve(rt);
        if a == Ty::Error || b == Ty::Error {
            return self.bad(pos);
        }
        if a == Ty::Never || b == Ty::Never {
            return (te(TKind::Seq(vec![l, r], None), pos), Ty::Never);
        }
        let scaling = matches!(op, Mul | Div | Pow);
        match (matches!(a, Ty::Var(_)), matches!(b, Ty::Var(_))) {
            (true, true) => {
                if matches!(op, Eq | Ne) {
                    self.unify(&a, &b);
                    let k = if op == Eq { Bin2::EqV } else { Bin2::NeV };
                    return (bin(k, l, r, pos), Ty::Bool);
                }
                self.err_hint(pos, "cannot infer the types of these values", "add a type annotation, e.g. `|x: Float| ...`");
                return self.bad(pos);
            }
            (true, false) => {
                let t = if scaling && matches!(b, Ty::Num(_)) { Ty::float() } else { b.clone() };
                self.unify(&a, &t);
                a = t;
            }
            (false, true) => {
                let t = if scaling && matches!(a, Ty::Num(_)) { Ty::float() } else { a.clone() };
                self.unify(&b, &t);
                b = t;
            }
            (false, false) => {}
        }
        match op {
            Add | Sub | Mod => {
                let (ki, kf, verb) = match op {
                    Add => (Bin2::AddI, Bin2::AddF, "add"),
                    Sub => (Bin2::SubI, Bin2::SubF, "subtract"),
                    _ => (Bin2::ModI, Bin2::ModF, "take the remainder of"),
                };
                match (&a, &b) {
                    (Ty::Int, Ty::Int) => (bin(ki, l, r, pos), Ty::Int),
                    (Ty::Num(d1), Ty::Num(d2)) => {
                        if d1 == d2 {
                            (bin(kf, l, r, pos), a.clone())
                        } else {
                            self.unit_mismatch(verb, &a, &b, pos)
                        }
                    }
                    (Ty::Int, Ty::Num(d)) => {
                        if d.is_none() {
                            (bin(kf, Self::to_float(l), r, pos), b.clone())
                        } else {
                            self.unit_mismatch(verb, &a, &b, pos)
                        }
                    }
                    (Ty::Num(d), Ty::Int) => {
                        if d.is_none() {
                            (bin(kf, l, Self::to_float(r), pos), a.clone())
                        } else {
                            self.unit_mismatch(verb, &a, &b, pos)
                        }
                    }
                    (Ty::Str, Ty::Str) if op == Add => (bin(Bin2::Concat, l, r, pos), Ty::Str),
                    (Ty::List(x), Ty::List(y)) if op == Add => {
                        if !self.unify(x, y) {
                            let msg = format!("cannot join {} and {}", self.show(&a), self.show(&b));
                            self.err(pos, msg);
                            return self.bad(pos);
                        }
                        (te(TKind::CallBuiltin(B::ConcatL as u16, vec![l, r]), pos), a.clone())
                    }
                    (Ty::Str, _) | (_, Ty::Str) if op == Add => {
                        let msg = format!("cannot add {} and {}", self.show(&a), self.show(&b));
                        self.err_hint(pos, msg, "put the value inside the text, e.g. `\"total: {x}\"`, or use `str(x)`");
                        self.bad(pos)
                    }
                    _ => {
                        let msg = format!("cannot {} {} and {}", verb, self.show(&a), self.show(&b));
                        self.err(pos, msg);
                        self.bad(pos)
                    }
                }
            }
            Mul => match (&a, &b) {
                (Ty::Int, Ty::Int) => (bin(Bin2::MulI, l, r, pos), Ty::Int),
                (Ty::Num(d1), Ty::Num(d2)) => match d1.mul(d2) {
                    Some(d) => (bin(Bin2::MulF, l, r, pos), Ty::Num(d)),
                    None => self.dim_overflow(pos),
                },
                (Ty::Int, Ty::Num(_)) => (bin(Bin2::MulF, Self::to_float(l), r, pos), b.clone()),
                (Ty::Num(_), Ty::Int) => (bin(Bin2::MulF, l, Self::to_float(r), pos), a.clone()),
                (Ty::Str, Ty::Int) => {
                    self.err_hint(pos, "cannot multiply Str and Int", "to repeat text use `repeat(text, n)`");
                    self.bad(pos)
                }
                _ => {
                    let msg = format!("cannot multiply {} and {}", self.show(&a), self.show(&b));
                    self.err(pos, msg);
                    self.bad(pos)
                }
            },
            Div => match (&a, &b) {
                (Ty::Int, Ty::Int) => (bin(Bin2::DivF, Self::to_float(l), Self::to_float(r), pos), Ty::float()),
                (Ty::Num(d1), Ty::Num(d2)) => match d1.div(d2) {
                    Some(d) => (bin(Bin2::DivF, l, r, pos), Ty::Num(d)),
                    None => self.dim_overflow(pos),
                },
                (Ty::Int, Ty::Num(d2)) => match Dim::none().div(d2) {
                    Some(d) => (bin(Bin2::DivF, Self::to_float(l), r, pos), Ty::Num(d)),
                    None => self.dim_overflow(pos),
                },
                (Ty::Num(_), Ty::Int) => (bin(Bin2::DivF, l, Self::to_float(r), pos), a.clone()),
                _ => {
                    let msg = format!("cannot divide {} by {}", self.show(&a), self.show(&b));
                    self.err(pos, msg);
                    self.bad(pos)
                }
            },
            IDiv => match (&a, &b) {
                (Ty::Int, Ty::Int) => (bin(Bin2::DivI, l, r, pos), Ty::Int),
                _ => {
                    let msg = format!("`div` needs two Int values, found {} and {}", self.show(&a), self.show(&b));
                    self.err_hint(pos, msg, "for decimals use `/`; to round down use `floor(a / b)`");
                    self.bad(pos)
                }
            },
            Pow => self.pow(l, &a, r, &b, pos),
            Eq | Ne => {
                let (ki, kf, kv) = if op == Eq {
                    (Bin2::EqI, Bin2::EqF, Bin2::EqV)
                } else {
                    (Bin2::NeI, Bin2::NeF, Bin2::NeV)
                };
                match (&a, &b) {
                    (Ty::Int, Ty::Int) => (bin(ki, l, r, pos), Ty::Bool),
                    (Ty::Num(d1), Ty::Num(d2)) => {
                        if d1 == d2 {
                            (bin(kf, l, r, pos), Ty::Bool)
                        } else {
                            self.unit_mismatch("compare", &a, &b, pos)
                        }
                    }
                    (Ty::Int, Ty::Num(d)) => {
                        if d.is_none() {
                            (bin(kf, Self::to_float(l), r, pos), Ty::Bool)
                        } else {
                            self.unit_mismatch("compare", &a, &b, pos)
                        }
                    }
                    (Ty::Num(d), Ty::Int) => {
                        if d.is_none() {
                            (bin(kf, l, Self::to_float(r), pos), Ty::Bool)
                        } else {
                            self.unit_mismatch("compare", &a, &b, pos)
                        }
                    }
                    _ => {
                        if self.unify(&a, &b) {
                            (bin(kv, l, r, pos), Ty::Bool)
                        } else {
                            let msg = format!("cannot compare {} with {}", self.show(&a), self.show(&b));
                            self.err(pos, msg);
                            self.bad(pos)
                        }
                    }
                }
            }
            Lt | Le | Gt | Ge => {
                let (ki, kf, ks) = match op {
                    Lt => (Bin2::LtI, Bin2::LtF, Bin2::LtS),
                    Le => (Bin2::LeI, Bin2::LeF, Bin2::LeS),
                    Gt => (Bin2::GtI, Bin2::GtF, Bin2::GtS),
                    _ => (Bin2::GeI, Bin2::GeF, Bin2::GeS),
                };
                match (&a, &b) {
                    (Ty::Int, Ty::Int) => (bin(ki, l, r, pos), Ty::Bool),
                    (Ty::Num(d1), Ty::Num(d2)) => {
                        if d1 == d2 {
                            (bin(kf, l, r, pos), Ty::Bool)
                        } else {
                            self.unit_mismatch("compare", &a, &b, pos)
                        }
                    }
                    (Ty::Int, Ty::Num(d)) => {
                        if d.is_none() {
                            (bin(kf, Self::to_float(l), r, pos), Ty::Bool)
                        } else {
                            self.unit_mismatch("compare", &a, &b, pos)
                        }
                    }
                    (Ty::Num(d), Ty::Int) => {
                        if d.is_none() {
                            (bin(kf, l, Self::to_float(r), pos), Ty::Bool)
                        } else {
                            self.unit_mismatch("compare", &a, &b, pos)
                        }
                    }
                    (Ty::Str, Ty::Str) => (bin(ks, l, r, pos), Ty::Bool),
                    _ => {
                        let msg = format!("cannot order {} and {}", self.show(&a), self.show(&b));
                        self.err_hint(pos, msg, "only numbers and text can be compared with `<` and `>`");
                        self.bad(pos)
                    }
                }
            }
        }
    }

    fn pow(&mut self, l: TExpr, a: &Ty, r: TExpr, b: &Ty, pos: Pos) -> (TExpr, Ty) {
        match (a, b) {
            (Ty::Int, Ty::Int) => (bin(Bin2::PowI, l, r, pos), Ty::Int),
            (Ty::Int, Ty::Num(e)) if e.is_none() => (bin(Bin2::PowF, Self::to_float(l), r, pos), Ty::float()),
            (Ty::Num(d), Ty::Int) if d.is_none() => (bin(Bin2::PowFI, l, r, pos), Ty::float()),
            (Ty::Num(d), Ty::Num(e)) if d.is_none() && e.is_none() => (bin(Bin2::PowF, l, r, pos), Ty::float()),
            (_, Ty::Num(e)) if !e.is_none() => {
                self.err(pos, "an exponent cannot have a unit");
                self.bad(pos)
            }
            (Ty::Num(d), Ty::Int | Ty::Num(_)) => match r.kind {
                TKind::Int(n) => match i32::try_from(n).ok().and_then(|n| d.pow(n, 1)) {
                    Some(nd) => (bin(Bin2::PowFI, l, r, pos), Ty::Num(nd)),
                    None => self.dim_overflow(pos),
                },
                TKind::Float(f) => {
                    let mut found = None;
                    for den in 1..=4i32 {
                        let x = f * den as f64;
                        if (x - x.round()).abs() < 1e-9 && x.abs() < 1000.0 {
                            found = Some((x.round() as i32, den));
                            break;
                        }
                    }
                    match found.and_then(|(n, den)| d.pow(n, den)) {
                        Some(nd) => (bin(Bin2::PowF, l, r, pos), Ty::Num(nd)),
                        None => {
                            self.err_hint(
                                pos,
                                "this exponent cannot be used on a value with units",
                                "use a whole number or a simple fraction such as 0.5",
                            );
                            self.bad(pos)
                        }
                    }
                }
                _ => {
                    let msg = format!("the exponent of a value with units ({}) must be a plain number", self.show(a));
                    self.err_hint(pos, msg, "write it as a literal, e.g. `x ^ 2`, or convert first with `x in m`");
                    self.bad(pos)
                }
            },
            _ => {
                let msg = format!("cannot raise {} to the power {}", self.show(a), self.show(b));
                self.err(pos, msg);
                self.bad(pos)
            }
        }
    }

    fn coalesce(&mut self, a: &Expr, b: &Expr, pos: Pos) -> (TExpr, Ty) {
        let (av, at) = self.expr(a, &Expect::Infer);
        let (variant, inner) = match self.resolve(&at) {
            Ty::Enum(E_OPTION, args) => (V_SOME, args[0].clone()),
            Ty::Enum(E_RESULT, args) => (V_OK, args[0].clone()),
            Ty::Error => {
                let _ = self.expr(b, &Expect::Infer);
                return self.bad(pos);
            }
            other => {
                let msg = format!("`??` needs an Option or Result on its left, found {}", self.show(&other));
                self.err_hint(pos, msg, "this value is never missing, so `??` is not needed");
                return self.bad(pos);
            }
        };
        let bv = self.expect_ty(b, &inner, "the default after `??`");
        (te(TKind::Coalesce(bx(av), bx(bv), variant), pos), inner)
    }

    fn convert(&mut self, inner: &Expr, u: &UnitExpr, pos: Pos) -> (TExpr, Ty) {
        let (v, vt) = self.expr(inner, &Expect::Infer);
        let (dim, num, den) = match self.unit_def(u, true) {
            Some(x) => x,
            None => return self.bad(pos),
        };
        match self.resolve(&vt) {
            Ty::Num(d) => {
                if d != dim {
                    let msg = format!("unit mismatch: cannot convert {} to {}", self.show(&vt), dim_name(&dim, &self.bases));
                    self.err(pos, msg);
                    return self.bad(pos);
                }
                (scale(v, den, num), Ty::float())
            }
            Ty::Int => {
                self.err_hint(pos, "this is a plain Int: it has no unit to convert", "give it a unit first, e.g. `5 m in mm`");
                self.bad(pos)
            }
            Ty::Error => self.bad(pos),
            other => {
                let msg = format!("`in` converts numbers with units, but this is {}", self.show(&other));
                self.err(pos, msg);
                self.bad(pos)
            }
        }
    }

    fn index(&mut self, obj: &Expr, idx: &Expr, pos: Pos) -> (TExpr, Ty) {
        let (o, ot) = self.expr(obj, &Expect::Infer);
        let before = self.assigns;
        match self.resolve(&ot) {
            Ty::List(el) => {
                let i = self.expect_ty(idx, &Ty::Int, "list index");
                let o = self.keep(o, before);
                (te(TKind::IndexList(bx(o), bx(i)), pos), *el)
            }
            Ty::Map(k, v) => {
                let i = self.expect_ty(idx, &k, "map key");
                let o = self.keep(o, before);
                (te(TKind::IndexMap(bx(o), bx(i)), pos), *v)
            }
            Ty::Str => {
                let i = self.expect_ty(idx, &Ty::Int, "text index");
                let o = self.keep(o, before);
                (te(TKind::IndexStr(bx(o), bx(i)), pos), Ty::Str)
            }
            Ty::Tuple(_) => {
                self.err_hint(pos, "tuples are not indexed with `[ ]`", "use `.0`, `.1`, ...");
                self.bad(pos)
            }
            Ty::Var(_) => {
                self.err_hint(pos, "the type of this value is not known yet", "add a type annotation");
                self.bad(pos)
            }
            Ty::Error => {
                let _ = self.expr(idx, &Expect::Infer);
                self.bad(pos)
            }
            other => {
                let msg = format!("cannot index into {}", self.show(&other));
                self.err(pos, msg);
                self.bad(pos)
            }
        }
    }

    fn try_expr(&mut self, inner: &Expr, pos: Pos) -> (TExpr, Ty) {
        let (v, vt) = self.expr(inner, &Expect::Infer);
        if matches!(self.cx().kind, CtxKind::Main | CtxKind::Test) {
            self.err_hint(pos, "`?` can only be used inside a function", "here use `?? default`, `match` or `.unwrap()`");
            return self.bad(pos);
        }
        let ret = self.cx().ret.clone();
        let (eid, variant, payload, word) = match self.resolve(&vt) {
            Ty::Enum(E_OPTION, args) => (E_OPTION, V_SOME, args[0].clone(), "an Option"),
            Ty::Enum(E_RESULT, args) => (E_RESULT, V_OK, args[0].clone(), "a Result"),
            Ty::Error => return self.bad(pos),
            other => {
                let msg = format!("`?` needs an Option or Result, found {}", self.show(&other));
                self.err(pos, msg);
                return self.bad(pos);
            }
        };
        let f = self.fresh();
        if !self.unify(&ret, &Ty::Enum(eid, vec![f])) {
            let msg = format!("`?` on {} needs a function that returns {}, but this one returns {}", word, word, self.show(&ret));
            self.err_hint(pos, msg, "change the return type, or handle the value with `match` or `??`");
            return self.bad(pos);
        }
        (te(TKind::Try(bx(v), variant), pos), payload)
    }

    // ---- calls

    fn call_name(&mut self, name: &str, recv: Option<&Expr>, args: &[Arg], pos: Pos, ex: &Expect) -> (TExpr, Ty) {
        if let Some(r) = recv {
            if let ExprKind::Ident(n) = &r.kind {
                let known = !matches!(self.lookup(n), Found::None)
                    || self.variant_names.contains_key(n)
                    || self.fn_names.contains_key(n)
                    || self.future_tops.contains_key(n)
                    || self.struct_names.contains_key(n);
                if !known {
                    return self.ident(n, r.pos);
                }
            }
        }
        if !self.struct_names.contains_key(name) {
            if let Some(a) = args.iter().find(|a| a.name.is_some()) {
                self.err_hint(a.value.pos, "named arguments are only for building a struct", "pass the values in order");
                return self.bad(pos);
            }
        }
        if recv.is_none() {
            match self.lookup(name) {
                Found::Local(slot, ty, _, _) => {
                    if matches!(self.resolve(&ty), Ty::Fn(_, _) | Ty::Var(_)) {
                        return self.call_closure(te(TKind::Local(slot), pos), ty, args, pos);
                    }
                }
                Found::Top(id) => {
                    let ty = self.tops[id as usize].ty.clone();
                    if matches!(self.resolve(&ty), Ty::Fn(_, _)) {
                        let (cv, ct) = self.top_ref(id, pos);
                        return self.call_closure(cv, ct, args, pos);
                    }
                }
                Found::None => {}
            }
        }
        let mut argv: Vec<&Expr> = Vec::with_capacity(args.len() + 1);
        if let Some(r) = recv {
            argv.push(r);
        }
        for a in args {
            argv.push(&a.value);
        }
        let has_user = self.fn_names.get(name).map_or(false, |cs| cs.iter().any(|c| c.rank == 2));
        if !has_user {
            if let Some(r) = self.special(name, &argv, pos) {
                return r;
            }
        }
        if let Some(&sid) = self.struct_names.get(name) {
            return self.new_struct(sid, name, recv, args, pos, ex);
        }
        if let Some(&vid) = self.variant_names.get(name) {
            return self.new_variant(vid, name, &argv, pos, ex);
        }
        self.call_fn(name, recv, &argv, pos, ex)
    }

    fn call_value(&mut self, callee: &Expr, args: &[Arg], pos: Pos) -> (TExpr, Ty) {
        if let Some(a) = args.iter().find(|a| a.name.is_some()) {
            self.err_hint(a.value.pos, "named arguments are only for building a struct", "pass the values in order");
            return self.bad(pos);
        }
        let (cv, ct) = self.expr(callee, &Expect::Infer);
        self.call_closure(cv, ct, args, pos)
    }

    fn call_closure(&mut self, cv: TExpr, ct: Ty, args: &[Arg], pos: Pos) -> (TExpr, Ty) {
        match self.resolve(&ct) {
            Ty::Fn(ptys, ret) => {
                if ptys.len() != args.len() {
                    let msg = format!("this function takes {}, found {}", count(ptys.len(), "argument"), args.len());
                    self.err(pos, msg);
                    return self.bad(pos);
                }
                let mut vals = Vec::with_capacity(args.len());
                for (i, a) in args.iter().enumerate() {
                    vals.push(self.expect_ty(&a.value, &ptys[i], &format!("argument {}", i + 1)));
                }
                (te(TKind::CallClosure(bx(cv), vals), pos), *ret)
            }
            Ty::Var(_) => {
                let mut vals = Vec::with_capacity(args.len());
                let mut tys = Vec::with_capacity(args.len());
                for a in args {
                    let (v, t) = self.expr(&a.value, &Expect::Infer);
                    vals.push(v);
                    tys.push(t);
                }
                let ret = self.fresh();
                self.unify(&ct, &Ty::Fn(tys, Box::new(ret.clone())));
                (te(TKind::CallClosure(bx(cv), vals), pos), ret)
            }
            Ty::Error => self.bad(pos),
            other => {
                let msg = format!("cannot call a value of type {}", self.show(&other));
                self.err(pos, msg);
                self.bad(pos)
            }
        }
    }

    /// The built-in forms that need more than an ordinary signature.
    fn special(&mut self, name: &str, argv: &[&Expr], pos: Pos) -> Option<(TExpr, Ty)> {
        let n = argv.len();
        let wrong = |c: &mut Checker, want: &str| -> Option<(TExpr, Ty)> {
            c.err(pos, format!("`{}` takes {}, found {}", name, want, n));
            Some(c.bad(pos))
        };
        match name {
            "print" => {
                let mut parts = Vec::new();
                for (i, a) in argv.iter().enumerate() {
                    if i > 0 {
                        parts.push(te(TKind::Str(" ".to_string()), pos));
                    }
                    let (v, t) = self.expr(a, &Expect::Infer);
                    let at = Shown { expr: *a, kind: ShowKind::Print, after: None };
                    self.display(v, &t, None, at, &mut parts);
                }
                let s = self.strcat(parts, pos);
                Some((te(TKind::CallBuiltin(B::Print as u16, vec![s]), pos), Ty::Unit))
            }
            "str" => {
                if n != 1 {
                    return wrong(self, "1 argument");
                }
                let (v, t) = self.expr(argv[0], &Expect::Infer);
                let mut parts = Vec::new();
                let at = Shown { expr: argv[0], kind: ShowKind::Str, after: None };
                self.display(v, &t, None, at, &mut parts);
                Some((self.strcat(parts, pos), Ty::Str))
            }
            "assert" => {
                if n != 1 && n != 2 {
                    return wrong(self, "1 or 2 arguments");
                }
                let c = self.expect_ty(argv[0], &Ty::Bool, "the condition of `assert`");
                let m = if n == 2 {
                    self.expect_ty(argv[1], &Ty::Str, "the message of `assert`")
                } else {
                    te(TKind::Str("assertion failed".to_string()), pos)
                };
                Some((te(TKind::CallBuiltin(B::Assert as u16, vec![c, m]), pos), Ty::Unit))
            }
            "assert_eq" => {
                if n != 2 {
                    return wrong(self, "2 arguments");
                }
                let (l, lt) = self.expr(argv[0], &Expect::Infer);
                let (r, rt) = self.expr(argv[1], &Expect::Has(lt.clone()));
                let (eq, ty) = self.bin_typed(BinOp::Eq, l, &lt, r, &rt, pos);
                match eq.kind {
                    TKind::Bin(_, a, b) => {
                        let mut args = vec![*a, *b];
                        args.extend(self.assert_eq_unit(&lt, argv));
                        Some((te(TKind::CallBuiltin(B::AssertEq as u16, args), pos), Ty::Unit))
                    }
                    other => Some((te(other, pos), ty)),
                }
            }
            "sqrt" => {
                if n != 1 {
                    return wrong(self, "1 argument");
                }
                let (v, t) = self.expr(argv[0], &Expect::Infer);
                let (v, d) = match self.resolve(&t) {
                    Ty::Int => (Self::to_float(v), Dim::none()),
                    Ty::Num(d) => (v, d),
                    Ty::Var(_) => {
                        self.unify(&t, &Ty::float());
                        (v, Dim::none())
                    }
                    Ty::Error => return Some(self.bad(pos)),
                    other => {
                        let msg = format!("`sqrt` needs a number, found {}", self.show(&other));
                        self.err(argv[0].pos, msg);
                        return Some(self.bad(pos));
                    }
                };
                match d.pow(1, 2) {
                    Some(rd) => Some((te(TKind::CallBuiltin(B::Sqrt as u16, vec![v]), pos), Ty::Num(rd))),
                    None => Some(self.dim_overflow(pos)),
                }
            }
            "sum" => {
                if n != 1 {
                    return wrong(self, "1 argument (a list)");
                }
                let (v, t) = self.expr(argv[0], &Expect::Infer);
                let el = match self.resolve(&t) {
                    Ty::List(el) => self.resolve(&el),
                    Ty::Error => return Some(self.bad(pos)),
                    other => {
                        let msg = format!("`sum` needs a list of numbers, found {}", self.show(&other));
                        self.err(argv[0].pos, msg);
                        return Some(self.bad(pos));
                    }
                };
                match el {
                    Ty::Int => Some((te(TKind::CallBuiltin(B::SumI as u16, vec![v]), pos), Ty::Int)),
                    Ty::Num(d) => Some((te(TKind::CallBuiltin(B::SumF as u16, vec![v]), pos), Ty::Num(d))),
                    Ty::Var(_) => {
                        self.unify(&el, &Ty::Int);
                        Some((te(TKind::CallBuiltin(B::SumI as u16, vec![v]), pos), Ty::Int))
                    }
                    Ty::Error => Some(self.bad(pos)),
                    other => {
                        let msg = format!("`sum` needs a list of numbers, found a list of {}", self.show(&other));
                        self.err(argv[0].pos, msg);
                        Some(self.bad(pos))
                    }
                }
            }
            "mean" | "median" | "stddev" => {
                // A list of Int or of Float in any unit; the answer is a Float in the same unit.
                if n != 1 {
                    return wrong(self, "1 argument (a list)");
                }
                let (v, t) = self.expr(argv[0], &Expect::Infer);
                let el = match self.resolve(&t) {
                    Ty::List(el) => self.resolve(&el),
                    Ty::Error => return Some(self.bad(pos)),
                    other => {
                        let msg = format!("`{}` needs a list of numbers, found {}", name, self.show(&other));
                        self.err(argv[0].pos, msg);
                        return Some(self.bad(pos));
                    }
                };
                let d = match el {
                    Ty::Int => Dim::none(),
                    Ty::Num(d) => d,
                    Ty::Var(_) => {
                        self.unify(&el, &Ty::float());
                        Dim::none()
                    }
                    Ty::Error => return Some(self.bad(pos)),
                    other => {
                        let msg = format!("`{}` needs a list of numbers, found a list of {}", name, self.show(&other));
                        self.err(argv[0].pos, msg);
                        return Some(self.bad(pos));
                    }
                };
                let id = match name {
                    "mean" => B::Mean,
                    "median" => B::Median,
                    _ => B::Stddev,
                };
                Some((te(TKind::CallBuiltin(id as u16, vec![v]), pos), Ty::Num(d)))
            }
            "panic" => {
                if n != 1 {
                    return wrong(self, "1 argument (the message)");
                }
                let m = self.expect_ty(argv[0], &Ty::Str, "the message of `panic`");
                Some((te(TKind::CallBuiltin(B::Panic as u16, vec![m]), pos), Ty::Never))
            }
            "host" => {
                if n != 2 {
                    return wrong(self, "2 arguments (name, payload)");
                }
                let a = self.expect_ty(argv[0], &Ty::Str, "the name in `host`");
                let b = self.expect_ty(argv[1], &Ty::Str, "the payload in `host`");
                Some((te(TKind::Host(bx(a), bx(b)), pos), Ty::Enum(E_RESULT, vec![Ty::Str])))
            }
            _ => None,
        }
    }

    fn new_struct(&mut self, sid: u32, name: &str, recv: Option<&Expr>, args: &[Arg], pos: Pos, ex: &Expect) -> (TExpr, Ty) {
        if recv.is_some() {
            let msg = format!("`{}` is a struct and cannot be called with a dot", name);
            self.err_hint(pos, msg, format!("build one with `{}(...)`", name));
            return self.bad(pos);
        }
        let ntp = self.structs[sid as usize].tparams.len();
        let vars = self.fresh_vars(ntp);
        let fields: Vec<(String, Ty)> = self.structs[sid as usize]
            .fields
            .iter()
            .map(|(n, t)| (n.clone(), Self::inst(t, &vars)))
            .collect();
        let sty = Ty::Struct(sid, vars);
        if let Expect::Has(t) = ex {
            let _ = self.unify(&sty, t);
        }
        let listing = fields
            .iter()
            .map(|(n, t)| format!("{}: {}", n, self.show(t)))
            .collect::<Vec<_>>()
            .join(", ");
        let named = args.iter().filter(|a| a.name.is_some()).count();
        if named != 0 && named != args.len() {
            self.err_hint(pos, "give a name to every field, or to none", format!("the fields are: {}", listing));
            return (te(TKind::Unit, pos), sty);
        }
        let mut slots: Vec<Option<TExpr>> = fields.iter().map(|_| None).collect();
        if named > 0 {
            for a in args {
                let fname = a.name.as_deref().unwrap_or("");
                match fields.iter().position(|(n, _)| n == fname) {
                    Some(i) => {
                        if slots[i].is_some() {
                            self.err(a.value.pos, format!("field `{}` is given twice", fname));
                        }
                        let v = self.expect_ty(&a.value, &fields[i].1, &format!("field `{}`", fname));
                        slots[i] = Some(v);
                    }
                    None => {
                        let msg = format!("`{}` has no field `{}`", name, fname);
                        match closest(fname, fields.iter().map(|(n, _)| n.as_str())) {
                            Some(c) => self.err_hint(a.value.pos, msg, format!("did you mean `{}`?", c)),
                            None => self.err_hint(a.value.pos, msg, format!("the fields are: {}", listing)),
                        }
                    }
                }
            }
            let missing: Vec<&str> = fields
                .iter()
                .zip(&slots)
                .filter(|(_, s)| s.is_none())
                .map(|((n, _), _)| n.as_str())
                .collect();
            if !missing.is_empty() && named == args.len() {
                let msg = format!("`{}` is missing: {}", name, missing.join(", "));
                self.err_hint(pos, msg, format!("the fields are: {}", listing));
                return (te(TKind::Unit, pos), sty);
            }
        } else {
            if args.len() != fields.len() {
                let msg = format!("`{}` has {}, found {}", name, count(fields.len(), "field"), count(args.len(), "value"));
                self.err_hint(pos, msg, format!("the fields are: {}", listing));
                return (te(TKind::Unit, pos), sty);
            }
            for (i, a) in args.iter().enumerate() {
                let v = self.expect_ty(&a.value, &fields[i].1, &format!("field `{}`", fields[i].0));
                slots[i] = Some(v);
            }
        }
        let vals: Vec<TExpr> = slots.into_iter().map(|s| s.unwrap_or(te(TKind::Unit, pos))).collect();
        (te(TKind::NewStruct(sid, vals), pos), sty)
    }

    fn new_variant(&mut self, vid: u32, name: &str, argv: &[&Expr], pos: Pos, ex: &Expect) -> (TExpr, Ty) {
        let (eid, ftys) = {
            let v = &self.variants[vid as usize];
            (v.enum_id, v.fields.clone())
        };
        let ntp = self.enums[eid as usize].tparams.len();
        let vars = self.fresh_vars(ntp);
        let ety = Ty::Enum(eid, vars.clone());
        if let Expect::Has(t) = ex {
            let _ = self.unify(&ety, t);
        }
        if argv.len() != ftys.len() {
            let msg = format!("`{}` holds {}, found {}", name, count(ftys.len(), "value"), argv.len());
            self.err(pos, msg);
            return (te(TKind::Unit, pos), ety);
        }
        let mut vals = Vec::with_capacity(argv.len());
        for (a, ft) in argv.iter().zip(&ftys) {
            let want = Self::inst(ft, &vars);
            vals.push(self.expect_ty(a, &want, &format!("the value in `{}`", name)));
        }
        if vals.is_empty() {
            (te(TKind::Tag(vid), pos), ety)
        } else {
            (te(TKind::NewEnum(vid, vals), pos), ety)
        }
    }

    fn call_fn(&mut self, name: &str, recv: Option<&Expr>, argv: &[&Expr], pos: Pos, ex: &Expect) -> (TExpr, Ty) {
        let cands: Vec<CandInfo> = self.fn_names.get(name).cloned().unwrap_or_default();
        if cands.is_empty() {
            return self.unknown_fn(name, recv, pos);
        }
        let n = argv.len();
        let fit: Vec<CandInfo> = cands.iter().copied().filter(|c| c.arity == n).collect();
        if fit.is_empty() {
            let off = if recv.is_some() { 1 } else { 0 };
            let mut arities: Vec<usize> = cands.iter().map(|c| c.arity.saturating_sub(off)).collect();
            arities.sort();
            arities.dedup();
            let list = arities.iter().map(|a| a.to_string()).collect::<Vec<_>>().join(" or ");
            let place = if recv.is_some() { " after the dot" } else { "" };
            self.err(pos, format!("`{}` takes {} argument(s){}, found {}", name, list, place, n - off));
            return self.bad(pos);
        }
        if fit.len() == 1 {
            return self.call_cand(fit[0].cand, name, argv, None, pos, ex);
        }
        if n == 0 {
            let best = fit.iter().copied().max_by_key(|c| c.rank).unwrap();
            return self.call_cand(best.cand, name, argv, None, pos, ex);
        }
        let (v0, t0) = self.expr(argv[0], &Expect::Infer);
        let r0 = self.resolve(&t0);
        match r0 {
            Ty::Error => return self.bad(pos),
            Ty::Var(_) => {
                let msg = format!("cannot tell which `{}` to call: the type of this value is not known yet", name);
                self.err_hint(argv[0].pos, msg, "add a type annotation");
                return self.bad(pos);
            }
            _ => {}
        }
        let h = head_sig(&r0);
        let mut best: Option<(u32, CandInfo)> = None;
        for c in &fit {
            let q = if c.head == h {
                3
            } else if c.head == Head::Any {
                2
            } else if h == Head::Int && c.head == Head::Num {
                1
            } else {
                0
            };
            if q == 0 {
                continue;
            }
            let score = q * 4 + c.rank as u32;
            if best.map_or(true, |(s, _)| score > s) {
                best = Some((score, *c));
            }
        }
        match best {
            Some((_, c)) => self.call_cand(c.cand, name, argv, Some((v0, t0)), pos, ex),
            None => {
                let msg = format!("`{}` does not work on {}", name, self.show(&r0));
                self.err(argv[0].pos, msg);
                self.bad(pos)
            }
        }
    }

    fn unknown_fn(&mut self, name: &str, recv: Option<&Expr>, pos: Pos) -> (TExpr, Ty) {
        if recv.is_none() && !matches!(self.lookup(name), Found::None) {
            self.err(pos, format!("`{}` is a variable, not a function", name));
            return self.bad(pos);
        }
        let msg = format!("unknown function `{}`", name);
        if let Some(a) = alias_hint(name) {
            self.err_hint(pos, msg, format!("did you mean `{}`?", a));
            return self.bad(pos);
        }
        if name == "pow" {
            self.err_hint(pos, msg, "use the `^` operator, e.g. `x ^ 2`");
            return self.bad(pos);
        }
        let near = closest(
            name,
            self.fn_names
                .keys()
                .map(|s| s.as_str())
                .chain(SPECIAL.iter().copied())
                .chain(self.struct_names.keys().map(|s| s.as_str()))
                .chain(self.variant_names.keys().map(|s| s.as_str())),
        );
        match near {
            Some(c) => self.err_hint(pos, msg, format!("did you mean `{}`?", c)),
            None => self.err(pos, msg),
        }
        self.bad(pos)
    }

    fn call_cand(
        &mut self,
        cand: Cand,
        name: &str,
        argv: &[&Expr],
        first: Option<(TExpr, Ty)>,
        pos: Pos,
        ex: &Expect,
    ) -> (TExpr, Ty) {
        let (ntp, bounds, params, ret) = match cand {
            Cand::Fn(fid) => {
                let ret = self.fn_ret(fid, pos);
                let f = &self.fns[fid as usize];
                (f.tparams.len(), f.bounds.clone(), f.params.clone(), ret)
            }
            Cand::Builtin(b) => {
                let s = &self.bsigs[b as usize];
                (s.ntp, s.bounds.clone(), s.params.clone(), s.ret.clone())
            }
        };
        let vars = self.fresh_vars(ntp);
        if let (Cand::Fn(fid), true) = (cand, ntp > 0) {
            self.note_generic_use(fid, &vars, pos, name, argv);
        }
        let ptys: Vec<Ty> = params.iter().map(|p| Self::inst(p, &vars)).collect();
        let rty = Self::inst(&ret, &vars);
        if let Expect::Has(t) = ex {
            let _ = self.unify(&rty, t);
        }
        let mut first = first;
        let mut out: Vec<TExpr> = Vec::with_capacity(argv.len());
        for (i, a) in argv.iter().enumerate() {
            let (v, vt) = match first.take() {
                Some(x) => x,
                None => self.expr(a, &Expect::Has(ptys[i].clone())),
            };
            if self.unify(&vt, &ptys[i]) {
                out.push(v);
                continue;
            }
            let (got, want) = (self.resolve(&vt), self.resolve(&ptys[i]));
            if got == Ty::Int && want.is_float() {
                out.push(Self::to_float(v));
                continue;
            }
            // `max(1, 2.5)`: the type parameter was fixed to Int by an earlier argument; widen it.
            if let (true, Ty::Int, Ty::Param(k)) = (got.is_float(), &want, &params[i]) {
                let k = *k;
                let bare = |p: &Ty| matches!(p, Ty::Param(j) if *j == k);
                let only_bare = params.iter().all(|p| bare(p) || !contains_param(p, k));
                if let (true, Some(Ty::Var(var))) = (only_bare, vars.get(k as usize)) {
                    if self.subst[*var as usize] == Some(Ty::Int) {
                        self.subst[*var as usize] = Some(Ty::float());
                        for j in 0..i {
                            if bare(&params[j]) {
                                let old = std::mem::replace(&mut out[j], te(TKind::Unit, pos));
                                out[j] = Self::to_float(old);
                            }
                        }
                        out.push(v);
                        continue;
                    }
                }
            }
            let what = format!("argument {} of `{}`", i + 1, name);
            out.push(self.coerce(v, &vt, &ptys[i], a.pos, &what));
        }
        for (k, bound) in bounds.iter().enumerate() {
            let bound = match bound {
                Some(b) => *b,
                None => continue,
            };
            let t = self.resolve(&vars[k]);
            let ok = match &t {
                Ty::Var(_) | Ty::Error | Ty::Never => true,
                // A type parameter of the enclosing function: its own bound must be at least as strict.
                Ty::Param(j) => match self.tbounds.get(*j as usize).copied().flatten() {
                    Some(have) => have == bound || bound == Bound::Ord,
                    None => false,
                },
                Ty::Int => true,
                Ty::Num(_) => bound != Bound::Key,
                Ty::Str => bound != Bound::Num,
                _ => false,
            };
            if !ok {
                let need = match bound {
                    Bound::Num => "numbers",
                    Bound::Ord => "numbers or text",
                    Bound::Key => "Int or Str",
                };
                let msg = format!("`{}` works on {}, found {}", name, need, self.show(&t));
                if let Ty::Param(_) = &t {
                    let word = match bound {
                        Bound::Num => "Num",
                        Bound::Ord => "Ord",
                        Bound::Key => "Key",
                    };
                    let hint = format!("give the type parameter a bound: `[{}: {}]`", self.show(&t), word);
                    self.err_hint(pos, msg, hint);
                } else {
                    self.err(pos, msg);
                }
            }
        }
        let kind = match cand {
            Cand::Fn(fid) => TKind::Call(fid, out),
            Cand::Builtin(b) => TKind::CallBuiltin(b, out),
        };
        (te(kind, pos), rty)
    }

    // ---- text

    /// Push the parts that show `v` as text (used by `print`, `str` and `"{...}"`).
    /// A number with a unit is shown only in a unit the program names (see `show.rs`).
    pub(crate) fn display(&mut self, v: TExpr, ty: &Ty, spec: Option<&str>, at: Shown, out: &mut Vec<TExpr>) {
        let pos = at.expr.pos;
        let (mut align, mut width, mut prec) = (0i64, -1i64, -1i64);
        let mut unit_text = String::new();
        if let Some(raw) = spec {
            let mut rest = raw.trim();
            if let Some(r) = rest.strip_prefix('<') {
                align = 1;
                rest = r;
            } else if let Some(r) = rest.strip_prefix('>') {
                align = 2;
                rest = r;
            }
            let mut digits = rest.chars().take_while(|c| c.is_ascii_digit()).count();
            // In `{f:1/s}` the `1` starts the unit; it is not a width.
            if digits > 0 && rest[digits..].starts_with('/') && rest[..digits].ends_with('1') {
                digits -= 1;
            }
            if digits > 0 {
                width = rest[..digits].parse::<i64>().unwrap_or(0).min(500);
                rest = &rest[digits..];
            }
            if let Some(dot) = rest.rfind('.') {
                let p = &rest[dot + 1..];
                if !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()) {
                    prec = p.parse::<i64>().unwrap_or(0).min(30);
                    rest = &rest[..dot];
                } else {
                    self.err_hint(pos, format!("cannot read the format `{}`", raw), FORMAT_HELP);
                    return;
                }
            }
            unit_text = rest.trim().to_string();
        }
        if !unit_text.is_empty() {
            let ue = match parse_unit_str(&unit_text, pos) {
                Ok(u) => u,
                Err(_) => {
                    self.err_hint(pos, format!("cannot read the format `{}`", spec.unwrap_or("")), FORMAT_HELP);
                    return;
                }
            };
            let (dim, num, den) = match self.unit_def(&ue, false) {
                Some(x) => x,
                None => return,
            };
            let v = match self.resolve(ty) {
                Ty::Num(d) if d == dim => v,
                Ty::Int if dim.is_none() => Self::to_float(v),
                // A lambda parameter whose type is not known yet: the unit named here decides it.
                Ty::Var(_) if self.unify(ty, &Ty::Num(dim.clone())) => v,
                Ty::Error => return,
                other => {
                    let msg = format!("unit mismatch: cannot show {} as {}", self.show(&other), unit_text);
                    let list_of_unit = matches!(&other, Ty::List(inner) if self.resolve(inner) == Ty::Num(dim.clone()));
                    if other == Ty::Int || other.is_float() {
                        let digits = if prec >= 0 { format!(":.{}", prec) } else { String::new() };
                        let hint = format!(
                            "this value is a plain number without a unit (`x in {u}` gives a plain number); \
                             keep the unit on the value by removing `in {u}` where it is made, \
                             or put the unit after the braces: `{{value{d}}} {u}`",
                            u = unit_text,
                            d = digits
                        );
                        self.err_hint(pos, msg, hint);
                    } else if list_of_unit {
                        let hint = format!(
                            "a list has no single number to show; show the numbers one by one in a loop, \
                             or for the plain numbers use `.map(|x| x in {})`",
                            unit_text
                        );
                        self.err_hint(pos, msg, hint);
                    } else {
                        self.err(pos, msg);
                    }
                    return;
                }
            };
            out.push(fmt_call(scale(v, den, num), width, prec, align, pos));
            out.push(te(TKind::Str(format!(" {}", unit_text)), pos));
            self.note_shown_unit(at.expr, &unit_text);
            if let (ShowKind::Interp, Some(after)) = (at.kind, at.after) {
                self.second_unit_after(&unit_text, after, pos);
            }
            return;
        }
        match self.resolve(ty) {
            Ty::Str if width < 0 && prec < 0 => out.push(v),
            Ty::Unit => self.err(pos, "this expression gives no value to show"),
            Ty::Error => {}
            _ => {
                self.check_shown(ty, spec, at);
                out.push(fmt_call(v, width, prec, align, pos));
            }
        }
    }

    /// `speed * 2 * pi / 60` where `speed` already has a unit: a speed written with `rpm`
    /// is stored as an angular speed, so the rpm to rad/s factor would be applied twice.
    fn rpm_twice(&mut self, e: &Expr) {
        let (mut over, mut under): (Vec<&Expr>, Vec<&Expr>) = (Vec::new(), Vec::new());
        let mut todo: Vec<(&Expr, bool)> = vec![(e, false)];
        while let Some((x, flipped)) = todo.pop() {
            match &x.kind {
                ExprKind::Binary(BinOp::Mul, l, r) => {
                    todo.push((l, flipped));
                    todo.push((r, flipped));
                }
                ExprKind::Binary(BinOp::Div, l, r) => {
                    todo.push((l, flipped));
                    todo.push((r, !flipped));
                }
                _ if flipped => under.push(x),
                _ => over.push(x),
            }
            if over.len() + under.len() > 24 {
                return;
            }
        }
        let time = Dim::base(2);
        let frequency = Dim::of(&[(2, -1)]);
        let is_number = |x: &Expr, n: f64| match &x.kind {
            ExprKind::Int(i) => *i as f64 == n,
            ExprKind::Float(f) => *f == n,
            _ => false,
        };
        for (side, other) in [(&over, &under), (&under, &over)] {
            let pi = side.iter().any(|x| matches!(&x.kind, ExprKind::Ident(s) if s == "pi"));
            let two = side.iter().any(|x| is_number(x, 2.0));
            let sixty = other.iter().any(|x| match &x.kind {
                ExprKind::UnitLit(v, u) => {
                    let plain_time = u.factors.len() == 1
                        && u.factors[0].1 == 1
                        && self.units.get(&u.factors[0].0).is_some_and(|d| d.dim == time);
                    *v == 60.0 && !plain_time
                }
                _ => is_number(x, 60.0),
            });
            let thirty = other.iter().any(|x| is_number(x, 30.0));
            if !(pi && ((two && sixty) || (!two && thirty))) {
                continue;
            }
            for x in side.iter() {
                let (ty, who) = match &x.kind {
                    ExprKind::Ident(name) => match self.lookup(name) {
                        Found::Local(_, ty, _, _) => (ty, format!("`{}`", name)),
                        Found::Top(id) => (self.tops[id as usize].ty.clone(), format!("`{}`", name)),
                        Found::None => continue,
                    },
                    ExprKind::UnitLit(_, u) if u.factors.len() == 1 && u.factors[0].1 == 1 => {
                        match self.units.get(&u.factors[0].0) {
                            Some(d) => (Ty::Num(d.dim.clone()), "this value".to_string()),
                            None => continue,
                        }
                    }
                    _ => continue,
                };
                if self.resolve(&ty) != Ty::Num(frequency.clone()) {
                    continue;
                }
                let factor = if two { "2 * pi / 60" } else { "pi / 30" };
                self.err_hint(
                    x.pos,
                    format!("`{}` turns a plain rpm number into rad/s, but {} already has a unit", factor, who),
                    "a speed written with `rpm` is stored as an angular speed, so use it as it is, \
                     for example `torque = power / speed`",
                );
                return;
            }
        }
    }

    /// Join text parts into one Str expression.
    pub(crate) fn strcat(&mut self, parts: Vec<TExpr>, pos: Pos) -> TExpr {
        let mut merged: Vec<TExpr> = Vec::with_capacity(parts.len());
        for p in parts {
            if let TKind::Str(s) = &p.kind {
                if s.is_empty() {
                    continue;
                }
                if let Some(TExpr {
                    kind: TKind::Str(prev), ..
                }) = merged.last_mut()
                {
                    prev.push_str(s);
                    continue;
                }
            }
            merged.push(p);
        }
        while merged.len() > 32 {
            let mut grouped: Vec<TExpr> = Vec::with_capacity(merged.len() / 32 + 1);
            let mut it = merged.into_iter().peekable();
            while it.peek().is_some() {
                let chunk: Vec<TExpr> = it.by_ref().take(32).collect();
                if chunk.len() == 1 {
                    grouped.extend(chunk);
                } else {
                    grouped.push(te(TKind::StrCat(chunk), pos));
                }
            }
            merged = grouped;
        }
        match merged.len() {
            0 => te(TKind::Str(String::new()), pos),
            1 => merged.pop().unwrap(),
            _ => te(TKind::StrCat(merged), pos),
        }
    }

    // ---- fields

    pub(crate) fn field_of(&mut self, obj_ty: &Ty, name: &str, pos: Pos) -> Option<(u32, Ty)> {
        let t = self.resolve(obj_ty);
        let is_fn = self.fn_names.contains_key(name) || SPECIAL.contains(&name);
        match &t {
            Ty::Struct(sid, targs) => {
                let s = &self.structs[*sid as usize];
                if let Some(i) = s.fields.iter().position(|(n, _)| n == name) {
                    return Some((i as u32, Self::inst(&s.fields[i].1, targs)));
                }
                let msg = format!("`{}` has no field `{}`", s.name, name);
                let near = closest(name, s.fields.iter().map(|(n, _)| n.as_str()));
                let listing = s.fields.iter().map(|(n, _)| n.as_str()).collect::<Vec<_>>().join(", ");
                match near {
                    Some(c) => self.err_hint(pos, msg, format!("did you mean `{}`?", c)),
                    None if is_fn => self.err_hint(pos, msg, format!("`{0}` is a function: write `x.{0}()`", name)),
                    None => self.err_hint(pos, msg, format!("the fields are: {}", listing)),
                }
                None
            }
            Ty::Error => None,
            Ty::Tuple(_) => {
                self.err_hint(pos, "tuples have no named fields", "use `.0`, `.1`, ...");
                None
            }
            Ty::Var(_) => {
                self.err_hint(pos, "the type of this value is not known yet", "add a type annotation");
                None
            }
            Ty::Map(_, _) => {
                let msg = format!("a Map has no field `{}`", name);
                self.err_hint(pos, msg, format!("read a key with `m[\"{0}\"]` or `m.get(\"{0}\")`", name));
                None
            }
            other => {
                let msg = format!("{} has no field `{}`", self.show(other), name);
                if is_fn {
                    self.err_hint(pos, msg, format!("`{0}` is a function: write `x.{0}()`", name));
                } else {
                    self.err(pos, msg);
                }
                None
            }
        }
    }

    // ---- lambdas, branches and patterns

    fn lambda(&mut self, params: &[(String, Option<TypeExpr>, Pos)], body: &Expr, ex: &Expect, pos: Pos) -> (TExpr, Ty) {
        let (want_params, want_ret): (Option<Vec<Ty>>, Option<Ty>) = match ex {
            Expect::Has(t) => match self.resolve(t) {
                Ty::Fn(ps, r) if ps.len() == params.len() => (Some(ps), Some(*r)),
                _ => (None, None),
            },
            _ => (None, None),
        };
        let fid = self.new_lambda(pos);
        let mut ptys: Vec<Ty> = Vec::with_capacity(params.len());
        let mut scope: Vec<Local> = Vec::with_capacity(params.len());
        for (i, (name, ann, ppos)) in params.iter().enumerate() {
            let ty = match (ann, &want_params) {
                (Some(t), _) => self.ty_expr(t),
                (None, Some(ps)) => ps[i].clone(),
                (None, None) => self.fresh(),
            };
            if name != "_" && scope.iter().any(|l| &l.name == name) {
                self.err(*ppos, format!("parameter `{}` is listed twice", name));
            }
            scope.push(Local {
                name: name.clone(),
                slot: i as u32,
                ty: ty.clone(),
                mutable: false,
                captured: false,
            });
            ptys.push(ty);
        }
        let ret = match want_ret {
            Some(r) => r,
            None => self.fresh(),
        };
        self.ctxs.push(FnCtx {
            kind: CtxKind::Lambda,
            scopes: vec![scope],
            nslots: params.len() as u32,
            ret: ret.clone(),
            loop_depth: 0,
            captures: Vec::new(),
        });
        let body_t = match self.resolve(&ret) {
            Ty::Unit => self.discard_expr(body).0,
            Ty::Var(_) => {
                let (b, bt) = self.expr(body, &Expect::Infer);
                self.coerce(b, &bt, &ret, body.pos, "lambda result")
            }
            _ => {
                let (b, bt) = self.expr(body, &Expect::Has(ret.clone()));
                self.coerce(b, &bt, &ret, body.pos, "lambda result")
            }
        };
        let ctx = self.ctxs.pop().unwrap();
        let (outer, inner): (Vec<u32>, Vec<u32>) = ctx.captures.iter().copied().unzip();
        let f = &mut self.fns[fid as usize];
        f.params = ptys.clone();
        f.ret = Some(ret.clone());
        f.state = 2;
        f.body = Some(body_t);
        f.nslots = ctx.nslots;
        f.captures = inner;
        (te(TKind::Lambda(fid, outer), pos), Ty::Fn(ptys, Box::new(ret)))
    }

    fn if_expr(&mut self, c: &Expr, then: &Block, els: Option<&Expr>, ex: &Expect, pos: Pos) -> (TExpr, Ty) {
        let cv = self.expect_ty(c, &Ty::Bool, "condition");
        let els = match els {
            Some(e) => e,
            None => {
                let (t, _) = self.block(then, &Expect::Discard);
                return (te(TKind::If(bx(cv), bx(t), None, true), pos), Ty::Unit);
            }
        };
        let (t, tt) = self.block(then, ex);
        let (e, et) = self.expr(els, ex);
        let then_pos = then.tail.as_ref().map(|x| x.pos).unwrap_or(then.pos);
        let (mut vals, ty, unit) = self.join(vec![(t, tt, then_pos), (e, et, els.pos)], ex, pos);
        let e = vals.pop().unwrap();
        let t = vals.pop().unwrap();
        (te(TKind::If(bx(cv), bx(t), Some(bx(e)), unit), pos), ty)
    }

    fn match_expr(&mut self, scrut: &Expr, arms: &[Arm], ex: &Expect, pos: Pos) -> (TExpr, Ty) {
        let (sv, st) = self.expr(scrut, &Expect::Infer);
        let discard = matches!(ex, Expect::Discard);
        let mut heads: Vec<(TPat, Option<TExpr>)> = Vec::with_capacity(arms.len());
        let mut branches: Vec<(TExpr, Ty, Pos)> = Vec::with_capacity(arms.len());
        for arm in arms {
            self.push_scope();
            self.note_pat(&arm.pat, None, Some(scrut));
            let pat = self.pattern(&arm.pat, &st);
            let guard = arm.guard.as_ref().map(|g| self.expect_ty(g, &Ty::Bool, "match guard"));
            let (b, bt) = if discard {
                self.discard_expr(&arm.body)
            } else {
                self.expr(&arm.body, ex)
            };
            self.pop_scope();
            heads.push((pat, guard));
            branches.push((b, bt, arm.body.pos));
        }
        self.exhaustive(arms, &st, pos);
        let (vals, ty, unit) = self.join(branches, ex, pos);
        let tarms: Vec<TArm> = heads
            .into_iter()
            .zip(vals)
            .map(|((pat, guard), body)| TArm { pat, guard, body })
            .collect();
        (te(TKind::Match(bx(sv), tarms, unit), pos), ty)
    }

    /// Give the branches of an `if` or `match` one type. The flag says the result is not a value.
    fn join(&mut self, branches: Vec<(TExpr, Ty, Pos)>, ex: &Expect, pos: Pos) -> (Vec<TExpr>, Ty, bool) {
        let tys: Vec<Ty> = branches.iter().map(|(_, t, _)| self.resolve(t)).collect();
        let all_never = !tys.is_empty() && tys.iter().all(|t| *t == Ty::Never);
        match ex {
            Expect::Discard => {
                let vals = branches.into_iter().map(|(v, _, _)| v).collect();
                (vals, if all_never { Ty::Never } else { Ty::Unit }, true)
            }
            Expect::Has(want) => {
                let mut vals = Vec::with_capacity(branches.len());
                for (v, t, p) in branches {
                    vals.push(self.coerce(v, &t, want, p, "this branch"));
                }
                let unit = matches!(self.resolve(want), Ty::Unit);
                (vals, want.clone(), unit)
            }
            Expect::Infer | Expect::Tail => {
                if all_never {
                    return (branches.into_iter().map(|(v, _, _)| v).collect(), Ty::Never, true);
                }
                if tys.iter().any(|t| *t == Ty::Unit) {
                    return (branches.into_iter().map(|(v, _, _)| v).collect(), Ty::Unit, true);
                }
                let live: Vec<&Ty> = tys.iter().filter(|t| !matches!(t, Ty::Never | Ty::Error)).collect();
                let numbers = !live.is_empty() && live.iter().all(|t| matches!(t, Ty::Int) || t.is_float());
                let mixed = numbers && live.iter().any(|t| t.is_float()) && live.iter().any(|t| matches!(t, Ty::Int));
                if mixed {
                    let vals = branches
                        .into_iter()
                        .zip(&tys)
                        .map(|((v, _, _), t)| if *t == Ty::Int { Self::to_float(v) } else { v })
                        .collect();
                    return (vals, Ty::float(), false);
                }
                let mut ty: Option<Ty> = None;
                let mut clash: Option<(Ty, Ty)> = None;
                for t in &tys {
                    if matches!(t, Ty::Never | Ty::Error) {
                        continue;
                    }
                    match &ty {
                        None => ty = Some(t.clone()),
                        Some(cur) => {
                            if !self.unify(cur, t) && clash.is_none() {
                                clash = Some((cur.clone(), t.clone()));
                            }
                        }
                    }
                }
                let vals: Vec<TExpr> = branches.into_iter().map(|(v, _, _)| v).collect();
                match clash {
                    None => (vals, ty.unwrap_or(Ty::Error), false),
                    Some(_) if matches!(ex, Expect::Tail) => (vals, Ty::Unit, true),
                    Some((x, y)) => {
                        let msg = format!("the branches give different types: {} and {}", self.show(&x), self.show(&y));
                        self.err_hint(pos, msg, "make every branch give the same type");
                        (vals, Ty::Error, false)
                    }
                }
            }
        }
    }

    fn pattern(&mut self, p: &Pat, ty: &Ty) -> TPat {
        match p {
            Pat::Wild(_) => TPat::Wild,
            Pat::Bind(name, pos) => {
                if let Some(&vid) = self.variant_names.get(name) {
                    return self.variant_pat(vid, name, &[], ty, *pos);
                }
                if name.chars().next().map_or(false, |c| c.is_uppercase()) {
                    let msg = format!("unknown variant `{}`", name);
                    match closest(name, self.variant_names.keys().map(|s| s.as_str())) {
                        Some(c) => self.err_hint(*pos, msg, format!("did you mean `{}`?", c)),
                        None => self.err_hint(*pos, msg, "a name that catches the value must start with a lower-case letter"),
                    }
                    return TPat::Wild;
                }
                match self.declare(name, ty.clone(), false, *pos) {
                    Decl::Local(slot) => TPat::Bind(slot),
                    Decl::Top(_) => TPat::Wild,
                }
            }
            Pat::Int(i, pos) => {
                self.literal_pat(ty, &Ty::Int, "a whole number", *pos);
                TPat::Int(*i)
            }
            Pat::Str(s, pos) => {
                self.literal_pat(ty, &Ty::Str, "text", *pos);
                TPat::Str(s.clone())
            }
            Pat::Bool(b, pos) => {
                self.literal_pat(ty, &Ty::Bool, "true/false", *pos);
                TPat::Bool(*b)
            }
            Pat::Variant(name, subs, pos) => match self.variant_names.get(name) {
                Some(&vid) => self.variant_pat(vid, name, subs, ty, *pos),
                None => {
                    let msg = format!("unknown variant `{}`", name);
                    match closest(name, self.variant_names.keys().map(|s| s.as_str())) {
                        Some(c) => self.err_hint(*pos, msg, format!("did you mean `{}`?", c)),
                        None => self.err(*pos, msg),
                    }
                    TPat::Wild
                }
            },
            Pat::Tuple(subs, pos) => {
                let elems = self.fresh_vars(subs.len());
                if !self.unify(ty, &Ty::Tuple(elems.clone())) {
                    let msg = format!("this pattern is a tuple of {}, but the value is {}", subs.len(), self.show(ty));
                    self.err(*pos, msg);
                    return TPat::Wild;
                }
                let mut out = Vec::with_capacity(subs.len());
                for (s, t) in subs.iter().zip(&elems) {
                    out.push(self.pattern(s, t));
                }
                TPat::Tuple(out)
            }
        }
    }

    fn literal_pat(&mut self, ty: &Ty, want: &Ty, word: &str, pos: Pos) {
        if !self.unify(ty, want) {
            let msg = format!("this pattern is {}, but the value is {}", word, self.show(ty));
            self.err(pos, msg);
        }
    }

    fn variant_pat(&mut self, vid: u32, name: &str, subs: &[Pat], ty: &Ty, pos: Pos) -> TPat {
        let (eid, ftys) = {
            let v = &self.variants[vid as usize];
            (v.enum_id, v.fields.clone())
        };
        let ntp = self.enums[eid as usize].tparams.len();
        let vars = self.fresh_vars(ntp);
        if !self.unify(ty, &Ty::Enum(eid, vars.clone())) {
            let ename = self.enums[eid as usize].name.clone();
            let msg = format!("`{}` belongs to {}, but the value is {}", name, ename, self.show(ty));
            self.err(pos, msg);
            return TPat::Wild;
        }
        if subs.len() != ftys.len() {
            let msg = format!("`{}` holds {}, but the pattern has {}", name, count(ftys.len(), "value"), subs.len());
            self.err(pos, msg);
            return TPat::Wild;
        }
        let mut out = Vec::with_capacity(subs.len());
        for (s, ft) in subs.iter().zip(&ftys) {
            let t = Self::inst(ft, &vars);
            out.push(self.pattern(s, &t));
        }
        TPat::Variant(vid, out)
    }

    fn irrefutable(&self, p: &Pat) -> bool {
        match p {
            Pat::Wild(_) => true,
            Pat::Bind(name, _) => !self.variant_names.contains_key(name),
            Pat::Tuple(subs, _) => subs.iter().all(|s| self.irrefutable(s)),
            _ => false,
        }
    }

    /// A conservative check that some arm always matches.
    fn exhaustive(&mut self, arms: &[Arm], st: &Ty, pos: Pos) {
        if arms.iter().any(|a| a.guard.is_none() && self.irrefutable(&a.pat)) {
            return;
        }
        let hint = "add the missing arms, or a final `_ => ...` arm";
        match self.resolve(st) {
            Ty::Enum(eid, _) => {
                let mut missing: Vec<String> = Vec::new();
                for &vid in &self.enums[eid as usize].variants {
                    let vname = &self.variants[vid as usize].name;
                    let covered = arms.iter().any(|a| {
                        a.guard.is_none()
                            && match &a.pat {
                                Pat::Variant(n, subs, _) => n == vname && subs.iter().all(|s| self.irrefutable(s)),
                                Pat::Bind(n, _) => n == vname,
                                _ => false,
                            }
                    });
                    if !covered {
                        missing.push(vname.clone());
                    }
                }
                if !missing.is_empty() {
                    self.err_hint(pos, format!("this `match` does not cover: {}", missing.join(", ")), hint);
                }
            }
            Ty::Bool => {
                let has = |want: bool| {
                    arms.iter()
                        .any(|a| a.guard.is_none() && matches!(&a.pat, Pat::Bool(b, _) if *b == want))
                };
                if !(has(true) && has(false)) {
                    self.err_hint(pos, "this `match` does not cover both `true` and `false`", hint);
                }
            }
            Ty::Error => {}
            _ => self.err_hint(pos, "this `match` may not cover every value", "add a final `_ => ...` arm"),
        }
    }
}
