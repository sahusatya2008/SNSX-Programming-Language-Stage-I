use crate::security_policy_for_entry;
use anyhow::{bail, Context, Result};
use snsx_ai_engine::{GeneratedSnsxProgram, SnsxCodingAgent};
use snsx_compiler::{audit_source, Target};
use snsx_stdlib::ModuleRegistry;
use std::fs;
use std::path::{Component, Path, PathBuf};

#[derive(Debug, Clone)]
pub enum ShellCommand {
    Help,
    Tools,
    Docs,
    Manual,
    Spec,
    TechSpec,
    History,
    Status,
    PermissionsShow,
    DependenciesShow,
    Agent(String),
    AgentWrite { path: String, prompt: String },
    Run,
    Build(Target),
    Audit,
    Doctor,
    Clear,
    Open(String),
    Pwd,
    Ls(Option<String>),
    Tree(Option<String>),
    Find(String),
    Search(String),
    Outline(Option<String>),
    Cat(String),
    Touch(String),
    Mkdir(String),
    Move { from: String, to: String },
    Copy { from: String, to: String },
    Remove(String),
    EntryShow,
    EntrySet(String),
    PackageList,
    PackageAdd(String),
    PackageRemove(String),
    ModuleList,
    ModuleInstall(String),
    ModuleScaffold(String),
    SnippetList,
    SnippetCreate { kind: String, path: String },
    ManifestShow,
    PermSet { name: String, enabled: bool },
    Host(String),
}

#[derive(Debug, Clone, Copy)]
pub struct SnippetInfo {
    pub name: &'static str,
    pub description: &'static str,
}

#[derive(Debug, Clone, Copy)]
struct PackageRecipe {
    name: &'static str,
    description: &'static str,
    modules: &'static [&'static str],
    allow_fs: bool,
    allow_network: bool,
    allow_ai: bool,
}

const PACKAGE_RECIPES: &[PackageRecipe] = &[
    PackageRecipe {
        name: "foundation",
        description: "Base console and math surfaces for most SNSX projects.",
        modules: &["std.io", "std.math"],
        allow_fs: false,
        allow_network: false,
        allow_ai: false,
    },
    PackageRecipe {
        name: "ai-lab",
        description: "AI-native composition with math and design helpers.",
        modules: &["std.ai", "std.math", "std.design"],
        allow_fs: false,
        allow_network: false,
        allow_ai: true,
    },
    PackageRecipe {
        name: "secure-core",
        description: "Strict error handling and sentinel guard rails.",
        modules: &["std.error", "std.sentinel"],
        allow_fs: false,
        allow_network: false,
        allow_ai: false,
    },
    PackageRecipe {
        name: "web-console",
        description: "Web and design surfaces for browser-style output.",
        modules: &["std.web", "std.design"],
        allow_fs: false,
        allow_network: false,
        allow_ai: false,
    },
    PackageRecipe {
        name: "network-grid",
        description: "Network, security, and error stack for distributed work.",
        modules: &["std.net", "std.error", "std.sentinel"],
        allow_fs: false,
        allow_network: true,
        allow_ai: false,
    },
];

fn import_surface(module: &str) -> &'static str {
    match module {
        "std.design" | "std.sentinel" => "lib",
        "std.error" => "package",
        _ => "module",
    }
}

const SNIPPETS: &[SnippetInfo] = &[
    SnippetInfo {
        name: "starter",
        description: "Minimal SNSX entry program for a new file.",
    },
    SnippetInfo {
        name: "mind",
        description: "AI-native prompt flow plus a starter entry path.",
    },
    SnippetInfo {
        name: "service",
        description: "Typed reusable flows with validation and orchestration.",
    },
    SnippetInfo {
        name: "design",
        description: "Structured board/panel composition using std.design.",
    },
    SnippetInfo {
        name: "worker",
        description: "Concurrent task launch and wait template.",
    },
    SnippetInfo {
        name: "tensor",
        description: "Tensor and matrix workbench template.",
    },
];

const BLOCKED_HOST_SHELL_PREFIXES: &[&str] = &[
    "rm ",
    "rm\t",
    "sudo ",
    "sudo\t",
    "dd ",
    "mkfs",
    "shutdown",
    "reboot",
    "halt",
    "poweroff",
    "kill ",
    "kill\t",
    "killall ",
    "killall\t",
    "pkill ",
    "pkill\t",
    "launchctl ",
    "diskutil erase",
];

