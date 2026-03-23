pub mod ast;
pub mod codegen;
pub mod diag;
pub mod ir;
pub mod lexer;
pub mod parser;
pub mod security;
pub mod semantic;

use anyhow::Result;
use ast::Module;
use codegen::{llvm, native, vm, wasm};
use diag::{Diagnostic, Span};
use ir::{lower_module, optimize_program, Program};
use parser::Parser;
use security::{audit_module, SecurityPolicy};
use semantic::{analyze_module, SemanticModel};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    Vm,
    Llvm,
    Wasm,
    NativeX86_64,
    NativeAarch64,
}

#[derive(Debug, Clone)]
pub struct CompileOptions {
    pub target: Target,
    pub optimize: bool,
    pub deterministic: bool,
}

impl Default for CompileOptions {
    fn default() -> Self {
        Self {
            target: Target::Vm,
            optimize: true,
            deterministic: false,
        }
    }
}

#[derive(Debug, Clone)]
pub enum Artifact {
    Vm(vm::BytecodeModule),
    Text(String),
    Binary(Vec<u8>),
}

#[derive(Debug, Clone)]
pub struct Compilation {
    pub ast: Module,
    pub semantic: SemanticModel,
    pub ir: Program,
    pub artifact: Artifact,
}

pub fn parse_source(path: &str, source: &str) -> std::result::Result<Module, Vec<Diagnostic>> {
    let tokens = lexer::lex(source);
    let mut parser = Parser::new(path, source, tokens);
    parser.parse_module()
}

fn parse_source_graph(path: &str, source: &str) -> std::result::Result<Module, Vec<Diagnostic>> {
    let entry_path = PathBuf::from(path);
    let root = resolve_workspace_root(&entry_path);
    let mut diagnostics = Vec::new();
    let mut visited = HashSet::new();
    let mut modules = Vec::new();
    load_module_recursive(
        &entry_path,
        source,
        &root,
        &mut visited,
        &mut modules,
        &mut diagnostics,
    );
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }
    Ok(merge_modules(path, modules))
}

fn load_module_recursive(
    path: &Path,
    source: &str,
    root: &Path,
    visited: &mut HashSet<PathBuf>,
    modules: &mut Vec<Module>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let key = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    if !visited.insert(key) {
        return;
    }

    let module = match parse_source(&path.display().to_string(), source) {
        Ok(module) => module,
        Err(mut parse_diags) => {
            diagnostics.append(&mut parse_diags);
            return;
        }
    };

    for use_decl in &module.uses {
        let Some(import_path) = resolve_local_module_path(root, &use_decl.path) else {
            continue;
        };
        let import_source = match fs::read_to_string(&import_path) {
            Ok(source) => source,
            Err(_) => {
                diagnostics.push(
                    Diagnostic::error(
                        format!(
                            "could not resolve local SNSX module '{}'",
                            use_decl.path.join(".")
                        ),
                        Some(use_decl.span),
                    )
                    .with_help(
                        "multi-file SNSX apps expect local modules under the project `src/app/` tree.",
                    )
                    .with_fix(format!(
                        "Create {} or remove `bring <module/{}>`.",
                        import_path.display(),
                        use_decl.path.join("/")
                    )),
                );
                continue;
            }
        };
        load_module_recursive(
            &import_path,
            &import_source,
            root,
            visited,
            modules,
            diagnostics,
        );
    }

    modules.push(module);
}

fn resolve_workspace_root(entry_path: &Path) -> PathBuf {
    for ancestor in entry_path.ancestors() {
        if ancestor.join("snsx.toml").exists() {
            return ancestor.to_path_buf();
        }
    }
    entry_path
        .parent()
        .unwrap_or(Path::new("."))
        .to_path_buf()
}

fn resolve_local_module_path(root: &Path, segments: &[String]) -> Option<PathBuf> {
    if segments.first().map(String::as_str) != Some("app") || segments.len() < 2 {
        return None;
    }
    let mut path = root.join("src");
    for segment in segments {
        path.push(segment);
    }
    path.set_extension("snsx");
    Some(path)
}

