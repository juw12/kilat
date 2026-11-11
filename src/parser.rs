//! Parser: tokens -> syntax tree. Recursive descent; the first syntax error aborts.

use crate::ast::*;
use crate::lexer::{lex, StrPart, Tok, Token};
use crate::types::{Diag, Pos};
use std::rc::Rc;

pub type PResult<T> = Result<T, Diag>;

const MAX_DEPTH: u32 = 120;
const MAX_CHAIN: u32 = 400;

pub struct Parser {
    toks: Vec<Token>,
    p: usize,
    file: u16,
    internal: bool,
    depth: u32,
}

fn symbol(t: &Tok) -> &'static str {
    match t {
        Tok::KwFn => "fn",
        Tok::KwLet => "let",
        Tok::KwVar => "var",
        Tok::KwIf => "if",
        Tok::KwElse => "else",
        Tok::KwWhile => "while",
        Tok::KwFor => "for",
        Tok::KwIn => "in",
        Tok::KwMatch => "match",
        Tok::KwReturn => "return",
        Tok::KwBreak => "break",
        Tok::KwContinue => "continue",
        Tok::KwStruct => "struct",
        Tok::KwEnum => "enum",
        Tok::KwTrue => "true",
        Tok::KwFalse => "false",
        Tok::KwAnd => "and",
        Tok::KwOr => "or",
        Tok::KwNot => "not",
        Tok::KwDiv => "div",
        Tok::LParen => "(",
        Tok::RParen => ")",
        Tok::LBrack => "[",
        Tok::RBrack => "]",
        Tok::LBrace => "{",
        Tok::RBrace => "}",
        Tok::Comma => ",",
        Tok::Colon => ":",
        Tok::Dot => ".",
        Tok::DotDot => "..",
        Tok::DotDotEq => "..=",
        Tok::Arrow => "->",
        Tok::FatArrow => "=>",
        Tok::Plus => "+",
        Tok::Minus => "-",
        Tok::Star => "*",
        Tok::Slash => "/",
        Tok::Percent => "%",
        Tok::Caret => "^",
        Tok::Assign => "=",
        Tok::EqEq => "==",
        Tok::NotEq => "!=",
        Tok::Lt => "<",
        Tok::Le => "<=",
        Tok::Gt => ">",
        Tok::Ge => ">=",
        Tok::PlusEq => "+=",
        Tok::MinusEq => "-=",
        Tok::StarEq => "*=",
        Tok::SlashEq => "/=",
        Tok::Bang => "!",
        Tok::Question => "?",
        Tok::QQ => "??",
        Tok::Pipe => "|",
        Tok::PipePipe => "||",
        _ => "?",
    }
}

fn describe(t: &Tok) -> String {
    match t {
        Tok::Int(_) | Tok::Float(_) => "a number".to_string(),
        Tok::Str(_) => "a string".to_string(),
        Tok::Ident(s) => format!("`{}`", s),
        Tok::Newline => "the end of the line".to_string(),
        Tok::Eof => "the end of the file".to_string(),
        other => format!("`{}`", symbol(other)),
    }
}

fn plain_string(t: &Tok) -> Option<String> {
    if let Tok::Str(parts) = t {
        let mut s = String::new();
        for part in parts {
            match part {
                StrPart::Lit(x) => s.push_str(x),
                StrPart::Code(_, _) => return None,
            }
        }
        Some(s)
    } else {
        None
    }
}

/// Split `expr:spec` at the first top-level `:` that is outside strings and brackets.
fn split_spec(code: &str) -> (String, Option<String>) {
    let chars: Vec<char> = code.chars().collect();
    let mut depth = 0i32;
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        match c {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth -= 1,
            '"' | '\'' => {
                i += 1;
                while i < chars.len() && chars[i] != c {
                    if chars[i] == '\\' {
                        i += 1;
                    }
                    i += 1;
                }
            }
            ':' if depth == 0 => {
                let expr: String = chars[..i].iter().collect();
                let spec: String = chars[i + 1..].iter().collect();
                return (expr, Some(spec));
            }
            _ => {}
        }
        i += 1;
    }
    (code.to_string(), None)
}

impl Parser {
    fn new(toks: Vec<Token>, file: u16, internal: bool, depth: u32) -> Parser {
        Parser {
            toks,
            p: 0,
            file,
            internal,
            depth,
        }
    }

    fn peek(&self) -> &Tok {
        &self.toks[self.p].tok
    }

    fn peek_at(&self, k: usize) -> &Tok {
        let i = (self.p + k).min(self.toks.len() - 1);
        &self.toks[i].tok
    }

    fn pos(&self) -> Pos {
        self.toks[self.p].pos
    }

    fn next(&mut self) -> Token {
        let t = self.toks[self.p].clone();
        if self.p + 1 < self.toks.len() {
            self.p += 1;
        }
        t
    }

    fn at(&self, t: &Tok) -> bool {
        self.peek() == t
    }

    fn at_eof(&self) -> bool {
        matches!(self.peek(), Tok::Eof)
    }

    fn eat(&mut self, t: &Tok) -> bool {
        if self.at(t) {
            self.next();
            true
        } else {
            false
        }
    }

    fn skip_nl(&mut self) {
        while matches!(self.peek(), Tok::Newline) {
            self.next();
        }
    }

    fn err<T>(&self, msg: String) -> PResult<T> {
        Err(Diag::new(self.pos(), msg))
    }

    fn expect(&mut self, t: &Tok, what: &str) -> PResult<Pos> {
        if self.at(t) {
            let pos = self.pos();
            self.next();
            Ok(pos)
        } else {
            self.err(format!("expected {}, found {}", what, describe(self.peek())))
        }
    }

    fn ident(&mut self, what: &str) -> PResult<(String, Pos)> {
        let pos = self.pos();
        match self.peek().clone() {
            Tok::Ident(s) => {
                self.next();
                Ok((s, pos))
            }
            other => {
                let d = Diag::new(pos, format!("expected {}, found {}", what, describe(&other)));
                let is_word = symbol(&other).chars().all(|c| c.is_alphabetic());
                if is_word && symbol(&other) != "?" {
                    Err(d.with_hint(format!("`{}` is a reserved word; pick another name", symbol(&other))))
                } else {
                    Err(d)
                }
            }
        }
    }

