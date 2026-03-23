mod shell;
mod studio;
mod web;

use anyhow::Result;
use crossterm::{
    cursor, execute,
    terminal::{self, ClearType},
};
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use snsx_compiler::compile_vm_with_policy;
use snsx_compiler::security::SecurityPolicy;
use snsx_vm::{VirtualMachine, VmOptions, VmTrap};
use std::io::{stdout, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc::channel;

pub use studio::run_studio;
pub use web::{run_web_studio, WebStudioOptions};

#[derive(Debug, Clone)]
pub struct PreviewSnapshot {
    pub source: String,
    pub output: String,
    pub trace: Vec<String>,
}

pub fn render_once(path: &Path, options: VmOptions) -> Result<PreviewSnapshot> {
    let source = std::fs::read_to_string(path)?;
    let policy = security_policy_for_entry(path)?;
    let module = match compile_vm_with_policy(&path.display().to_string(), &source, &policy) {
        Ok(module) => module,
        Err(errs) => {
            return Ok(PreviewSnapshot {
                source,
                output: format_diagnostics(&errs),
                trace: Vec::new(),
            });
        }
    };
    let mut vm = VirtualMachine::new(module, options);
    let result = match vm.execute_main(Vec::new()) {
        Ok(result) => result,
        Err(error) => {
            if let Some(VmTrap::InputRequested(request)) = error.downcast_ref::<VmTrap>() {
                let mut output = request.stdout.clone();
                if !output.is_empty() && !output.ends_with('\n') {
                    output.push('\n');
                }
                output.push_str(&format!(
                    "Program is waiting for input line {}.",
                    request.consumed_lines + 1
                ));
                return Ok(PreviewSnapshot {
                    source,
                    output,
                    trace: request.trace.clone(),
                });
            }
            return Ok(PreviewSnapshot {
                source,
                output: format!("Runtime error\n{error}"),
                trace: Vec::new(),
            });
        }
    };
    Ok(PreviewSnapshot {
        source,
        output: result.stdout,
        trace: result.trace,
    })
}

pub fn run_live(path: &Path, options: VmOptions) -> Result<()> {
    let path = path.to_path_buf();
    draw(&path, options.clone())?;
    let (tx, rx) = channel();
    let mut watcher: RecommendedWatcher = notify::recommended_watcher(move |event| {
        let _ = tx.send(event);
    })?;
    watcher.watch(
        path.parent().unwrap_or(Path::new(".")),
        RecursiveMode::Recursive,
    )?;
    loop {
        if let Ok(event) = rx.recv() {
            if let Ok(event) = event {
                let changed = event.paths.iter().any(|changed| same_file(changed, &path));
                if changed {
                    draw(&path, options.clone())?;
                }
            }
        }
    }
}

fn draw(path: &PathBuf, options: VmOptions) -> Result<()> {
    let snapshot = render_once(path, options)?;
    let (width, _) = terminal::size().unwrap_or((120, 40));
    let column = (width as usize / 2).saturating_sub(2).max(20);
    let left = wrap_lines(&snapshot.source, column);
    let right = wrap_lines(&snapshot.output, column);
    let rows = left.len().max(right.len()).max(1);
    let mut out = stdout();
    execute!(out, terminal::Clear(ClearType::All), cursor::MoveTo(0, 0))?;
    writeln!(out, "SNSX Live Preview [{}]", path.display())?;
    writeln!(out, "{:<width$} | {}", "Source", "Output", width = column)?;
    writeln!(out, "{}-+-{}", "-".repeat(column), "-".repeat(column))?;
    for row in 0..rows {
        let l = left.get(row).cloned().unwrap_or_default();
        let r = right.get(row).cloned().unwrap_or_default();
        writeln!(out, "{:<width$} | {}", l, r, width = column)?;
    }
    if !snapshot.trace.is_empty() {
        writeln!(out, "\nTrace:")?;
        for line in snapshot.trace.iter().take(10) {
            writeln!(out, "{}", line)?;
        }
    }
    out.flush()?;
    Ok(())
}

fn wrap_lines(text: &str, width: usize) -> Vec<String> {
    let mut out = Vec::new();
    for line in text.lines() {
        if line.len() <= width {
            out.push(line.to_string());
            continue;
        }
        let mut start = 0usize;
        while start < line.len() {
            let end = (start + width).min(line.len());
            out.push(line[start..end].to_string());
            start = end;
        }
    }
    if out.is_empty() {
        out.push(String::new());
    }
    out
}

fn same_file(left: &Path, right: &Path) -> bool {
    left.file_name() == right.file_name()
}

pub fn format_diagnostics(diags: &[snsx_compiler::diag::Diagnostic]) -> String {
    diags
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n")
}

pub(crate) fn security_policy_for_entry(entry: &Path) -> Result<SecurityPolicy> {
    let Some(manifest_path) = find_manifest(entry) else {
        return Ok(SecurityPolicy::standalone());
    };
    let raw = std::fs::read_to_string(manifest_path)?;
    let manifest = raw.parse::<toml::Value>()?;
    let permissions = manifest
        .get("permissions")
        .and_then(|value| value.as_table());
    Ok(SecurityPolicy {
        strict: true,
        allow_fs: permissions
            .and_then(|table| table.get("fs"))
            .and_then(|value| value.as_bool())
            .unwrap_or(false),
        allow_network: permissions
            .and_then(|table| table.get("net"))
            .and_then(|value| value.as_bool())
            .unwrap_or(false),
        allow_ai: permissions
            .and_then(|table| table.get("ai"))
            .and_then(|value| value.as_bool())
            .unwrap_or(true),
    })
}

fn find_manifest(entry: &Path) -> Option<PathBuf> {
    let mut current = entry.parent()?.to_path_buf();
    loop {
        let candidate = current.join("snsx.toml");
        if candidate.exists() {
            return Some(candidate);
        }
        if !current.pop() {
            return None;
        }
    }
}
