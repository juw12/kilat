//! Syntax tree produced by the parser.

use crate::types::Pos;
use std::rc::Rc;

#[derive(Clone, Debug)]
pub struct UnitExpr {
    /// (unit name, integer exponent), e.g. `m/s^2` is [("m", 1), ("s", -2)].
    pub factors: Vec<(String, i32)>,
    pub pos: Pos,
}

#[derive(Clone, Debug)]
pub enum TypeExpr {
    Name(String, Vec<TypeExpr>, Pos),
    Unit(UnitExpr),
    Tuple(Vec<TypeExpr>, Pos),
    Fn(Vec<TypeExpr>, Box<TypeExpr>, Pos),
    Opt(Box<TypeExpr>, Pos),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    IDiv,
    Pow,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

#[derive(Clone, Debug)]
pub struct Expr {
    pub kind: ExprKind,
    pub pos: Pos,
}

#[derive(Clone, Debug)]
pub struct Arg {
    pub name: Option<String>,
    pub value: Expr,
}

#[derive(Clone, Debug)]
pub enum InterpPart {
    Lit(String),
    /// Expression and optional format spec (the text after `:`).
    Expr(Box<Expr>, Option<String>),
}

#[derive(Clone, Debug)]
pub enum ExprKind {
    Int(i64),
    Float(f64),
    Bool(bool),
    Str(String),
    Interp(Vec<InterpPart>),
    UnitLit(f64, UnitExpr),
    Ident(String),
    List(Vec<Expr>),
    Map(Vec<(Expr, Expr)>),
    Tuple(Vec<Expr>),
    Neg(Box<Expr>),
    Not(Box<Expr>),
    Binary(BinOp, Box<Expr>, Box<Expr>),
    And(Box<Expr>, Box<Expr>),
    Or(Box<Expr>, Box<Expr>),
    Coalesce(Box<Expr>, Box<Expr>),
    Convert(Box<Expr>, UnitExpr),
    Call(Box<Expr>, Vec<Arg>),
    Method(Box<Expr>, String, Vec<Arg>),
    Field(Box<Expr>, String),
    TupleIndex(Box<Expr>, u32),
    Index(Box<Expr>, Box<Expr>),
    Try(Box<Expr>),
    Lambda(Vec<(String, Option<TypeExpr>, Pos)>, Box<Expr>),
    If(Box<Expr>, Block, Option<Box<Expr>>),
    Match(Box<Expr>, Vec<Arm>),
    Block(Block),
}

#[derive(Clone, Debug)]
pub struct Block {
    pub stmts: Vec<Stmt>,
    pub tail: Option<Box<Expr>>,
    pub pos: Pos,
}

#[derive(Clone, Debug)]
pub struct Arm {
    pub pat: Pat,
    pub guard: Option<Expr>,
    pub body: Expr,
    pub pos: Pos,
}

#[derive(Clone, Debug)]
pub enum Pat {
    Wild(Pos),
    Bind(String, Pos),
    Int(i64, Pos),
    Str(String, Pos),
    Bool(bool, Pos),
    Variant(String, Vec<Pat>, Pos),
    Tuple(Vec<Pat>, Pos),
}

impl Pat {
    pub fn pos(&self) -> Pos {
        match self {
            Pat::Wild(p)
            | Pat::Bind(_, p)
            | Pat::Int(_, p)
            | Pat::Str(_, p)
            | Pat::Bool(_, p)
            | Pat::Variant(_, _, p)
            | Pat::Tuple(_, p) => *p,
        }
    }
}

#[derive(Clone, Debug)]
pub enum Stmt {
    Let {
        pat: Pat,
        ty: Option<TypeExpr>,
        init: Expr,
        mutable: bool,
        pos: Pos,
    },
    Assign {
        target: Expr,
        op: Option<BinOp>,
        value: Expr,
        pos: Pos,
    },
    Expr(Expr),
    While {
        cond: Expr,
        body: Block,
        pos: Pos,
    },
    For {
        pat: Pat,
        iter: Expr,
        /// `Some((end, inclusive))` for `a..b` / `a..=b` loops.
        range_end: Option<(Expr, bool)>,
        body: Block,
        pos: Pos,
    },
    Return(Option<Expr>, Pos),
    Break(Pos),
    Continue(Pos),
}

#[derive(Clone, Debug)]
pub struct FnDecl {
    pub name: String,
    pub tparams: Vec<String>,
    /// One entry per type parameter: the bound name (`Num`, `Ord`, `Key`) and where it was written.
    pub tbounds: Vec<Option<(String, Pos)>>,
    pub params: Vec<(String, TypeExpr, Pos)>,
    pub ret: Option<TypeExpr>,
    pub body: Block,
    pub pos: Pos,
}

#[derive(Clone, Debug)]
pub struct StructDecl {
    pub name: String,
    pub tparams: Vec<String>,
    pub fields: Vec<(String, TypeExpr, Pos)>,
    pub pos: Pos,
}

#[derive(Clone, Debug)]
pub struct EnumDecl {
    pub name: String,
    pub tparams: Vec<String>,
    pub variants: Vec<(String, Vec<TypeExpr>, Pos)>,
    pub pos: Pos,
}

#[derive(Clone, Debug)]
pub struct UnitDecl {
    pub name: String,
    /// `Some((value, unit))` for derived units like `unit mile = 1609.344 m`.
    pub def: Option<(f64, UnitExpr)>,
    pub pos: Pos,
}

#[derive(Clone, Debug)]
pub enum Item {
    Fn(Rc<FnDecl>),
    Struct(StructDecl),
    Enum(EnumDecl),
    Unit(UnitDecl),
    Import(String, Pos),
    Test(String, Rc<Block>, Pos),
    Stmt(Stmt),
}