    fn enter(&mut self, pos: Pos) -> PResult<()> {
        self.depth += 1;
        if self.depth > MAX_DEPTH {
            Err(Diag::new(pos, "code is nested too deeply").with_hint("split it into smaller pieces"))
        } else {
            Ok(())
        }
    }

    fn leave(&mut self) {
        self.depth -= 1;
    }

    fn chain(&self, n: &mut u32) -> PResult<()> {
        *n += 1;
        if *n > MAX_CHAIN {
            Err(Diag::new(self.pos(), "expression is too long").with_hint("split it into several statements"))
        } else {
            Ok(())
        }
    }

    // ---------------------------------------------------------------- items

    fn item(&mut self) -> PResult<Item> {
        let pos = self.pos();
        let item = match self.peek().clone() {
            Tok::KwFn => Item::Fn(Rc::new(self.fn_decl()?)),
            Tok::KwStruct => Item::Struct(self.struct_decl()?),
            Tok::KwEnum => Item::Enum(self.enum_decl()?),
            Tok::Ident(s) if s == "unit" && matches!(self.peek_at(1), Tok::Ident(_)) => {
                Item::Unit(self.unit_decl()?)
            }
            Tok::Ident(s) if s == "import" && matches!(self.peek_at(1), Tok::Str(_)) => {
                self.next();
                let t = self.next();
                match plain_string(&t.tok) {
                    Some(name) => Item::Import(name, pos),
                    None => return Err(Diag::new(t.pos, "an import needs a plain file name")),
                }
            }
            Tok::Ident(s) if s == "test" && matches!(self.peek_at(1), Tok::Str(_)) => {
                self.next();
                let t = self.next();
                let name = match plain_string(&t.tok) {
                    Some(name) => name,
                    None => return Err(Diag::new(t.pos, "a test name cannot contain `{...}`")),
                };
                let body = self.block()?;
                Item::Test(name, Rc::new(body), pos)
            }
            _ => Item::Stmt(self.stmt()?),
        };
        self.end_stmt()?;
        Ok(item)
    }

    fn tparams(&mut self) -> PResult<Vec<String>> {
        let mut out = Vec::new();
        if self.eat(&Tok::LBrack) {
            while !self.at(&Tok::RBrack) {
                out.push(self.ident("a type parameter name")?.0);
                if !self.eat(&Tok::Comma) {
                    break;
                }
            }
            self.expect(&Tok::RBrack, "`]`")?;
        }
        Ok(out)
    }

    /// Type parameters of a function, where each may carry a bound: `[T, K: Ord]`.
    fn tparams_bounded(&mut self) -> PResult<(Vec<String>, Vec<Option<(String, Pos)>>)> {
        let mut names = Vec::new();
        let mut bounds = Vec::new();
        if self.eat(&Tok::LBrack) {
            while !self.at(&Tok::RBrack) {
                names.push(self.ident("a type parameter name")?.0);
                if self.eat(&Tok::Colon) {
                    let (bound, bpos) = self.ident("a bound (`Num`, `Ord` or `Key`)")?;
                    bounds.push(Some((bound, bpos)));
                } else {
                    bounds.push(None);
                }
                if !self.eat(&Tok::Comma) {
                    break;
                }
            }
            self.expect(&Tok::RBrack, "`]`")?;
        }
        Ok((names, bounds))
    }

    fn open_brace(&mut self) -> PResult<Pos> {
        if matches!(self.peek(), Tok::Newline) && matches!(self.peek_at(1), Tok::LBrace) {
            self.next();
        }
        if self.at(&Tok::Colon) {
            return Err(Diag::new(self.pos(), "expected `{`, found `:`")
                .with_hint("blocks use braces: `if x > 0 { ... }`"));
        }
        self.expect(&Tok::LBrace, "`{`")
    }

    fn fn_decl(&mut self) -> PResult<FnDecl> {
        let pos = self.pos();
        self.next();
        if self.at(&Tok::LParen) {
            return Err(Diag::new(pos, "a function needs a name")
                .with_hint("for an inline function write a lambda: `|x| x + 1`"));
        }
        let (name, _) = self.ident("a function name")?;
        let (tparams, tbounds) = self.tparams_bounded()?;
        self.expect(&Tok::LParen, "`(`")?;
        let mut params = Vec::new();
        while !self.at(&Tok::RParen) {
            let (pname, ppos) = self.ident("a parameter name")?;
            if !self.eat(&Tok::Colon) {
                return Err(Diag::new(ppos, format!("parameter `{}` needs a type", pname)).with_hint(
                    format!(
                        "write `{0}: Int`, `{0}: Float`, `{0}: Str`, or a unit such as `{0}: mm`",
                        pname
                    ),
                ));
            }
            let ty = self.type_()?;
            params.push((pname, ty, ppos));
            if !self.eat(&Tok::Comma) {
                break;
            }
        }
        self.expect(&Tok::RParen, "`)`")?;
        let ret = if self.eat(&Tok::Arrow) {
            Some(self.type_()?)
        } else {
            None
        };
        let body = self.block()?;
        Ok(FnDecl {
            name,
            tparams,
            tbounds,
            params,
            ret,
            body,
            pos,
        })
    }

    fn struct_decl(&mut self) -> PResult<StructDecl> {
        let pos = self.pos();
        self.next();
        let (name, _) = self.ident("a struct name")?;
        let tparams = self.tparams()?;
        self.open_brace()?;
        let mut fields = Vec::new();
        loop {
            self.skip_nl();
            if self.eat(&Tok::RBrace) {
                break;
            }
            let (fname, fpos) = self.ident("a field name")?;
            self.expect(&Tok::Colon, "`:` and the field type")?;
            let ty = self.type_()?;
            fields.push((fname, ty, fpos));
            if !self.eat(&Tok::Comma) && !matches!(self.peek(), Tok::Newline | Tok::RBrace) {
                return self.err(format!(
                    "expected `,` or a new line between fields, found {}",
                    describe(self.peek())
                ));
            }
        }
        Ok(StructDecl {
            name,
            tparams,
            fields,
            pos,
        })
    }

