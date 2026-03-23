use crate::ast::*;
use crate::diag::Diagnostic;
use crate::semantic::{FunctionSignature, SemanticModel};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone)]
pub struct SecurityPolicy {
    pub strict: bool,
    pub allow_fs: bool,
    pub allow_network: bool,
    pub allow_ai: bool,
}

impl SecurityPolicy {
    pub fn standalone() -> Self {
        Self {
            strict: true,
            allow_fs: false,
            allow_network: false,
            allow_ai: true,
        }
    }

    pub fn permissive() -> Self {
        Self {
            strict: false,
            allow_fs: true,
            allow_network: true,
            allow_ai: true,
        }
    }
}

pub fn audit_module(
    module: &Module,
    semantic: &SemanticModel,
    policy: &SecurityPolicy,
) -> Vec<Diagnostic> {
    if !policy.strict {
        return Vec::new();
    }

    let mut diagnostics = Vec::new();
    for use_decl in &module.uses {
        audit_use(use_decl, policy, &mut diagnostics);
    }

    for item in &module.items {
        match item {
            Item::Function(function) => {
                audit_function(function, &semantic.functions, policy, &mut diagnostics);
            }
            Item::Actor(actor) => {
                for method in &actor.methods {
                    audit_function(method, &semantic.functions, policy, &mut diagnostics);
                }
            }
        }
    }

    diagnostics
}

fn audit_use(use_decl: &UseDecl, policy: &SecurityPolicy, diagnostics: &mut Vec<Diagnostic>) {
    let joined = use_decl.path.join(".");
    match joined.as_str() {
        "std.net" if !policy.allow_network => diagnostics.push(
            Diagnostic::error(
                "network module imported without network permission",
                Some(use_decl.span),
            )
            .with_code("SNSX-SEC-001")
            .with_help(
                "strict mode blocks network-capable modules unless the project explicitly enables network access.",
            )
            .with_fix("Set `[permissions].net = true` in `snsx.toml`, or remove `bring <module/std.net>`."),
        ),
        "std.ai" if !policy.allow_ai => diagnostics.push(
            Diagnostic::error("AI module imported without AI permission", Some(use_decl.span))
                .with_code("SNSX-SEC-002")
                .with_help(
                    "strict mode prevents model access unless the project explicitly allows AI execution.",
                )
                .with_fix("Set `[permissions].ai = true` in `snsx.toml`, or remove `bring <module/std.ai>`."),
        ),
        _ => {}
    }
}

fn audit_function(
    function: &Function,
    functions: &HashMap<String, FunctionSignature>,
    policy: &SecurityPolicy,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut tainted_locals = HashSet::new();
    if function.name != "main" {
        for param in &function.params {
            if param.ty.is_none() {
                diagnostics.push(
                    Diagnostic::error(
                        format!("parameter '{}' is missing an explicit type", param.name),
                        Some(param.span),
                    )
                    .with_code("SNSX-SEC-010")
                    .with_help(
                        "strict SNSX requires typed function contracts so data boundaries stay reviewable and auditable.",
                    )
                    .with_fix(format!(
                        "Add a type to `{}` using a `takes` clause or `{}: Type` syntax.",
                        param.name, param.name
                    )),
                );
            }
        }
        if function.return_type.is_none() && !function.body.is_empty() {
            diagnostics.push(
                Diagnostic::error(
                    format!("flow '{}' is missing an explicit output contract", function.name),
                    Some(function.span),
                )
                .with_code("SNSX-SEC-011")
                .with_help(
                    "strict SNSX requires every reusable flow to declare what it returns so callers can reason about trust and ownership.",
                )
                .with_fix(format!(
                    "Add `gives Type` to the structured flow block or `-> Type` to the function header for '{}'.",
                    function.name
                )),
            );
        }
    }

    if function.is_ai && !policy.allow_ai {
        diagnostics.push(
            Diagnostic::error(
                format!("AI flow '{}' is declared without AI permission", function.name),
                Some(function.span),
            )
            .with_code("SNSX-SEC-012")
            .with_help(
                "AI flows may send data into model execution paths, so strict mode requires the project to opt in explicitly.",
            )
            .with_fix("Set `[permissions].ai = true` in `snsx.toml`, or replace the AI flow with a local deterministic flow."),
        );
    }

    for stmt in &function.body {
        audit_stmt(stmt, functions, policy, diagnostics, &mut tainted_locals);
    }
}

