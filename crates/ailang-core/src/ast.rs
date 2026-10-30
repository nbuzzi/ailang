/// AST nodes for the ailang pipeline language.
/// Every ailang expression compiles down to a Pipeline of Stages.

#[derive(Debug, Clone, PartialEq)]
pub struct Pipeline {
    pub stages: Vec<Stage>,
    /// Optional sink (> "file" or >> "file")
    pub sink: Option<Sink>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Stage {
    /// A function call: name(arg1, arg2, ...)
    Call(Call),
    /// An inline filter shorthand: ? expr
    Filter(Expr),
    /// A map/transform: map({k: expr, ...})
    Map(MapExpr),
    /// Field access shorthand: .field
    FieldAccess(String),
    /// Bare keyword: json, yaml, csv, lines, count, trim, sha256, md5, ...
    Keyword(String),
    /// Let binding: let x = pipeline
    Let(String, Box<Pipeline>),
    /// Variable reference: $x
    Var(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Call {
    pub name: String,
    pub args: Vec<Expr>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MapExpr {
    pub fields: Vec<(String, Expr)>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Sink {
    /// > "path"  (overwrite)
    Write(String),
    /// >> "path" (append)
    Append(String),
}

/// Expressions used inside arguments and map values
#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(String),
    /// Field path: .field or .nested.field
    Field(Vec<String>),
    /// Method call on field: .field.contains("x")
    FieldMethod(Vec<String>, String, Vec<Expr>),
    /// Binary operation: .age > 18
    BinOp(Box<Expr>, BinOp, Box<Expr>),
    /// Negation: !expr
    Not(Box<Expr>),
    /// Nested pipeline as argument
    Pipeline(Box<Pipeline>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum BinOp {
    Eq,   // ==
    Ne,   // !=
    Gt,   // >
    Ge,   // >=
    Lt,   // <
    Le,   // <=
    And,  // &&
    Or,   // ||
    Add,  // +
    Sub,  // -
    Mul,  // *
    Div,  // /
}