pub fn parse_shell_command(input: &str) -> Result<ShellCommand> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        bail!("SNSX shell command cannot be empty");
    }

    let (internal_only, command) = if let Some(rest) = trimmed.strip_prefix(':') {
        (true, rest.trim())
    } else if let Some(rest) = trimmed.strip_prefix("snsx ") {
        (true, rest.trim())
    } else {
        (false, trimmed)
    };

    if let Some(rest) = command.strip_prefix('!') {
        let rest = rest.trim();
        if rest.is_empty() {
            bail!("host shell escape cannot be empty");
        }
        return Ok(ShellCommand::Host(rest.to_string()));
    }
    if let Some(rest) = command.strip_prefix("shell ") {
        let rest = rest.trim();
        if rest.is_empty() {
            bail!("shell expects a command after 'shell'");
        }
        return Ok(ShellCommand::Host(rest.to_string()));
    }

    let mut parts = command.split_whitespace();
    let Some(head) = parts.next() else {
        bail!("SNSX shell command cannot be empty");
    };

    let shell_command = match head {
        "help" | "?" => ShellCommand::Help,
        "tools" | "toolbox" => ShellCommand::Tools,
        "docs" => ShellCommand::Docs,
        "manual" => ShellCommand::Manual,
        "spec" => ShellCommand::Spec,
        "techspec" | "technical-spec" => ShellCommand::TechSpec,
        "history" => ShellCommand::History,
        "status" | "state" => ShellCommand::Status,
        "permissions" | "policy" => ShellCommand::PermissionsShow,
        "deps" | "dependencies" => ShellCommand::DependenciesShow,
        "agent" | "ai" => match parts.next() {
            Some("write") => {
                let prefix = format!("{head} write");
                let (path, prompt) =
                    parse_path_and_rest(command, &prefix, "agent write <path> <prompt>")?;
                ShellCommand::AgentWrite { path, prompt }
            }
            Some(_) => ShellCommand::Agent(expect_rest(command, head, "agent <prompt>")?),
            None => bail!("agent expects a prompt or `agent write <path> <prompt>`"),
        },
        "run" => ShellCommand::Run,
        "audit" | "security" => ShellCommand::Audit,
        "doctor" => ShellCommand::Doctor,
        "clear" => ShellCommand::Clear,
        "pwd" => ShellCommand::Pwd,
        "build" => {
            let target = parts
                .next()
                .map(parse_target_name)
                .transpose()?
                .unwrap_or(Target::Vm);
            ShellCommand::Build(target)
        }
        "open" => ShellCommand::Open(expect_rest(command, head, "open <path>")?),
        "ls" => ShellCommand::Ls(optional_rest(command, head)),
        "tree" => ShellCommand::Tree(optional_rest(command, head)),
        "find" => ShellCommand::Find(expect_rest(command, head, "find <path-fragment>")?),
        "search" | "grep" => {
            ShellCommand::Search(expect_rest(command, head, "search <text>")?)
        }
        "outline" => ShellCommand::Outline(optional_rest(command, head)),
        "cat" => ShellCommand::Cat(expect_rest(command, head, "cat <path>")?),
        "touch" => ShellCommand::Touch(expect_rest(command, head, "touch <path>")?),
        "mkdir" => ShellCommand::Mkdir(expect_rest(command, head, "mkdir <path>")?),
        "mv" | "move" => {
            let (from, to) = parse_two_args(command, head, "mv <from> <to>")?;
            ShellCommand::Move { from, to }
        }
        "cp" | "copy" => {
            let (from, to) = parse_two_args(command, head, "cp <from> <to>")?;
            ShellCommand::Copy { from, to }
        }
        "rm" | "delete" => ShellCommand::Remove(expect_rest(command, head, "rm <path>")?),
        "entry" => match optional_rest(command, head) {
            Some(path) => ShellCommand::EntrySet(path),
            None => ShellCommand::EntryShow,
        },
        "manifest" => ShellCommand::ManifestShow,
        "install" => match parts.next() {
            Some("package") | Some("pkg") => {
                ShellCommand::PackageAdd(expect_nth_rest(command, 2, "install package <name>")?)
            }
            Some("module") | Some("mod") | Some("lib") => {
                ShellCommand::ModuleInstall(expect_nth_rest(command, 2, "install module <name>")?)
            }
            Some(_) => ShellCommand::PackageAdd(expect_rest(command, head, "install <package>")?),
            None => bail!("install expects a package or module name"),
        },
        "uninstall" | "remove" => match parts.next() {
            Some("package") | Some("pkg") => ShellCommand::PackageRemove(expect_nth_rest(
                command,
                2,
                "remove package <name>",
            )?),
            Some("module") | Some("mod") | Some("lib") => ShellCommand::PackageRemove(
                expect_nth_rest(command, 2, "remove module <name>")?,
            ),
            Some(_) => ShellCommand::PackageRemove(expect_rest(command, head, "remove <name>")?),
            None => bail!("remove expects a package or module name"),
        },
        "perm" | "permission" => {
            let (name, state) = parse_two_args(command, head, "perm <fs|net|ai> <on|off>")?;
            ShellCommand::PermSet {
                name,
                enabled: parse_permission_state(&state)?,
            }
        }
        "new" | "create" => match parts.next() {
            Some("file") => ShellCommand::Touch(expect_nth_rest(command, 2, "new file <path>")?),
            Some("folder") | Some("dir") | Some("directory") => {
                ShellCommand::Mkdir(expect_nth_rest(command, 2, "new folder <path>")?)
            }
            Some(other) => {
                bail!("unknown create target '{other}'. Use `new file <path>` or `new folder <path>`.")
            }
            None => bail!("new expects `file` or `folder`"),
        },
        "package" | "pkg" => match parts.next() {
            Some("list") => ShellCommand::PackageList,
            Some("add") => ShellCommand::PackageAdd(expect_nth_rest(command, 2, "package add <name>")?),
            Some("remove") | Some("rm") => {
                ShellCommand::PackageRemove(expect_nth_rest(command, 2, "package remove <name>")?)
            }
            Some(other) => bail!(
                "unknown package action '{other}'. Use `package list`, `package add`, or `package remove`."
            ),
            None => bail!("package expects `list`, `add`, or `remove`"),
        },
        "module" | "mod" => match parts.next() {
            Some("list") => ShellCommand::ModuleList,
            Some("install") => {
                ShellCommand::ModuleInstall(expect_nth_rest(command, 2, "module install <name>")?)
            }
            Some("scaffold") => ShellCommand::ModuleScaffold(expect_nth_rest(
                command,
                2,
                "module scaffold <name>",
            )?),
            Some(other) => bail!(
                "unknown module action '{other}'. Use `module list`, `module install`, or `module scaffold`."
            ),
            None => bail!("module expects `list`, `install`, or `scaffold`"),
        },
        "snippet" => match parts.next() {
            Some("list") => ShellCommand::SnippetList,
            Some("create") => {
                let (kind, path) =
                    parse_two_args_after(command, 2, "snippet create <kind> <path>")?;
                ShellCommand::SnippetCreate { kind, path }
            }
            Some(other) => bail!(
                "unknown snippet action '{other}'. Use `snippet list` or `snippet create`."
            ),
            None => bail!("snippet expects `list` or `create`"),
        },
        other => {
            if internal_only {
                bail!("unknown SNSX shell command '{}'", other);
            }
            return Ok(ShellCommand::Host(command.to_string()));
        }
    };

    Ok(shell_command)
}

pub fn shell_help_lines() -> Vec<String> {
    vec![
        "SNSX shell commands:".to_string(),
        "help | tools | status | permissions | deps | docs | manual | spec | techspec | history"
            .to_string(),
        "agent <prompt> | agent write <path> <prompt>".to_string(),
        "run | build [vm|wasm|llvm|native-x86_64|native-aarch64] | audit | security | doctor"
            .to_string(),
        "open <path> | entry [path] | manifest".to_string(),
        "pwd | ls [path] | tree [path] | cat <path>".to_string(),
        "find <path-fragment> | search <text> | outline [path]".to_string(),
        "touch <path> | mkdir <path> | mv <from> <to> | cp <from> <to> | rm <path>".to_string(),
        "package list | package add <name> | package remove <name> | install <package>".to_string(),
        "module list | module install <name> | module scaffold <name> | install module <name>"
            .to_string(),
        "snippet list | snippet create <starter|mind|service|design|worker|tensor> <path>"
            .to_string(),
        "perm <fs|net|ai> <on|off>".to_string(),
        "shell <cmd> or !<cmd> runs a guarded host shell command inside the workspace.".to_string(),
    ]
}

pub fn shell_tool_lines() -> Vec<String> {
    vec![
        "SNSX terminal toolset:".to_string(),
        "Workspace tools: ls, tree, find, search, outline, open, cat, touch, mkdir, mv, cp, rm"
            .to_string(),
        "Project tools: run, build, audit, doctor, status, manifest, entry".to_string(),
        "Security tools: permissions, perm <name> <on|off>, audit, doctor".to_string(),
        "Package tools: package list/add/remove, module list/install/scaffold, install module ..."
            .to_string(),
        "Authoring tools: snippet list/create, agent <prompt>, agent write <path> <prompt>"
            .to_string(),
        "Host bridge: shell <cmd> or !<cmd> for guarded host-shell work inside the workspace"
            .to_string(),
    ]
}

pub fn documentation_lines() -> Vec<String> {
    let root = repo_root();
    vec![
        "SNSX documentation surfaces:".to_string(),
        format!("manual    {}", manual_doc_path().display()),
        format!("spec      {}", spec_doc_path().display()),
        format!("techspec  {}", technical_spec_doc_path().display()),
        format!("readme    {}", root.join("README.md").display()),
        "Web studio routes: /docs/manual /docs/spec /docs/techspec /docs/readme".to_string(),
    ]
}

pub fn agent_preview(prompt: &str) -> Result<GeneratedSnsxProgram> {
    SnsxCodingAgent::generate(prompt)
}