    fn enum_decl(&mut self) -> PResult<EnumDecl> {
        let pos = self.pos();
        self.next();
        let (name, _) = self.ident("an enum name")?;
        let tparams = self.tparams()?;
        self.open_brace()?;
        let mut variants = Vec::new();
        loop {
            self.skip_nl();
            if self.eat(&Tok::RBrace) {
                break;
            }
            let (vname, vpos) = self.ident("a variant name")?;
            let mut fields = Vec::new();
            if self.eat(&Tok::LParen) {
                while !self.at(&Tok::RParen) {
                    fields.push(self.type_()?);
                    if !self.eat(&Tok::Comma) {
                        break;
                    }
                }
                self.expect(&Tok::RParen, "`)`")?;
            }
            variants.push((vname, fields, vpos));
            if !self.eat(&Tok::Comma) && !matches!(self.peek(), Tok::Newline | Tok::RBrace) {
                return self.err(format!(
                    "expected `,` or a new line between variants, found {}",
                    describe(self.peek())
                ));
            }
        }
        Ok(EnumDecl {
            name,
            tparams,
            variants,
            pos,
        })
    }

    fn unit_decl(&mut self) -> PResult<UnitDecl> {
        let pos = self.pos();
        self.next();
        let (name, _) = self.ident("a unit name")?;
        let def = if self.eat(&Tok::Assign) {
            let t = self.next();
            let value = match t.tok {
                Tok::Int(v) => v as f64,
                Tok::Float(v) => v,
                other => {
                    return Err(Diag::new(
                        t.pos,
                        format!("expected a number, found {}", describe(&other)),
                    )
                    .with_hint("define a unit like `unit mile = 1609.344 m`"))
                }
            };
            let unit = self.unit_expr(true)?;
            Some((value, unit))
        } else {
            None
        };
        Ok(UnitDecl { name, def, pos })
    }

    // ----------------------------------------------------------- statements

    fn end_stmt(&mut self) -> PResult<()> {
        match self.peek() {
            Tok::Newline => {
                self.next();
                Ok(())
            }
            Tok::RBrace | Tok::Eof => Ok(()),
            other => {
                let d = Diag::new(
                    self.pos(),
                    format!("unexpected {} after the end of the statement", describe(other)),
                );
                let prev = if self.p > 0 {
                    Some(&self.toks[self.p - 1].tok)
                } else {
                    None
                };
                let hint = match (prev, other) {
                    (_, Tok::Colon) => "blocks use braces, not `:` - write `if x > 0 { ... }`",
                    (Some(Tok::Ident(w)), _) if w == "def" || w == "function" || w == "func" => {
                        "declare a function with `fn name(x: Int) -> Int { ... }`"
                    }
                    (Some(Tok::Ident(w)), _) if w == "elif" || w == "elsif" => "use `else if`",
                    (Some(Tok::Ident(w)), _) if w == "const" || w == "val" => {
                        "declare a variable with `let name = value` (or `var` if it changes)"
                    }
                    _ => "put each statement on its own line",
                };
                Err(d.with_hint(hint))
            }
        }
    }

    fn assign_op(&self) -> Option<Option<BinOp>> {
        match self.peek() {
            Tok::Assign => Some(None),
            Tok::PlusEq => Some(Some(BinOp::Add)),
            Tok::MinusEq => Some(Some(BinOp::Sub)),
            Tok::StarEq => Some(Some(BinOp::Mul)),
            Tok::SlashEq => Some(Some(BinOp::Div)),
            _ => None,
        }
    }

    fn expr_stmt(&mut self) -> PResult<Stmt> {
        let pos = self.pos();
        let e = self.expr()?;
        if let Some(op) = self.assign_op() {
            self.next();
            self.skip_nl();
            let value = self.expr()?;
            return Ok(Stmt::Assign {
                target: e,
                op,
                value,
                pos,
            });
        }
        Ok(Stmt::Expr(e))
    }

    /// A single statement used as an expression body (lambda body, match arm).
    fn stmt_as_expr(&mut self, s: Stmt, pos: Pos) -> Expr {
        match s {
            Stmt::Expr(e) => e,
            other => Expr {
                kind: ExprKind::Block(Block {
                    stmts: vec![other],
                    tail: None,
                    pos,
                }),
                pos,
            },
        }
    }

    fn cond_expr(&mut self) -> PResult<Expr> {
        let e = self.expr()?;
        if self.at(&Tok::Assign) {
            return Err(Diag::new(self.pos(), "`=` assigns a value; it cannot be used as a condition")
                .with_hint("use `==` to compare"));
        }
        Ok(e)
    }

