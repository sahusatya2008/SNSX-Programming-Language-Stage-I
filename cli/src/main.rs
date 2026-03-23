use anyhow::{anyhow, Context, Result};
use base64::Engine;
use clap::{Parser, Subcommand, ValueEnum};
use ed25519_dalek::{Signer, SigningKey};
use rand_core::OsRng;
use serde::{Deserialize, Serialize};
use sha3::{Digest, Sha3_256};
use snsx_ai_engine::SnsxCodingAgent;
use snsx_compiler::security::SecurityPolicy;
use snsx_compiler::{
    compile_source_with_policy, compile_vm_with_policy, write_artifact, CompileOptions, Target,
};
use snsx_distributed_engine::{ClusterState, Node, TaskSpec};
use snsx_runtime::{SandboxPolicy, Value};
use snsx_vm::{VirtualMachine, VmOptions};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Parser)]
#[command(name = "snsx")]
#[command(about = "SNSX language CLI")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Init {
        name: String,
    },
    Start {
        #[arg(long, value_enum, default_value_t = LaunchMode::Terminal)]
        mode: LaunchMode,
        #[arg(long)]
        file: Option<PathBuf>,
        #[arg(long)]
        trace: bool,
        #[arg(long)]
        deterministic: bool,
        #[arg(long)]
        stdin_text: Option<String>,
        #[arg(long)]
        stdin_file: Option<PathBuf>,
        #[arg(long, default_value = "127.0.0.1")]
        host: String,
        #[arg(long, default_value_t = 8860)]
        port: u16,
        #[arg(long)]
        open: bool,
    },
    Run {
        #[arg(long)]
        file: Option<PathBuf>,
        #[arg(long)]
        watch: bool,
        #[arg(long)]
        trace: bool,
        #[arg(long)]
        deterministic: bool,
        #[arg(long)]
        stdin_text: Option<String>,
        #[arg(long)]
        stdin_file: Option<PathBuf>,
    },
    Build {
        #[arg(long)]
        file: Option<PathBuf>,
        #[arg(long, value_enum, default_value_t = BuildTarget::Vm)]
        target: BuildTarget,
        #[arg(long)]
        out: Option<PathBuf>,
        #[arg(long)]
        deterministic: bool,
    },
    Ide {
        #[arg(long)]
        file: Option<PathBuf>,
        #[arg(long)]
        trace: bool,
        #[arg(long)]
        deterministic: bool,
        #[arg(long)]
        stdin_text: Option<String>,
        #[arg(long)]
        stdin_file: Option<PathBuf>,
    },
    Studio {
        #[arg(long)]
        file: Option<PathBuf>,
        #[arg(long)]
        trace: bool,
        #[arg(long)]
        deterministic: bool,
        #[arg(long)]
        stdin_text: Option<String>,
        #[arg(long)]
        stdin_file: Option<PathBuf>,
    },
    Deploy {
        #[arg(long)]
        file: Option<PathBuf>,
        #[arg(long, default_value = "local")]
        cluster: String,
        #[arg(long)]
        out: Option<PathBuf>,
    },
    Agent {
        #[arg(long)]
        prompt: String,
        #[arg(long)]
        file: Option<PathBuf>,
        #[arg(long)]
        apply: bool,
    },
}

#[derive(Copy, Clone, Debug, ValueEnum)]
enum BuildTarget {
    Vm,
    Llvm,
    Wasm,
    NativeX8664,
    NativeAarch64,
}

#[derive(Copy, Clone, Debug, ValueEnum, PartialEq, Eq)]
enum LaunchMode {
    Terminal,
    App,
    Web,
}