pub fn agent_apply(
    root: &Path,
    entry: &Path,
    base: &Path,
    prompt: &str,
    target_path: Option<&str>,
) -> Result<(GeneratedSnsxProgram, PathBuf, Vec<String>)> {
    let artifact = agent_preview(prompt)?;
    let relative = target_path
        .map(str::trim)
        .filter(|path| !path.is_empty())
        .unwrap_or(&artifact.suggested_path);
    let mut relative = relative.replace('\\', "/");
    if !relative.ends_with(".snsx") {
        relative.push_str(".snsx");
    }
    let target = resolve_workspace_input(root, base, &relative)?;
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)?;
    }

    let mut manifest = ensure_manifest(root, entry)?;
    let mut lines = vec![
        format!("SNS AI recipe {}", artifact.recipe),
        format!("Target {}", workspace_path(root, &target)),
        format!("Title {}", artifact.title),
        format!("Generated files {}", artifact.files.len().max(1)),
    ];

    for dependency in &artifact.dependencies {
        lines.push(install_module_into_workspace(
            root,
            dependency,
            &mut manifest,
        )?);
    }

    let permissions = table_mut(&mut manifest, "permissions")?;
    if artifact.permissions.fs {
        permissions.insert("fs".to_string(), toml::Value::Boolean(true));
    }
    if artifact.permissions.net {
        permissions.insert("net".to_string(), toml::Value::Boolean(true));
    }
    if artifact.permissions.ai {
        permissions.insert("ai".to_string(), toml::Value::Boolean(true));
    }
    write_manifest(root, &manifest)?;

    write_agent_artifact(root, &target, &artifact)?;

    lines.push("SNS AI coding agent generated and wrote the SNSX source file.".to_string());
    lines.push(format!("Summary {}", artifact.summary));
    for file in &artifact.files {
        lines.push(format!("File {}", file.path));
    }
    for note in &artifact.notes {
        lines.push(format!("Note {note}"));
    }
    Ok((artifact, target, lines))
}

fn write_agent_artifact(root: &Path, target: &Path, artifact: &GeneratedSnsxProgram) -> Result<()> {
    let mut wrote_entry = false;
    for file in &artifact.files {
        let destination = if file.path == artifact.suggested_path {
            wrote_entry = true;
            target.to_path_buf()
        } else {
            root.join(&file.path)
        };
        write_generated_code(&destination, &file.code)?;
    }
    if !wrote_entry {
        write_generated_code(target, &artifact.code)?;
    }
    Ok(())
}

fn write_generated_code(path: &Path, code: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut content = code.to_string();
    if !content.ends_with('\n') {
        content.push('\n');
    }
    fs::write(path, content)?;
    Ok(())
}

pub fn manual_doc_path() -> PathBuf {
    repo_root().join("docs").join("SNSX_Training_Manual.md")
}

pub fn spec_doc_path() -> PathBuf {
    repo_root().join("spec").join("SNSX.md")
}

pub fn technical_spec_doc_path() -> PathBuf {
    repo_root()
        .join("docs")
        .join("SNSX_Technical_Specification.md")
}

pub fn readme_doc_path() -> PathBuf {
    repo_root().join("README.md")
}

pub fn validate_host_shell_command(command: &str) -> Result<()> {
    let normalized = command.trim().to_lowercase();
    if normalized.is_empty() {
        bail!("host shell command cannot be empty");
    }
    for prefix in BLOCKED_HOST_SHELL_PREFIXES {
        let trimmed = prefix.trim_end();
        if normalized == trimmed || normalized.starts_with(prefix) {
            bail!(
                "host shell blocked by SNSX terminal policy: use the SNSX workspace command for this action instead"
            );
        }
    }
    Ok(())
}

pub fn snippet_catalog() -> &'static [SnippetInfo] {
    SNIPPETS
}

pub fn list_directory(root: &Path, base: &Path, input: Option<&str>) -> Result<Vec<String>> {
    let target = match input {
        Some(path) if !path.trim().is_empty() => resolve_workspace_input(root, base, path)?,
        _ => base.to_path_buf(),
    };
    if !target.exists() {
        bail!("path does not exist: {}", workspace_path(root, &target));
    }
    match workspace_node_kind(&target) {
        WorkspaceNodeKind::File => {
            let metadata = fs::metadata(&target)?;
            return Ok(vec![
                format!("file {}", workspace_path(root, &target)),
                format!("bytes {}", metadata.len()),
            ]);
        }
        WorkspaceNodeKind::Other => {
            return Ok(vec![
                format!("node {}", workspace_path(root, &target)),
                "special filesystem entry (not editable as a regular SNSX file)".to_string(),
            ]);
        }
        WorkspaceNodeKind::Dir => {}
    }

    let mut entries = fs::read_dir(&target)?
        .filter_map(|entry| entry.ok())
        .collect::<Vec<_>>();
    entries.sort_by_key(|entry| {
        (
            !matches!(workspace_node_kind(&entry.path()), WorkspaceNodeKind::Dir),
            entry.file_name().to_string_lossy().to_lowercase(),
        )
    });

    let mut lines = vec![format!("Listing {}", workspace_path(root, &target))];
    for entry in entries {
        let label = match workspace_node_kind(&entry.path()) {
            WorkspaceNodeKind::Dir => "dir ",
            WorkspaceNodeKind::File => "file",
            WorkspaceNodeKind::Other => "node",
        };
        lines.push(format!("{} {}", label, workspace_path(root, &entry.path())));
    }
    if lines.len() == 1 {
        lines.push("(empty)".to_string());
    }
    Ok(lines)
}

pub fn render_tree(root: &Path, base: &Path, input: Option<&str>) -> Result<Vec<String>> {
    let target = match input {
        Some(path) if !path.trim().is_empty() => resolve_workspace_input(root, base, path)?,
        _ => base.to_path_buf(),
    };
    if !target.exists() {
        bail!("path does not exist: {}", workspace_path(root, &target));
    }

    let mut lines = Vec::new();
    append_tree_lines(root, &target, 0, &mut lines)?;
    Ok(lines)
}

