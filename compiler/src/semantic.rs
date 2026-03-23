use crate::ast::*;
use crate::diag::Diagnostic;
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Type {
    Int,
    Float,
    Bool,
    String,
    Bytes,
    Unit,
    Tensor,
    Array(Box<Type>),
    Task(Box<Type>),
    Function(Vec<Type>, Box<Type>),
    Unknown,
    Named(String),
}

impl Type {
    pub fn is_copy(&self) -> bool {
        matches!(self, Type::Int | Type::Float | Type::Bool | Type::Unit)
    }
}

#[derive(Debug, Clone)]
pub struct FunctionSignature {
    pub params: Vec<Type>,
    pub ret: Type,
    pub is_async: bool,
    pub is_ai: bool,
    pub prompt: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SemanticModel {
    pub functions: HashMap<String, FunctionSignature>,
}

pub fn analyze_module(_path: &str, module: &Module) -> Result<SemanticModel, Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();
    let mut functions = builtin_functions();

    for item in &module.items {
        match item {
            Item::Function(function) => {
                if functions.contains_key(&function.name) {
                    diagnostics.push(Diagnostic::error(
                        format!("duplicate function '{}'", function.name),
                        Some(function.span),
                    ));
                    continue;
                }
                let params = function
                    .params
                    .iter()
                    .map(|param| param.ty.as_ref().map(resolve_type).unwrap_or(Type::Unknown))
                    .collect::<Vec<_>>();
                let ret = function
                    .return_type
                    .as_ref()
                    .map(resolve_type)
                    .unwrap_or(Type::Unit);
                functions.insert(
                    function.name.clone(),
                    FunctionSignature {
                        params,
                        ret,
                        is_async: function.is_async,
                        is_ai: function.is_ai,
                        prompt: function.prompt.clone(),
                    },
                );
            }
            Item::Actor(actor) => {
                for method in &actor.methods {
                    let fq = format!("{}::{}", actor.name, method.name);
                    if functions.contains_key(&fq) {
                        diagnostics.push(Diagnostic::error(
                            format!("duplicate actor method '{}'", fq),
                            Some(method.span),
                        ));
                        continue;
                    }
                    let params = method
                        .params
                        .iter()
                        .map(|param| param.ty.as_ref().map(resolve_type).unwrap_or(Type::Unknown))
                        .collect::<Vec<_>>();
                    let ret = method
                        .return_type
                        .as_ref()
                        .map(resolve_type)
                        .unwrap_or(Type::Unit);
                    functions.insert(
                        fq,
                        FunctionSignature {
                            params,
                            ret,
                            is_async: method.is_async,
                            is_ai: method.is_ai,
                            prompt: method.prompt.clone(),
                        },
                    );
                }
            }
        }
    }

    for item in &module.items {
        if let Item::Function(function) = item {
            analyze_function(function, &functions, &mut diagnostics);
        }
    }

    if diagnostics.is_empty() {
        Ok(SemanticModel { functions })
    } else {
        Err(diagnostics)
    }
}

fn analyze_function(
    function: &Function,
    functions: &HashMap<String, FunctionSignature>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut locals = HashMap::new();
    for param in &function.params {
        locals.insert(
            param.name.clone(),
            param.ty.as_ref().map(resolve_type).unwrap_or(Type::Unknown),
        );
    }
    let expected_return = function
        .return_type
        .as_ref()
        .map(resolve_type)
        .unwrap_or(Type::Unit);
    for stmt in &function.body {
        analyze_stmt(stmt, functions, &mut locals, &expected_return, diagnostics);
    }
}

