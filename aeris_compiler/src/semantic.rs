use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::ast::{BinaryOp, Expr, Param, Pattern, Program, Stmt, TypeExpr, UnaryOp};
use crate::diag::Diagnostic;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CheckedProgram {
    pub program: Program,
    pub summary: ProgramSummary,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProgramSummary {
    pub effects: Vec<String>,
    pub capabilities: Vec<String>,
    pub functions: Vec<FunctionSummary>,
    pub actors: Vec<String>,
    pub types: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FunctionSummary {
    pub name: String,
    pub effect: String,
    pub params: Vec<(String, String)>,
    pub return_type: String,
}

#[derive(Clone, Debug)]
struct FunctionSig {
    effect: String,
    params: Vec<Param>,
    return_type: TypeExpr,
}

pub fn analyze(program: Program, require_main: bool) -> Result<CheckedProgram, Vec<Diagnostic>> {
    let mut diag = Vec::new();
    let builtin_effects: HashSet<String> = ["pure", "io", "actor", "state"]
        .into_iter()
        .map(str::to_string)
        .collect();
    let builtin_capabilities: HashSet<String> = ["Console", "Clock", "FileSystem", "Network"]
        .into_iter()
        .map(str::to_string)
        .collect();
    let mut effects = builtin_effects.clone();
    let mut capabilities = builtin_capabilities.clone();
    let mut types: HashSet<String> = ["Int", "Bool", "Text", "Unit", "ActorHandle"]
        .into_iter()
        .map(str::to_string)
        .collect();
    let mut functions = HashMap::new();
    let mut actors = HashSet::new();
    let mut constructors: HashMap<String, Vec<TypeExpr>> = HashMap::new();
    let mut declared_effects = HashSet::new();
    let mut declared_capabilities = HashSet::new();

    for item in &program.effects {
        effects.insert(item.name.clone());
        if !builtin_effects.contains(&item.name) && !declared_effects.insert(item.name.clone()) {
            diag.push(
                Diagnostic::error("AERIS-SEM-001", format!("duplicate effect '{}'", item.name))
                    .with_span(item.span.clone())
                    .with_help("rename or remove the duplicate effect declaration"),
            );
        }
    }

    for item in &program.capabilities {
        capabilities.insert(item.name.clone());
        if !builtin_capabilities.contains(&item.name)
            && !declared_capabilities.insert(item.name.clone())
        {
            diag.push(
                Diagnostic::error(
                    "AERIS-SEM-002",
                    format!("duplicate capability '{}'", item.name),
                )
                .with_span(item.span.clone()),
            );
        }
    }

    for item in &program.types {
        if !types.insert(item.name.clone()) {
            diag.push(
                Diagnostic::error("AERIS-SEM-003", format!("duplicate type '{}'", item.name))
                    .with_span(item.span.clone()),
            );
        }
        for variant in &item.variants {
            constructors.insert(
                variant.name.clone(),
                variant
                    .fields
                    .iter()
                    .map(|field| field.ty.clone())
                    .collect(),
            );
            if !types.insert(variant.name.clone()) {
                diag.push(
                    Diagnostic::error(
                        "AERIS-SEM-004",
                        format!("duplicate constructor '{}'", variant.name),
                    )
                    .with_span(variant.span.clone()),
                );
            }
        }
    }

    for actor in &program.actors {
        if !actors.insert(actor.name.clone()) {
            diag.push(
                Diagnostic::error("AERIS-SEM-005", format!("duplicate actor '{}'", actor.name))
                    .with_span(actor.span.clone()),
            );
        }
    }

    for function in &program.functions {
        if functions.contains_key(&function.name) {
            diag.push(
                Diagnostic::error(
                    "AERIS-SEM-006",
                    format!("duplicate function '{}'", function.name),
                )
                .with_span(function.span.clone()),
            );
        } else {
            functions.insert(
                function.name.clone(),
                FunctionSig {
                    effect: function.effect.clone(),
                    params: function.params.clone(),
                    return_type: function.return_type.clone(),
                },
            );
        }
    }

    if require_main && !functions.contains_key("main") {
        diag.push(
            Diagnostic::error("AERIS-SEM-007", "no 'main' entry point found")
                .with_help("declare fn main(...) -> Int !io { ... }"),
        );
    }

    for function in &program.functions {
        if !effects.contains(&function.effect) {
            diag.push(
                Diagnostic::error(
                    "AERIS-SEM-008",
                    format!("unknown effect '{}'", function.effect),
                )
                .with_span(function.span.clone())
                .with_help("declare the effect or use one of: pure, io, actor, state"),
            );
        }
        validate_type(
            &function.return_type,
            &types,
            &capabilities,
            &mut diag,
            &function.name,
        );
        let mut env = HashMap::new();
        for param in &function.params {
            validate_type(&param.ty, &types, &capabilities, &mut diag, &function.name);
            if env.insert(param.name.clone(), param.ty.clone()).is_some() {
                diag.push(
                    Diagnostic::error(
                        "AERIS-SEM-009",
                        format!("duplicate parameter '{}'", param.name),
                    )
                    .with_span(param.span.clone()),
                );
            }
            if let Some(predicate) = &param.refinement {
                validate_predicate(predicate, &param.name, &mut diag);
            }
        }
        if let Some(predicate) = &function.where_clause {
            validate_predicate(predicate, "<function>", &mut diag);
        }
        analyze_block(
            &function.body,
            &functions,
            &actors,
            &constructors,
            &capabilities,
            &mut env,
            &function.effect,
            &mut diag,
        );
    }

    for actor in &program.actors {
        let mut env = HashMap::new();
        for param in &actor.params {
            validate_type(&param.ty, &types, &capabilities, &mut diag, &actor.name);
            env.insert(param.name.clone(), param.ty.clone());
        }
        for handler in &actor.handlers {
            bind_pattern(&handler.pattern, &constructors, &mut env);
            analyze_block(
                &handler.body,
                &functions,
                &actors,
                &constructors,
                &capabilities,
                &mut env.clone(),
                "actor",
                &mut diag,
            );
        }
    }

    if diag
        .iter()
        .any(|d| matches!(d.severity, crate::diag::Severity::Error))
    {
        Err(diag)
    } else {
        let summary = ProgramSummary {
            effects: effects.into_iter().collect(),
            capabilities: capabilities.into_iter().collect(),
            functions: program
                .functions
                .iter()
                .map(|function| FunctionSummary {
                    name: function.name.clone(),
                    effect: function.effect.clone(),
                    params: function
                        .params
                        .iter()
                        .map(|param| (param.name.clone(), param.ty.name().to_string()))
                        .collect(),
                    return_type: function.return_type.name().to_string(),
                })
                .collect(),
            actors: program
                .actors
                .iter()
                .map(|actor| actor.name.clone())
                .collect(),
            types: program.types.iter().map(|item| item.name.clone()).collect(),
        };
        Ok(CheckedProgram { program, summary })
    }
}

fn validate_type(
    ty: &TypeExpr,
    known_types: &HashSet<String>,
    capabilities: &HashSet<String>,
    diag: &mut Vec<Diagnostic>,
    owner: &str,
) {
    match ty {
        TypeExpr::Named { name, args } => {
            if !known_types.contains(name) && !capabilities.contains(name) && name != "Fn" {
                diag.push(
                    Diagnostic::error(
                        "AERIS-SEM-010",
                        format!("unknown type '{}' in '{}'", name, owner),
                    )
                    .with_help("declare the type/capability before using it"),
                );
            }
            for arg in args {
                validate_type(arg, known_types, capabilities, diag, owner);
            }
        }
    }
}

fn validate_predicate(predicate: &Expr, subject: &str, diag: &mut Vec<Diagnostic>) {
    match predicate {
        Expr::Binary { op, left, right } => match (&**left, &**right, op) {
            (Expr::Variable(name), Expr::Int(_), BinaryOp::Ne | BinaryOp::Gt | BinaryOp::Ge)
                if name == subject || subject == "<function>" => {}
            (Expr::Variable(name), Expr::Bool(_), BinaryOp::Eq | BinaryOp::Ne)
                if name == subject || subject == "<function>" => {}
            _ => diag.push(
                Diagnostic::warning(
                    "AERIS-SEM-011",
                    "refinement predicates are currently limited to simple comparisons",
                )
                .with_help("prefer forms like y != 0 or size > 0"),
            ),
        },
        _ => diag.push(Diagnostic::warning(
            "AERIS-SEM-012",
            "non-binary predicate may not be enforceable in the bootstrap compiler",
        )),
    }
}

fn analyze_block(
    block: &crate::ast::Block,
    functions: &HashMap<String, FunctionSig>,
    actors: &HashSet<String>,
    constructors: &HashMap<String, Vec<TypeExpr>>,
    capabilities: &HashSet<String>,
    env: &mut HashMap<String, TypeExpr>,
    current_effect: &str,
    diag: &mut Vec<Diagnostic>,
) {
    for stmt in &block.statements {
        match stmt {
            Stmt::Let { name, value, .. } => {
                let ty = value
                    .as_ref()
                    .and_then(|expr| {
                        infer_expr_type(
                            expr,
                            functions,
                            actors,
                            constructors,
                            env,
                            capabilities,
                            current_effect,
                            diag,
                        )
                    })
                    .unwrap_or_else(|| TypeExpr::simple("Unit"));
                env.insert(name.clone(), ty);
            }
            Stmt::Mut { name, value, .. } => {
                let ty = infer_expr_type(
                    value,
                    functions,
                    actors,
                    constructors,
                    env,
                    capabilities,
                    current_effect,
                    diag,
                )
                .unwrap_or_else(|| TypeExpr::simple("Unit"));
                env.insert(name.clone(), ty);
            }
            Stmt::Require { condition, .. } => {
                let ty = infer_expr_type(
                    condition,
                    functions,
                    actors,
                    constructors,
                    env,
                    capabilities,
                    current_effect,
                    diag,
                );
                if !matches!(ty, Some(TypeExpr::Named { name, .. }) if name == "Bool") {
                    diag.push(
                        Diagnostic::error("AERIS-SEM-013", "require expects a Bool condition")
                            .with_help("compare values explicitly before calling require"),
                    );
                }
            }
            Stmt::Return { value, .. } | Stmt::Expr { value, .. } => {
                let _ = infer_expr_type(
                    value,
                    functions,
                    actors,
                    constructors,
                    env,
                    capabilities,
                    current_effect,
                    diag,
                );
            }
        }
    }

    if let Some(expr) = &block.tail {
        let _ = infer_expr_type(
            expr,
            functions,
            actors,
            constructors,
            env,
            capabilities,
            current_effect,
            diag,
        );
    }
}

fn infer_expr_type(
    expr: &Expr,
    functions: &HashMap<String, FunctionSig>,
    actors: &HashSet<String>,
    constructors: &HashMap<String, Vec<TypeExpr>>,
    env: &HashMap<String, TypeExpr>,
    capabilities: &HashSet<String>,
    current_effect: &str,
    diag: &mut Vec<Diagnostic>,
) -> Option<TypeExpr> {
    match expr {
        Expr::Int(_) => Some(TypeExpr::simple("Int")),
        Expr::Bool(_) => Some(TypeExpr::simple("Bool")),
        Expr::Text(_) => Some(TypeExpr::simple("Text")),
        Expr::Variable(name) => env
            .get(name)
            .cloned()
            .or_else(|| functions.get(name).map(|_| TypeExpr::simple("Fn")))
            .or_else(|| {
                if actors.contains(name) {
                    Some(TypeExpr::simple("ActorFactory"))
                } else if capabilities.contains(name) {
                    Some(TypeExpr::simple(name.clone()))
                } else {
                    None
                }
            }),
        Expr::Unary { op, value } => {
            let inner = infer_expr_type(
                value,
                functions,
                actors,
                constructors,
                env,
                capabilities,
                current_effect,
                diag,
            )?;
            match (op, inner.name()) {
                (UnaryOp::Neg, "Int") => Some(TypeExpr::simple("Int")),
                (UnaryOp::Not, "Bool") => Some(TypeExpr::simple("Bool")),
                _ => {
                    diag.push(
                        Diagnostic::error("AERIS-SEM-014", "invalid unary operation")
                            .with_help("'-' applies to Int and '!' applies to Bool"),
                    );
                    None
                }
            }
        }
        Expr::Binary { op, left, right } => {
            let left_ty = infer_expr_type(
                left,
                functions,
                actors,
                constructors,
                env,
                capabilities,
                current_effect,
                diag,
            )?;
            let right_ty = infer_expr_type(
                right,
                functions,
                actors,
                constructors,
                env,
                capabilities,
                current_effect,
                diag,
            )?;
            match op {
                BinaryOp::Add => {
                    if left_ty.name() == "Text" || right_ty.name() == "Text" {
                        Some(TypeExpr::simple("Text"))
                    } else if left_ty.name() == "Int" && right_ty.name() == "Int" {
                        Some(TypeExpr::simple("Int"))
                    } else {
                        diag.push(
                            Diagnostic::error("AERIS-SEM-015", "type mismatch in addition")
                                .with_help("use Int + Int or Text + Text/Text"),
                        );
                        None
                    }
                }
                BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Mod => {
                    if left_ty.name() == "Int" && right_ty.name() == "Int" {
                        Some(TypeExpr::simple("Int"))
                    } else {
                        diag.push(Diagnostic::error(
                            "AERIS-SEM-016",
                            "arithmetic operators require Int operands",
                        ));
                        None
                    }
                }
                BinaryOp::Eq
                | BinaryOp::Ne
                | BinaryOp::Lt
                | BinaryOp::Le
                | BinaryOp::Gt
                | BinaryOp::Ge
                | BinaryOp::And
                | BinaryOp::Or => Some(TypeExpr::simple("Bool")),
            }
        }
        Expr::If {
            condition,
            then_branch,
            else_branch,
        } => {
            let cond_ty = infer_expr_type(
                condition,
                functions,
                actors,
                constructors,
                env,
                capabilities,
                current_effect,
                diag,
            )?;
            if cond_ty.name() != "Bool" {
                diag.push(Diagnostic::error(
                    "AERIS-SEM-017",
                    "if condition must be Bool",
                ));
            }
            let mut then_env = env.clone();
            let mut else_env = env.clone();
            analyze_block(
                then_branch,
                functions,
                actors,
                constructors,
                capabilities,
                &mut then_env,
                current_effect,
                diag,
            );
            analyze_block(
                else_branch,
                functions,
                actors,
                constructors,
                capabilities,
                &mut else_env,
                current_effect,
                diag,
            );
            if let Some(expr) = &then_branch.tail {
                infer_expr_type(
                    expr,
                    functions,
                    actors,
                    constructors,
                    &then_env,
                    capabilities,
                    current_effect,
                    diag,
                )
            } else {
                Some(TypeExpr::simple("Unit"))
            }
        }
        Expr::Match { scrutinee, arms } => {
            let _ = infer_expr_type(
                scrutinee,
                functions,
                actors,
                constructors,
                env,
                capabilities,
                current_effect,
                diag,
            );
            for arm in arms {
                let mut local = env.clone();
                bind_pattern(&arm.pattern, constructors, &mut local);
                analyze_block(
                    &arm.body,
                    functions,
                    actors,
                    constructors,
                    capabilities,
                    &mut local,
                    current_effect,
                    diag,
                );
            }
            Some(TypeExpr::simple("Unit"))
        }
        Expr::Lambda { .. } => Some(TypeExpr::simple("Fn")),
        Expr::Call { callee, args } => {
            if let Expr::Variable(name) = &**callee {
                if let Some(ty) = check_builtin(name, args, env, current_effect, capabilities, diag)
                {
                    return Some(ty);
                }
                if constructors.contains_key(name) {
                    return Some(TypeExpr::simple(name.clone()));
                }
                if let Some(sig) = functions.get(name) {
                    if sig.effect != "pure" && sig.effect != current_effect {
                        diag.push(
                            Diagnostic::error(
                                "AERIS-SEM-018",
                                format!(
                                    "effect '{}' cannot call '{}'-effect function '{}'",
                                    current_effect, sig.effect, name
                                ),
                            )
                            .with_help(
                                "split the effectful code into a function with the matching effect",
                            ),
                        );
                    }
                    for (index, arg) in args.iter().enumerate() {
                        let _ = infer_expr_type(
                            arg,
                            functions,
                            actors,
                            constructors,
                            env,
                            capabilities,
                            current_effect,
                            diag,
                        );
                        if let Some(param) = sig.params.get(index) {
                            if let Some(predicate) = &param.refinement {
                                if violates_refinement(predicate, &param.name, arg) {
                                    diag.push(
                                        Diagnostic::error(
                                            "AERIS-SEM-019",
                                            format!(
                                                "argument {} violates refinement on '{}'",
                                                index + 1,
                                                param.name
                                            ),
                                        )
                                        .with_help("provide a value that satisfies the declared constraint"),
                                    );
                                }
                            }
                        }
                    }
                    return Some(sig.return_type.clone());
                }
            }
            let _ = infer_expr_type(
                callee,
                functions,
                actors,
                constructors,
                env,
                capabilities,
                current_effect,
                diag,
            );
            for arg in args {
                let _ = infer_expr_type(
                    arg,
                    functions,
                    actors,
                    constructors,
                    env,
                    capabilities,
                    current_effect,
                    diag,
                );
            }
            Some(TypeExpr::simple("Unit"))
        }
    }
}

fn check_builtin(
    name: &str,
    args: &[Expr],
    env: &HashMap<String, TypeExpr>,
    current_effect: &str,
    capabilities: &HashSet<String>,
    diag: &mut Vec<Diagnostic>,
) -> Option<TypeExpr> {
    match name {
        "print" => {
            if current_effect != "io" {
                diag.push(
                    Diagnostic::error("AERIS-SEM-020", "print requires an !io function")
                        .with_help("move printing logic into a function annotated with !io"),
                );
            }
            ensure_capability_arg("Console", args.first(), env, capabilities, diag);
            Some(TypeExpr::simple("Unit"))
        }
        "read_line" => {
            if current_effect != "io" {
                diag.push(
                    Diagnostic::error("AERIS-SEM-021", "read_line requires an !io function")
                        .with_help("read input only inside !io functions"),
                );
            }
            ensure_capability_arg("Console", args.first(), env, capabilities, diag);
            Some(TypeExpr::simple("Text"))
        }
        "spawn" | "emit" | "drain" => {
            if current_effect != "actor" {
                diag.push(
                    Diagnostic::error(
                        "AERIS-SEM-022",
                        format!("{name} requires an !actor function"),
                    )
                    .with_help("declare the function with !actor when using actor primitives"),
                );
            }
            Some(match name {
                "spawn" => TypeExpr::simple("ActorHandle"),
                "drain" => TypeExpr::simple("Int"),
                _ => TypeExpr::simple("Unit"),
            })
        }
        "assert" => Some(TypeExpr::simple("Unit")),
        _ => None,
    }
}

fn ensure_capability_arg(
    expected: &str,
    arg: Option<&Expr>,
    env: &HashMap<String, TypeExpr>,
    capabilities: &HashSet<String>,
    diag: &mut Vec<Diagnostic>,
) {
    let Some(expr) = arg else {
        diag.push(
            Diagnostic::error(
                "AERIS-SEM-023",
                format!("missing required capability '{}'", expected),
            )
            .with_help("pass the explicit capability token to this call"),
        );
        return;
    };

    match expr {
        Expr::Variable(name) => match env.get(name) {
            Some(TypeExpr::Named { name, .. }) if name == expected => {}
            Some(TypeExpr::Named { name, .. }) if capabilities.contains(name) => {
                diag.push(Diagnostic::error(
                    "AERIS-SEM-024",
                    format!("expected capability '{}', found '{}'", expected, name),
                ))
            }
            _ => diag.push(
                Diagnostic::error(
                    "AERIS-SEM-025",
                    format!("'{}' is not bound to capability '{}'", name, expected),
                )
                .with_help("thread the capability through the function signature explicitly"),
            ),
        },
        _ => diag.push(Diagnostic::error(
            "AERIS-SEM-026",
            format!(
                "capability '{}' must be passed as an explicit token",
                expected
            ),
        )),
    }
}

fn violates_refinement(predicate: &Expr, subject: &str, arg: &Expr) -> bool {
    match (predicate, arg) {
        (
            Expr::Binary {
                op: BinaryOp::Ne,
                left,
                right,
            },
            Expr::Int(value),
        ) => matches!(
            (&**left, &**right),
            (Expr::Variable(name), Expr::Int(disallowed)) if name == subject && value == disallowed
        ),
        (
            Expr::Binary {
                op: BinaryOp::Gt,
                left,
                right,
            },
            Expr::Int(value),
        ) => matches!(
            (&**left, &**right),
            (Expr::Variable(name), Expr::Int(minimum)) if name == subject && value <= minimum
        ),
        _ => false,
    }
}

fn bind_pattern(
    pattern: &Pattern,
    constructors: &HashMap<String, Vec<TypeExpr>>,
    env: &mut HashMap<String, TypeExpr>,
) {
    match pattern {
        Pattern::Identifier(name) => {
            env.insert(name.clone(), TypeExpr::simple("Unit"));
        }
        Pattern::Variant { name, bindings } => {
            if let Some(field_types) = constructors.get(name) {
                for (binding, ty) in bindings.iter().zip(field_types.iter()) {
                    env.insert(binding.clone(), ty.clone());
                }
            } else {
                for binding in bindings {
                    env.insert(binding.clone(), TypeExpr::simple("Unit"));
                }
            }
        }
        Pattern::Wildcard | Pattern::Int(_) | Pattern::Bool(_) | Pattern::Text(_) => {}
    }
}