    fn stmt(&mut self) -> PResult<Stmt> {
        let pos = self.pos();
        let s = match self.peek() {
            Tok::KwLet | Tok::KwVar => {
                let mutable = self.at(&Tok::KwVar);
                self.next();
                let pat = self.pattern()?;
                let ty = if self.eat(&Tok::Colon) {
                    Some(self.type_()?)
                } else {
                    None
                };
                if !self.eat(&Tok::Assign) {
                    return Err(Diag::new(
                        self.pos(),
                        format!("expected `=`, found {}", describe(self.peek())),
                    )
                    .with_hint("every variable needs a starting value: `let x = 0`"));
                }
                self.skip_nl();
                let init = self.expr()?;
                Stmt::Let {
                    pat,
                    ty,
                    init,
                    mutable,
                    pos,
                }
            }
            Tok::KwWhile => {
                self.next();
                let cond = self.cond_expr()?;
                let body = self.block()?;
                Stmt::While { cond, body, pos }
            }
            Tok::KwFor => {
                self.next();
                let pat = self.pattern()?;
                if !self.eat(&Tok::KwIn) {
                    return Err(Diag::new(
                        self.pos(),
                        format!("expected `in`, found {}", describe(self.peek())),
                    )
                    .with_hint("a loop looks like `for x in items { ... }` or `for i in 0..10 { ... }`"));
                }
                let iter = self.expr()?;
                let range_end = if self.eat(&Tok::DotDot) {
                    Some((self.expr()?, false))
                } else if self.eat(&Tok::DotDotEq) {
                    Some((self.expr()?, true))
                } else {
                    None
                };
                let body = self.block()?;
                Stmt::For {
                    pat,
                    iter,
                    range_end,
                    body,
                    pos,
                }
            }
            Tok::KwReturn => {
                self.next();
                if matches!(self.peek(), Tok::Newline | Tok::RBrace | Tok::Eof | Tok::Comma) {
                    Stmt::Return(None, pos)
                } else {
                    Stmt::Return(Some(self.expr()?), pos)
                }
            }
            Tok::KwBreak => {
                self.next();
                Stmt::Break(pos)
            }
            Tok::KwContinue => {
                self.next();
                Stmt::Continue(pos)
            }
            Tok::KwFn => {
                return Err(Diag::new(pos, "functions can only be declared at the top level")
                    .with_hint("move it out of this block, or use a lambda: `let f = |x| x + 1`"));
            }
            Tok::KwStruct | Tok::KwEnum => {
                return Err(Diag::new(pos, "types can only be declared at the top level"));
            }
            _ => self.expr_stmt()?,
        };
        Ok(s)
    }

    fn block(&mut self) -> PResult<Block> {
        let pos = self.open_brace()?;
        self.enter(pos)?;
        let mut stmts = Vec::new();
        loop {
            self.skip_nl();
            if self.at(&Tok::RBrace) {
                break;
            }
            if self.at_eof() {
                return Err(Diag::new(pos, "this `{` is never closed").with_hint("add a matching `}`"));
            }
            let s = self.stmt()?;
            self.end_stmt()?;
            stmts.push(s);
        }
        self.next();
        self.leave();
        let tail = if matches!(stmts.last(), Some(Stmt::Expr(_))) {
            match stmts.pop() {
                Some(Stmt::Expr(e)) => Some(Box::new(e)),
                _ => None,
            }
        } else {
            None
        };
        Ok(Block { stmts, tail, pos })
    }

    // ---------------------------------------------------------- expressions

    pub fn expr(&mut self) -> PResult<Expr> {
        self.coalesce()
    }

    fn coalesce(&mut self) -> PResult<Expr> {
        let left = self.or_expr()?;
        if self.at(&Tok::QQ) {
            let pos = self.pos();
            self.next();
            self.skip_nl();
            self.enter(pos)?;
            let right = self.coalesce()?;
            self.leave();
            return Ok(Expr {
                kind: ExprKind::Coalesce(Box::new(left), Box::new(right)),
                pos,
            });
        }
        Ok(left)
    }

    fn or_expr(&mut self) -> PResult<Expr> {
        let mut left = self.and_expr()?;
        let mut n = 0;
        while matches!(self.peek(), Tok::KwOr | Tok::PipePipe) {
            let pos = self.pos();
            self.chain(&mut n)?;
            self.next();
            self.skip_nl();
            let right = self.and_expr()?;
            left = Expr {
                kind: ExprKind::Or(Box::new(left), Box::new(right)),
                pos,
            };
        }
        Ok(left)
    }

    fn and_expr(&mut self) -> PResult<Expr> {
        let mut left = self.not_expr()?;
        let mut n = 0;
        while self.at(&Tok::KwAnd) {
            let pos = self.pos();
            self.chain(&mut n)?;
            self.next();
            self.skip_nl();
            let right = self.not_expr()?;
            left = Expr {
                kind: ExprKind::And(Box::new(left), Box::new(right)),
                pos,
            };
        }
        Ok(left)
    }

    fn not_expr(&mut self) -> PResult<Expr> {
        if self.at(&Tok::KwNot) {
            let pos = self.pos();
            self.next();
            self.enter(pos)?;
            let e = self.not_expr()?;
            self.leave();
            return Ok(Expr {
                kind: ExprKind::Not(Box::new(e)),
                pos,
            });
        }
        self.comparison()
    }

    fn cmp_op(&self) -> Option<BinOp> {
        match self.peek() {
            Tok::EqEq => Some(BinOp::Eq),
            Tok::NotEq => Some(BinOp::Ne),
            Tok::Lt => Some(BinOp::Lt),
            Tok::Le => Some(BinOp::Le),
            Tok::Gt => Some(BinOp::Gt),
            Tok::Ge => Some(BinOp::Ge),
            _ => None,
        }
    }

    fn comparison(&mut self) -> PResult<Expr> {
        let left = self.convert()?;
        let op = match self.cmp_op() {
            Some(op) => op,
            None => return Ok(left),
        };
        let pos = self.pos();
        self.next();
        self.skip_nl();
        let right = self.convert()?;
        if self.cmp_op().is_some() {
            return Err(Diag::new(self.pos(), "comparisons cannot be chained")
                .with_hint("write `a < b and b < c`"));
        }
        Ok(Expr {
            kind: ExprKind::Binary(op, Box::new(left), Box::new(right)),
            pos,
        })
    }

    fn convert(&mut self) -> PResult<Expr> {
        let mut left = self.additive()?;
        while self.at(&Tok::KwIn) {
            let pos = self.pos();
            self.next();
            if !matches!(self.peek(), Tok::Ident(_)) {
                return Err(Diag::new(
                    self.pos(),
                    format!("expected a unit after `in`, found {}", describe(self.peek())),
                )
                .with_hint("`x in mm` converts a quantity to a number; to test membership use `xs.contains(x)`"));
            }
            let unit = self.unit_expr(true)?;
            left = Expr {
                kind: ExprKind::Convert(Box::new(left), unit),
                pos,
            };
        }
        Ok(left)
    }