fn audit_stmt(
    stmt: &Stmt,
    functions: &HashMap<String, FunctionSignature>,
    policy: &SecurityPolicy,
    diagnostics: &mut Vec<Diagnostic>,
    tainted_locals: &mut HashSet<String>,
) {
    match stmt {
        Stmt::Let { name, expr, .. } => {
            audit_expr(expr, functions, policy, diagnostics, tainted_locals);
            if contains_sensitive_input_or_tainted(expr, tainted_locals) {
                tainted_locals.insert(name.clone());
            } else {
                tainted_locals.remove(name);
            }
        }
        Stmt::Assign { target, expr, .. } => {
            audit_expr(expr, functions, policy, diagnostics, tainted_locals);
            if contains_sensitive_input_or_tainted(expr, tainted_locals) {
                tainted_locals.insert(target.clone());
            } else {
                tainted_locals.remove(target);
            }
        }
        Stmt::Expr(expr) => {
            audit_expr(expr, functions, policy, diagnostics, tainted_locals);
        }
        Stmt::Return { expr, .. } => {
            if let Some(expr) = expr {
                audit_expr(expr, functions, policy, diagnostics, tainted_locals);
            }
        }
        Stmt::If {
            condition,
            then_branch,
            else_branch,
            ..
        } => {
            audit_expr(condition, functions, policy, diagnostics, tainted_locals);
            let mut then_tainted = tainted_locals.clone();
            for stmt in then_branch {
                audit_stmt(stmt, functions, policy, diagnostics, &mut then_tainted);
            }
            let mut else_tainted = tainted_locals.clone();
            for stmt in else_branch {
                audit_stmt(stmt, functions, policy, diagnostics, &mut else_tainted);
            }
            for name in then_tainted.into_iter().chain(else_tainted.into_iter()) {
                tainted_locals.insert(name);
            }
        }
        Stmt::While {
            condition,
            body,
            span,
        } => {
            if matches!(condition, Expr::Bool(true, _)) {
                diagnostics.push(
                    Diagnostic::error("unbounded `during true` loop is blocked in strict mode", Some(*span))
                        .with_code("SNSX-SEC-020")
                        .with_help(
                            "an always-true loop can hide denial-of-service bugs and makes runtime safety analysis unreliable.",
                        )
                        .with_fix(
                            "Use a bounded condition such as a counter or task state check instead of `true`.",
                        ),
                );
            }
            audit_expr(condition, functions, policy, diagnostics, tainted_locals);
            let mut inner_tainted = tainted_locals.clone();
            for stmt in body {
                audit_stmt(stmt, functions, policy, diagnostics, &mut inner_tainted);
            }
            for name in inner_tainted {
                tainted_locals.insert(name);
            }
        }
        Stmt::Match { expr, arms, .. } => {
            audit_expr(expr, functions, policy, diagnostics, tainted_locals);
            for arm in arms {
                let mut arm_tainted = tainted_locals.clone();
                for stmt in &arm.body {
                    audit_stmt(stmt, functions, policy, diagnostics, &mut arm_tainted);
                }
                for name in arm_tainted {
                    tainted_locals.insert(name);
                }
            }
        }
    }
}