pub fn find_workspace_paths(root: &Path, query: &str) -> Result<Vec<String>> {
    let needle = query.trim().to_lowercase();
    if needle.is_empty() {
        bail!("find expects a non-empty path fragment");
    }

    let mut all_paths = Vec::new();
    collect_workspace_paths(root, root, &mut all_paths)?;
    let mut hits = all_paths
        .into_iter()
        .filter_map(|path| {
            let label = workspace_path(root, &path);
            let file_name = path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or_default()
                .to_lowercase();
            let label_lower = label.to_lowercase();
            if file_name.contains(&needle) || label_lower.contains(&needle) {
                let kind = match workspace_node_kind(&path) {
                    WorkspaceNodeKind::Dir => "dir ",
                    WorkspaceNodeKind::File => "file",
                    WorkspaceNodeKind::Other => "node",
                };
                Some((label_lower, format!("{kind} {label}")))
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    hits.sort_by(|left, right| left.0.cmp(&right.0));

    let mut lines = vec![format!("File matches for '{query}':")];
    if hits.is_empty() {
        lines.push("(no matches)".to_string());
    } else {
        for (_, label) in hits.into_iter().take(120) {
            lines.push(label);
        }
    }
    Ok(lines)
}

pub fn search_workspace_text(root: &Path, query: &str) -> Result<Vec<String>> {
    let needle = query.trim().to_lowercase();
    if needle.is_empty() {
        bail!("search expects non-empty text");
    }

    let mut matches = Vec::new();
    collect_text_matches(root, root, &needle, query, &mut matches)?;
    let mut lines = vec![format!("Content matches for '{query}':")];
    if matches.is_empty() {
        lines.push("(no matches)".to_string());
    } else {
        lines.extend(matches.into_iter().take(160));
    }
    Ok(lines)
}

pub fn outline_workspace_file(root: &Path, path: &Path) -> Result<Vec<String>> {
    match workspace_node_kind(path) {
        WorkspaceNodeKind::File => {}
        WorkspaceNodeKind::Dir => bail!("outline expects a file, not a folder"),
        WorkspaceNodeKind::Other => bail!("outline expects a regular file"),
    }
    let raw = fs::read_to_string(path)
        .with_context(|| format!("failed to read {}", workspace_path(root, path)))?;
    let mut lines = vec![format!("Outline for {}", workspace_path(root, path))];
    let mut found = 0usize;
    for (index, line) in raw.lines().enumerate() {
        let trimmed = line.trim();
        let label = if let Some(name) = trimmed.strip_prefix("need ") {
            Some(format!("{:>4} import  {}", index + 1, name.trim()))
        } else if let Some(name) = trimmed.strip_prefix("bring ") {
            Some(format!("{:>4} import  {}", index + 1, name.trim()))
        } else if let Some(name) = trimmed.strip_prefix("bundle ") {
            Some(format!("{:>4} bundle  {}", index + 1, name.trim()))
        } else if let Some(name) = trimmed.strip_prefix("flow ") {
            Some(format!("{:>4} flow    {}", index + 1, name.trim()))
        } else if let Some(name) = trimmed.strip_prefix("mind ") {
            Some(format!("{:>4} mind    {}", index + 1, name.trim()))
        } else if let Some(name) = trimmed.strip_prefix("later ") {
            Some(format!("{:>4} later   {}", index + 1, name.trim()))
        } else if let Some(name) = trimmed.strip_prefix("agent ") {
            Some(format!(
                "{:>4} agent   {}",
                index + 1,
                name.trim_end_matches(':').trim()
            ))
        } else if trimmed == "entry" || trimmed.starts_with("entry ") {
            Some(format!("{:>4} entry", index + 1))
        } else if trimmed.starts_with("guard ") {
            Some(format!("{:>4} guard", index + 1))
        } else {
            None
        };
        if let Some(label) = label {
            lines.push(label);
            found += 1;
        }
    }
    if found == 0 {
        lines.push("No SNSX declarations detected in this file.".to_string());
    }
    Ok(lines)
}

pub fn doctor_report(root: &Path, entry: &Path) -> Result<Vec<String>> {
    let manifest = root.join("snsx.toml");
    let dependencies = manifest_dependencies(root).unwrap_or_default();
    let (dirs, files, snsx_files) = workspace_counts(root, root)?;
    let policy = security_policy_for_entry(entry)?;
    let source = fs::read_to_string(entry)
        .with_context(|| format!("failed to read {}", workspace_path(root, entry)))?;
    let audit = match audit_source(&entry.display().to_string(), &source, &policy) {
        Ok(_) => vec![
            "Audit: clean".to_string(),
            "Compiler gate: unlocked".to_string(),
        ],
        Err(diags) => vec![
            format!("Audit: {} blocker(s)", diags.len()),
            "Compiler gate: locked until blockers are fixed".to_string(),
            format!("Top blocker: {}", diags[0]),
        ],
    };

    let mut lines = vec![
        "SNSX workspace doctor".to_string(),
        format!("Root: {}", root.display()),
        format!("Entry: {}", workspace_path(root, entry)),
        format!(
            "Manifest: {}",
            if manifest.exists() {
                "present"
            } else {
                "missing"
            }
        ),
        format!(
            "Permissions: strict | fs {} | net {} | ai {}",
            on_off(policy.allow_fs),
            on_off(policy.allow_network),
            on_off(policy.allow_ai)
        ),
        format!("Workspace: {dirs} folder(s), {files} file(s), {snsx_files} .snsx source file(s)"),
        format!("Dependencies tracked: {}", dependencies.len()),
    ];
    if dependencies.is_empty() {
        lines.push("Dependencies: std only".to_string());
    } else {
        lines.push(format!("Dependencies: {}", dependencies.join(", ")));
    }
    lines.push(String::new());
    lines.extend(audit);
    lines.push(String::new());
    lines.push("Useful next commands:".to_string());
    lines.push("  outline".to_string());
    lines.push("  find main".to_string());
    lines.push("  search guard".to_string());
    lines.push("  snippet list".to_string());
    Ok(lines)
}

pub fn terminal_status_report(root: &Path, entry: &Path) -> Result<Vec<String>> {
    let manifest = root.join("snsx.toml");
    let dependencies = manifest_dependencies(root).unwrap_or_default();
    let (dirs, files, snsx_files) = workspace_counts(root, root)?;
    let policy = security_policy_for_entry(entry)?;
    let mut lines = vec![
        "SNSX terminal status".to_string(),
        format!("Root: {}", root.display()),
        format!("Entry: {}", workspace_path(root, entry)),
        format!(
            "Manifest: {}",
            if manifest.exists() {
                "present"
            } else {
                "missing"
            }
        ),
        format!(
            "Permissions: strict | fs {} | net {} | ai {}",
            on_off(policy.allow_fs),
            on_off(policy.allow_network),
            on_off(policy.allow_ai)
        ),
        format!("Workspace: {dirs} folder(s), {files} file(s), {snsx_files} .snsx source file(s)"),
        format!(
            "Dependencies: {}",
            if dependencies.is_empty() {
                "std only".to_string()
            } else {
                dependencies.join(", ")
            }
        ),
    ];
    lines.push(String::new());
    lines.push("Suggested terminal commands:".to_string());
    lines.push("  run".to_string());
    lines.push("  audit".to_string());
    lines.push("  package list".to_string());
    lines.push("  module list".to_string());
    Ok(lines)
}

pub fn permissions_report(entry: &Path) -> Result<Vec<String>> {
    let policy = security_policy_for_entry(entry)?;
    Ok(vec![
        "SNSX permission policy".to_string(),
        format!("Entry: {}", entry.display()),
        format!("Strict mode: {}", if policy.strict { "on" } else { "off" }),
        format!("Filesystem: {}", on_off(policy.allow_fs)),
        format!("Network: {}", on_off(policy.allow_network)),
        format!("AI: {}", on_off(policy.allow_ai)),
        "Use `perm fs on`, `perm net on`, or `perm ai on` to update the workspace policy."
            .to_string(),
    ])
}

pub fn dependencies_report(root: &Path) -> Result<Vec<String>> {
    let registry = ModuleRegistry::new();
    let installed = manifest_dependencies(root).unwrap_or_default();
    let mut lines = vec!["SNSX dependency report".to_string()];
    if installed.is_empty() {
        lines.push("Manifest dependencies: std only".to_string());
    } else {
        lines.push(format!("Manifest dependencies: {}", installed.join(", ")));
    }
    lines.push(String::new());
    lines.push("Built-in module availability:".to_string());
    for name in registry.names() {
        let vendor = vendored_module_path(root, name);
        let state = if vendor.exists() {
            format!("vendored at {}", workspace_path(root, &vendor))
        } else if installed.iter().any(|dep| dep == name) {
            "tracked in manifest".to_string()
        } else {
            "available".to_string()
        };
        lines.push(format!("{:<16} {}", name, state));
    }
    Ok(lines)
}

pub fn scaffold_snippet(root: &Path, base: &Path, kind: &str, path: &str) -> Result<PathBuf> {
    let mut relative = path.trim().replace('\\', "/");
    if relative.is_empty() {
        bail!("snippet create expects a target path");
    }
    if !relative.ends_with(".snsx") {
        relative.push_str(".snsx");
    }
    let target = resolve_workspace_input(root, base, &relative)?;
    if target.exists() {
        bail!(
            "snippet target already exists: {}",
            workspace_path(root, &target)
        );
    }
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)?;
    }

    let stem = target
        .file_stem()
        .and_then(|name| name.to_str())
        .unwrap_or("snippet");
    let symbol = sanitize_symbol_name(stem);
    let title = stem.replace('_', " ");
    let template = snippet_template(kind, &symbol, &title)?;
    fs::write(&target, template)?;
    Ok(target)
}

pub fn read_workspace_file(root: &Path, base: &Path, input: &str) -> Result<String> {
    let path = resolve_workspace_input(root, base, input)?;
    if path.is_dir() {
        bail!("cat expects a file, not a folder");
    }
    fs::read_to_string(&path)
        .with_context(|| format!("failed to read {}", workspace_path(root, &path)))
}

pub fn copy_workspace_path(root: &Path, base: &Path, from: &str, to: &str) -> Result<Vec<String>> {
    let from_path = resolve_workspace_input(root, base, from)?;
    if !from_path.exists() {
        bail!(
            "copy source does not exist: {}",
            workspace_path(root, &from_path)
        );
    }
    let to_path = resolve_workspace_input(root, root, to)?;
    if to_path.exists() {
        bail!(
            "copy target already exists: {}",
            workspace_path(root, &to_path)
        );
    }
    if let Some(parent) = to_path.parent() {
        fs::create_dir_all(parent)?;
    }
    copy_recursively(&from_path, &to_path)?;
    Ok(vec![format!(
        "Copied {} to {}",
        workspace_path(root, &from_path),
        workspace_path(root, &to_path)
    )])
}

pub fn install_package(root: &Path, entry: &Path, spec: &str) -> Result<Vec<String>> {
    let spec = spec.trim();
    if spec.is_empty() {
        bail!("package name cannot be empty");
    }

    let mut manifest = ensure_manifest(root, entry)?;
    let mut lines = Vec::new();
    if let Some(recipe) = package_recipe(spec) {
        lines.push(format!("Installing package {}", recipe.name));
        lines.push(recipe.description.to_string());
        for module in recipe.modules {
            let installed = install_module_into_workspace(root, module, &mut manifest)?;
            lines.push(installed);
        }
        apply_recipe_permissions(&mut manifest, recipe)?;
        lines.push("Recommended import:".to_string());
        for module in recipe.modules {
            lines.push(format!("bring <{}/{module}>", import_surface(module)));
        }
        write_manifest(root, &manifest)?;
        lines.push("Manifest updated.".to_string());
        return Ok(lines);
    }

    let installed = install_module_into_workspace(root, spec, &mut manifest)?;
    lines.push(installed);
    write_manifest(root, &manifest)?;
    lines.push("Manifest updated.".to_string());
    Ok(lines)
}

pub fn remove_package(root: &Path, entry: &Path, spec: &str) -> Result<Vec<String>> {
    let spec = spec.trim();
    if spec.is_empty() {
        bail!("package name cannot be empty");
    }

    let mut manifest = ensure_manifest(root, entry)?;
    let registry = ModuleRegistry::new();
    let mut lines = Vec::new();
    let modules = if let Some(recipe) = package_recipe(spec) {
        recipe.modules.to_vec()
    } else if registry.source(spec).is_ok() {
        vec![spec]
    } else {
        bail!("unknown SNSX package or module '{}'", spec);
    };

    let dependencies = table_mut(&mut manifest, "dependencies")?;
    for module in modules {
        dependencies.remove(module);
        let vendor = vendored_module_path(root, module);
        if vendor.exists() {
            if vendor.is_dir() {
                fs::remove_dir_all(&vendor)?;
            } else {
                fs::remove_file(&vendor)?;
            }
            prune_empty_ancestors(vendor.parent(), &root.join("vendor"))?;
            lines.push(format!("Removed {}", workspace_path(root, &vendor)));
        } else {
            lines.push(format!("Dependency {} removed from manifest", module));
        }
    }
    write_manifest(root, &manifest)?;
    lines.push("Manifest updated.".to_string());
    Ok(lines)
}

pub fn list_packages(root: &Path) -> Result<Vec<String>> {
    let installed = manifest_dependencies(root).unwrap_or_default();
    let registry = ModuleRegistry::new();
    let mut lines = vec!["Built-in SNSX packages:".to_string()];
    for recipe in PACKAGE_RECIPES {
        lines.push(format!("bundle {:<14} {}", recipe.name, recipe.description));
    }
    lines.push(String::new());
    lines.push("Installable stdlib modules:".to_string());
    for name in registry.names() {
        let marker = if installed.iter().any(|dep| dep == name) {
            "installed"
        } else {
            "available"
        };
        lines.push(format!("module {:<16} {}", name, marker));
    }
    Ok(lines)
}

pub fn list_modules(root: &Path) -> Result<Vec<String>> {
    let registry = ModuleRegistry::new();
    let installed = manifest_dependencies(root).unwrap_or_default();
    let mut lines = vec!["SNSX modules:".to_string()];
    for name in registry.names() {
        let vendor = vendored_module_path(root, name);
        let state = if vendor.exists() {
            format!("vendored at {}", workspace_path(root, &vendor))
        } else if installed.iter().any(|dep| dep == name) {
            "tracked in manifest".to_string()
        } else {
            "not installed".to_string()
        };
        lines.push(format!("{:<16} {}", name, state));
    }
    Ok(lines)
}

pub fn scaffold_module(root: &Path, base: &Path, name: &str) -> Result<PathBuf> {
    let mut relative = name.trim().replace('\\', "/");
    if relative.is_empty() {
        bail!("module scaffold expects a module name or path");
    }
    if !relative.ends_with(".snsx") {
        relative.push_str(".snsx");
    }
    if !relative.starts_with("modules/") {
        relative = format!("modules/{relative}");
    }
    let target = resolve_workspace_input(root, base, &relative)?;
    if target.exists() {
        bail!(
            "module scaffold target already exists: {}",
            workspace_path(root, &target)
        );
    }
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)?;
    }
    let stem = target
        .file_stem()
        .and_then(|name| name.to_str())
        .unwrap_or("module");
    let symbol = sanitize_symbol_name(stem);
    let title = stem.replace('_', " ");
    let template = format!(
        "bring <module/std.io> as io\n\nflow {symbol}\n    gives Int\n    show \"{title}\"\n    0\n"
    );
    fs::write(&target, template)?;
    Ok(target)
}

