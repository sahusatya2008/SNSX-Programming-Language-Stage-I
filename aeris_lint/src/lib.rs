use serde::{Deserialize, Serialize};

use aeris_compiler::ast::{Pattern, Program};
use aeris_compiler::parser;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LintFinding {
    pub level: String,
    pub rule: String,
    pub message: String,
    pub help: String,
}

pub fn lint_source(
    source: &str,
) -> Result<Vec<LintFinding>, Vec<aeris_compiler::diag::Diagnostic>> {
    let program = parser::parse(source)?;
    Ok(run_lints(&program))
}

fn run_lints(program: &Program) -> Vec<LintFinding> {
    let mut findings = Vec::new();

    if program.module.is_none() {
        findings.push(LintFinding {
            level: "warning".to_string(),
            rule: "AERIS-LINT-001".to_string(),
            message: "module declaration is missing".to_string(),
            help: "declare 'module your.app;' so package boundaries stay explicit".to_string(),
        });
    }

    for function in &program.functions {
        if !is_snake_case(&function.name) {
            findings.push(LintFinding {
                level: "warning".to_string(),
                rule: "AERIS-LINT-002".to_string(),
                message: format!("function '{}' should use snake_case", function.name),
                help: "rename the function to snake_case for consistent APIs".to_string(),
            });
        }
        if function.effect == "pure"
            && function
                .body
                .statements
                .iter()
                .any(|stmt| format!("{stmt:?}").contains("print"))
        {
            findings.push(LintFinding {
                level: "error".to_string(),
                rule: "AERIS-LINT-003".to_string(),
                message: format!("pure function '{}' appears to perform IO", function.name),
                help: "move printing/reading into an !io function".to_string(),
            });
        }
    }

    for actor in &program.actors {
        if !starts_uppercase(&actor.name) {
            findings.push(LintFinding {
                level: "warning".to_string(),
                rule: "AERIS-LINT-004".to_string(),
                message: format!("actor '{}' should use UpperCamelCase", actor.name),
                help: "reserve UpperCamelCase names for actors and algebraic variants".to_string(),
            });
        }
        if actor
            .handlers
            .iter()
            .all(|handler| !matches!(handler.pattern, Pattern::Wildcard))
        {
            findings.push(LintFinding {
                level: "note".to_string(),
                rule: "AERIS-LINT-005".to_string(),
                message: format!("actor '{}' has no fallback handler", actor.name),
                help: "consider adding 'on _ => { ... }' for unexpected messages".to_string(),
            });
        }
    }

    findings
}

fn is_snake_case(name: &str) -> bool {
    name.chars()
        .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '_')
}

fn starts_uppercase(name: &str) -> bool {
    name.chars()
        .next()
        .map(|ch| ch.is_ascii_uppercase())
        .unwrap_or(false)
}