fn merge_modules(entry_path: &str, modules: Vec<Module>) -> Module {
    let mut uses = Vec::new();
    let mut items = Vec::new();
    let mut span = Span {
        start: usize::MAX,
        end: 0,
    };

    for module in modules {
        span.start = span.start.min(module.span.start);
        span.end = span.end.max(module.span.end);
        uses.extend(module.uses);
        items.extend(module.items);
    }

    if span.start == usize::MAX {
        span.start = 0;
    }

    Module {
        path: entry_path.to_string(),
        uses,
        items,
        span,
    }
}

pub fn compile_source(
    path: &str,
    source: &str,
    options: &CompileOptions,
) -> std::result::Result<Compilation, Vec<Diagnostic>> {
    compile_source_with_policy(path, source, options, &SecurityPolicy::standalone())
}

pub fn compile_source_with_policy(
    path: &str,
    source: &str,
    options: &CompileOptions,
    policy: &SecurityPolicy,
) -> std::result::Result<Compilation, Vec<Diagnostic>> {
    let ast = parse_source_graph(path, source)?;
    let semantic = analyze_module(path, &ast)?;
    let security_diagnostics = audit_module(&ast, &semantic, policy);
    if !security_diagnostics.is_empty() {
        return Err(security_diagnostics);
    }
    let mut ir = lower_module(&ast, &semantic)?;
    if options.optimize {
        optimize_program(&mut ir);
    }

    let artifact = match options.target {
        Target::Vm => Artifact::Vm(vm::emit_bytecode(&ir, &semantic)),
        Target::Llvm => Artifact::Text(llvm::emit_llvm(&ir, &semantic)),
        Target::Wasm => Artifact::Text(wasm::emit_wat(&ir, &semantic)),
        Target::NativeX86_64 => {
            Artifact::Text(native::emit_asm(&ir, &semantic, native::Arch::X86_64))
        }
        Target::NativeAarch64 => {
            Artifact::Text(native::emit_asm(&ir, &semantic, native::Arch::Aarch64))
        }
    };

    Ok(Compilation {
        ast,
        semantic,
        ir,
        artifact,
    })
}

pub fn compile_vm(
    path: &str,
    source: &str,
) -> std::result::Result<vm::BytecodeModule, Vec<Diagnostic>> {
    compile_vm_with_policy(path, source, &SecurityPolicy::standalone())
}

pub fn compile_vm_with_policy(
    path: &str,
    source: &str,
    policy: &SecurityPolicy,
) -> std::result::Result<vm::BytecodeModule, Vec<Diagnostic>> {
    let compilation = compile_source_with_policy(
        path,
        source,
        &CompileOptions {
            target: Target::Vm,
            ..CompileOptions::default()
        },
        policy,
    )?;
    match compilation.artifact {
        Artifact::Vm(module) => Ok(module),
        _ => unreachable!("vm target always returns bytecode"),
    }
}

pub fn audit_source(
    path: &str,
    source: &str,
    policy: &SecurityPolicy,
) -> std::result::Result<(Module, SemanticModel), Vec<Diagnostic>> {
    let ast = parse_source_graph(path, source)?;
    let semantic = analyze_module(path, &ast)?;
    let diagnostics = audit_module(&ast, &semantic, policy);
    if diagnostics.is_empty() {
        Ok((ast, semantic))
    } else {
        Err(diagnostics)
    }
}

pub fn emit_ir(path: &str, source: &str) -> std::result::Result<Program, Vec<Diagnostic>> {
    let ast = parse_source_graph(path, source)?;
    let semantic = analyze_module(path, &ast)?;
    lower_module(&ast, &semantic)
}

pub fn write_artifact(path: &std::path::Path, artifact: &Artifact) -> Result<()> {
    match artifact {
        Artifact::Vm(module) => {
            let bytes = serde_json::to_vec_pretty(module)?;
            std::fs::write(path, bytes)?;
        }
        Artifact::Text(text) => std::fs::write(path, text)?,
        Artifact::Binary(bytes) => std::fs::write(path, bytes)?,
    }
    Ok(())
}