fn analyze_stmt(
    stmt: &Stmt,
    functions: &HashMap<String, FunctionSignature>,
    locals: &mut HashMap<String, Type>,
    expected_return: &Type,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match stmt {
        Stmt::Let { name, ty, expr, .. } => {
            let expr_ty = analyze_expr(expr, functions, locals, diagnostics);
            if let Some(declared) = ty.as_ref().map(resolve_type) {
                if !type_compatible(&declared, &expr_ty) {
                    diagnostics.push(Diagnostic::error(
                        format!(
                            "type mismatch for '{}': expected {:?}, found {:?}",
                            name, declared, expr_ty
                        ),
                        Some(expr.span()),
                    ));
                }
                locals.insert(name.clone(), declared);
            } else {
                locals.insert(name.clone(), expr_ty);
            }
        }
        Stmt::Assign { target, expr, span } => {
            let expr_ty = analyze_expr(expr, functions, locals, diagnostics);
            let Some(existing) = locals.get(target).cloned() else {
                diagnostics.push(Diagnostic::error(
                    format!("assignment to unknown variable '{}'", target),
                    Some(*span),
                ));
                return;
            };
            if !type_compatible(&existing, &expr_ty) {
                diagnostics.push(Diagnostic::error(
                    format!("cannot assign {:?} to {:?}", expr_ty, existing),
                    Some(expr.span()),
                ));
            }
        }
        Stmt::Expr(expr) => {
            let _ = analyze_expr(expr, functions, locals, diagnostics);
        }
        Stmt::Return { expr, span } => {
            let actual = expr
                .as_ref()
                .map(|expr| analyze_expr(expr, functions, locals, diagnostics))
                .unwrap_or(Type::Unit);
            if !type_compatible(expected_return, &actual) {
                diagnostics.push(Diagnostic::error(
                    format!(
                        "return type mismatch: expected {:?}, found {:?}",
                        expected_return, actual
                    ),
                    Some(*span),
                ));
            }
        }
        Stmt::If {
            condition,
            then_branch,
            else_branch,
            ..
        } => {
            let condition_ty = analyze_expr(condition, functions, locals, diagnostics);
            if condition_ty != Type::Bool && condition_ty != Type::Unknown {
                diagnostics.push(Diagnostic::error(
                    "if condition must be Bool",
                    Some(condition.span()),
                ));
            }
            let mut then_locals = locals.clone();
            for stmt in then_branch {
                analyze_stmt(
                    stmt,
                    functions,
                    &mut then_locals,
                    expected_return,
                    diagnostics,
                );
            }
            let mut else_locals = locals.clone();
            for stmt in else_branch {
                analyze_stmt(
                    stmt,
                    functions,
                    &mut else_locals,
                    expected_return,
                    diagnostics,
                );
            }
        }
        Stmt::While {
            condition, body, ..
        } => {
            let condition_ty = analyze_expr(condition, functions, locals, diagnostics);
            if condition_ty != Type::Bool && condition_ty != Type::Unknown {
                diagnostics.push(Diagnostic::error(
                    "while condition must be Bool",
                    Some(condition.span()),
                ));
            }
            let mut inner = locals.clone();
            for stmt in body {
                analyze_stmt(stmt, functions, &mut inner, expected_return, diagnostics);
            }
        }
        Stmt::Match { expr, arms, .. } => {
            let subject_ty = analyze_expr(expr, functions, locals, diagnostics);
            for arm in arms {
                validate_pattern(&arm.pattern, &subject_ty, diagnostics);
                let mut local_copy = locals.clone();
                for stmt in &arm.body {
                    analyze_stmt(
                        stmt,
                        functions,
                        &mut local_copy,
                        expected_return,
                        diagnostics,
                    );
                }
            }
        }
    }
}

