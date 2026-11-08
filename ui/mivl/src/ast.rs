use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Null,
    Bool(bool),
    Number(f64),
    /// CSS-like dimension, resolved against the containing box at evaluation time.
    Length(f64, String),
    String(String),
    List(Vec<Expr>),
    Path(Vec<String>),
    Unary(Operator, Box<Expr>),
    Binary(Box<Expr>, Operator, Box<Expr>),
    Conditional(Box<Expr>, Box<Expr>, Box<Expr>),
    Call(String, Vec<Expr>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Operator {
    Add, Sub, Mul, Div, Rem, Eq, Ne, Lt, Le, Gt, Ge, And, Or, Not,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Property {
    pub name: String,
    pub declared_type: Option<String>,
    pub expression: Expr,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Statement {
    /// An ordinary property mutation, whose RHS may invoke a Rust command.
    Assign { target: Vec<String>, operator: Operator, expression: Expr },
    /// An event-only expression, e.g. `invoke("save", text)`.
    Call(Expr),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Node {
    pub kind: String,
    pub properties: Vec<Property>,
    pub handlers: BTreeMap<String, Vec<Statement>>,
    pub children: Vec<Node>,
    /// Repeated component template for model-backed ListView/GridView.
    pub delegate: Option<Box<Node>>,
}