fn audit_expr(
    expr: &Expr,
    functions: &HashMap<String, FunctionSignature>,
    policy: &SecurityPolicy,
    diagnostics: &mut Vec<Diagnostic>,
    tainted_locals: &HashSet<String>,
) {
    match expr {
        Expr::Array(items, _) => {
            for item in items {
                audit_expr(item, functions, policy, diagnostics, tainted_locals);
            }
        }
        Expr::Unary { expr, .. } => {
            audit_expr(expr, functions, policy, diagnostics, tainted_locals)
        }
        Expr::Binary { left, right, .. } => {
            audit_expr(left, functions, policy, diagnostics, tainted_locals);
            audit_expr(right, functions, policy, diagnostics, tainted_locals);
        }
        Expr::Call { callee, args, span } => {
            if let Expr::Ident(name, _) = &**callee {
                if is_print_like(name)
                    && args
                        .iter()
                        .any(|arg| contains_sensitive_input_or_tainted(arg, tainted_locals))
                {
                    diagnostics.push(
                        Diagnostic::error(
                            "raw input is being displayed without validation or redaction",
                            Some(*span),
                        )
                        .with_code("SNSX-SEC-030")
                        .with_help(
                            "strict mode blocks direct printing of user input because secrets, tokens, and unsafe payloads can leak immediately.",
                        )
                        .with_fix(
                            "Store the input in a variable, validate it, and only then show a sanitized value.",
                        ),
                    );
                }
                if matches!(name.as_str(), "read_text" | "write_text" | "append_text")
                    && !policy.allow_fs
                {
                    diagnostics.push(
                        Diagnostic::error(
                            format!(
                                "filesystem access '{}' attempted without filesystem permission",
                                name
                            ),
                            Some(*span),
                        )
                        .with_code("SNSX-SEC-031")
                        .with_help(
                            "file access changes the trust boundary of the program and must be declared explicitly in strict mode.",
                        )
                        .with_fix("Set `[permissions].fs = true` in `snsx.toml`, or remove the file access."),
                    );
                }
                if name == "rpc_call" && !policy.allow_network {
                    diagnostics.push(
                        Diagnostic::error(
                            "network call attempted without network permission",
                            Some(*span),
                        )
                        .with_code("SNSX-SEC-032")
                        .with_help(
                            "network effects expose the process to external state and require explicit opt-in in strict mode.",
                        )
                        .with_fix("Set `[permissions].net = true` in `snsx.toml`, or remove the network call."),
                    );
                }
                if let Some(signature) = functions.get(name) {
                    if signature.is_ai && !policy.allow_ai {
                        diagnostics.push(
                            Diagnostic::error(
                                format!("AI call '{}' is blocked by project policy", name),
                                Some(*span),
                            )
                            .with_code("SNSX-SEC-033")
                            .with_help(
                                "strict mode stops AI execution when the project has not explicitly granted model access.",
                            )
                            .with_fix("Set `[permissions].ai = true` in `snsx.toml`, or replace the AI call."),
                        );
                    }
                }
            }
            audit_expr(callee, functions, policy, diagnostics, tainted_locals);
            for arg in args {
                audit_expr(arg, functions, policy, diagnostics, tainted_locals);
            }
        }
        Expr::Member { object, .. } => {
            audit_expr(object, functions, policy, diagnostics, tainted_locals)
        }
        Expr::Index { object, index, .. } => {
            audit_expr(object, functions, policy, diagnostics, tainted_locals);
            audit_expr(index, functions, policy, diagnostics, tainted_locals);
        }
        Expr::Int(_, _)
        | Expr::Float(_, _)
        | Expr::Bool(_, _)
        | Expr::String(_, _)
        | Expr::Ident(_, _) => {}
    }
}

fn contains_sensitive_input_or_tainted(expr: &Expr, tainted_locals: &HashSet<String>) -> bool {
    match expr {
        Expr::Call { callee, args, .. } => {
            if let Expr::Ident(name, _) = &**callee {
                if name == "sanitize" {
                    return false;
                }
                if matches!(name.as_str(), "write_text" | "append_text") {
                    return false;
                }
                if matches!(
                    name.as_str(),
                    "input" | "ask" | "read_line" | "read_stdin" | "stdin"
                ) {
                    return true;
                }
            }
            contains_sensitive_input_or_tainted(callee, tainted_locals)
                || args
                    .iter()
                    .any(|arg| contains_sensitive_input_or_tainted(arg, tainted_locals))
        }
        Expr::Array(items, _) => items
            .iter()
            .any(|item| contains_sensitive_input_or_tainted(item, tainted_locals)),
        Expr::Unary { expr, .. } => contains_sensitive_input_or_tainted(expr, tainted_locals),
        Expr::Binary { left, right, .. } => {
            contains_sensitive_input_or_tainted(left, tainted_locals)
                || contains_sensitive_input_or_tainted(right, tainted_locals)
        }
        Expr::Member { object, .. } => contains_sensitive_input_or_tainted(object, tainted_locals),
        Expr::Index { object, index, .. } => {
            contains_sensitive_input_or_tainted(object, tainted_locals)
                || contains_sensitive_input_or_tainted(index, tainted_locals)
        }
        Expr::Int(_, _) | Expr::Float(_, _) | Expr::Bool(_, _) | Expr::String(_, _) => false,
        Expr::Ident(name, _) => tainted_locals.contains(name),
    }
}

fn is_print_like(name: &str) -> bool {
    matches!(name, "print" | "show" | "watch")
}