    fn additive(&mut self) -> PResult<Expr> {
        let mut left = self.multiplicative()?;
        let mut n = 0;
        loop {
            let op = match self.peek() {
                Tok::Plus => BinOp::Add,
                Tok::Minus => BinOp::Sub,
                _ => break,
            };
            let pos = self.pos();
            self.chain(&mut n)?;
            self.next();
            self.skip_nl();
            let right = self.multiplicative()?;
            left = Expr {
                kind: ExprKind::Binary(op, Box::new(left), Box::new(right)),
                pos,
            };
        }
        Ok(left)
    }

    fn multiplicative(&mut self) -> PResult<Expr> {
        let mut left = self.unary()?;
        let mut n = 0;
        loop {
            let op = match self.peek() {
                Tok::Star => BinOp::Mul,
                Tok::Slash => BinOp::Div,
                Tok::Percent => BinOp::Mod,
                Tok::KwDiv => BinOp::IDiv,
                _ => break,
            };
            let pos = self.pos();
            self.chain(&mut n)?;
            self.next();
            self.skip_nl();
            let right = self.unary()?;
            left = Expr {
                kind: ExprKind::Binary(op, Box::new(left), Box::new(right)),
                pos,
            };
        }
        Ok(left)
    }

    fn unary(&mut self) -> PResult<Expr> {
        let pos = self.pos();
        match self.peek() {
            Tok::Minus => {
                self.next();
                self.enter(pos)?;
                let e = self.unary()?;
                self.leave();
                Ok(Expr {
                    kind: ExprKind::Neg(Box::new(e)),
                    pos,
                })
            }
            Tok::Bang => {
                self.next();
                self.enter(pos)?;
                let e = self.unary()?;
                self.leave();
                Ok(Expr {
                    kind: ExprKind::Not(Box::new(e)),
                    pos,
                })
            }
            _ => self.power(),
        }
    }

    fn power(&mut self) -> PResult<Expr> {
        let base = self.postfix()?;
        if self.at(&Tok::Caret) {
            let pos = self.pos();
            self.next();
            self.skip_nl();
            self.enter(pos)?;
            let exp = self.unary()?;
            self.leave();
            return Ok(Expr {
                kind: ExprKind::Binary(BinOp::Pow, Box::new(base), Box::new(exp)),
                pos,
            });
        }
        Ok(base)
    }

    fn args(&mut self) -> PResult<Vec<Arg>> {
        let mut out = Vec::new();
        while !self.at(&Tok::RParen) {
            let named = matches!(self.peek(), Tok::Ident(_)) && matches!(self.peek_at(1), Tok::Colon);
            let name = if named {
                let (n, _) = self.ident("an argument name")?;
                self.next();
                Some(n)
            } else {
                None
            };
            let value = self.expr()?;
            out.push(Arg { name, value });
            if !self.eat(&Tok::Comma) {
                break;
            }
        }
        self.expect(&Tok::RParen, "`,` or `)`")?;
        Ok(out)
    }

    fn postfix(&mut self) -> PResult<Expr> {
        let mut e = self.primary()?;
        let mut n = 0;
        loop {
            let pos = self.pos();
            match self.peek() {
                Tok::LParen => {
                    self.chain(&mut n)?;
                    self.next();
                    let args = self.args()?;
                    e = Expr {
                        kind: ExprKind::Call(Box::new(e), args),
                        pos,
                    };
                }
                Tok::LBrack => {
                    self.chain(&mut n)?;
                    self.next();
                    let idx = self.expr()?;
                    self.expect(&Tok::RBrack, "`]`")?;
                    e = Expr {
                        kind: ExprKind::Index(Box::new(e), Box::new(idx)),
                        pos,
                    };
                }
                Tok::Dot => {
                    self.chain(&mut n)?;
                    self.next();
                    match self.peek().clone() {
                        Tok::Ident(name) => {
                            self.next();
                            if self.at(&Tok::LParen) {
                                self.next();
                                let args = self.args()?;
                                e = Expr {
                                    kind: ExprKind::Method(Box::new(e), name, args),
                                    pos,
                                };
                            } else {
                                e = Expr {
                                    kind: ExprKind::Field(Box::new(e), name),
                                    pos,
                                };
                            }
                        }
                        Tok::Int(i) => {
                            self.next();
                            e = Expr {
                                kind: ExprKind::TupleIndex(Box::new(e), i.clamp(0, 1_000_000) as u32),
                                pos,
                            };
                        }
                        other => {
                            return self.err(format!(
                                "expected a field or method name after `.`, found {}",
                                describe(&other)
                            ));
                        }
                    }
                }
                Tok::Question => {
                    self.chain(&mut n)?;
                    self.next();
                    e = Expr {
                        kind: ExprKind::Try(Box::new(e)),
                        pos,
                    };
                }
                Tok::Newline if matches!(self.peek_at(1), Tok::Dot) => {
                    self.next();
                }
                _ => break,
            }
        }
        Ok(e)
    }

    fn maybe_unit(&mut self, value: f64, lit: Expr, line: u32) -> PResult<Expr> {
        if matches!(self.peek(), Tok::Ident(_)) && self.toks[self.p].pos.line == line {
            let unit = self.unit_expr(true)?;
            return Ok(Expr {
                kind: ExprKind::UnitLit(value, unit),
                pos: lit.pos,
            });
        }
        Ok(lit)
    }

    fn string_expr(&mut self, parts: Vec<StrPart>, pos: Pos) -> PResult<Expr> {
        if parts.iter().all(|p| matches!(p, StrPart::Lit(_))) {
            let mut s = String::new();
            for part in parts {
                if let StrPart::Lit(x) = part {
                    s.push_str(&x);
                }
            }
            return Ok(Expr {
                kind: ExprKind::Str(s),
                pos,
            });
        }
        let mut out = Vec::new();
        for part in parts {
            match part {
                StrPart::Lit(s) => out.push(InterpPart::Lit(s)),
                StrPart::Code(code, cpos) => {
                    let (src, spec) = split_spec(&code);
                    if src.trim().is_empty() {
                        return Err(Diag::new(cpos, "empty `{}` in string")
                            .with_hint("put an expression inside, or write `{{` and `}}` for literal braces"));
                    }
                    let toks = lex(&src, self.file, cpos.line, cpos.col as u32, self.internal)?;
                    let mut sub = Parser::new(toks, self.file, self.internal, self.depth);
                    sub.enter(cpos)?;
                    sub.skip_nl();
                    let e = sub.expr()?;
                    sub.skip_nl();
                    if !sub.at_eof() {
                        return Err(Diag::new(
                            sub.pos(),
                            format!("unexpected {} inside `{{...}}`", describe(sub.peek())),
                        )
                        .with_hint("write `{{` and `}}` for literal braces"));
                    }
                    out.push(InterpPart::Expr(Box::new(e), spec));
                }
            }
        }
        Ok(Expr {
            kind: ExprKind::Interp(out),
            pos,
        })
    }

