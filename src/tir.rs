//! Typed intermediate representation produced by the checker.
//! Every type-directed decision (Int vs Float operators, which builtin, which
//! overload) is already made here, so the compiler never needs types.

use crate::types::Pos;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Bin2 {
    AddI,
    SubI,
    MulI,
    DivI,
    ModI,
    PowI,
    AddF,
    SubF,
    MulF,
    DivF,
    ModF,
    PowF,
    PowFI,
    EqI,
    NeI,
    LtI,
    LeI,
    GtI,
    GeI,
    EqF,
    NeF,
    LtF,
    LeF,
    GtF,
    GeF,
    EqV,
    NeV,
    LtS,
    LeS,
    GtS,
    GeS,
    Concat,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Un1 {
    NegI,
    NegF,
    Not,
    IntToFloat,
}

#[derive(Clone, Debug)]
pub struct TExpr {
    pub kind: TKind,
    pub pos: Pos,
}

#[derive(Clone, Debug)]
pub enum TKind {
    Unit,
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(String),
    Local(u32),
    Top(u32),
    FnRef(u32),
    Tag(u32),
    Bin(Bin2, Box<TExpr>, Box<TExpr>),
    Un(Un1, Box<TExpr>),
    /// Force a private copy of a variable read (it is reassigned later in the same expression).
    Copy(Box<TExpr>),
    And(Box<TExpr>, Box<TExpr>),
    Or(Box<TExpr>, Box<TExpr>),
    Call(u32, Vec<TExpr>),
    CallClosure(Box<TExpr>, Vec<TExpr>),
    CallBuiltin(u16, Vec<TExpr>),
    Host(Box<TExpr>, Box<TExpr>),
    NewList(Vec<TExpr>),
    NewMap(Vec<(TExpr, TExpr)>),
    NewTuple(Vec<TExpr>),
    NewStruct(u32, Vec<TExpr>),
    NewEnum(u32, Vec<TExpr>),
    Field(Box<TExpr>, u32),
    IndexList(Box<TExpr>, Box<TExpr>),
    IndexMap(Box<TExpr>, Box<TExpr>),
    IndexStr(Box<TExpr>, Box<TExpr>),
    /// Function id and the enclosing-function slots whose values are captured.
    Lambda(u32, Vec<u32>),
    /// Condition, then, else, and whether the whole expression is unit-valued.
    If(Box<TExpr>, Box<TExpr>, Option<Box<TExpr>>, bool),
    Match(Box<TExpr>, Vec<TArm>, bool),
    Seq(Vec<TExpr>, Option<Box<TExpr>>),
    StrCat(Vec<TExpr>),
    /// `e?`: return early unless `e` is the given value-carrying variant.
    Try(Box<TExpr>, u32),
    /// `a ?? b`: payload of `a` if it is the given variant, else `b`.
    Coalesce(Box<TExpr>, Box<TExpr>, u32),
    Let(u32, Box<TExpr>),
    LetTop(u32, Box<TExpr>),
    SetLocal(u32, Box<TExpr>),
    SetTop(u32, Box<TExpr>),
    SetIndexList(Box<TExpr>, Box<TExpr>, Box<TExpr>),
    SetIndexMap(Box<TExpr>, Box<TExpr>, Box<TExpr>),
    SetField(Box<TExpr>, u32, Box<TExpr>),
    While(Box<TExpr>, Box<TExpr>),
    ForRange {
        slot: u32,
        start: Box<TExpr>,
        end: Box<TExpr>,
        inclusive: bool,
        body: Box<TExpr>,
    },
    ForList {
        slot: u32,
        list: Box<TExpr>,
        body: Box<TExpr>,
    },
    Return(Option<Box<TExpr>>),
    Break,
    Continue,
}

#[derive(Clone, Debug)]
pub struct TArm {
    pub pat: TPat,
    pub guard: Option<TExpr>,
    pub body: TExpr,
}

#[derive(Clone, Debug)]
pub enum TPat {
    Wild,
    Bind(u32),
    Int(i64),
    Bool(bool),
    Str(String),
    Variant(u32, Vec<TPat>),
    Tuple(Vec<TPat>),
}

#[derive(Clone, Debug)]
pub struct TFn {
    pub name: String,
    pub nparams: u32,
    /// Slots (inside this function) that receive the captured values, in capture order.
    pub captures: Vec<u32>,
    pub nslots: u32,
    pub body: TExpr,
    pub ret_unit: bool,
    pub pos: Pos,
}

#[derive(Clone, Debug)]
pub struct Checked {
    pub fns: Vec<TFn>,
    pub structs: Vec<(String, Vec<String>)>,
    pub variants: Vec<(String, u32)>,
    /// (name, is it read or written outside the main body)
    pub tops: Vec<(String, bool)>,
    pub main: u32,
    pub tests: Vec<(String, u32)>,
}