fn analyze_expr(
    expr: &Expr,
    functions: &HashMap<String, FunctionSignature>,
    locals: &HashMap<String, Type>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Type {
    match expr {
        Expr::Int(_, _) => Type::Int,
        Expr::Float(_, _) => Type::Float,
        Expr::Bool(_, _) => Type::Bool,
        Expr::String(_, _) => Type::String,
        Expr::Ident(name, span) => locals
            .get(name)
            .cloned()
            .or_else(|| {
                functions
                    .get(name)
                    .map(|sig| Type::Function(sig.params.clone(), Box::new(sig.ret.clone())))
            })
            .unwrap_or_else(|| {
                diagnostics.push(Diagnostic::error(
                    format!("unknown identifier '{}'", name),
                    Some(*span),
                ));
                Type::Unknown
            }),
        Expr::Array(items, _) => {
            let item_ty = items
                .iter()
                .map(|item| analyze_expr(item, functions, locals, diagnostics))
                .fold(Type::Unknown, unify_types);
            Type::Array(Box::new(item_ty))
        }
        Expr::Unary { op, expr, span } => {
            let inner = analyze_expr(expr, functions, locals, diagnostics);
            match op {
                UnaryOp::Neg => inner,
                UnaryOp::Await => match inner {
                    Type::Task(item) => *item,
                    Type::Unknown => Type::Unknown,
                    other => {
                        diagnostics.push(Diagnostic::error(
                            format!("await expects Task, found {:?}", other),
                            Some(*span),
                        ));
                        Type::Unknown
                    }
                },
                UnaryOp::Spawn | UnaryOp::Parallel => match &**expr {
                    Expr::Call { callee, .. } => {
                        let callee_ty = analyze_expr(callee, functions, locals, diagnostics);
                        match callee_ty {
                            Type::Function(_, ret) => Type::Task(ret),
                            _ => Type::Task(Box::new(Type::Unknown)),
                        }
                    }
                    _ => Type::Task(Box::new(Type::Unknown)),
                },
                UnaryOp::Move => inner,
                UnaryOp::Not => Type::Bool,
            }
        }
        Expr::Binary {
            op,
            left,
            right,
            span,
        } => {
            let left_ty = analyze_expr(left, functions, locals, diagnostics);
            let right_ty = analyze_expr(right, functions, locals, diagnostics);
            match op {
                BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Mod => {
                    if !type_compatible(&left_ty, &right_ty) {
                        diagnostics.push(Diagnostic::error(
                            format!("binary operands mismatch: {:?} vs {:?}", left_ty, right_ty),
                            Some(*span),
                        ));
                    }
                    left_ty
                }
                BinaryOp::Eq
                | BinaryOp::Ne
                | BinaryOp::Lt
                | BinaryOp::Le
                | BinaryOp::Gt
                | BinaryOp::Ge => Type::Bool,
            }
        }
        Expr::Call { callee, args, span } => {
            if let Expr::Ident(name, _) = &**callee {
                if name == "watch" && !locals.contains_key(name) {
                    if args.len() != 2 {
                        diagnostics.push(Diagnostic::error(
                            format!("call arity mismatch: expected 2, found {}", args.len()),
                            Some(*span),
                        ));
                        return Type::Unknown;
                    }
                    let label_ty = analyze_expr(&args[0], functions, locals, diagnostics);
                    if !type_compatible(&Type::String, &label_ty) {
                        diagnostics.push(Diagnostic::error(
                            format!(
                                "call argument mismatch: expected {:?}, found {:?}",
                                Type::String,
                                label_ty
                            ),
                            Some(args[0].span()),
                        ));
                    }
                    let value_ty = analyze_expr(&args[1], functions, locals, diagnostics);
                    return value_ty;
                }
                if name == "repair" && !locals.contains_key(name) {
                    if args.len() != 2 {
                        diagnostics.push(Diagnostic::error(
                            format!("call arity mismatch: expected 2, found {}", args.len()),
                            Some(*span),
                        ));
                        return Type::Unknown;
                    }
                    let primary_ty = analyze_expr(&args[0], functions, locals, diagnostics);
                    let fallback_ty = analyze_expr(&args[1], functions, locals, diagnostics);
                    return unify_types(primary_ty, fallback_ty);
                }
            }
            let callee_ty = analyze_expr(callee, functions, locals, diagnostics);
            match callee_ty {
                Type::Function(params, ret) => {
                    if params.len() != args.len() && !params.is_empty() {
                        diagnostics.push(Diagnostic::error(
                            format!(
                                "call arity mismatch: expected {}, found {}",
                                params.len(),
                                args.len()
                            ),
                            Some(*span),
                        ));
                    }
                    for (param_ty, arg) in params.iter().zip(args) {
                        let arg_ty = analyze_expr(arg, functions, locals, diagnostics);
                        if !type_compatible(param_ty, &arg_ty) {
                            diagnostics.push(Diagnostic::error(
                                format!(
                                    "call argument mismatch: expected {:?}, found {:?}",
                                    param_ty, arg_ty
                                ),
                                Some(arg.span()),
                            ));
                        }
                    }
                    *ret
                }
                other => {
                    diagnostics.push(Diagnostic::error(
                        format!("cannot call value of type {:?}", other),
                        Some(*span),
                    ));
                    Type::Unknown
                }
            }
        }
        Expr::Member { object, field, .. } => {
            let object_ty = analyze_expr(object, functions, locals, diagnostics);
            match object_ty {
                Type::Named(base) => Type::Named(format!("{base}.{field}")),
                _ => Type::Unknown,
            }
        }
        Expr::Index { object, .. } => match analyze_expr(object, functions, locals, diagnostics) {
            Type::Array(item) => *item,
            other => {
                diagnostics.push(Diagnostic::error(
                    format!("cannot index {:?}", other),
                    Some(object.span()),
                ));
                Type::Unknown
            }
        },
    }
}

fn builtin_functions() -> HashMap<String, FunctionSignature> {
    HashMap::from([
        (
            "print".to_string(),
            FunctionSignature {
                params: vec![Type::Unknown],
                ret: Type::Unit,
                is_async: false,
                is_ai: false,
                prompt: None,
            },
        ),
        (
            "show".to_string(),
            FunctionSignature {
                params: vec![Type::Unknown],
                ret: Type::Unit,
                is_async: false,
                is_ai: false,
                prompt: None,
            },
        ),
        (
            "read_line".to_string(),
            FunctionSignature {
                params: vec![],
                ret: Type::String,
                is_async: false,
                is_ai: false,
                prompt: None,
            },
        ),
        (
            "ask".to_string(),
            FunctionSignature {
                params: vec![],
                ret: Type::String,
                is_async: false,
                is_ai: false,
                prompt: None,
            },
        ),
        (
            "input".to_string(),
            FunctionSignature {
                params: vec![],
                ret: Type::String,
                is_async: false,
                is_ai: false,
                prompt: None,
            },
        ),
        (
            "read_stdin".to_string(),
            FunctionSignature {
                params: vec![],
                ret: Type::String,
                is_async: false,
                is_ai: false,
                prompt: None,
            },
        ),
        (
            "read_text".to_string(),
            FunctionSignature {
                params: vec![Type::String],
                ret: Type::String,
                is_async: false,
                is_ai: false,
                prompt: None,
            },
        ),
        (
            "write_text".to_string(),
            FunctionSignature {
                params: vec![Type::String, Type::String],
                ret: Type::String,
                is_async: false,
                is_ai: false,
                prompt: None,
            },
        ),
        (
            "append_text".to_string(),
            FunctionSignature {
                params: vec![Type::String, Type::String],
                ret: Type::String,
                is_async: false,
                is_ai: false,
                prompt: None,
            },
        ),
        (
            "len".to_string(),
            FunctionSignature {
                params: vec![Type::Unknown],
                ret: Type::Int,
                is_async: false,
                is_ai: false,
                prompt: None,
            },
        ),
        (
            "contains".to_string(),
            FunctionSignature {
                params: vec![Type::String, Type::String],
                ret: Type::Bool,
                is_async: false,
                is_ai: false,
                prompt: None,
            },
        ),
        (
            "is_digits".to_string(),
            FunctionSignature {
                params: vec![Type::String],
                ret: Type::Bool,
                is_async: false,
                is_ai: false,
                prompt: None,
            },
        ),
        (
            "is_email".to_string(),
            FunctionSignature {
                params: vec![Type::String],
                ret: Type::Bool,
                is_async: false,
                is_ai: false,
                prompt: None,
            },
        ),
        (
            "int".to_string(),
            FunctionSignature {
                params: vec![Type::String],
                ret: Type::Int,
                is_async: false,
                is_ai: false,
                prompt: None,
            },
        ),
        (
            "is_int".to_string(),
            FunctionSignature {
                params: vec![Type::String],
                ret: Type::Bool,
                is_async: false,
                is_ai: false,
                prompt: None,
            },
        ),
        (
            "float".to_string(),
            FunctionSignature {
                params: vec![Type::String],
                ret: Type::Float,
                is_async: false,
                is_ai: false,
                prompt: None,
            },
        ),
        (
            "sanitize".to_string(),
            FunctionSignature {
                params: vec![Type::Unknown],
                ret: Type::Unknown,
                is_async: false,
                is_ai: false,
                prompt: None,
            },
        ),
        (
            "tensor".to_string(),
            FunctionSignature {
                params: vec![
                    Type::Array(Box::new(Type::Unknown)),
                    Type::Array(Box::new(Type::Int)),
                ],
                ret: Type::Tensor,
                is_async: false,
                is_ai: false,
                prompt: None,
            },
        ),
        (
            "sha3".to_string(),
            FunctionSignature {
                params: vec![Type::String],
                ret: Type::String,
                is_async: false,
                is_ai: false,
                prompt: None,
            },
        ),
        (
            "seal".to_string(),
            FunctionSignature {
                params: vec![Type::String],
                ret: Type::String,
                is_async: false,
                is_ai: false,
                prompt: None,
            },
        ),
        (
            "shape".to_string(),
            FunctionSignature {
                params: vec![Type::Unknown],
                ret: Type::String,
                is_async: false,
                is_ai: false,
                prompt: None,
            },
        ),
        (
            "watch".to_string(),
            FunctionSignature {
                params: vec![Type::String, Type::Unknown],
                ret: Type::Unknown,
                is_async: false,
                is_ai: false,
                prompt: None,
            },
        ),
        (
            "guard".to_string(),
            FunctionSignature {
                params: vec![Type::Bool, Type::String],
                ret: Type::Unit,
                is_async: false,
                is_ai: false,
                prompt: None,
            },
        ),
        (
            "fail".to_string(),
            FunctionSignature {
                params: vec![Type::String],
                ret: Type::Unit,
                is_async: false,
                is_ai: false,
                prompt: None,
            },
        ),
        (
            "repair".to_string(),
            FunctionSignature {
                params: vec![Type::Unknown, Type::Unknown],
                ret: Type::Unknown,
                is_async: false,
                is_ai: false,
                prompt: None,
            },
        ),
        (
            "blend".to_string(),
            FunctionSignature {
                params: vec![Type::Array(Box::new(Type::Unknown)), Type::String],
                ret: Type::String,
                is_async: false,
                is_ai: false,
                prompt: None,
            },
        ),
        (
            "draft".to_string(),
            FunctionSignature {
                params: vec![Type::String, Type::String],
                ret: Type::String,
                is_async: false,
                is_ai: false,
                prompt: None,
            },
        ),
        (
            "panel".to_string(),
            FunctionSignature {
                params: vec![Type::String, Type::String],
                ret: Type::String,
                is_async: false,
                is_ai: false,
                prompt: None,
            },
        ),
        (
            "stack".to_string(),
            FunctionSignature {
                params: vec![Type::Array(Box::new(Type::Unknown))],
                ret: Type::String,
                is_async: false,
                is_ai: false,
                prompt: None,
            },
        ),
        (
            "field".to_string(),
            FunctionSignature {
                params: vec![Type::String, Type::Unknown],
                ret: Type::String,
                is_async: false,
                is_ai: false,
                prompt: None,
            },
        ),
        (
            "matmul".to_string(),
            FunctionSignature {
                params: vec![Type::Tensor, Type::Tensor],
                ret: Type::Tensor,
                is_async: false,
                is_ai: false,
                prompt: None,
            },
        ),
        (
            "relu".to_string(),
            FunctionSignature {
                params: vec![Type::Tensor],
                ret: Type::Tensor,
                is_async: false,
                is_ai: false,
                prompt: None,
            },
        ),
        (
            "sleep_ticks".to_string(),
            FunctionSignature {
                params: vec![Type::Int],
                ret: Type::Unit,
                is_async: false,
                is_ai: false,
                prompt: None,
            },
        ),
        (
            "pulse".to_string(),
            FunctionSignature {
                params: vec![Type::Int],
                ret: Type::Unit,
                is_async: false,
                is_ai: false,
                prompt: None,
            },
        ),
    ])
}

fn resolve_type(ty: &TypeExpr) -> Type {
    match &ty.kind {
        TypeExprKind::Path(path) => match path.join(".").as_str() {
            "Int" => Type::Int,
            "Float" => Type::Float,
            "Bool" => Type::Bool,
            "String" => Type::String,
            "Bytes" => Type::Bytes,
            "Unit" => Type::Unit,
            "Tensor" => Type::Tensor,
            other => Type::Named(other.to_string()),
        },
        TypeExprKind::Generic { base, args } => match base.join(".").as_str() {
            "Array" => Type::Array(Box::new(
                args.get(0).map(resolve_type).unwrap_or(Type::Unknown),
            )),
            "Task" => Type::Task(Box::new(
                args.get(0).map(resolve_type).unwrap_or(Type::Unknown),
            )),
            other => Type::Named(other.to_string()),
        },
    }
}

fn type_compatible(expected: &Type, actual: &Type) -> bool {
    if matches!(expected, Type::Unknown) || matches!(actual, Type::Unknown) {
        return true;
    }
    match (expected, actual) {
        (Type::Array(a), Type::Array(b)) => type_compatible(a, b),
        (Type::Task(a), Type::Task(b)) => type_compatible(a, b),
        (Type::Function(a_params, a_ret), Type::Function(b_params, b_ret)) => {
            a_params.len() == b_params.len()
                && a_params
                    .iter()
                    .zip(b_params)
                    .all(|(a, b)| type_compatible(a, b))
                && type_compatible(a_ret, b_ret)
        }
        _ => expected == actual,
    }
}

fn unify_types(left: Type, right: Type) -> Type {
    if left == Type::Unknown {
        return right;
    }
    if right == Type::Unknown || left == right {
        return left;
    }
    Type::Unknown
}

fn validate_pattern(pattern: &Pattern, subject: &Type, diagnostics: &mut Vec<Diagnostic>) {
    let pattern_ty = match pattern {
        Pattern::Wildcard(_) | Pattern::Ident(_, _) => return,
        Pattern::Int(_, _) => Type::Int,
        Pattern::Float(_, _) => Type::Float,
        Pattern::Bool(_, _) => Type::Bool,
        Pattern::String(_, _) => Type::String,
    };
    if !type_compatible(subject, &pattern_ty) {
        diagnostics.push(Diagnostic::error(
            format!("pattern {:?} does not match {:?}", pattern_ty, subject),
            Some(pattern.span()),
        ));
    }
}