    fn lambda(&mut self, pos: Pos, has_params: bool) -> PResult<Expr> {
        let mut params = Vec::new();
        if has_params {
            while !self.at(&Tok::Pipe) {
                let (name, ppos) = self.ident("a parameter name")?;
                let ty = if self.eat(&Tok::Colon) {
                    Some(self.type_()?)
                } else {
                    None
                };
                params.push((name, ty, ppos));
                if !self.eat(&Tok::Comma) {
                    break;
                }
            }
            self.expect(&Tok::Pipe, "`|` to end the lambda parameters")?;
        }
        self.enter(pos)?;
        let bpos = self.pos();
        let s = self.expr_stmt()?;
        let body = self.stmt_as_expr(s, bpos);
        self.leave();
        Ok(Expr {
            kind: ExprKind::Lambda(params, Box::new(body)),
            pos,
        })
    }

    fn primary(&mut self) -> PResult<Expr> {
        let pos = self.pos();
        match self.peek() {
            Tok::LBrace => {
                let b = self.block()?;
                return Ok(Expr {
                    kind: ExprKind::Block(b),
                    pos,
                });
            }
            Tok::KwIf => return self.if_expr(),
            Tok::KwMatch => return self.match_expr(),
            _ => {}
        }
        let t = self.next();
        match t.tok {
            Tok::Int(v) => {
                let lit = Expr {
                    kind: ExprKind::Int(v),
                    pos,
                };
                self.maybe_unit(v as f64, lit, pos.line)
            }
            Tok::Float(v) => {
                let lit = Expr {
                    kind: ExprKind::Float(v),
                    pos,
                };
                self.maybe_unit(v, lit, pos.line)
            }
            Tok::Str(parts) => self.string_expr(parts, pos),
            Tok::KwTrue => Ok(Expr {
                kind: ExprKind::Bool(true),
                pos,
            }),
            Tok::KwFalse => Ok(Expr {
                kind: ExprKind::Bool(false),
                pos,
            }),
            Tok::Ident(name) => Ok(Expr {
                kind: ExprKind::Ident(name),
                pos,
            }),
            Tok::LParen => {
                self.enter(pos)?;
                if self.eat(&Tok::RParen) {
                    self.leave();
                    return Ok(Expr {
                        kind: ExprKind::Tuple(Vec::new()),
                        pos,
                    });
                }
                let first = self.expr()?;
                let e = if self.eat(&Tok::Comma) {
                    let mut items = vec![first];
                    while !self.at(&Tok::RParen) {
                        items.push(self.expr()?);
                        if !self.eat(&Tok::Comma) {
                            break;
                        }
                    }
                    self.expect(&Tok::RParen, "`)`")?;
                    if items.len() == 1 {
                        items.pop().unwrap()
                    } else {
                        Expr {
                            kind: ExprKind::Tuple(items),
                            pos,
                        }
                    }
                } else {
                    self.expect(&Tok::RParen, "`)`")?;
                    first
                };
                self.leave();
                Ok(e)
            }
            Tok::LBrack => {
                self.enter(pos)?;
                if self.eat(&Tok::RBrack) {
                    self.leave();
                    return Ok(Expr {
                        kind: ExprKind::List(Vec::new()),
                        pos,
                    });
                }
                if self.at(&Tok::Colon) && matches!(self.peek_at(1), Tok::RBrack) {
                    self.next();
                    self.next();
                    self.leave();
                    return Ok(Expr {
                        kind: ExprKind::Map(Vec::new()),
                        pos,
                    });
                }
                let first = self.expr()?;
                let e = if self.eat(&Tok::Colon) {
                    let value = self.expr()?;
                    let mut pairs = vec![(first, value)];
                    while self.eat(&Tok::Comma) {
                        if self.at(&Tok::RBrack) {
                            break;
                        }
                        let k = self.expr()?;
                        self.expect(&Tok::Colon, "`:` between a map key and its value")?;
                        let v = self.expr()?;
                        pairs.push((k, v));
                    }
                    self.expect(&Tok::RBrack, "`,` or `]`")?;
                    Expr {
                        kind: ExprKind::Map(pairs),
                        pos,
                    }
                } else {
                    let mut items = vec![first];
                    while self.eat(&Tok::Comma) {
                        if self.at(&Tok::RBrack) {
                            break;
                        }
                        items.push(self.expr()?);
                    }
                    self.expect(&Tok::RBrack, "`,` or `]`")?;
                    Expr {
                        kind: ExprKind::List(items),
                        pos,
                    }
                };
                self.leave();
                Ok(e)
            }
            Tok::Pipe => self.lambda(pos, true),
            Tok::PipePipe => self.lambda(pos, false),
            Tok::KwFn => Err(Diag::new(pos, "`fn` cannot be used inside an expression")
                .with_hint("for an inline function write a lambda: `|x| x + 1`")),
            other => Err(Diag::new(
                pos,
                format!("expected an expression, found {}", describe(&other)),
            )),
        }
    }

    fn else_part(&mut self) -> PResult<Option<Expr>> {
        let k = if matches!(self.peek(), Tok::Newline) { 1 } else { 0 };
        if !matches!(self.peek_at(k), Tok::KwElse) {
            return Ok(None);
        }
        self.p += k + 1;
        let pos = self.pos();
        if self.at(&Tok::KwIf) {
            Ok(Some(self.if_expr()?))
        } else {
            let b = self.block()?;
            Ok(Some(Expr {
                kind: ExprKind::Block(b),
                pos,
            }))
        }
    }