#[derive(Debug, Serialize, Deserialize)]
struct Manifest {
    package: Package,
    permissions: Permissions,
    dependencies: HashMap<String, String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct Package {
    name: String,
    version: String,
    entry: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct Permissions {
    fs: bool,
    net: bool,
    ai: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct DeploymentBundle {
    package: String,
    cluster: String,
    digest_sha3: String,
    signature: String,
    artifact_path: String,
    artifact_bytes: Vec<u8>,
    plan: snsx_distributed_engine::ClusterPlan,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Init { name } => init_project(&name),
        Command::Start {
            mode,
            file,
            trace,
            deterministic,
            stdin_text,
            stdin_file,
            host,
            port,
            open,
        } => start_project(
            mode,
            file,
            trace,
            deterministic,
            stdin_text,
            stdin_file,
            host,
            port,
            open,
        ),
        Command::Run {
            file,
            watch,
            trace,
            deterministic,
            stdin_text,
            stdin_file,
        } => run_project(file, watch, trace, deterministic, stdin_text, stdin_file),
        Command::Build {
            file,
            target,
            out,
            deterministic,
        } => build_project(file, target, out, deterministic),
        Command::Ide {
            file,
            trace,
            deterministic,
            stdin_text,
            stdin_file,
        } => ide_project(file, trace, deterministic, stdin_text, stdin_file),
        Command::Studio {
            file,
            trace,
            deterministic,
            stdin_text,
            stdin_file,
        } => studio_project(file, trace, deterministic, stdin_text, stdin_file),
        Command::Deploy { file, cluster, out } => deploy_project(file, &cluster, out),
        Command::Agent {
            prompt,
            file,
            apply,
        } => agent_project(&prompt, file, apply),
    }
}

fn start_project(
    mode: LaunchMode,
    file: Option<PathBuf>,
    trace: bool,
    deterministic: bool,
    stdin_text: Option<String>,
    stdin_file: Option<PathBuf>,
    host: String,
    port: u16,
    open: bool,
) -> Result<()> {
    let entry = resolve_entry(file)?;
    let root = resolve_root(&entry);
    let stdin = resolve_stdin(stdin_text, stdin_file)?;
    let options = VmOptions {
        deterministic,
        trace,
        sandbox: sandbox_for_entry(&entry)?,
        stdin,
        allow_host_stdin: false,
    };
    match mode {
        LaunchMode::Terminal => snsx_ide::run_studio(&root, &entry, options),
        LaunchMode::App | LaunchMode::Web => snsx_ide::run_web_studio(
            &root,
            &entry,
            options,
            snsx_ide::WebStudioOptions {
                host,
                port,
                auto_open: open || mode == LaunchMode::App,
                app_mode: mode == LaunchMode::App,
            },
        ),
    }
}

fn init_project(name: &str) -> Result<()> {
    let root = PathBuf::from(name);
    fs::create_dir_all(root.join("src"))?;
    let manifest = Manifest {
        package: Package {
            name: name.to_string(),
            version: "0.1.0".to_string(),
            entry: "src/main.snsx".to_string(),
        },
        permissions: Permissions {
            fs: false,
            net: false,
            ai: true,
        },
        dependencies: HashMap::from([("std".to_string(), "builtin".to_string())]),
    };
    fs::write(root.join("snsx.toml"), toml::to_string_pretty(&manifest)?)?;
    fs::write(
        root.join("src/main.snsx"),
        r#"bring <module/std.io> as io
bring <module/std.ai> as ai
bring <lib/std.design> as design
bring <lib/std.sentinel> as sentry
bring <package/std.error> as errors

mind summarize
    takes text: String
    gives String
    from "Summarize this text"

entry
    hold raw = "SNSX ships with its own syntax, design layer, and strict compiler."
    guard raw != "", "raw input cannot be blank"
    hold seen = watch("raw", raw)
    hold summary = seen |> summarize
    guard len(summary) > 0, "summary must not be empty"
    hold card = panel("Summary", summary)
    hold meta = stack([field("shape", shape(summary)), field("seal", seal(summary))])
    hold board = draft("SNSX Board", stack([card, meta]))
    gate len(board) > 0:
        show board
    otherwise:
        fail "board rendering failed"
    0
"#,
    )?;
    fs::write(root.join(".gitignore"), "dist/\ntarget/\n")?;
    println!("Created SNSX project at {}", root.display());
    Ok(())
}

fn run_project(
    file: Option<PathBuf>,
    watch: bool,
    trace: bool,
    deterministic: bool,
    stdin_text: Option<String>,
    stdin_file: Option<PathBuf>,
) -> Result<()> {
    let entry = resolve_entry(file)?;
    let allow_host_stdin = !watch;
    let stdin = resolve_stdin(stdin_text, stdin_file)?;
    if watch {
        return snsx_ide::run_live(
            &entry,
            VmOptions {
                deterministic,
                trace,
                sandbox: sandbox_for_entry(&entry)?,
                stdin,
                allow_host_stdin: false,
            },
        );
    }
    let source = fs::read_to_string(&entry)?;
    let security = compiler_security_policy(&entry)?;
    let module = compile_vm_with_policy(&entry.display().to_string(), &source, &security)
        .map_err(|errs| anyhow!(format_diagnostics(&errs)))?;
    let mut vm = VirtualMachine::new(
        module,
        VmOptions {
            deterministic,
            trace,
            sandbox: sandbox_for_entry(&entry)?,
            stdin,
            allow_host_stdin,
        },
    );
    let result = vm.execute_main(Vec::new())?;
    print!("{}", result.stdout);
    if trace {
        for line in result.trace {
            println!("{line}");
        }
    }
    Ok(())
}

fn build_project(
    file: Option<PathBuf>,
    target: BuildTarget,
    out: Option<PathBuf>,
    deterministic: bool,
) -> Result<()> {
    let entry = resolve_entry(file)?;
    let source = fs::read_to_string(&entry)?;
    let target = match target {
        BuildTarget::Vm => Target::Vm,
        BuildTarget::Llvm => Target::Llvm,
        BuildTarget::Wasm => Target::Wasm,
        BuildTarget::NativeX8664 => Target::NativeX86_64,
        BuildTarget::NativeAarch64 => Target::NativeAarch64,
    };
    let security = compiler_security_policy(&entry)?;
    let compilation = compile_source_with_policy(
        &entry.display().to_string(),
        &source,
        &CompileOptions {
            target,
            optimize: true,
            deterministic,
        },
        &security,
    )
    .map_err(|errs| anyhow!(format_diagnostics(&errs)))?;
    let output = out.unwrap_or_else(|| default_output_path(&entry, target));
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }
    write_artifact(&output, &compilation.artifact)?;
    println!("Wrote {}", output.display());
    Ok(())
}

fn ide_project(
    file: Option<PathBuf>,
    trace: bool,
    deterministic: bool,
    stdin_text: Option<String>,
    stdin_file: Option<PathBuf>,
) -> Result<()> {
    let entry = resolve_entry(file)?;
    let stdin = resolve_stdin(stdin_text, stdin_file)?;
    snsx_ide::run_live(
        &entry,
        VmOptions {
            deterministic,
            trace,
            sandbox: sandbox_for_entry(&entry)?,
            stdin,
            allow_host_stdin: false,
        },
    )
}

fn studio_project(
    file: Option<PathBuf>,
    trace: bool,
    deterministic: bool,
    stdin_text: Option<String>,
    stdin_file: Option<PathBuf>,
) -> Result<()> {
    let entry = resolve_entry(file)?;
    let root = resolve_root(&entry);
    let stdin = resolve_stdin(stdin_text, stdin_file)?;
    snsx_ide::run_studio(
        &root,
        &entry,
        VmOptions {
            deterministic,
            trace,
            sandbox: sandbox_for_entry(&entry)?,
            stdin,
            allow_host_stdin: false,
        },
    )
}

fn deploy_project(file: Option<PathBuf>, cluster: &str, out: Option<PathBuf>) -> Result<()> {
    let entry = resolve_entry(file)?;
    let source = fs::read_to_string(&entry)?;
    let security = compiler_security_policy(&entry)?;
    let module = compile_vm_with_policy(&entry.display().to_string(), &source, &security)
        .map_err(|errs| anyhow!(format_diagnostics(&errs)))?;
    let artifact_bytes = serde_json::to_vec_pretty(&module)?;
    let digest = Sha3_256::digest(&artifact_bytes);
    let signing_key = load_or_create_signing_key()?;
    let signature = signing_key.sign(&artifact_bytes).to_bytes();
    let plan = build_cluster_plan(cluster);
    let manifest = find_manifest(&entry).and_then(|path| load_manifest(&path).ok());
    let package = manifest
        .as_ref()
        .map(|manifest| manifest.package.name.clone())
        .unwrap_or_else(|| "standalone".to_string());
    let bundle = DeploymentBundle {
        package: package.clone(),
        cluster: cluster.to_string(),
        digest_sha3: hex(&digest),
        signature: base64::engine::general_purpose::STANDARD.encode(signature),
        artifact_path: entry.display().to_string(),
        artifact_bytes,
        plan,
    };
    let output = out.unwrap_or_else(|| {
        entry
            .parent()
            .unwrap_or(Path::new("."))
            .join("dist")
            .join(format!("{package}.snsxp.json"))
    });
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&output, serde_json::to_vec_pretty(&bundle)?)?;
    println!("Deployment bundle written to {}", output.display());
    Ok(())
}

