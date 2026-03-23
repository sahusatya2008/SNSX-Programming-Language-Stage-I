use std::fs;
use std::path::{Path, PathBuf};

use aeris_compiler::{compile_source, CompileOptions};
use aeris_fmt::format_source;
use aeris_lint::lint_source;
use aeris_pkg::{init_project, load_manifest};
use aeris_runtime::{run, RunOptions};
use anyhow::{Context, Result};
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "aeris")]
#[command(about = "AERIS bootstrap compiler and toolchain")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Init {
        path: PathBuf,
        #[arg(long)]
        name: Option<String>,
    },
    Check {
        #[arg(long)]
        file: Option<PathBuf>,
        #[arg(long)]
        manifest: Option<PathBuf>,
    },
    Run {
        #[arg(long)]
        file: Option<PathBuf>,
        #[arg(long)]
        manifest: Option<PathBuf>,
        #[arg(long)]
        stdin_text: Option<String>,
    },
    Ir {
        #[arg(long)]
        file: PathBuf,
    },
    Build {
        #[arg(long)]
        file: PathBuf,
        #[arg(long, default_value = "build")]
        out_dir: PathBuf,
    },
    Fmt {
        #[arg(long)]
        file: PathBuf,
        #[arg(long)]
        write: bool,
    },
    Lint {
        #[arg(long)]
        file: PathBuf,
    },
    Pkg {
        #[command(subcommand)]
        command: PkgCommand,
    },
}

#[derive(Subcommand)]
enum PkgCommand {
    Manifest {
        #[arg(long, default_value = "Aeris.toml")]
        path: PathBuf,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Init { path, name } => {
            let project_name = name.unwrap_or_else(|| infer_name(&path));
            init_project(&path, &project_name)?;
            println!("Initialized AERIS project at {}", path.display());
        }
        Command::Check { file, manifest } => {
            let file = resolve_file(file, manifest)?;
            let source = read_file(&file)?;
            let compiled = compile_source(
                file.display().to_string(),
                &source,
                CompileOptions::default(),
            )
            .map_err(render_compile_failure)?;
            println!(
                "AERIS check passed: {} function(s), {} actor(s), {} type(s)",
                compiled.checked.summary.functions.len(),
                compiled.checked.summary.actors.len(),
                compiled.checked.summary.types.len()
            );
        }
        Command::Run {
            file,
            manifest,
            stdin_text,
        } => {
            let file = resolve_file(file, manifest)?;
            let source = read_file(&file)?;
            let compiled = compile_source(
                file.display().to_string(),
                &source,
                CompileOptions::default(),
            )
            .map_err(render_compile_failure)?;
            let execution = run(
                &compiled,
                RunOptions {
                    stdin_text,
                    ..RunOptions::default()
                },
            )?;
            print!("{}", execution.stdout);
            if !execution.actor_trace.is_empty() {
                for line in execution.actor_trace {
                    eprintln!("actor: {line}");
                }
            }
            eprintln!("exit: {}", execution.exit_code);
        }
        Command::Ir { file } => {
            let source = read_file(&file)?;
            let compiled = compile_source(
                file.display().to_string(),
                &source,
                CompileOptions::default(),
            )
            .map_err(render_compile_failure)?;
            println!("{}", serde_json::to_string_pretty(&compiled.ir)?);
        }
        Command::Build { file, out_dir } => {
            let source = read_file(&file)?;
            let compiled = compile_source(
                file.display().to_string(),
                &source,
                CompileOptions::default(),
            )
            .map_err(render_compile_failure)?;
            fs::create_dir_all(&out_dir)?;
            let out_file = out_dir.join("aeris.ir.json");
            fs::write(&out_file, serde_json::to_string_pretty(&compiled.ir)?)?;
            println!("Wrote {}", out_file.display());
        }
        Command::Fmt { file, write } => {
            let source = read_file(&file)?;
            let formatted = format_source(&source)?;
            if write {
                fs::write(&file, formatted)?;
            } else {
                println!("{formatted}");
            }
        }
        Command::Lint { file } => {
            let source = read_file(&file)?;
            let findings = lint_source(&source).map_err(render_compile_failure)?;
            if findings.is_empty() {
                println!("No lint findings");
            } else {
                for finding in findings {
                    println!(
                        "[{}] {}: {}\nhelp: {}\n",
                        finding.rule, finding.level, finding.message, finding.help
                    );
                }
            }
        }
        Command::Pkg { command } => match command {
            PkgCommand::Manifest { path } => {
                let manifest = load_manifest(&path)?;
                println!("{}", toml::to_string_pretty(&manifest)?);
            }
        },
    }
    Ok(())
}

fn resolve_file(file: Option<PathBuf>, manifest: Option<PathBuf>) -> Result<PathBuf> {
    if let Some(file) = file {
        return Ok(file);
    }
    let manifest_path = manifest.unwrap_or_else(|| PathBuf::from("Aeris.toml"));
    let manifest = load_manifest(&manifest_path)?;
    let root = manifest_path.parent().unwrap_or_else(|| Path::new("."));
    Ok(root.join(manifest.package.entry))
}

fn read_file(path: &Path) -> Result<String> {
    fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))
}

fn infer_name(path: &Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("aeris_app")
        .replace('-', "_")
}

fn render_compile_failure(diag: Vec<aeris_compiler::diag::Diagnostic>) -> anyhow::Error {
    let mut rendered = String::new();
    for item in diag {
        rendered.push_str(&format!("[{}] {}\n", item.code, item.message));
        if let Some(help) = item.help {
            rendered.push_str(&format!("help: {help}\n"));
        }
        if let Some(detail) = item.detail {
            rendered.push_str(&format!("detail: {detail}\n"));
        }
        if let Some(span) = item.span {
            rendered.push_str(&format!("at line {}, column {}\n", span.line, span.column));
        }
        rendered.push('\n');
    }
    anyhow::anyhow!(rendered)
}