    fn if_expr(&mut self) -> PResult<Expr> {
        let pos = self.pos();
        self.next();
        self.enter(pos)?;
        let kind = if self.at(&Tok::KwLet) {
            self.next();
            let pat = self.pattern()?;
            self.expect(&Tok::Assign, "`=`")?;
            let scrut = self.expr()?;
            let then_b = self.block()?;
            let then_pos = then_b.pos;
            let else_e = match self.else_part()? {
                Some(e) => e,
                None => Expr {
                    kind: ExprKind::Block(Block {
                        stmts: Vec::new(),
                        tail: None,
                        pos,
                    }),
                    pos,
                },
            };
            let arms = vec![
                Arm {
                    pat,
                    guard: None,
                    body: Expr {
                        kind: ExprKind::Block(then_b),
                        pos: then_pos,
                    },
                    pos,
                },
                Arm {
                    pat: Pat::Wild(pos),
                    guard: None,
                    body: else_e,
                    pos,
                },
            ];
            ExprKind::Match(Box::new(scrut), arms)
        } else {
            let cond = self.cond_expr()?;
            let then_b = self.block()?;
            let else_e = self.else_part()?;
            ExprKind::If(Box::new(cond), then_b, else_e.map(Box::new))
        };
        self.leave();
        Ok(Expr { kind, pos })
    }

    fn match_expr(&mut self) -> PResult<Expr> {
        let pos = self.pos();
        self.next();
        self.enter(pos)?;
        let scrut = self.expr()?;
        self.open_brace()?;
        let mut arms = Vec::new();
        loop {
            self.skip_nl();
            if self.eat(&Tok::RBrace) {
                break;
            }
            if self.at_eof() {
                return Err(Diag::new(pos, "this `match` is never closed").with_hint("add a matching `}`"));
            }
            let apos = self.pos();
            let pat = self.pattern()?;
            let guard = if self.eat(&Tok::KwIf) {
                Some(self.expr()?)
            } else {
                None
            };
            if !self.eat(&Tok::FatArrow) {
                return Err(Diag::new(
                    self.pos(),
                    format!("expected `=>`, found {}", describe(self.peek())),
                )
                .with_hint("a match arm looks like `pattern => value`"));
            }
            self.skip_nl();
            let bpos = self.pos();
            let s = self.stmt()?;
            let body = self.stmt_as_expr(s, bpos);
            arms.push(Arm {
                pat,
                guard,
                body,
                pos: apos,
            });
            if self.eat(&Tok::Comma) {
                continue;
            }
            if !matches!(self.peek(), Tok::Newline | Tok::RBrace) {
                return self.err(format!(
                    "expected `,` or a new line after the match arm, found {}",
                    describe(self.peek())
                ));
            }
        }
        self.leave();
        Ok(Expr {
            kind: ExprKind::Match(Box::new(scrut), arms),
            pos,
        })
    }

    // ------------------------------------------------------------- patterns

    fn pattern(&mut self) -> PResult<Pat> {
        let pos = self.pos();
        match self.peek().clone() {
            Tok::Ident(name) => {
                self.next();
                if name == "_" {
                    Ok(Pat::Wild(pos))
                } else if self.at(&Tok::LParen) {
                    self.next();
                    self.enter(pos)?;
                    let mut subs = Vec::new();
                    while !self.at(&Tok::RParen) {
                        subs.push(self.pattern()?);
                        if !self.eat(&Tok::Comma) {
                            break;
                        }
                    }
                    self.expect(&Tok::RParen, "`)`")?;
                    self.leave();
                    Ok(Pat::Variant(name, subs, pos))
                } else {
                    Ok(Pat::Bind(name, pos))
                }
            }
            Tok::Int(v) => {
                self.next();
                Ok(Pat::Int(v, pos))
            }
            Tok::Minus => {
                self.next();
                match self.peek().clone() {
                    Tok::Int(v) => {
                        self.next();
                        Ok(Pat::Int(-v, pos))
                    }
                    other => self.err(format!("expected a number after `-`, found {}", describe(&other))),
                }
            }
            Tok::Float(_) => Err(Diag::new(pos, "cannot match on a Float")
                .with_hint("bind a name and use a guard: `x if x > 0.5 => ...`")),
            Tok::Str(_) => {
                let t = self.next();
                match plain_string(&t.tok) {
                    Some(s) => Ok(Pat::Str(s, pos)),
                    None => Err(Diag::new(pos, "a string pattern cannot contain `{...}`")),
                }
            }
            Tok::KwTrue => {
                self.next();
                Ok(Pat::Bool(true, pos))
            }
            Tok::KwFalse => {
                self.next();
                Ok(Pat::Bool(false, pos))
            }
            Tok::LParen => {
                self.next();
                self.enter(pos)?;
                let mut subs = Vec::new();
                while !self.at(&Tok::RParen) {
                    subs.push(self.pattern()?);
                    if !self.eat(&Tok::Comma) {
                        break;
                    }
                }
                self.expect(&Tok::RParen, "`)`")?;
                self.leave();
                if subs.len() == 1 {
                    Ok(subs.pop().unwrap())
                } else {
                    Ok(Pat::Tuple(subs, pos))
                }
            }
            other => self.err(format!("expected a name or pattern, found {}", describe(&other))),
        }
    }

    // ------------------------------------------------------- units and types

    fn unit_exp(&mut self, tight: bool) -> PResult<i32> {
        if !self.at(&Tok::Caret) {
            return Ok(1);
        }
        if tight && self.toks[self.p].space {
            return Ok(1);
        }
        let cpos = self.pos();
        self.next();
        let neg = self.eat(&Tok::Minus);
        match self.peek().clone() {
            Tok::Int(n) if n <= 12 => {
                self.next();
                Ok(if neg { -(n as i32) } else { n as i32 })
            }
            _ => Err(Diag::new(cpos, "a unit exponent must be a small whole number, e.g. `m^2`")
                .with_hint("to raise a quantity to a power, put spaces around `^`: `(5 m) ^ n`")),
        }
    }