fn agent_project(prompt: &str, file: Option<PathBuf>, apply: bool) -> Result<()> {
    let artifact = SnsxCodingAgent::generate(prompt)?;
    if !apply {
        println!("SNS AI Coding Agent");
        println!("Title: {}", artifact.title);
        println!("Recipe: {}", artifact.recipe);
        println!("Suggested path: {}", artifact.suggested_path);
        println!("Generated files: {}", artifact.files.len().max(1));
        println!("Summary: {}", artifact.summary);
        println!(
            "Dependencies: {}",
            if artifact.dependencies.is_empty() {
                "none".to_string()
            } else {
                artifact.dependencies.join(", ")
            }
        );
        if !artifact.notes.is_empty() {
            println!("Notes:");
            for note in &artifact.notes {
                println!("- {note}");
            }
        }
        for file in artifact.files.iter().filter(|file| !file.code.trim().is_empty()) {
            println!();
            println!("== {} ==", file.path);
            println!("{}", file.code);
        }
        if artifact.files.is_empty() {
            println!();
            println!("{}", artifact.code);
        }
        return Ok(());
    }

    let cwd = std::env::current_dir()?;
    let root = resolve_workspace_root(file.as_deref()).unwrap_or_else(|| cwd.clone());
    let target = match file {
        Some(path) => path,
        None => root.join(&artifact.suggested_path),
    };
    write_agent_artifact(&root, &target, &artifact)?;
    sync_generated_manifest(&root, &target, &artifact)?;
    println!("SNS AI generated {}", target.display());
    println!("Recipe: {}", artifact.recipe);
    println!("Summary: {}", artifact.summary);
    Ok(())
}