pub fn set_permission(root: &Path, entry: &Path, name: &str, enabled: bool) -> Result<Vec<String>> {
    let mut manifest = ensure_manifest(root, entry)?;
    let permissions = table_mut(&mut manifest, "permissions")?;
    let key = match name {
        "fs" | "filesystem" => "fs",
        "net" | "network" => "net",
        "ai" => "ai",
        other => bail!("unknown permission '{}'. Use fs, net, or ai.", other),
    };
    permissions.insert(key.to_string(), toml::Value::Boolean(enabled));
    write_manifest(root, &manifest)?;
    Ok(vec![
        format!(
            "Permission {} set to {}",
            key,
            if enabled { "on" } else { "off" }
        ),
        "Manifest updated.".to_string(),
    ])
}

pub fn manifest_snapshot(root: &Path) -> Result<String> {
    let manifest_path = root.join("snsx.toml");
    if !manifest_path.exists() {
        return Ok("snsx.toml not found in this workspace.".to_string());
    }
    fs::read_to_string(&manifest_path).with_context(|| "failed to read snsx.toml".to_string())
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")))
        .to_path_buf()
}

pub fn resolve_workspace_input(root: &Path, base: &Path, input: &str) -> Result<PathBuf> {
    let input = input.trim();
    if input.is_empty() {
        bail!("path cannot be empty");
    }
    if input == "." {
        return Ok(root.to_path_buf());
    }
    let raw = Path::new(input);
    if raw.is_absolute() {
        bail!("absolute paths are not allowed inside the SNSX workspace");
    }
    let mut target = base.to_path_buf();
    for component in raw.components() {
        match component {
            Component::CurDir => {}
            Component::Normal(part) => target.push(part),
            Component::ParentDir => bail!("parent directory segments are not allowed"),
            Component::RootDir | Component::Prefix(_) => {
                bail!("path must stay inside the current workspace")
            }
        }
    }
    if !target.starts_with(root) {
        bail!("path must stay inside the current workspace");
    }
    Ok(target)
}