    fn unit_expr(&mut self, tight: bool) -> PResult<UnitExpr> {
        let pos = self.pos();
        let mut factors = Vec::new();
        match self.peek().clone() {
            Tok::Ident(name) => {
                self.next();
                let e = self.unit_exp(tight)?;
                factors.push((name, e));
            }
            Tok::Int(1) if !tight => {
                self.next();
            }
            other => {
                return self.err(format!("expected a unit, found {}", describe(&other)));
            }
        }
        loop {
            let sign = match self.peek() {
                Tok::Star => 1,
                Tok::Slash => -1,
                _ => break,
            };
            let next_is_ident = matches!(self.peek_at(1), Tok::Ident(_));
            if !next_is_ident {
                break;
            }
            if tight {
                let glued = !self.toks[self.p].space && !self.toks[self.p + 1].space;
                if !glued {
                    break;
                }
            }
            self.next();
            let (name, _) = self.ident("a unit")?;
            let e = self.unit_exp(tight)?;
            factors.push((name, sign * e));
        }
        Ok(UnitExpr { factors, pos })
    }

    fn type_list(&mut self, close: &Tok) -> PResult<Vec<TypeExpr>> {
        let mut out = Vec::new();
        while !self.at(close) {
            out.push(self.type_()?);
            if !self.eat(&Tok::Comma) {
                break;
            }
        }
        Ok(out)
    }

    pub fn type_(&mut self) -> PResult<TypeExpr> {
        let pos = self.pos();
        self.enter(pos)?;
        let mut t = match self.peek().clone() {
            Tok::KwFn => {
                self.next();
                self.expect(&Tok::LParen, "`(`")?;
                let params = self.type_list(&Tok::RParen)?;
                self.expect(&Tok::RParen, "`)`")?;
                let ret = if self.eat(&Tok::Arrow) {
                    self.type_()?
                } else {
                    TypeExpr::Tuple(Vec::new(), pos)
                };
                TypeExpr::Fn(params, Box::new(ret), pos)
            }
            Tok::LParen => {
                self.next();
                let mut items = self.type_list(&Tok::RParen)?;
                self.expect(&Tok::RParen, "`)`")?;
                if items.len() == 1 {
                    items.pop().unwrap()
                } else {
                    TypeExpr::Tuple(items, pos)
                }
            }
            Tok::Ident(name) => {
                let is_unit = match self.peek_at(1) {
                    Tok::Caret => true,
                    Tok::Slash | Tok::Star => matches!(self.peek_at(2), Tok::Ident(_)),
                    _ => false,
                };
                if is_unit {
                    TypeExpr::Unit(self.unit_expr(false)?)
                } else {
                    self.next();
                    let args = if self.eat(&Tok::LBrack) {
                        let a = self.type_list(&Tok::RBrack)?;
                        self.expect(&Tok::RBrack, "`]`")?;
                        a
                    } else {
                        Vec::new()
                    };
                    TypeExpr::Name(name, args, pos)
                }
            }
            Tok::Int(1) if matches!(self.peek_at(1), Tok::Slash) => TypeExpr::Unit(self.unit_expr(false)?),
            other => {
                return self.err(format!("expected a type, found {}", describe(&other)));
            }
        };
        loop {
            if self.eat(&Tok::Question) {
                t = TypeExpr::Opt(Box::new(t), pos);
            } else if self.eat(&Tok::QQ) {
                t = TypeExpr::Opt(Box::new(TypeExpr::Opt(Box::new(t), pos)), pos);
            } else {
                break;
            }
        }
        self.leave();
        Ok(t)
    }
}

/// Parse a whole source file into items.
pub fn parse_file(src: &str, file: u16, internal: bool) -> PResult<Vec<Item>> {
    let toks = lex(src, file, 1, 1, internal)?;
    let mut p = Parser::new(toks, file, internal, 0);
    let mut items = Vec::new();
    loop {
        p.skip_nl();
        if p.at_eof() {
            break;
        }
        items.push(p.item()?);
    }
    Ok(items)
}

pub type Sig = (Vec<(String, Option<String>)>, Vec<TypeExpr>, TypeExpr);

/// Parse a builtin signature such as `[T: Ord](List[T]) -> List[T]`.
pub fn parse_sig(sig: &str) -> PResult<Sig> {
    let toks = lex(sig, 0, 1, 1, true)?;
    let mut p = Parser::new(toks, 0, true, 0);
    let mut tparams = Vec::new();
    if p.eat(&Tok::LBrack) {
        while !p.at(&Tok::RBrack) {
            let (name, _) = p.ident("a type parameter")?;
            let bound = if p.eat(&Tok::Colon) {
                Some(p.ident("a bound")?.0)
            } else {
                None
            };
            tparams.push((name, bound));
            if !p.eat(&Tok::Comma) {
                break;
            }
        }
        p.expect(&Tok::RBrack, "`]`")?;
    }
    p.expect(&Tok::LParen, "`(`")?;
    let params = p.type_list(&Tok::RParen)?;
    p.expect(&Tok::RParen, "`)`")?;
    let ret = if p.eat(&Tok::Arrow) {
        p.type_()?
    } else {
        TypeExpr::Tuple(Vec::new(), Pos::default())
    };
    Ok((tparams, params, ret))
}

/// Parse a unit written inside a format spec, e.g. `MPa` or `m/s^2`.
pub fn parse_unit_str(text: &str, pos: Pos) -> PResult<UnitExpr> {
    let toks = lex(text, pos.file, pos.line, pos.col as u32, false)?;
    let mut p = Parser::new(toks, pos.file, false, 0);
    let u = p.unit_expr(false)?;
    p.skip_nl();
    if !p.at_eof() {
        return p.err(format!("unexpected {} in the unit", describe(p.peek())));
    }
    Ok(u)
}