fn resolve_entry(file: Option<PathBuf>) -> Result<PathBuf> {
    if let Some(file) = file {
        return Ok(file);
    }
    let cwd = std::env::current_dir()?;
    if cwd.join("snsx.toml").exists() {
        let manifest = load_manifest(&cwd.join("snsx.toml"))?;
        return Ok(cwd.join(manifest.package.entry));
    }
    let fallback = cwd.join("src/main.snsx");
    if fallback.exists() {
        Ok(fallback)
    } else {
        Err(anyhow!("could not find snsx.toml or src/main.snsx"))
    }
}

fn load_manifest(path: &Path) -> Result<Manifest> {
    let raw = fs::read_to_string(path)?;
    Ok(toml::from_str(&raw)?)
}

fn sandbox_for_entry(entry: &Path) -> Result<SandboxPolicy> {
    if let Some(manifest_path) = find_manifest(entry) {
        let manifest = load_manifest(&manifest_path)?;
        return Ok(SandboxPolicy {
            allow_fs: manifest.permissions.fs,
            allow_network: manifest.permissions.net,
            allow_ai: manifest.permissions.ai,
            deterministic: false,
            max_memory_bytes: 64 * 1024 * 1024,
        });
    }
    Ok(SandboxPolicy::default())
}

fn compiler_security_policy(entry: &Path) -> Result<SecurityPolicy> {
    if let Some(manifest_path) = find_manifest(entry) {
        let manifest = load_manifest(&manifest_path)?;
        return Ok(SecurityPolicy {
            strict: true,
            allow_fs: manifest.permissions.fs,
            allow_network: manifest.permissions.net,
            allow_ai: manifest.permissions.ai,
        });
    }
    Ok(SecurityPolicy::standalone())
}