pub fn workspace_path(root: &Path, path: &Path) -> String {
    if path == root {
        ".".to_string()
    } else {
        path.strip_prefix(root)
            .unwrap_or(path)
            .display()
            .to_string()
            .replace('\\', "/")
    }
}

fn parse_target_name(raw: &str) -> Result<Target> {
    match raw.trim() {
        "" | "vm" => Ok(Target::Vm),
        "llvm" => Ok(Target::Llvm),
        "wasm" => Ok(Target::Wasm),
        "native-x86_64" | "x86_64" => Ok(Target::NativeX86_64),
        "native-aarch64" | "aarch64" => Ok(Target::NativeAarch64),
        other => bail!("unknown build target '{}'", other),
    }
}

fn parse_permission_state(raw: &str) -> Result<bool> {
    match raw {
        "on" | "true" | "allow" | "yes" => Ok(true),
        "off" | "false" | "deny" | "no" => Ok(false),
        other => bail!("unknown permission state '{}'. Use on/off.", other),
    }
}

fn parse_path_and_rest(command: &str, prefix: &str, usage: &str) -> Result<(String, String)> {
    let rest = command
        .strip_prefix(prefix)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| anyhow::anyhow!("{}", usage))?;
    let mut parts = rest.splitn(2, char::is_whitespace);
    let path = parts.next().unwrap_or_default().trim();
    let prompt = parts.next().unwrap_or_default().trim();
    if path.is_empty() || prompt.is_empty() {
        bail!("{usage}");
    }
    Ok((path.to_string(), prompt.to_string()))
}

fn expect_rest(command: &str, head: &str, usage: &str) -> Result<String> {
    optional_rest(command, head).ok_or_else(|| anyhow::anyhow!("{}", usage))
}

fn optional_rest(command: &str, head: &str) -> Option<String> {
    let rest = command[head.len()..].trim();
    if rest.is_empty() {
        None
    } else {
        Some(rest.to_string())
    }
}

fn expect_nth_rest(command: &str, consumed_tokens: usize, usage: &str) -> Result<String> {
    let tokens = command.split_whitespace().collect::<Vec<_>>();
    if tokens.len() <= consumed_tokens {
        bail!("{usage}");
    }
    let prefix = tokens[..consumed_tokens].join(" ");
    let rest = command[prefix.len()..].trim();
    if rest.is_empty() {
        bail!("{usage}");
    }
    Ok(rest.to_string())
}

fn parse_two_args(command: &str, head: &str, usage: &str) -> Result<(String, String)> {
    let rest = expect_rest(command, head, usage)?;
    let mut parts = rest.split_whitespace();
    let Some(first) = parts.next() else {
        bail!("{usage}");
    };
    let Some(second) = parts.next() else {
        bail!("{usage}");
    };
    if parts.next().is_some() {
        bail!("{usage}");
    }
    Ok((first.to_string(), second.to_string()))
}

fn parse_two_args_after(
    command: &str,
    consumed_tokens: usize,
    usage: &str,
) -> Result<(String, String)> {
    let rest = expect_nth_rest(command, consumed_tokens, usage)?;
    let mut parts = rest.split_whitespace();
    let Some(first) = parts.next() else {
        bail!("{usage}");
    };
    let Some(second) = parts.next() else {
        bail!("{usage}");
    };
    if parts.next().is_some() {
        bail!("{usage}");
    }
    Ok((first.to_string(), second.to_string()))
}

fn package_recipe(name: &str) -> Option<PackageRecipe> {
    PACKAGE_RECIPES
        .iter()
        .copied()
        .find(|recipe| recipe.name == name)
}

fn append_tree_lines(
    root: &Path,
    current: &Path,
    depth: usize,
    lines: &mut Vec<String>,
) -> Result<()> {
    let kind = workspace_node_kind(current);
    let label = if current == root {
        root.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or(".")
            .to_string()
    } else {
        current
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("?")
            .to_string()
    };
    let prefix = match kind {
        WorkspaceNodeKind::Dir => "dir ",
        WorkspaceNodeKind::File => "file",
        WorkspaceNodeKind::Other => "node",
    };
    lines.push(format!("{}{} {}", "  ".repeat(depth), prefix, label));
    if kind == WorkspaceNodeKind::Dir {
        let mut entries = fs::read_dir(current)?
            .filter_map(|entry| entry.ok())
            .filter(|entry| !is_hidden_workspace_entry(&entry.file_name().to_string_lossy()))
            .filter(|entry| !is_workspace_symlink(&entry.path()))
            .collect::<Vec<_>>();
        entries.sort_by_key(|entry| {
            (
                !matches!(workspace_node_kind(&entry.path()), WorkspaceNodeKind::Dir),
                entry.file_name().to_string_lossy().to_lowercase(),
            )
        });
        for entry in entries {
            append_tree_lines(root, &entry.path(), depth + 1, lines)?;
        }
    }
    Ok(())
}

