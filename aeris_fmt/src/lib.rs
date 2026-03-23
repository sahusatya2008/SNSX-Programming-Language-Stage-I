use anyhow::Result;

use aeris_compiler::ast::{BinaryOp, Block, Expr, Pattern, Program, Stmt, TypeExpr};
use aeris_compiler::parser;

pub fn format_source(source: &str) -> Result<String> {
    let program = parser::parse(source).map_err(|diag| anyhow::anyhow!(render_diags(&diag)))?;
    Ok(format_program(&program))
}

fn render_diags(diag: &[aeris_compiler::diag::Diagnostic]) -> String {
    diag.iter()
        .map(|item| format!("[{}] {}", item.code, item.message))
        .collect::<Vec<_>>()
        .join("\n")
}

fn format_program(program: &Program) -> String {
    let mut lines = Vec::new();
    if let Some(module) = &program.module {
        lines.push(format!("module {};", module.path.join(".")));
        lines.push(String::new());
    }
    for item in &program.uses {
        let mut line = format!("use {}", item.path.join("."));
        if let Some(alias) = &item.alias {
            line.push_str(&format!(" as {alias}"));
        }
        line.push(';');
        lines.push(line);
    }
    if !program.uses.is_empty() {
        lines.push(String::new());
    }
    for item in &program.effects {
        lines.push(format!("effect {};", item.name));
    }
    for item in &program.capabilities {
        lines.push(format!("cap {};", item.name));
    }
    if !program.effects.is_empty() || !program.capabilities.is_empty() {
        lines.push(String::new());
    }
    for item in &program.types {
        let variants = item
            .variants
            .iter()
            .map(|variant| {
                if variant.fields.is_empty() {
                    variant.name.clone()
                } else {
                    let fields = variant
                        .fields
                        .iter()
                        .map(|field| format!("{}: {}", field.name, format_type(&field.ty)))
                        .collect::<Vec<_>>()
                        .join(", ");
                    format!("{}({fields})", variant.name)
                }
            })
            .collect::<Vec<_>>()
            .join(" | ");
        let generics = if item.generics.is_empty() {
            String::new()
        } else {
            format!("<{}>", item.generics.join(", "))
        };
        lines.push(format!("type {}{} = {};", item.name, generics, variants));
    }
    if !program.types.is_empty() {
        lines.push(String::new());
    }
    for actor in &program.actors {
        let params = actor
            .params
            .iter()
            .map(format_param)
            .collect::<Vec<_>>()
            .join(", ");
        lines.push(format!("actor {}({params}) {{", actor.name));
        for handler in &actor.handlers {
            lines.push(format!(
                "    on {} => {}",
                format_pattern(&handler.pattern),
                format_block(&handler.body, 1)
            ));
        }
        lines.push("}".to_string());
        lines.push(String::new());
    }
    for function in &program.functions {
        let generics = if function.generics.is_empty() {
            String::new()
        } else {
            format!("<{}>", function.generics.join(", "))
        };
        let params = function
            .params
            .iter()
            .map(format_param)
            .collect::<Vec<_>>()
            .join(", ");
        let mut header = format!(
            "fn {}{}({params}) -> {} !{}",
            function.name,
            generics,
            format_type(&function.return_type),
            function.effect
        );
        if let Some(predicate) = &function.where_clause {
            header.push_str(&format!(" where {}", format_expr(predicate)));
        }
        header.push(' ');
        header.push_str(&format_block(&function.body, 0));
        lines.push(header);
        lines.push(String::new());
    }
    while matches!(lines.last(), Some(last) if last.is_empty()) {
        lines.pop();
    }
    lines.join("\n")
}

fn format_param(param: &aeris_compiler::ast::Param) -> String {
    let mut out = format!("{}: {}", param.name, format_type(&param.ty));
    if let Some(predicate) = &param.refinement {
        out.push_str(&format!(" where {}", format_expr(predicate)));
    }
    out
}