fn find_manifest(entry: &Path) -> Option<PathBuf> {
    let mut current = entry.parent().map(Path::to_path_buf)?;
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

fn resolve_workspace_root(path: Option<&Path>) -> Option<PathBuf> {
    path.and_then(find_manifest)
        .and_then(|manifest| manifest.parent().map(Path::to_path_buf))
        .or_else(|| path.map(default_root_for_target))
        .or_else(|| {
            std::env::current_dir()
                .ok()
                .and_then(|cwd| cwd.join("snsx.toml").exists().then_some(cwd))
        })
}

fn default_root_for_target(path: &Path) -> PathBuf {
    let parent = path.parent().unwrap_or(Path::new("."));
    if parent.file_name().and_then(|name| name.to_str()) == Some("src") {
        parent.parent().unwrap_or(parent).to_path_buf()
    } else {
        parent.to_path_buf()
    }
}

fn sync_generated_manifest(
    root: &Path,
    target: &Path,
    artifact: &snsx_ai_engine::GeneratedSnsxProgram,
) -> Result<()> {
    let manifest_path = root.join("snsx.toml");
    let relative_entry = target
        .strip_prefix(root)
        .unwrap_or(target)
        .to_string_lossy()
        .replace('\\', "/");

    let mut manifest = if manifest_path.exists() {
        load_manifest(&manifest_path)?
    } else {
        Manifest {
            package: Package {
                name: root
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("snsx_project")
                    .replace(' ', "_"),
                version: "0.1.0".to_string(),
                entry: relative_entry.clone(),
            },
            permissions: Permissions {
                fs: artifact.permissions.fs,
                net: artifact.permissions.net,
                ai: artifact.permissions.ai,
            },
            dependencies: HashMap::from([("std".to_string(), "builtin".to_string())]),
        }
    };

    if manifest.package.entry.is_empty() {
        manifest.package.entry = relative_entry;
    }
    manifest.permissions.fs |= artifact.permissions.fs;
    manifest.permissions.net |= artifact.permissions.net;
    manifest.permissions.ai |= artifact.permissions.ai;
    for dependency in &artifact.dependencies {
        manifest
            .dependencies
            .insert(dependency.clone(), "builtin".to_string());
    }

    fs::write(&manifest_path, toml::to_string_pretty(&manifest)?)?;
    Ok(())
}

fn write_agent_artifact(
    root: &Path,
    target: &Path,
    artifact: &snsx_ai_engine::GeneratedSnsxProgram,
) -> Result<()> {
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

fn resolve_root(entry: &Path) -> PathBuf {
    find_manifest(entry)
        .and_then(|manifest| manifest.parent().map(Path::to_path_buf))
        .unwrap_or_else(|| entry.parent().unwrap_or(Path::new(".")).to_path_buf())
}

fn resolve_stdin(stdin_text: Option<String>, stdin_file: Option<PathBuf>) -> Result<String> {
    match (stdin_text, stdin_file) {
        (Some(_), Some(_)) => Err(anyhow!("use either --stdin-text or --stdin-file, not both")),
        (Some(text), None) => Ok(text),
        (None, Some(path)) => Ok(fs::read_to_string(path)?),
        (None, None) => Ok(String::new()),
    }
}

fn default_output_path(entry: &Path, target: Target) -> PathBuf {
    let stem = entry.file_stem().and_then(|s| s.to_str()).unwrap_or("main");
    let ext = match target {
        Target::Vm => "snsxbc",
        Target::Llvm => "ll",
        Target::Wasm => "wat",
        Target::NativeX86_64 => "x86_64.s",
        Target::NativeAarch64 => "aarch64.s",
    };
    entry
        .parent()
        .unwrap_or(Path::new("."))
        .join("dist")
        .join(format!("{stem}.{ext}"))
}

fn build_cluster_plan(cluster: &str) -> snsx_distributed_engine::ClusterPlan {
    let state = match cluster {
        "local" => ClusterState {
            nodes: vec![
                Node {
                    id: "local-a".to_string(),
                    cpu_cores: 4,
                    memory_mb: 4096,
                    gpu: false,
                    labels: vec!["edge".to_string()],
                },
                Node {
                    id: "local-b".to_string(),
                    cpu_cores: 8,
                    memory_mb: 8192,
                    gpu: true,
                    labels: vec!["gpu".to_string()],
                },
            ],
        },
        _ => ClusterState {
            nodes: vec![Node {
                id: format!("{cluster}-node"),
                cpu_cores: 8,
                memory_mb: 8192,
                gpu: true,
                labels: vec![cluster.to_string()],
            }],
        },
    };
    let task = TaskSpec {
        id: "deploy-main".to_string(),
        entry_function: "main".to_string(),
        cpu_cost: 2,
        memory_cost_mb: 256,
        requires_gpu: false,
        replicas: 2,
        payload: vec![Value::String("deploy".to_string())],
    };
    state.schedule(&[task])
}

fn load_or_create_signing_key() -> Result<SigningKey> {
    let home = std::env::var("HOME").context("HOME is not set")?;
    let key_dir = Path::new(&home).join(".snsx/keys");
    let key_path = key_dir.join("signing.key");
    if key_path.exists() {
        let raw = fs::read_to_string(&key_path)?;
        let bytes = base64::engine::general_purpose::STANDARD.decode(raw.trim())?;
        let array: [u8; 32] = bytes
            .try_into()
            .map_err(|_| anyhow!("invalid signing key length"))?;
        return Ok(SigningKey::from_bytes(&array));
    }
    fs::create_dir_all(&key_dir)?;
    let key = SigningKey::generate(&mut OsRng);
    fs::write(
        &key_path,
        base64::engine::general_purpose::STANDARD.encode(key.to_bytes()),
    )?;
    Ok(key)
}

fn format_diagnostics(diags: &[snsx_compiler::diag::Diagnostic]) -> String {
    diags
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n")
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect::<String>()
}