fn collect_workspace_paths(root: &Path, current: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    if current != root {
        out.push(current.to_path_buf());
    }
    if workspace_node_kind(current) == WorkspaceNodeKind::Dir {
        let mut entries = fs::read_dir(current)?
            .filter_map(|entry| entry.ok())
            .filter(|entry| !is_hidden_workspace_entry(&entry.file_name().to_string_lossy()))
            .filter(|entry| !is_workspace_symlink(&entry.path()))
            .collect::<Vec<_>>();
        entries.sort_by_key(|entry| {
            (
                !matches!(workspace_node_kind(&entry.path()), WorkspaceNodeKind::Dir),
                entry.file_name().to_string_lossy().to_lowercase(),
            )
        });
        for entry in entries {
            collect_workspace_paths(root, &entry.path(), out)?;
        }
    }
    Ok(())
}

fn collect_text_matches(
    root: &Path,
    current: &Path,
    needle_lower: &str,
    needle_display: &str,
    out: &mut Vec<String>,
) -> Result<()> {
    if out.len() >= 200 {
        return Ok(());
    }
    if workspace_node_kind(current) == WorkspaceNodeKind::Dir {
        let mut entries = fs::read_dir(current)?
            .filter_map(|entry| entry.ok())
            .filter(|entry| !is_hidden_workspace_entry(&entry.file_name().to_string_lossy()))
            .filter(|entry| !is_workspace_symlink(&entry.path()))
            .collect::<Vec<_>>();
        entries.sort_by_key(|entry| entry.file_name().to_string_lossy().to_lowercase());
        for entry in entries {
            collect_text_matches(root, &entry.path(), needle_lower, needle_display, out)?;
            if out.len() >= 200 {
                break;
            }
        }
        return Ok(());
    }

    if !is_searchable_text_file(current) {
        return Ok(());
    }

    let metadata = fs::metadata(current)?;
    if metadata.len() > 256 * 1024 {
        return Ok(());
    }

    let Ok(raw) = fs::read_to_string(current) else {
        return Ok(());
    };
    for (index, line) in raw.lines().enumerate() {
        if line.to_lowercase().contains(needle_lower) {
            out.push(format!(
                "{}:{} | {}",
                workspace_path(root, current),
                index + 1,
                trim_search_line(line, needle_display)
            ));
            if out.len() >= 200 {
                break;
            }
        }
    }
    Ok(())
}

fn workspace_counts(root: &Path, current: &Path) -> Result<(usize, usize, usize)> {
    match workspace_node_kind(current) {
        WorkspaceNodeKind::File => {
            let is_snsx = current.extension().and_then(|ext| ext.to_str()) == Some("snsx");
            return Ok((0, 1, usize::from(is_snsx)));
        }
        WorkspaceNodeKind::Other => return Ok((0, 0, 0)),
        WorkspaceNodeKind::Dir => {}
    }

    let mut dirs = if current == root { 0 } else { 1 };
    let mut files = 0usize;
    let mut snsx_files = 0usize;
    for entry in fs::read_dir(current)?
        .filter_map(|entry| entry.ok())
        .filter(|entry| !is_hidden_workspace_entry(&entry.file_name().to_string_lossy()))
        .filter(|entry| !is_workspace_symlink(&entry.path()))
    {
        let (child_dirs, child_files, child_snsx) = workspace_counts(root, &entry.path())?;
        dirs += child_dirs;
        files += child_files;
        snsx_files += child_snsx;
    }
    Ok((dirs, files, snsx_files))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WorkspaceNodeKind {
    Dir,
    File,
    Other,
}

fn workspace_node_kind(path: &Path) -> WorkspaceNodeKind {
    let Ok(metadata) = fs::symlink_metadata(path) else {
        return WorkspaceNodeKind::Other;
    };
    let file_type = metadata.file_type();
    if file_type.is_dir() {
        WorkspaceNodeKind::Dir
    } else if file_type.is_file() {
        WorkspaceNodeKind::File
    } else if file_type.is_symlink() {
        match fs::metadata(path) {
            Ok(target) if target.is_dir() => WorkspaceNodeKind::Dir,
            Ok(target) if target.is_file() => WorkspaceNodeKind::File,
            _ => WorkspaceNodeKind::Other,
        }
    } else {
        WorkspaceNodeKind::Other
    }
}

fn is_workspace_symlink(path: &Path) -> bool {
    fs::symlink_metadata(path)
        .map(|metadata| metadata.file_type().is_symlink())
        .unwrap_or(false)
}

fn copy_recursively(from: &Path, to: &Path) -> Result<()> {
    if from.is_dir() {
        fs::create_dir_all(to)?;
        for entry in fs::read_dir(from)? {
            let entry = entry?;
            copy_recursively(&entry.path(), &to.join(entry.file_name()))?;
        }
    } else {
        fs::copy(from, to)
            .with_context(|| format!("failed to copy {} to {}", from.display(), to.display()))?;
    }
    Ok(())
}

fn snippet_template(kind: &str, symbol: &str, title: &str) -> Result<String> {
    match kind {
        "starter" => Ok(format!(
            "bring <module/std.io> as io\n\nentry\n    show \"{title}\"\n    0\n"
        )),
        "mind" => Ok(format!(
            "bring <module/std.io> as io\nbring <module/std.ai> as ai\n\nmind summarize_{symbol}\n    takes text: String\n    gives String\n    from \"Summarize this text in one sentence\"\n\nentry\n    hold raw = \"{title} uses an AI-native SNSX snippet.\"\n    hold summary = raw |> summarize_{symbol}\n    show summary\n    0\n"
        )),
        "service" => Ok(format!(
            "bring <module/std.io> as io\n\nflow validate_{symbol}\n    takes raw: String\n    gives String\n    guard raw != \"\", \"input cannot be blank\"\n    raw\n\nflow render_{symbol}\n    takes value: String\n    gives String\n    \"{title}: \" + value\n\nentry\n    hold raw = ask()\n    hold safe = validate_{symbol}(raw)\n    show render_{symbol}(safe)\n    0\n"
        )),
        "design" => Ok(format!(
            "bring <lib/std.design> as design\nbring <lib/std.sentinel> as sentry\n\nentry\n    hold summary = watch(\"summary\", \"{title}\")\n    hold card = panel(\"Summary\", summary)\n    hold meta = stack([field(\"shape\", shape(summary)), field(\"seal\", seal(summary))])\n    hold board = draft(\"{title}\", stack([card, meta]))\n    show board\n    0\n"
        )),
        "worker" => Ok(format!(
            "flow work_{symbol}\n    takes x: Int\n    gives Int\n    x * 10\n\nentry\n    hold task = launch work_{symbol}(7)\n    hold value = wait task\n    show value\n    0\n"
        )),
        "tensor" => Ok(
            "entry\n    hold a = tensor([1, 2, 3, 4], [2, 2])\n    hold b = tensor([1, 0, 0, 1], [2, 2])\n    hold c = matmul(a, b)\n    show c\n    0\n".to_string(),
        ),
        other => bail!(
            "unknown snippet '{}'. Use starter, mind, service, design, worker, or tensor.",
            other
        ),
    }
}

fn ensure_manifest(root: &Path, entry: &Path) -> Result<toml::Value> {
    let manifest_path = root.join("snsx.toml");
    if manifest_path.exists() {
        let raw = fs::read_to_string(&manifest_path)?;
        return Ok(raw.parse::<toml::Value>()?);
    }

    let package_name = root
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("snsx_project")
        .replace(' ', "_");
    let entry_path = workspace_path(root, entry);

    let mut package = toml::Table::new();
    package.insert("name".to_string(), toml::Value::String(package_name));
    package.insert(
        "version".to_string(),
        toml::Value::String("0.1.0".to_string()),
    );
    package.insert("entry".to_string(), toml::Value::String(entry_path));

    let mut permissions = toml::Table::new();
    permissions.insert("fs".to_string(), toml::Value::Boolean(false));
    permissions.insert("net".to_string(), toml::Value::Boolean(false));
    permissions.insert("ai".to_string(), toml::Value::Boolean(true));

    let mut dependencies = toml::Table::new();
    dependencies.insert(
        "std".to_string(),
        toml::Value::String("builtin".to_string()),
    );

    let mut root_table = toml::Table::new();
    root_table.insert("package".to_string(), toml::Value::Table(package));
    root_table.insert("permissions".to_string(), toml::Value::Table(permissions));
    root_table.insert("dependencies".to_string(), toml::Value::Table(dependencies));
    let manifest = toml::Value::Table(root_table);
    write_manifest(root, &manifest)?;
    Ok(manifest)
}

fn write_manifest(root: &Path, manifest: &toml::Value) -> Result<()> {
    fs::write(root.join("snsx.toml"), toml::to_string_pretty(manifest)?)?;
    Ok(())
}

fn table_mut<'a>(manifest: &'a mut toml::Value, key: &str) -> Result<&'a mut toml::Table> {
    if !manifest
        .as_table()
        .map(|table| table.contains_key(key))
        .unwrap_or(false)
    {
        manifest
            .as_table_mut()
            .context("manifest root must be a table")?
            .insert(key.to_string(), toml::Value::Table(toml::Table::new()));
    }
    manifest
        .get_mut(key)
        .and_then(|value| value.as_table_mut())
        .with_context(|| format!("invalid snsx.toml: [{key}] must be a table"))
}

