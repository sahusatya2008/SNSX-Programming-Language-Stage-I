use serde::{Deserialize, Serialize};

use crate::diag::Span;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Program {
    pub module: Option<ModuleDecl>,
    pub uses: Vec<UseDecl>,
    pub effects: Vec<EffectDecl>,
    pub capabilities: Vec<CapabilityDecl>,
    pub types: Vec<TypeDecl>,
    pub functions: Vec<FunctionDecl>,
    pub actors: Vec<ActorDecl>,
}

impl Program {
    pub fn new() -> Self {
        Self {
            module: None,
            uses: Vec::new(),
            effects: Vec::new(),
            capabilities: Vec::new(),
            types: Vec::new(),
            functions: Vec::new(),
            actors: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ModuleDecl {
    pub path: Vec<String>,
    pub span: Span,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UseDecl {
    pub path: Vec<String>,
    pub alias: Option<String>,
    pub span: Span,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EffectDecl {
    pub name: String,
    pub span: Span,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CapabilityDecl {
    pub name: String,
    pub span: Span,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TypeDecl {
    pub name: String,
    pub generics: Vec<String>,
    pub variants: Vec<VariantDecl>,
    pub span: Span,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VariantDecl {
    pub name: String,
    pub fields: Vec<FieldDecl>,
    pub span: Span,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FieldDecl {
    pub name: String,
    pub ty: TypeExpr,
    pub span: Span,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FunctionDecl {
    pub name: String,
    pub generics: Vec<String>,
    pub params: Vec<Param>,
    pub return_type: TypeExpr,
    pub effect: String,
    pub where_clause: Option<Expr>,
    pub body: Block,
    pub span: Span,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Param {
    pub name: String,
    pub ty: TypeExpr,
    pub refinement: Option<Expr>,
    pub span: Span,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ActorDecl {
    pub name: String,
    pub params: Vec<Param>,
    pub handlers: Vec<ActorHandler>,
    pub span: Span,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ActorHandler {
    pub pattern: Pattern,
    pub body: Block,
    pub span: Span,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Block {
    pub statements: Vec<Stmt>,
    pub tail: Option<Box<Expr>>,
    pub span: Span,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Stmt {
    Let {
        name: String,
        value: Option<Expr>,
        span: Span,
    },
    Mut {
        name: String,
        value: Expr,
        span: Span,
    },
    Require {
        condition: Expr,
        message: Option<String>,
        span: Span,
    },
    Return {
        value: Expr,
        span: Span,
    },
    Expr {
        value: Expr,
        span: Span,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Expr {
    Int(i64),
    Bool(bool),
    Text(String),
    Variable(String),
    Call {
        callee: Box<Expr>,
        args: Vec<Expr>,
    },
    Binary {
        op: BinaryOp,
        left: Box<Expr>,
        right: Box<Expr>,
    },
    Unary {
        op: UnaryOp,
        value: Box<Expr>,
    },
    If {
        condition: Box<Expr>,
        then_branch: Box<Block>,
        else_branch: Box<Block>,
    },
    Match {
        scrutinee: Box<Expr>,
        arms: Vec<MatchArm>,
    },
    Lambda {
        params: Vec<String>,
        body: Box<Expr>,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MatchArm {
    pub pattern: Pattern,
    pub body: Block,
    pub span: Span,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Pattern {
    Wildcard,
    Identifier(String),
    Int(i64),
    Bool(bool),
    Text(String),
    Variant { name: String, bindings: Vec<String> },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    And,
    Or,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum UnaryOp {
    Neg,
    Not,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum TypeExpr {
    Named { name: String, args: Vec<TypeExpr> },
}

impl TypeExpr {
    pub fn simple(name: impl Into<String>) -> Self {
        Self::Named {
            name: name.into(),
            args: Vec::new(),
        }
    }

    pub fn name(&self) -> &str {
        match self {
            Self::Named { name, .. } => name,
        }
    }
}
