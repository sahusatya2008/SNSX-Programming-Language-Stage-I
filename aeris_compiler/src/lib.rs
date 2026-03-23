pub mod ast;
pub mod diag;
pub mod ir;
pub mod lexer;
pub mod parser;
pub mod semantic;

use serde::{Deserialize, Serialize};

use crate::diag::Diagnostic;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CompileOptions {
    pub require_main: bool,
}

impl Default for CompileOptions {
    fn default() -> Self {
        Self { require_main: true }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CompiledUnit {
    pub source_name: String,
    pub checked: semantic::CheckedProgram,
    pub ir: ir::IrProgram,
}

pub fn compile_source(
    source_name: impl Into<String>,
    source: &str,
    options: CompileOptions,
) -> Result<CompiledUnit, Vec<Diagnostic>> {
    let program = parser::parse(source)?;
    let checked = semantic::analyze(program, options.require_main)?;
    let ir = ir::lower(&checked.program);
    Ok(CompiledUnit {
        source_name: source_name.into(),
        checked,
        ir,
    })
}