fn install_module_into_workspace(
    root: &Path,
    module: &str,
    manifest: &mut toml::Value,
) -> Result<String> {
    let registry = ModuleRegistry::new();
    let source = registry
        .source(module)
        .with_context(|| format!("unknown SNSX stdlib module '{}'", module))?;
    let vendor = vendored_module_path(root, module);
    if let Some(parent) = vendor.parent() {
        fs::create_dir_all(parent)?;
    }
    if !vendor.exists() {
        fs::write(&vendor, source)?;
    }

    let dependencies = table_mut(manifest, "dependencies")?;
    dependencies.insert(
        module.to_string(),
        toml::Value::String("builtin".to_string()),
    );

    match module {
        "std.net" => {
            let permissions = table_mut(manifest, "permissions")?;
            permissions.insert("net".to_string(), toml::Value::Boolean(true));
        }
        "std.ai" => {
            let permissions = table_mut(manifest, "permissions")?;
            permissions.insert("ai".to_string(), toml::Value::Boolean(true));
        }
        _ => {}
    }

    Ok(format!(
        "Installed {} at {}",
        module,
        workspace_path(root, &vendor)
    ))
}

fn apply_recipe_permissions(manifest: &mut toml::Value, recipe: PackageRecipe) -> Result<()> {
    let permissions = table_mut(manifest, "permissions")?;
    if recipe.allow_fs {
        permissions.insert("fs".to_string(), toml::Value::Boolean(true));
    }
    if recipe.allow_network {
        permissions.insert("net".to_string(), toml::Value::Boolean(true));
    }
    if recipe.allow_ai {
        permissions.insert("ai".to_string(), toml::Value::Boolean(true));
    }
    Ok(())
}

fn manifest_dependencies(root: &Path) -> Result<Vec<String>> {
    let manifest_path = root.join("snsx.toml");
    if !manifest_path.exists() {
        return Ok(Vec::new());
    }
    let raw = fs::read_to_string(&manifest_path)?;
    let manifest = raw.parse::<toml::Value>()?;
    let mut deps = manifest
        .get("dependencies")
        .and_then(|value| value.as_table())
        .map(|table| table.keys().cloned().collect::<Vec<_>>())
        .unwrap_or_default();
    deps.sort();
    Ok(deps)
}

fn vendored_module_path(root: &Path, module: &str) -> PathBuf {
    let mut path = root.join("vendor/modules");
    for segment in module.split('.') {
        path.push(segment);
    }
    path.set_extension("snsx");
    path
}

fn prune_empty_ancestors(mut current: Option<&Path>, stop: &Path) -> Result<()> {
    while let Some(path) = current {
        if path == stop || !path.starts_with(stop) {
            break;
        }
        if fs::read_dir(path)?.next().is_none() {
            fs::remove_dir(path)?;
            current = path.parent();
        } else {
            break;
        }
    }
    Ok(())
}

fn sanitize_symbol_name(raw: &str) -> String {
    let mut out = String::new();
    for ch in raw.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            out.push(ch.to_ascii_lowercase());
        } else {
            out.push('_');
        }
    }
    if out.is_empty() {
        "module".to_string()
    } else if out.chars().next().unwrap_or('m').is_ascii_digit() {
        format!("m_{out}")
    } else {
        out
    }
}

fn is_hidden_workspace_entry(name: &str) -> bool {
    matches!(name, ".git" | "target" | "dist")
}

fn on_off(enabled: bool) -> &'static str {
    if enabled {
        "on"
    } else {
        "off"
    }
}

fn is_searchable_text_file(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|ext| ext.to_str()),
        Some("snsx" | "toml" | "md" | "txt" | "json" | "yaml" | "yml" | "rs")
    )
}

fn trim_search_line(line: &str, needle: &str) -> String {
    let trimmed = line.trim();
    if trimmed.len() <= 140 {
        return trimmed.to_string();
    }
    let mut excerpt = trimmed.chars().take(140).collect::<String>();
    if needle.trim().is_empty() {
        return excerpt;
    }
    excerpt.push_str("...");
    excerpt
}

#[cfg(test)]
mod tests {
    use super::terminal_status_report;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[cfg(unix)]
    use std::os::unix::fs::symlink;

    fn temp_workspace(name: &str) -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("snsx_ide_{name}_{suffix}"))
    }

    #[test]
    #[cfg(unix)]
    fn status_report_skips_special_filesystem_entries() {
        let root = temp_workspace("status");
        fs::create_dir_all(&root).unwrap();
        let entry = root.join("main.snsx");
        fs::write(&entry, "entry\n    0\n").unwrap();
        symlink(root.join("missing_target"), root.join("broken_link")).unwrap();

        let report = terminal_status_report(&root, &entry).unwrap();
        assert!(report
            .iter()
            .any(|line| line.contains("SNSX terminal status")));

        fs::remove_dir_all(&root).unwrap();
    }
}