fn format_block(block: &Block, indent: usize) -> String {
    let pad = "    ".repeat(indent);
    let inner = "    ".repeat(indent + 1);
    let mut lines = vec!["{".to_string()];
    for stmt in &block.statements {
        lines.push(format!("{inner}{}", format_stmt(stmt)));
    }
    if let Some(expr) = &block.tail {
        lines.push(format!("{inner}{}", format_expr(expr)));
    }
    lines.push(format!("{pad}}}"));
    lines.join("\n")
}

fn format_stmt(stmt: &Stmt) -> String {
    match stmt {
        Stmt::Let { name, value, .. } => match value {
            Some(value) => format!("let {name} = {};", format_expr(value)),
            None => format!("let {name};"),
        },
        Stmt::Mut { name, value, .. } => format!("mut {name} = {};", format_expr(value)),
        Stmt::Require {
            condition, message, ..
        } => match message {
            Some(message) => format!("require {}, {:?};", format_expr(condition), message),
            None => format!("require {};", format_expr(condition)),
        },
        Stmt::Return { value, .. } => format!("return {};", format_expr(value)),
        Stmt::Expr { value, .. } => format!("{};", format_expr(value)),
    }
}

fn format_expr(expr: &Expr) -> String {
    match expr {
        Expr::Int(value) => value.to_string(),
        Expr::Bool(value) => value.to_string(),
        Expr::Text(value) => format!("{value:?}"),
        Expr::Variable(name) => name.clone(),
        Expr::Call { callee, args } => format!(
            "{}({})",
            format_expr(callee),
            args.iter().map(format_expr).collect::<Vec<_>>().join(", ")
        ),
        Expr::Binary { op, left, right } => format!(
            "{} {} {}",
            format_expr(left),
            binary_op(op),
            format_expr(right)
        ),
        Expr::Unary { op, value } => match op {
            aeris_compiler::ast::UnaryOp::Neg => format!("-{}", format_expr(value)),
            aeris_compiler::ast::UnaryOp::Not => format!("!{}", format_expr(value)),
        },
        Expr::If {
            condition,
            then_branch,
            else_branch,
        } => format!(
            "if {} {} else {}",
            format_expr(condition),
            format_block(then_branch, 0),
            format_block(else_branch, 0)
        ),
        Expr::Match { scrutinee, arms } => {
            let mut out = format!("match {} {{\n", format_expr(scrutinee));
            for arm in arms {
                out.push_str(&format!(
                    "    {} => {},\n",
                    format_pattern(&arm.pattern),
                    format_block(&arm.body, 1)
                ));
            }
            out.push('}');
            out
        }
        Expr::Lambda { params, body } => {
            format!("|{}| {}", params.join(", "), format_expr(body))
        }
    }
}

fn format_pattern(pattern: &Pattern) -> String {
    match pattern {
        Pattern::Wildcard => "_".to_string(),
        Pattern::Identifier(name) => name.clone(),
        Pattern::Int(value) => value.to_string(),
        Pattern::Bool(value) => value.to_string(),
        Pattern::Text(value) => format!("{value:?}"),
        Pattern::Variant { name, bindings } => format!("{}({})", name, bindings.join(", ")),
    }
}

fn format_type(ty: &TypeExpr) -> String {
    match ty {
        TypeExpr::Named { name, args } if args.is_empty() => name.clone(),
        TypeExpr::Named { name, args } => format!(
            "{}<{}>",
            name,
            args.iter().map(format_type).collect::<Vec<_>>().join(", ")
        ),
    }
}

fn binary_op(op: &BinaryOp) -> &'static str {
    match op {
        BinaryOp::Add => "+",
        BinaryOp::Sub => "-",
        BinaryOp::Mul => "*",
        BinaryOp::Div => "/",
        BinaryOp::Mod => "%",
        BinaryOp::Eq => "==",
        BinaryOp::Ne => "!=",
        BinaryOp::Lt => "<",
        BinaryOp::Le => "<=",
        BinaryOp::Gt => ">",
        BinaryOp::Ge => ">=",
        BinaryOp::And => "&&",
        BinaryOp::Or => "||",
    }
}
