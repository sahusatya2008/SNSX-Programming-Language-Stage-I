use crate::shell::{
    agent_apply, agent_preview, dependencies_report, doctor_report, documentation_lines,
    find_workspace_paths, manual_doc_path, outline_workspace_file, parse_shell_command,
    permissions_report, readme_doc_path, scaffold_snippet, search_workspace_text, shell_help_lines,
    shell_tool_lines, snippet_catalog, spec_doc_path, technical_spec_doc_path,
    terminal_status_report, validate_host_shell_command, ShellCommand,
};
use crate::{format_diagnostics, security_policy_for_entry};
use anyhow::{anyhow, bail, Context, Result};
use serde::Serialize;
use serde_json::{json, Value as JsonValue};
use snsx_compiler::security::SecurityPolicy;
use snsx_compiler::{
    audit_source, compile_source_with_policy, compile_vm_with_policy, write_artifact, Artifact,
    CompileOptions, Target,
};
use snsx_runtime::SandboxPolicy;
use snsx_vm::{VirtualMachine, VmOptions, VmTrap};
use std::env;
use std::fs;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::{Component, Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone)]
pub struct WebStudioOptions {
    pub host: String,
    pub port: u16,
    pub auto_open: bool,
    pub app_mode: bool,
}

impl Default for WebStudioOptions {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 8860,
            auto_open: false,
            app_mode: false,
        }
    }
}

#[derive(Debug)]
struct WebStudio {
    root: PathBuf,
    entry: Mutex<PathBuf>,
    shell_history: Mutex<Vec<String>>,
    options: VmOptions,
    token: String,
    app_mode: bool,
}

#[derive(Debug)]
struct HttpRequest {
    method: String,
    path: String,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

#[derive(Debug, Clone, Serialize)]
struct FileNode {
    name: String,
    path: String,
    is_dir: bool,
    children: Vec<FileNode>,
}

#[derive(Debug, Clone)]
struct AuditReport {
    passed: bool,
    blockers: usize,
    output: String,
    policy: SecurityPolicy,
}

#[derive(Debug, Clone, Serialize)]
struct SearchHit {
    path: String,
    line: Option<usize>,
    excerpt: String,
}

#[derive(Debug, Clone, Serialize)]
struct OutlineItem {
    line: usize,
    kind: String,
    label: String,
}

pub fn run_web_studio(
    root: &Path,
    entry: &Path,
    options: VmOptions,
    config: WebStudioOptions,
) -> Result<()> {
    let root = root.to_path_buf();
    let listener = bind_web_listener(&config.host, config.port)?;
    let address = listener.local_addr()?;
    let server = Arc::new(WebStudio {
        root,
        entry: Mutex::new(entry.to_path_buf()),
        shell_history: Mutex::new(Vec::new()),
        options,
        token: session_token(),
        app_mode: config.app_mode,
    });
    let url = format!("http://{}", address);
    println!(
        "SNSX {} Studio running at {}",
        if config.app_mode { "App" } else { "Web" },
        url
    );
    if config.auto_open {
        if let Err(error) = open_browser(&url, config.app_mode) {
            eprintln!("warning: failed to open browser automatically: {error}");
        }
    }

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let server = server.clone();
                let address = address;
                thread::spawn(move || {
                    if let Err(error) = handle_connection(server, stream, address) {
                        if !is_transient_connection_error(&error) {
                            eprintln!("snsx web studio connection error: {error}");
                        }
                    }
                });
            }
            Err(error) => eprintln!("snsx web studio accept error: {error}"),
        }
    }
    Ok(())
}

fn bind_web_listener(host: &str, preferred_port: u16) -> Result<TcpListener> {
    const MAX_PORT_ATTEMPTS: u16 = 24;

    for offset in 0..=MAX_PORT_ATTEMPTS {
        let Some(port) = preferred_port.checked_add(offset) else {
            break;
        };
        match TcpListener::bind((host, port)) {
            Ok(listener) => {
                if offset > 0 {
                    eprintln!(
                        "warning: {}:{} is busy, using {}:{} instead",
                        host, preferred_port, host, port
                    );
                }
                return Ok(listener);
            }
            Err(error) if error.kind() == std::io::ErrorKind::AddrInUse => continue,
            Err(error) => {
                return Err(error).with_context(|| format!("failed to bind {}:{}", host, port));
            }
        }
    }

    Err(anyhow!(
        "failed to bind {}:{} through {}:{}",
        host,
        preferred_port,
        host,
        preferred_port.saturating_add(MAX_PORT_ATTEMPTS)
    ))
}

fn is_transient_connection_error(error: &anyhow::Error) -> bool {
    error.chain().any(|cause| {
        cause
            .downcast_ref::<std::io::Error>()
            .map(|io| {
                matches!(
                    io.kind(),
                    std::io::ErrorKind::WouldBlock
                        | std::io::ErrorKind::TimedOut
                        | std::io::ErrorKind::Interrupted
                        | std::io::ErrorKind::BrokenPipe
                        | std::io::ErrorKind::ConnectionReset
                        | std::io::ErrorKind::ConnectionAborted
                        | std::io::ErrorKind::UnexpectedEof
                )
            })
            .unwrap_or(false)
    })
}

fn handle_connection(
    server: Arc<WebStudio>,
    mut stream: TcpStream,
    address: SocketAddr,
) -> Result<()> {
    stream.set_read_timeout(Some(std::time::Duration::from_secs(5)))?;
    let request = match read_request(&mut stream) {
        Ok(request) => request,
        Err(error) => {
            let response = json_error(400, &error.to_string())?;
            write_response(
                &mut stream,
                response.status,
                &response.content_type,
                &response.body,
            )?;
            return Ok(());
        }
    };
    let is_api = request.path.starts_with("/api/");
    let response = match route_request(&server, address, request) {
        Ok(response) => response,
        Err(error) if is_api => json_error(400, &error.to_string())?,
        Err(error) => HttpResponse {
            status: 500,
            content_type: "text/plain; charset=utf-8".to_string(),
            body: error.to_string().into_bytes(),
        },
    };
    write_response(
        &mut stream,
        response.status,
        &response.content_type,
        &response.body,
    )?;
    Ok(())
}

struct HttpResponse {
    status: u16,
    content_type: String,
    body: Vec<u8>,
}

fn route_request(
    server: &WebStudio,
    address: SocketAddr,
    request: HttpRequest,
) -> Result<HttpResponse> {
    match (request.method.as_str(), request.path.as_str()) {
        ("GET", "/") => Ok(HttpResponse {
            status: 200,
            content_type: "text/html; charset=utf-8".to_string(),
            body: render_index(server, address)?.into_bytes(),
        }),
        ("GET", "/docs/manual") => Ok(doc_response("SNSX Manual", &manual_doc_path())?),
        ("GET", "/docs/spec") => Ok(doc_response("SNSX Specification", &spec_doc_path())?),
        ("GET", "/docs/techspec") => Ok(doc_response(
            "SNSX Technical Specification",
            &technical_spec_doc_path(),
        )?),
        ("GET", "/docs/readme") => Ok(doc_response("SNSX README", &readme_doc_path())?),
        ("GET", "/favicon.ico") => Ok(HttpResponse {
            status: 204,
            content_type: "text/plain; charset=utf-8".to_string(),
            body: Vec::new(),
        }),
        ("GET", "/api/state") => {
            require_token(server, &request)?;
            json_ok(server.state_payload()?)
        }
        ("GET", "/api/doctor") => {
            require_token(server, &request)?;
            json_ok(server.doctor_payload()?)
        }
        ("GET", "/api/snippets") => {
            require_token(server, &request)?;
            json_ok(server.snippet_list_payload()?)
        }
        ("POST", "/api/open") => {
            require_token(server, &request)?;
            let payload = parse_json_body(&request)?;
            let path = payload_string(&payload, "path")?;
            json_ok(server.open_payload(&path)?)
        }
        ("POST", "/api/save") => {
            require_token(server, &request)?;
            let payload = parse_json_body(&request)?;
            let path = payload_string(&payload, "path")?;
            let content = payload_string(&payload, "content")?;
            json_ok(server.save_payload(&path, &content)?)
        }
        ("POST", "/api/create") => {
            require_token(server, &request)?;
            let payload = parse_json_body(&request)?;
            let base = payload_string(&payload, "base")?;
            let path = payload_string(&payload, "path")?;
            let directory = payload_bool(&payload, "directory").unwrap_or(false);
            json_ok(server.create_payload(&base, &path, directory)?)
        }
        ("POST", "/api/rename") => {
            require_token(server, &request)?;
            let payload = parse_json_body(&request)?;
            let from = payload_string(&payload, "from")?;
            let to = payload_string(&payload, "to")?;
            json_ok(server.rename_payload(&from, &to)?)
        }
        ("POST", "/api/delete") => {
            require_token(server, &request)?;
            let payload = parse_json_body(&request)?;
            let path = payload_string(&payload, "path")?;
            json_ok(server.delete_payload(&path)?)
        }
        ("POST", "/api/set-entry") => {
            require_token(server, &request)?;
            let payload = parse_json_body(&request)?;
            let path = payload_string(&payload, "path")?;
            json_ok(server.set_entry_payload(&path)?)
        }
        ("POST", "/api/find") => {
            require_token(server, &request)?;
            let payload = parse_json_body(&request)?;
            let query = payload_string(&payload, "query")?;
            json_ok(server.find_payload(&query)?)
        }
        ("POST", "/api/search") => {
            require_token(server, &request)?;
            let payload = parse_json_body(&request)?;
            let query = payload_string(&payload, "query")?;
            json_ok(server.search_payload(&query)?)
        }
        ("POST", "/api/outline") => {
            require_token(server, &request)?;
            let payload = parse_json_body(&request)?;
            let path = payload_string_default(&payload, "path");
            json_ok(server.outline_payload(&path)?)
        }
        ("POST", "/api/snippet") => {
            require_token(server, &request)?;
            let payload = parse_json_body(&request)?;
            let kind = payload_string(&payload, "kind")?;
            let path = payload_string(&payload, "path")?;
            json_ok(server.snippet_payload(&kind, &path)?)
        }
        ("POST", "/api/agent-preview") => {
            require_token(server, &request)?;
            let payload = parse_json_body(&request)?;
            let prompt = payload_string(&payload, "prompt")?;
            json_ok(server.agent_preview_payload(&prompt)?)
        }
        ("POST", "/api/agent-apply") => {
            require_token(server, &request)?;
            let payload = parse_json_body(&request)?;
            let prompt = payload_string(&payload, "prompt")?;
            let path = payload_string(&payload, "path")?;
            json_ok(server.agent_apply_payload(&prompt, &path)?)
        }
        ("POST", "/api/run") => {
            require_token(server, &request)?;
            let payload = parse_json_body(&request)?;
            let stdin = payload_string_default(&payload, "stdin");
            json_ok(server.run_payload(&stdin)?)
        }
        ("POST", "/api/build") => {
            require_token(server, &request)?;
            let payload = parse_json_body(&request)?;
            let target = payload_string_default(&payload, "target");
            json_ok(server.build_payload(&target)?)
        }
        ("POST", "/api/audit") => {
            require_token(server, &request)?;
            json_ok(server.audit_payload()?)
        }
        ("POST", "/api/shell") => {
            require_token(server, &request)?;
            let payload = parse_json_body(&request)?;
            let command = payload_string(&payload, "command")?;
            let stdin = payload_string_default(&payload, "stdin");
            json_ok(server.shell_payload(&command, &stdin)?)
        }
        _ => json_error(404, "route not found"),
    }
}

impl WebStudio {
    fn current_entry(&self) -> PathBuf {
        self.entry
            .lock()
            .map(|entry| entry.clone())
            .unwrap_or_else(|_| self.root.join("src/main.snsx"))
    }

    fn set_current_entry(&self, path: PathBuf) -> Result<()> {
        let mut guard = self
            .entry
            .lock()
            .map_err(|_| anyhow!("entry state is poisoned"))?;
        *guard = path;
        Ok(())
    }

    fn state_payload(&self) -> Result<JsonValue> {
        let entry = self.current_entry();
        let audit = self.audit_report(&entry)?;
        Ok(json!({
            "root": self.root.display().to_string(),
            "entry": workspace_path(&self.root, &entry),
            "tree": build_tree(&self.root, &self.root)?,
            "mode": if self.app_mode { "app" } else { "web" },
            "summary": workspace_summary(&self.root, &entry, &audit)?,
        }))
    }

    fn doctor_payload(&self) -> Result<JsonValue> {
        let entry = self.current_entry();
        let audit = self.audit_report(&entry)?;
        Ok(json!({
            "ok": audit.passed,
            "title": "SNSX workspace doctor",
            "output": doctor_report(&self.root, &entry)?.join("\n"),
            "summary": workspace_summary(&self.root, &entry, &audit)?,
        }))
    }

    fn open_payload(&self, path: &str) -> Result<JsonValue> {
        let path = resolve_workspace_input(&self.root, &self.root, path)?;
        let content = fs::read_to_string(&path)
            .with_context(|| format!("failed to open {}", workspace_path(&self.root, &path)))?;
        Ok(json!({
            "path": workspace_path(&self.root, &path),
            "content": content,
            "is_entry": path == self.current_entry(),
        }))
    }

    fn save_payload(&self, path: &str, content: &str) -> Result<JsonValue> {
        let path = resolve_workspace_input(&self.root, &self.root, path)?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut raw = content.to_string();
        if !raw.ends_with('\n') {
            raw.push('\n');
        }
        fs::write(&path, raw)?;
        Ok(json!({
            "message": format!("Saved {}", workspace_path(&self.root, &path)),
            "tree": build_tree(&self.root, &self.root)?,
            "entry": workspace_path(&self.root, &self.current_entry()),
        }))
    }

    fn create_payload(&self, base: &str, relative: &str, directory: bool) -> Result<JsonValue> {
        let base = resolve_workspace_input(&self.root, &self.root, base)?;
        let target = resolve_workspace_input(&self.root, &base, relative)?;
        if directory {
            fs::create_dir_all(&target)?;
        } else {
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent)?;
            }
            if !target.exists() {
                fs::write(&target, default_file_template(&target))?;
            }
        }
        Ok(json!({
            "message": format!(
                "Created {} {}",
                if directory { "folder" } else { "file" },
                workspace_path(&self.root, &target)
            ),
            "path": workspace_path(&self.root, &target),
            "tree": build_tree(&self.root, &self.root)?,
        }))
    }

    fn rename_payload(&self, from: &str, to: &str) -> Result<JsonValue> {
        let from_path = resolve_workspace_input(&self.root, &self.root, from)?;
        if from_path == self.root {
            bail!("workspace root cannot be renamed");
        }
        if is_manifest_path(&self.root, &from_path) {
            bail!("snsx.toml is protected from rename");
        }
        let to_path = resolve_workspace_input(&self.root, &self.root, to)?;
        if to_path.exists() {
            bail!(
                "target already exists: {}",
                workspace_path(&self.root, &to_path)
            );
        }
        if let Some(parent) = to_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::rename(&from_path, &to_path)?;

        let current_entry = self.current_entry();
        if let Some(next_entry) = remap_workspace_path(&current_entry, &from_path, &to_path) {
            self.sync_manifest_entry(&next_entry)?;
            self.set_current_entry(next_entry)?;
        }

        Ok(json!({
            "message": format!(
                "Renamed {} to {}",
                workspace_path(&self.root, &from_path),
                workspace_path(&self.root, &to_path)
            ),
            "path": workspace_path(&self.root, &to_path),
            "entry": workspace_path(&self.root, &self.current_entry()),
            "tree": build_tree(&self.root, &self.root)?,
        }))
    }

    fn delete_payload(&self, path: &str) -> Result<JsonValue> {
        let path = resolve_workspace_input(&self.root, &self.root, path)?;
        if path == self.root {
            bail!("workspace root cannot be deleted");
        }
        if is_manifest_path(&self.root, &path) {
            bail!("snsx.toml is protected from deletion");
        }
        let current_entry = self.current_entry();
        if current_entry == path || current_entry.starts_with(&path) {
            bail!("cannot delete the active entry file or one of its parent folders");
        }
        let label = workspace_path(&self.root, &path);
        if path.is_dir() {
            fs::remove_dir_all(&path)?;
        } else {
            fs::remove_file(&path)?;
        }
        Ok(json!({
            "message": format!("Deleted {label}"),
            "tree": build_tree(&self.root, &self.root)?,
            "entry": workspace_path(&self.root, &self.current_entry()),
        }))
    }

    fn set_entry_payload(&self, path: &str) -> Result<JsonValue> {
        let path = resolve_workspace_input(&self.root, &self.root, path)?;
        if path.extension().and_then(|ext| ext.to_str()) != Some("snsx") {
            bail!("entry file must use the .snsx extension");
        }
        self.sync_manifest_entry(&path)?;
        self.set_current_entry(path.clone())?;
        Ok(json!({
            "message": format!("Entry set to {}", workspace_path(&self.root, &path)),
            "entry": workspace_path(&self.root, &path),
            "tree": build_tree(&self.root, &self.root)?,
        }))
    }

    fn find_payload(&self, query: &str) -> Result<JsonValue> {
        let lines = find_workspace_paths(&self.root, query)?;
        let results = lines
            .into_iter()
            .skip(1)
            .filter(|line| line != "(no matches)")
            .map(|path| json!({ "path": path }))
            .collect::<Vec<_>>();
        Ok(json!({
            "ok": true,
            "title": format!("File matches for '{query}'"),
            "results": results,
        }))
    }

    fn search_payload(&self, query: &str) -> Result<JsonValue> {
        let lines = search_workspace_text(&self.root, query)?;
        let hits = lines
            .into_iter()
            .skip(1)
            .filter_map(|line| parse_search_hit(&line))
            .collect::<Vec<_>>();
        Ok(json!({
            "ok": true,
            "title": format!("Content matches for '{query}'"),
            "results": hits,
        }))
    }

    fn outline_payload(&self, path: &str) -> Result<JsonValue> {
        let target = if path.trim().is_empty() {
            self.current_entry()
        } else {
            resolve_workspace_input(&self.root, &self.root, path)?
        };
        let lines = outline_workspace_file(&self.root, &target)?;
        let items = lines
            .into_iter()
            .skip(1)
            .filter_map(|line| parse_outline_item(&line))
            .collect::<Vec<_>>();
        Ok(json!({
            "ok": true,
            "title": format!("Outline for {}", workspace_path(&self.root, &target)),
            "path": workspace_path(&self.root, &target),
            "items": items,
        }))
    }

    fn snippet_list_payload(&self) -> Result<JsonValue> {
        let snippets = snippet_catalog()
            .iter()
            .map(|snippet| {
                json!({
                    "name": snippet.name,
                    "description": snippet.description,
                })
            })
            .collect::<Vec<_>>();
        Ok(json!({
            "ok": true,
            "snippets": snippets,
        }))
    }

    fn snippet_payload(&self, kind: &str, path: &str) -> Result<JsonValue> {
        let target = scaffold_snippet(&self.root, self.root.as_path(), kind, path)?;
        let open = self.open_payload(&workspace_path(&self.root, &target))?;
        Ok(json!({
            "ok": true,
            "title": format!("Scaffolded {kind} snippet"),
            "message": format!("Created {}", workspace_path(&self.root, &target)),
            "path": workspace_path(&self.root, &target),
            "tree": build_tree(&self.root, &self.root)?,
            "open": open,
            "entry": workspace_path(&self.root, &self.current_entry()),
        }))
    }

    fn agent_preview_payload(&self, prompt: &str) -> Result<JsonValue> {
        let artifact = agent_preview(prompt)?;
        Ok(json!({
            "ok": true,
            "title": format!("SNS AI preview · {}", artifact.title),
            "summary": artifact.summary.clone(),
            "recipe": artifact.recipe.clone(),
            "suggested_path": artifact.suggested_path.clone(),
            "file_count": artifact.files.len().max(1),
            "files": artifact.files.clone(),
            "dependencies": artifact.dependencies.clone(),
            "notes": artifact.notes.clone(),
            "code": artifact.code.clone(),
        }))
    }

    fn agent_apply_payload(&self, prompt: &str, path: &str) -> Result<JsonValue> {
        let entry = self.current_entry();
        let (artifact, target, lines) =
            agent_apply(&self.root, &entry, self.root.as_path(), prompt, Some(path))?;
        let open = self.open_payload(&workspace_path(&self.root, &target))?;
        Ok(json!({
            "ok": true,
            "title": format!("SNS AI applied · {}", artifact.title),
            "summary": artifact.summary.clone(),
            "recipe": artifact.recipe.clone(),
            "file_count": artifact.files.len().max(1),
            "files": artifact.files.clone(),
            "message": lines.join("\n"),
            "path": workspace_path(&self.root, &target),
            "tree": build_tree(&self.root, &self.root)?,
            "open": open,
            "entry": workspace_path(&self.root, &self.current_entry()),
        }))
    }

    fn run_payload(&self, stdin: &str) -> Result<JsonValue> {
        let entry = self.current_entry();
        let audit = self.audit_report(&entry)?;
        if !audit.passed {
            return Ok(json!({
                "ok": false,
                "title": "Run blocked by strict audit",
                "output": audit.output,
                "blockers": audit.blockers,
            }));
        }

        let source = fs::read_to_string(&entry)?;
        let module = compile_vm_with_policy(&entry.display().to_string(), &source, &audit.policy)
            .map_err(|errs| anyhow!(format_diagnostics(&errs)))?;
        let mut vm = VirtualMachine::new(module, self.vm_options(&entry, stdin)?);
        match vm.execute_main(Vec::new()) {
            Ok(result) => {
                let mut lines = if result.stdout.trim().is_empty() {
                    vec!["Program finished without stdout".to_string()]
                } else {
                    result
                        .stdout
                        .lines()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                };
                lines.push(String::new());
                lines.push(format!("Return value: {}", result.value));
                if !stdin.trim().is_empty() {
                    lines.push(format!("Input bytes: {}", stdin.len()));
                }
                if !result.trace.is_empty() {
                    lines.push(String::new());
                    lines.push("Trace:".to_string());
                    lines.extend(result.trace);
                }
                Ok(json!({
                    "ok": true,
                    "title": "Run output",
                    "output": lines.join("\n"),
                    "entry": workspace_path(&self.root, &entry),
                }))
            }
            Err(error) => {
                if let Some(VmTrap::InputRequested(request)) = error.downcast_ref::<VmTrap>() {
                    let mut lines = if request.stdout.trim().is_empty() {
                        vec!["Program is waiting for input".to_string()]
                    } else {
                        request
                            .stdout
                            .lines()
                            .map(ToString::to_string)
                            .collect::<Vec<_>>()
                    };
                    lines.push(String::new());
                    lines.push(format!(
                        "Program is waiting for input line {}.",
                        request.consumed_lines + 1
                    ));
                    if !request.trace.is_empty() {
                        lines.push(String::new());
                        lines.push("Trace:".to_string());
                        lines.extend(request.trace.iter().cloned());
                    }
                    Ok(json!({
                        "ok": false,
                        "needs_input": true,
                        "title": "Program waiting for input",
                        "output": lines.join("\n"),
                        "consumed_lines": request.consumed_lines,
                    }))
                } else {
                    Ok(json!({
                        "ok": false,
                        "title": "Runtime error",
                        "output": error.to_string(),
                    }))
                }
            }
        }
    }

    fn build_payload(&self, target_name: &str) -> Result<JsonValue> {
        let entry = self.current_entry();
        let audit = self.audit_report(&entry)?;
        if !audit.passed {
            return Ok(json!({
                "ok": false,
                "title": "Build blocked by strict audit",
                "output": audit.output,
                "blockers": audit.blockers,
            }));
        }
        let target = parse_target_name(target_name)?;
        let source = fs::read_to_string(&entry)?;
        let compilation = compile_source_with_policy(
            &entry.display().to_string(),
            &source,
            &CompileOptions {
                target,
                optimize: true,
                deterministic: self.options.deterministic,
            },
            &audit.policy,
        )
        .map_err(|errs| anyhow!(format_diagnostics(&errs)))?;
        let output_path = default_output_path(&entry, target);
        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent)?;
        }
        write_artifact(&output_path, &compilation.artifact)?;
        let mut lines = vec![
            format!("Build succeeded for {}", workspace_path(&self.root, &entry)),
            format!("Target: {}", target_label(target)),
            format!("Artifact: {}", output_path.display()),
            String::new(),
        ];
        lines.extend(render_artifact_preview(&compilation.artifact));
        Ok(json!({
            "ok": true,
            "title": format!("Build {}", target_label(target)),
            "output": lines.join("\n"),
            "artifact_path": output_path.display().to_string(),
        }))
    }

    fn audit_payload(&self) -> Result<JsonValue> {
        let audit = self.audit_report(&self.current_entry())?;
        Ok(json!({
            "ok": audit.passed,
            "title": if audit.passed { "Security audit passed" } else { "Security audit blocked" },
            "output": audit.output,
            "blockers": audit.blockers,
        }))
    }

    fn shell_payload(&self, command: &str, stdin: &str) -> Result<JsonValue> {
        let command = parse_shell_command(command)?;
        self.shell_history
            .lock()
            .map_err(|_| anyhow!("SNSX shell history lock poisoned"))?
            .push(command_label(&command));
        self.execute_snsx_shell_command(command, stdin)
    }

    fn execute_snsx_shell_command(&self, command: ShellCommand, stdin: &str) -> Result<JsonValue> {
        match command {
            ShellCommand::Help => {
                self.shell_result(true, "SNSX shell", shell_help_lines().join("\n"))
            }
            ShellCommand::Tools => {
                self.shell_result(true, "SNSX terminal", shell_tool_lines().join("\n"))
            }
            ShellCommand::Docs => {
                self.shell_result(true, "SNSX shell", documentation_lines().join("\n"))
            }
            ShellCommand::Manual => self.shell_result(
                true,
                "SNSX shell",
                format!("SNSX manual\n{}", manual_doc_path().display()),
            ),
            ShellCommand::Spec => self.shell_result(
                true,
                "SNSX shell",
                format!("SNSX specification\n{}", spec_doc_path().display()),
            ),
            ShellCommand::TechSpec => self.shell_result(
                true,
                "SNSX shell",
                format!(
                    "SNSX technical specification\n{}",
                    technical_spec_doc_path().display()
                ),
            ),
            ShellCommand::History => {
                self.shell_result(true, "SNSX shell", self.shell_history_output()?)
            }
            ShellCommand::Status => self.shell_result(
                true,
                "SNSX terminal",
                terminal_status_report(&self.root, &self.current_entry())?.join("\n"),
            ),
            ShellCommand::PermissionsShow => self.shell_result(
                true,
                "SNSX terminal",
                permissions_report(&self.current_entry())?.join("\n"),
            ),
            ShellCommand::DependenciesShow => self.shell_result(
                true,
                "SNSX terminal",
                dependencies_report(&self.root)?.join("\n"),
            ),
            ShellCommand::Agent(prompt) => {
                let artifact = agent_preview(&prompt)?;
                self.shell_result(
                    true,
                    "SNS AI agent",
                    format!(
                        "Title {}\nRecipe {}\nSuggested path {}\nSummary {}\n\n{}",
                        artifact.title,
                        artifact.recipe,
                        artifact.suggested_path,
                        artifact.summary,
                        artifact.code
                    ),
                )
            }
            ShellCommand::AgentWrite { path, prompt } => {
                let entry = self.current_entry();
                let (artifact, target, lines) = agent_apply(
                    &self.root,
                    &entry,
                    self.root.as_path(),
                    &prompt,
                    Some(&path),
                )?;
                let open = self.open_payload(&workspace_path(&self.root, &target))?;
                self.shell_result_with_open(
                    true,
                    "SNS AI agent",
                    format!(
                        "{}\n\nRecipe {}\n{}\n\n{}",
                        artifact.title,
                        artifact.recipe,
                        lines.join("\n"),
                        artifact.code
                    ),
                    Some(open),
                )
            }
            ShellCommand::Run => self.run_payload(stdin),
            ShellCommand::Build(target) => self.build_payload(target_label(target)),
            ShellCommand::Audit => self.audit_payload(),
            ShellCommand::Doctor => self.doctor_payload(),
            ShellCommand::Clear => {
                self.shell_result(true, "SNSX shell", "SNSX shell cleared".to_string())
            }
            ShellCommand::Open(path) => {
                let open = self.open_payload(&path)?;
                self.shell_result_with_open(
                    true,
                    "SNSX shell",
                    format!("Opened {path}"),
                    Some(open),
                )
            }
            ShellCommand::Pwd => {
                self.shell_result(true, "SNSX shell", self.root.display().to_string())
            }
            ShellCommand::Ls(path) => self.shell_result(
                true,
                "SNSX shell",
                crate::shell::list_directory(&self.root, self.root.as_path(), path.as_deref())?
                    .join("\n"),
            ),
            ShellCommand::Tree(path) => self.shell_result(
                true,
                "SNSX shell",
                crate::shell::render_tree(&self.root, self.root.as_path(), path.as_deref())?
                    .join("\n"),
            ),
            ShellCommand::Find(query) => self.shell_result(
                true,
                "SNSX shell",
                find_workspace_paths(&self.root, &query)?.join("\n"),
            ),
            ShellCommand::Search(query) => self.shell_result(
                true,
                "SNSX shell",
                search_workspace_text(&self.root, &query)?.join("\n"),
            ),
            ShellCommand::Outline(path) => {
                let target = if let Some(path) = path {
                    resolve_workspace_input(&self.root, &self.root, &path)?
                } else {
                    self.current_entry()
                };
                self.shell_result(
                    true,
                    "SNSX shell",
                    outline_workspace_file(&self.root, &target)?.join("\n"),
                )
            }
            ShellCommand::Cat(path) => self.shell_result(
                true,
                "SNSX shell",
                crate::shell::read_workspace_file(&self.root, self.root.as_path(), &path)?,
            ),
            ShellCommand::Touch(path) => {
                let created = self.create_payload(".", &path, false)?;
                let open = self.open_payload(&path)?;
                self.shell_result_with_open(
                    true,
                    "SNSX shell",
                    payload_string(&created, "message")?,
                    Some(open),
                )
            }
            ShellCommand::Mkdir(path) => {
                let created = self.create_payload(".", &path, true)?;
                self.shell_result(true, "SNSX shell", payload_string(&created, "message")?)
            }
            ShellCommand::Move { from, to } => {
                let renamed = self.rename_payload(&from, &to)?;
                let output = payload_string(&renamed, "message")?;
                let open = if to.ends_with(".snsx") {
                    Some(self.open_payload(&to)?)
                } else {
                    None
                };
                self.shell_result_with_open(true, "SNSX shell", output, open)
            }
            ShellCommand::Copy { from, to } => {
                let lines =
                    crate::shell::copy_workspace_path(&self.root, self.root.as_path(), &from, &to)?;
                let open = if to.ends_with(".snsx") {
                    Some(self.open_payload(&to)?)
                } else {
                    None
                };
                self.shell_result_with_open(true, "SNSX shell", lines.join("\n"), open)
            }
            ShellCommand::Remove(path) => {
                let deleted = self.delete_payload(&path)?;
                self.shell_result(true, "SNSX shell", payload_string(&deleted, "message")?)
            }
            ShellCommand::EntryShow => self.shell_result(
                true,
                "SNSX shell",
                format!(
                    "Entry {}",
                    workspace_path(&self.root, &self.current_entry())
                ),
            ),
            ShellCommand::EntrySet(path) => {
                let full_path = resolve_workspace_input(&self.root, &self.root, &path)?;
                if !full_path.exists() {
                    let _ = self.create_payload(".", &path, false)?;
                }
                let payload = self.set_entry_payload(&path)?;
                let message = payload_string(&payload, "message")?;
                let open = Some(self.open_payload(&path)?);
                self.shell_result_with_open(true, "SNSX shell", message, open)
            }
            ShellCommand::PackageList => self.shell_result(
                true,
                "SNSX shell",
                crate::shell::list_packages(&self.root)?.join("\n"),
            ),
            ShellCommand::PackageAdd(spec) => {
                let lines =
                    crate::shell::install_package(&self.root, &self.current_entry(), &spec)?;
                self.shell_result(true, "SNSX shell", lines.join("\n"))
            }
            ShellCommand::PackageRemove(spec) => {
                let lines = crate::shell::remove_package(&self.root, &self.current_entry(), &spec)?;
                self.shell_result(true, "SNSX shell", lines.join("\n"))
            }
            ShellCommand::ModuleList => self.shell_result(
                true,
                "SNSX shell",
                crate::shell::list_modules(&self.root)?.join("\n"),
            ),
            ShellCommand::ModuleInstall(spec) => {
                let lines =
                    crate::shell::install_package(&self.root, &self.current_entry(), &spec)?;
                self.shell_result(true, "SNSX shell", lines.join("\n"))
            }
            ShellCommand::ModuleScaffold(name) => {
                let path = crate::shell::scaffold_module(&self.root, self.root.as_path(), &name)?;
                let open = self.open_payload(&workspace_path(&self.root, &path))?;
                self.shell_result_with_open(
                    true,
                    "SNSX shell",
                    format!("Scaffolded module {}", workspace_path(&self.root, &path)),
                    Some(open),
                )
            }
            ShellCommand::SnippetList => {
                let output = snippet_catalog()
                    .iter()
                    .map(|snippet| format!("{:<8} {}", snippet.name, snippet.description))
                    .collect::<Vec<_>>()
                    .join("\n");
                self.shell_result(true, "SNSX shell", format!("SNSX snippets:\n{output}"))
            }
            ShellCommand::SnippetCreate { kind, path } => {
                let target = scaffold_snippet(&self.root, self.root.as_path(), &kind, &path)?;
                let open = self.open_payload(&workspace_path(&self.root, &target))?;
                self.shell_result_with_open(
                    true,
                    "SNSX shell",
                    format!(
                        "Scaffolded {} snippet at {}",
                        kind,
                        workspace_path(&self.root, &target)
                    ),
                    Some(open),
                )
            }
            ShellCommand::ManifestShow => self.shell_result(
                true,
                "SNSX shell",
                crate::shell::manifest_snapshot(&self.root)?,
            ),
            ShellCommand::PermSet { name, enabled } => {
                let lines = crate::shell::set_permission(
                    &self.root,
                    &self.current_entry(),
                    &name,
                    enabled,
                )?;
                self.shell_result(true, "SNSX shell", lines.join("\n"))
            }
            ShellCommand::Host(command) => self.host_shell_payload(&command),
        }
    }

    fn shell_result(&self, ok: bool, title: &str, output: String) -> Result<JsonValue> {
        Ok(json!({
            "ok": ok,
            "title": title,
            "output": output,
            "tree": build_tree(&self.root, &self.root)?,
            "entry": workspace_path(&self.root, &self.current_entry()),
        }))
    }

    fn shell_result_with_open(
        &self,
        ok: bool,
        title: &str,
        output: String,
        open: Option<JsonValue>,
    ) -> Result<JsonValue> {
        let mut payload = self.shell_result(ok, title, output)?;
        if let Some(open) = open {
            payload["open"] = open;
        }
        Ok(payload)
    }

    fn host_shell_payload(&self, command: &str) -> Result<JsonValue> {
        validate_host_shell_command(command)?;
        let shell = env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());
        let output = Command::new(&shell)
            .arg("-lc")
            .arg(command)
            .current_dir(&self.root)
            .output()
            .with_context(|| format!("failed to execute shell command via {}", shell))?;
        let mut log = String::new();
        if !output.stdout.is_empty() {
            log.push_str(&String::from_utf8_lossy(&output.stdout));
        }
        if !output.stderr.is_empty() {
            if !log.is_empty() && !log.ends_with('\n') {
                log.push('\n');
            }
            log.push_str(&String::from_utf8_lossy(&output.stderr));
        }
        if !log.ends_with('\n') && !log.is_empty() {
            log.push('\n');
        }
        log.push_str(&match output.status.code() {
            Some(code) => format!("Shell exited with status {code}"),
            None => "Shell terminated by signal".to_string(),
        });
        self.shell_result(output.status.success(), "SNSX host shell", log)
    }

    fn shell_history_output(&self) -> Result<String> {
        let history = self
            .shell_history
            .lock()
            .map_err(|_| anyhow!("SNSX shell history lock poisoned"))?;
        if history.is_empty() {
            return Ok("SNSX shell history:\n(no commands yet)".to_string());
        }
        let mut lines = vec!["SNSX shell history:".to_string()];
        lines.extend(
            history
                .iter()
                .enumerate()
                .map(|(index, item)| format!("{:>3}  {}", index + 1, item)),
        );
        Ok(lines.join("\n"))
    }

    fn audit_report(&self, entry: &Path) -> Result<AuditReport> {
        let source = fs::read_to_string(entry)?;
        let policy = security_policy_for_entry(entry)?;
        match audit_source(&entry.display().to_string(), &source, &policy) {
            Ok(_) => Ok(AuditReport {
                passed: true,
                blockers: 0,
                output: format!(
                    "Security audit passed\nPolicy: strict | fs {} | net {} | ai {}\nRun and build are unlocked.",
                    on_off(policy.allow_fs),
                    on_off(policy.allow_network),
                    on_off(policy.allow_ai)
                ),
                policy,
            }),
            Err(diags) => Ok(AuditReport {
                passed: false,
                blockers: diags.len(),
                output: format!(
                    "Security audit found {} blocker(s)\nPolicy: strict | fs {} | net {} | ai {}\n\n{}",
                    diags.len(),
                    on_off(policy.allow_fs),
                    on_off(policy.allow_network),
                    on_off(policy.allow_ai),
                    format_diagnostics(&diags)
                ),
                policy,
            }),
        }
    }

    fn vm_options(&self, entry: &Path, stdin: &str) -> Result<VmOptions> {
        Ok(VmOptions {
            deterministic: self.options.deterministic,
            trace: self.options.trace,
            sandbox: sandbox_policy_for_entry(entry)?,
            stdin: stdin.to_string(),
            allow_host_stdin: false,
        })
    }

    fn sync_manifest_entry(&self, entry: &Path) -> Result<()> {
        let manifest_path = self.root.join("snsx.toml");
        if !manifest_path.exists() {
            return Ok(());
        }
        let raw = fs::read_to_string(&manifest_path)?;
        let mut manifest = raw.parse::<toml::Value>()?;
        let relative = workspace_path(&self.root, entry);
        let package = manifest
            .get_mut("package")
            .and_then(|value| value.as_table_mut())
            .context("invalid snsx.toml: missing [package] table")?;
        package.insert("entry".to_string(), toml::Value::String(relative));
        fs::write(&manifest_path, toml::to_string_pretty(&manifest)?)?;
        Ok(())
    }
}

fn render_index(server: &WebStudio, address: SocketAddr) -> Result<String> {
    let bootstrap = json!({
        "token": server.token,
        "root": server.root.display().to_string(),
        "entry": workspace_path(&server.root, &server.current_entry()),
        "mode": if server.app_mode { "app" } else { "web" },
        "url": format!("http://{}", address),
    });
    Ok(INDEX_HTML.replace("__SNSX_BOOTSTRAP__", &bootstrap.to_string()))
}

fn doc_response(title: &str, path: &Path) -> Result<HttpResponse> {
    let source = fs::read_to_string(path)
        .with_context(|| format!("failed to read documentation {}", path.display()))?;
    Ok(HttpResponse {
        status: 200,
        content_type: "text/html; charset=utf-8".to_string(),
        body: render_doc_page(title, &source).into_bytes(),
    })
}

fn render_doc_page(title: &str, source: &str) -> String {
    format!(
        r#"<!doctype html>
<html lang="en">
  <head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <title>{title}</title>
    <style>
      :root {{
        color-scheme: dark;
        --bg: #071419;
        --panel: rgba(12, 27, 33, 0.92);
        --line: rgba(120, 181, 193, 0.18);
        --ink: #ecfbff;
        --muted: #99bfc9;
        --accent: #78d3c0;
      }}
      body {{
        margin: 0;
        min-height: 100vh;
        background:
          radial-gradient(circle at top, rgba(120, 211, 192, 0.12), transparent 30%),
          linear-gradient(180deg, #08161b, #040b0e 72%);
        color: var(--ink);
        font-family: "IBM Plex Sans", "Segoe UI", sans-serif;
      }}
      main {{
        max-width: 1080px;
        margin: 0 auto;
        padding: 28px;
      }}
      .hero {{
        display: flex;
        justify-content: space-between;
        gap: 16px;
        align-items: center;
        margin-bottom: 18px;
      }}
      .back {{
        color: #021014;
        background: linear-gradient(135deg, var(--accent), #f4d778);
        padding: 10px 14px;
        border-radius: 14px;
        text-decoration: none;
        font-weight: 700;
      }}
      .panel {{
        background: var(--panel);
        border: 1px solid var(--line);
        border-radius: 22px;
        padding: 18px;
        box-shadow: 0 28px 70px rgba(0, 0, 0, 0.34);
      }}
      pre {{
        margin: 0;
        white-space: pre-wrap;
        word-break: break-word;
        color: var(--ink);
        font-family: "IBM Plex Mono", monospace;
        line-height: 1.58;
      }}
      small {{
        color: var(--muted);
      }}
    </style>
  </head>
  <body>
    <main>
      <div class="hero">
        <div>
          <h1>{title}</h1>
          <small>Loaded from the local SNSX repository.</small>
        </div>
        <a class="back" href="/">Back to UI</a>
      </div>
      <section class="panel">
        <pre>{}</pre>
      </section>
    </main>
  </body>
</html>"#,
        escape_html(source)
    )
}

fn command_label(command: &ShellCommand) -> String {
    match command {
        ShellCommand::Help => "help".to_string(),
        ShellCommand::Tools => "tools".to_string(),
        ShellCommand::Docs => "docs".to_string(),
        ShellCommand::Manual => "manual".to_string(),
        ShellCommand::Spec => "spec".to_string(),
        ShellCommand::TechSpec => "techspec".to_string(),
        ShellCommand::History => "history".to_string(),
        ShellCommand::Status => "status".to_string(),
        ShellCommand::PermissionsShow => "permissions".to_string(),
        ShellCommand::DependenciesShow => "deps".to_string(),
        ShellCommand::Agent(prompt) => format!("agent {prompt}"),
        ShellCommand::AgentWrite { path, prompt } => format!("agent write {path} {prompt}"),
        ShellCommand::Run => "run".to_string(),
        ShellCommand::Build(target) => format!("build {}", target_label(*target)),
        ShellCommand::Audit => "audit".to_string(),
        ShellCommand::Doctor => "doctor".to_string(),
        ShellCommand::Clear => "clear".to_string(),
        ShellCommand::Open(path) => format!("open {path}"),
        ShellCommand::Pwd => "pwd".to_string(),
        ShellCommand::Ls(path) => path
            .as_deref()
            .map(|path| format!("ls {path}"))
            .unwrap_or_else(|| "ls".to_string()),
        ShellCommand::Tree(path) => path
            .as_deref()
            .map(|path| format!("tree {path}"))
            .unwrap_or_else(|| "tree".to_string()),
        ShellCommand::Find(query) => format!("find {query}"),
        ShellCommand::Search(query) => format!("search {query}"),
        ShellCommand::Outline(path) => path
            .as_deref()
            .map(|path| format!("outline {path}"))
            .unwrap_or_else(|| "outline".to_string()),
        ShellCommand::Cat(path) => format!("cat {path}"),
        ShellCommand::Touch(path) => format!("touch {path}"),
        ShellCommand::Mkdir(path) => format!("mkdir {path}"),
        ShellCommand::Move { from, to } => format!("mv {from} {to}"),
        ShellCommand::Copy { from, to } => format!("cp {from} {to}"),
        ShellCommand::Remove(path) => format!("rm {path}"),
        ShellCommand::EntryShow => "entry".to_string(),
        ShellCommand::EntrySet(path) => format!("entry {path}"),
        ShellCommand::PackageList => "package list".to_string(),
        ShellCommand::PackageAdd(name) => format!("package add {name}"),
        ShellCommand::PackageRemove(name) => format!("package remove {name}"),
        ShellCommand::ModuleList => "module list".to_string(),
        ShellCommand::ModuleInstall(name) => format!("module install {name}"),
        ShellCommand::ModuleScaffold(name) => format!("module scaffold {name}"),
        ShellCommand::SnippetList => "snippet list".to_string(),
        ShellCommand::SnippetCreate { kind, path } => format!("snippet create {kind} {path}"),
        ShellCommand::ManifestShow => "manifest".to_string(),
        ShellCommand::PermSet { name, enabled } => {
            format!("perm {name} {}", if *enabled { "on" } else { "off" })
        }
        ShellCommand::Host(command) => format!("!{command}"),
    }
}

fn escape_html(source: &str) -> String {
    source
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn build_tree(root: &Path, current: &Path) -> Result<FileNode> {
    let kind = workspace_node_kind(current);
    let name = if current == root {
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
    let mut node = FileNode {
        name,
        path: workspace_path(root, current),
        is_dir: kind == WorkspaceNodeKind::Dir,
        children: Vec::new(),
    };
    if kind == WorkspaceNodeKind::Dir {
        let mut entries = fs::read_dir(current)?
            .filter_map(|entry| entry.ok())
            .filter(|entry| {
                let name = entry.file_name();
                let name = name.to_string_lossy();
                !matches!(name.as_ref(), ".git" | "target" | "dist")
            })
            .filter(|entry| !is_workspace_symlink(&entry.path()))
            .collect::<Vec<_>>();
        entries.sort_by_key(|entry| {
            (
                !matches!(workspace_node_kind(&entry.path()), WorkspaceNodeKind::Dir),
                entry.file_name().to_string_lossy().to_lowercase(),
            )
        });
        for entry in entries {
            let child_path = entry.path();
            if workspace_node_kind(&child_path) == WorkspaceNodeKind::Other {
                continue;
            }
            node.children.push(build_tree(root, &child_path)?);
        }
    }
    Ok(node)
}

fn resolve_workspace_input(root: &Path, base: &Path, input: &str) -> Result<PathBuf> {
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

fn workspace_path(root: &Path, path: &Path) -> String {
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

fn is_manifest_path(root: &Path, path: &Path) -> bool {
    path == root.join("snsx.toml")
}

fn remap_workspace_path(current: &Path, previous: &Path, next: &Path) -> Option<PathBuf> {
    if current == previous {
        return Some(next.to_path_buf());
    }
    if !current.starts_with(previous) {
        return None;
    }
    let suffix = current.strip_prefix(previous).ok()?;
    Some(next.join(suffix))
}

fn default_file_template(path: &Path) -> &'static str {
    match path.extension().and_then(|ext| ext.to_str()) {
        Some("snsx") => "bring <module/std.io> as io\n\nentry\n    show \"SNSX Web Studio\"\n    0\n",
        Some("toml") => "[package]\nname = \"snsx_project\"\nversion = \"0.1.0\"\nentry = \"src/main.snsx\"\n\n[permissions]\nfs = false\nnet = false\nai = true\n",
        _ => "",
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

fn target_label(target: Target) -> &'static str {
    match target {
        Target::Vm => "vm",
        Target::Llvm => "llvm",
        Target::Wasm => "wasm",
        Target::NativeX86_64 => "native-x86_64",
        Target::NativeAarch64 => "native-aarch64",
    }
}

fn workspace_summary(root: &Path, entry: &Path, audit: &AuditReport) -> Result<JsonValue> {
    let (dirs, files, snsx_files) = workspace_counts(root, root)?;
    let dependencies = manifest_dependencies(root)?;
    Ok(json!({
        "dirs": dirs,
        "files": files,
        "snsx_files": snsx_files,
        "dependencies": dependencies,
        "dependency_count": dependencies.len(),
        "permissions": {
            "fs": audit.policy.allow_fs,
            "net": audit.policy.allow_network,
            "ai": audit.policy.allow_ai,
        },
        "blockers": audit.blockers,
        "entry": workspace_path(root, entry),
        "strict": audit.policy.strict,
    }))
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

fn render_artifact_preview(artifact: &Artifact) -> Vec<String> {
    match artifact {
        Artifact::Vm(module) => vec![
            "Bytecode preview:".to_string(),
            format!("Symbols: {}", module.symbols.len()),
            format!("Functions: {}", module.functions.len()),
            format!("Entry symbol index: {}", module.entry),
        ],
        Artifact::Text(text) => {
            let mut lines = vec!["Artifact preview:".to_string()];
            lines.extend(text.lines().take(18).map(ToString::to_string));
            lines
        }
        Artifact::Binary(bytes) => vec![
            "Binary artifact preview:".to_string(),
            format!("Bytes: {}", bytes.len()),
        ],
    }
}

fn sandbox_policy_for_entry(entry: &Path) -> Result<SandboxPolicy> {
    let Some(manifest_path) = find_manifest(entry) else {
        return Ok(SandboxPolicy::default());
    };
    let raw = fs::read_to_string(manifest_path)?;
    let manifest = raw.parse::<toml::Value>()?;
    let permissions = manifest
        .get("permissions")
        .and_then(|value| value.as_table());
    Ok(SandboxPolicy {
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
        deterministic: false,
        max_memory_bytes: 64 * 1024 * 1024,
    })
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
    let mut entries = fs::read_dir(current)?
        .filter_map(|entry| entry.ok())
        .filter(|entry| {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            !matches!(name.as_ref(), ".git" | "target" | "dist")
        })
        .filter(|entry| !is_workspace_symlink(&entry.path()))
        .collect::<Vec<_>>();
    entries.sort_by_key(|entry| entry.file_name().to_string_lossy().to_lowercase());
    for entry in entries {
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
        .map(|table| {
            table
                .keys()
                .filter(|name| name.as_str() != "std")
                .cloned()
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    deps.sort();
    Ok(deps)
}

fn parse_search_hit(line: &str) -> Option<SearchHit> {
    let (prefix, excerpt) = line.split_once(" | ")?;
    let (path, line_number) = prefix.rsplit_once(':')?;
    Some(SearchHit {
        path: path.to_string(),
        line: line_number.parse::<usize>().ok(),
        excerpt: excerpt.to_string(),
    })
}

fn parse_outline_item(line: &str) -> Option<OutlineItem> {
    let trimmed = line.trim();
    let mut parts = trimmed.split_whitespace();
    let line = parts.next()?.parse::<usize>().ok()?;
    let kind = parts.next()?.to_string();
    let label = parts.collect::<Vec<_>>().join(" ");
    Some(OutlineItem { line, kind, label })
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

fn read_request(stream: &mut TcpStream) -> Result<HttpRequest> {
    let mut buffer = Vec::new();
    let mut chunk = [0u8; 4096];
    let header_end = loop {
        let read = stream.read(&mut chunk)?;
        if read == 0 {
            bail!("connection closed before request headers were complete");
        }
        buffer.extend_from_slice(&chunk[..read]);
        if let Some(index) = find_header_end(&buffer) {
            break index;
        }
        if buffer.len() > 2 * 1024 * 1024 {
            bail!("request headers are too large");
        }
    };

    let head = String::from_utf8_lossy(&buffer[..header_end]).to_string();
    let mut lines = head.split("\r\n");
    let request_line = lines
        .next()
        .ok_or_else(|| anyhow!("missing request line"))?;
    let mut request_parts = request_line.split_whitespace();
    let method = request_parts
        .next()
        .ok_or_else(|| anyhow!("missing request method"))?
        .to_string();
    let path = request_parts
        .next()
        .ok_or_else(|| anyhow!("missing request path"))?
        .to_string();

    let mut headers = Vec::new();
    let mut content_length = 0usize;
    for line in lines {
        if line.is_empty() {
            continue;
        }
        if let Some((name, value)) = line.split_once(':') {
            let name = name.trim().to_ascii_lowercase();
            let value = value.trim().to_string();
            if name == "content-length" {
                content_length = value.parse::<usize>().unwrap_or(0);
            }
            headers.push((name, value));
        }
    }

    let mut body = buffer[header_end + 4..].to_vec();
    while body.len() < content_length {
        let read = stream.read(&mut chunk)?;
        if read == 0 {
            break;
        }
        body.extend_from_slice(&chunk[..read]);
    }
    body.truncate(content_length);

    Ok(HttpRequest {
        method,
        path,
        headers,
        body,
    })
}

fn find_header_end(buffer: &[u8]) -> Option<usize> {
    buffer.windows(4).position(|window| window == b"\r\n\r\n")
}

fn write_response(
    stream: &mut TcpStream,
    status: u16,
    content_type: &str,
    body: &[u8],
) -> Result<()> {
    let reason = match status {
        200 => "OK",
        204 => "No Content",
        400 => "Bad Request",
        401 => "Unauthorized",
        404 => "Not Found",
        500 => "Internal Server Error",
        _ => "OK",
    };
    write!(
        stream,
        "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\nX-Content-Type-Options: nosniff\r\nContent-Security-Policy: default-src 'self' 'unsafe-inline'; connect-src 'self'; img-src 'self' data:; style-src 'self' 'unsafe-inline'; script-src 'self' 'unsafe-inline'; frame-ancestors 'none'\r\n\r\n",
        status,
        reason,
        content_type,
        body.len()
    )?;
    stream.write_all(body)?;
    stream.flush()?;
    Ok(())
}

fn require_token(server: &WebStudio, request: &HttpRequest) -> Result<()> {
    let provided = request
        .headers
        .iter()
        .find(|(name, _)| name == "x-snsx-token")
        .map(|(_, value)| value.as_str())
        .unwrap_or_default();
    if provided != server.token {
        bail!("missing or invalid SNSX session token");
    }
    Ok(())
}

fn parse_json_body(request: &HttpRequest) -> Result<JsonValue> {
    if request.body.is_empty() {
        return Ok(JsonValue::Null);
    }
    Ok(serde_json::from_slice(&request.body)?)
}

fn payload_string(payload: &JsonValue, key: &str) -> Result<String> {
    payload
        .get(key)
        .and_then(|value| value.as_str())
        .map(ToString::to_string)
        .ok_or_else(|| anyhow!("missing string field '{}'", key))
}

fn payload_string_default(payload: &JsonValue, key: &str) -> String {
    payload
        .get(key)
        .and_then(|value| value.as_str())
        .unwrap_or_default()
        .to_string()
}

fn payload_bool(payload: &JsonValue, key: &str) -> Option<bool> {
    payload.get(key).and_then(|value| value.as_bool())
}

fn json_ok(payload: JsonValue) -> Result<HttpResponse> {
    Ok(HttpResponse {
        status: 200,
        content_type: "application/json; charset=utf-8".to_string(),
        body: serde_json::to_vec(&payload)?,
    })
}

fn json_error(status: u16, message: &str) -> Result<HttpResponse> {
    Ok(HttpResponse {
        status,
        content_type: "application/json; charset=utf-8".to_string(),
        body: serde_json::to_vec(&json!({ "ok": false, "error": message }))?,
    })
}

fn session_token() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    format!(
        "snsx-{}-{}-{:x}",
        std::process::id(),
        nanos,
        nanos ^ 0x5a5a_a5a5_u128
    )
}

fn open_browser(url: &str, app_mode: bool) -> Result<()> {
    #[cfg(target_os = "macos")]
    {
        if app_mode {
            let chrome_candidates = [
                "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
                "/Applications/Chromium.app/Contents/MacOS/Chromium",
            ];
            for candidate in chrome_candidates {
                if Path::new(candidate).exists() {
                    Command::new(candidate)
                        .arg(format!("--app={url}"))
                        .spawn()
                        .context("failed to launch Chrome app window")?;
                    return Ok(());
                }
            }
        }
        Command::new("open")
            .arg(url)
            .spawn()
            .context("failed to open browser")?;
        return Ok(());
    }

    #[cfg(target_os = "linux")]
    {
        if app_mode {
            let candidates = ["google-chrome", "chromium", "chromium-browser"];
            for candidate in candidates {
                if Command::new(candidate)
                    .arg(format!("--app={url}"))
                    .spawn()
                    .is_ok()
                {
                    return Ok(());
                }
            }
        }
        Command::new("xdg-open")
            .arg(url)
            .spawn()
            .context("failed to open browser")?;
        return Ok(());
    }

    #[cfg(target_os = "windows")]
    {
        if app_mode {
            let candidates = ["chrome", "msedge"];
            for candidate in candidates {
                if Command::new(candidate)
                    .arg(format!("--app={url}"))
                    .spawn()
                    .is_ok()
                {
                    return Ok(());
                }
            }
        }
        Command::new("cmd")
            .args(["/C", "start", "", url])
            .spawn()
            .context("failed to open browser")?;
        return Ok(());
    }

    #[allow(unreachable_code)]
    Err(anyhow!(
        "automatic browser launch is not supported on this platform"
    ))
}

fn on_off(value: bool) -> &'static str {
    if value {
        "on"
    } else {
        "off"
    }
}

const INDEX_HTML: &str = r#"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>SNSX Studio</title>
  <style>
    :root {
      --mx: 0.5;
      --my: 0.5;
      --scroll: 0;
      --bg: #040a11;
      --bg-deep: #02060b;
      --panel: rgba(7, 16, 24, 0.84);
      --panel-strong: rgba(9, 21, 31, 0.96);
      --panel-soft: rgba(8, 18, 26, 0.7);
      --line: rgba(122, 232, 229, 0.14);
      --line-strong: rgba(122, 232, 229, 0.32);
      --ink: #eefbfd;
      --muted: #8da9b7;
      --accent: #7bf3e8;
      --accent-strong: #c4fff8;
      --accent-warm: #ffb36f;
      --accent-blue: #3db8ff;
      --success: #63df9f;
      --warn: #ff8c5a;
      --danger: #ff6868;
      --shadow: 0 28px 90px rgba(0, 0, 0, 0.46);
      --mono: "JetBrains Mono", "Iosevka", "SFMono-Regular", "Menlo", monospace;
      --sans: "Space Grotesk", "Avenir Next", "Segoe UI", sans-serif;
      --radius: 26px;
    }
    * {
      box-sizing: border-box;
      scroll-behavior: smooth;
    }
    html, body {
      margin: 0;
      height: 100%;
      min-height: 100%;
      color: var(--ink);
      background:
        radial-gradient(circle at 10% 10%, rgba(61, 184, 255, 0.24), transparent 26%),
        radial-gradient(circle at 87% 13%, rgba(255, 179, 111, 0.18), transparent 22%),
        radial-gradient(circle at 52% 80%, rgba(123, 243, 232, 0.14), transparent 34%),
        linear-gradient(160deg, #03070d 0%, #06121b 40%, #040810 100%);
      font-family: var(--sans);
      overflow: hidden;
    }
    body::before,
    body::after {
      content: "";
      position: fixed;
      inset: -14%;
      pointer-events: none;
      z-index: 0;
      transform: translate3d(
        calc((var(--mx) - 0.5) * -30px),
        calc((var(--my) - 0.5) * -26px + var(--scroll) * -0.03px),
        0
      );
      transition: transform 180ms ease-out;
    }
    body::before {
      background:
        radial-gradient(circle, rgba(61, 184, 255, 0.12) 0%, transparent 56%) 12% 18% / 34% 34% no-repeat,
        radial-gradient(circle, rgba(255, 179, 111, 0.14) 0%, transparent 58%) 84% 18% / 30% 30% no-repeat,
        radial-gradient(circle, rgba(123, 243, 232, 0.1) 0%, transparent 58%) 66% 78% / 28% 28% no-repeat;
      filter: blur(28px);
    }
    body::after {
      background-image:
        linear-gradient(rgba(122, 232, 229, 0.05) 1px, transparent 1px),
        linear-gradient(90deg, rgba(122, 232, 229, 0.05) 1px, transparent 1px);
      background-size: 48px 48px;
      mask-image: radial-gradient(circle at center, black 44%, transparent 92%);
      opacity: 0.48;
    }
    .app {
      position: relative;
      z-index: 1;
      display: grid;
      grid-template-rows: auto auto minmax(0, 1fr) auto;
      gap: 10px;
      min-height: 100dvh;
      height: 100dvh;
      padding: 12px;
      overflow: hidden;
    }
    .app > * {
      min-width: 0;
    }
    .glass,
    .panel,
    .statusbar,
    .toolbar {
      background:
        linear-gradient(180deg, rgba(10, 23, 34, 0.94), rgba(6, 15, 22, 0.9)),
        radial-gradient(circle at top right, rgba(123, 243, 232, 0.08), transparent 28%);
      border: 1px solid var(--line);
      box-shadow: var(--shadow);
      backdrop-filter: blur(22px);
      border-radius: var(--radius);
    }
    .hero {
      display: grid;
      grid-template-columns: minmax(390px, 1.18fr) minmax(360px, 0.92fr);
      gap: 18px;
      padding: 14px 18px;
      align-items: start;
      overflow: hidden;
      position: relative;
      min-height: 156px;
    }
    .hero::before {
      content: "";
      position: absolute;
      inset: 0;
      background:
        radial-gradient(circle at 20% 0%, rgba(123, 243, 232, 0.1), transparent 24%),
        linear-gradient(90deg, rgba(61, 184, 255, 0.08), transparent 24%, transparent 78%, rgba(255, 179, 111, 0.05));
      pointer-events: none;
    }
    .hero-copy {
      position: relative;
      padding: 4px 4px 2px;
      min-width: 0;
    }
    .hero-copy::after {
      content: "";
      position: absolute;
      inset: -14px auto -18px -8px;
      width: 210px;
      border-radius: 999px;
      background: linear-gradient(135deg, rgba(61, 184, 255, 0.16), rgba(123, 243, 232, 0.1));
      filter: blur(22px);
      pointer-events: none;
    }
    .kicker {
      display: inline-flex;
      align-items: center;
      gap: 10px;
      padding: 8px 12px;
      border: 1px solid var(--line);
      border-radius: 999px;
      background: rgba(6, 18, 27, 0.7);
      font-size: 0.78rem;
      letter-spacing: 0.16em;
      text-transform: uppercase;
      color: var(--accent);
      box-shadow: inset 0 0 0 1px rgba(255,255,255,0.03);
    }
    .hero h1 {
      margin: 10px 0 6px;
      font-size: clamp(1.65rem, 2.6vw, 2.55rem);
      line-height: 0.96;
      letter-spacing: -0.05em;
      max-width: 12ch;
      text-wrap: balance;
    }
    .hero p {
      margin: 0;
      max-width: 66ch;
      color: var(--muted);
      line-height: 1.48;
      font-size: 0.88rem;
    }
    .hero-meta {
      display: flex;
      flex-wrap: wrap;
      gap: 8px;
      margin-top: 10px;
    }
    .hero-chip,
    .mode-pill,
    .permission-chip,
    .tab,
    .pane-tab {
      display: inline-flex;
      align-items: center;
      gap: 8px;
      border-radius: 999px;
      border: 1px solid var(--line);
      background: rgba(14, 30, 36, 0.72);
      color: var(--ink);
    }
    .hero-chip,
    .mode-pill,
    .permission-chip {
      padding: 8px 12px;
      font-size: 0.78rem;
    }
    .mode-pill {
      justify-self: end;
      color: #041116;
      background: linear-gradient(135deg, var(--accent-blue), var(--accent), var(--accent-warm));
      border: none;
      font-weight: 700;
      text-transform: uppercase;
      letter-spacing: 0.12em;
      min-width: 180px;
      justify-content: center;
      box-shadow: 0 0 0 1px rgba(255,255,255,0.08), 0 16px 34px rgba(61, 184, 255, 0.2);
    }
    .hero-metrics {
      display: grid;
      grid-template-columns: repeat(2, minmax(0, 1fr));
      gap: 10px;
      align-content: start;
      min-width: 0;
    }
    .metric-card {
      position: relative;
      padding: 18px;
      min-height: 112px;
      border-radius: 22px;
      border: 1px solid rgba(122, 232, 229, 0.16);
      background:
        radial-gradient(circle at top right, rgba(123, 243, 232, 0.12), transparent 34%),
        linear-gradient(180deg, rgba(12, 27, 39, 0.92), rgba(7, 18, 27, 0.82));
      transition: transform 180ms ease, border-color 180ms ease, box-shadow 180ms ease;
      overflow: hidden;
    }
    .metric-card::after {
      content: "";
      position: absolute;
      inset: auto 14px 0;
      height: 2px;
      background: linear-gradient(90deg, rgba(61, 184, 255, 0.42), rgba(123, 243, 232, 0.42), transparent);
      pointer-events: none;
    }
    .metric-card:hover {
      transform: translateY(-2px);
      border-color: rgba(123, 243, 232, 0.3);
      box-shadow: 0 18px 40px rgba(0, 0, 0, 0.32);
    }
    .metric-card span {
      display: block;
      color: var(--muted);
      text-transform: uppercase;
      letter-spacing: 0.12em;
      font-size: 0.72rem;
      margin-bottom: 8px;
    }
    .metric-card strong {
      display: block;
      font-size: 1.56rem;
      line-height: 1;
      letter-spacing: -0.04em;
      white-space: normal;
      overflow-wrap: anywhere;
    }
    .metric-card small {
      display: block;
      margin-top: 6px;
      color: var(--muted);
      white-space: normal;
      overflow-wrap: anywhere;
    }
    .toolbar {
      display: flex;
      flex-wrap: wrap;
      gap: 8px 10px;
      padding: 10px 12px;
      align-items: center;
      align-content: flex-start;
      max-height: none;
      overflow: visible;
      scrollbar-width: thin;
      scrollbar-color: rgba(100, 216, 218, 0.4) transparent;
    }
    button,
    a.docs-link,
    input,
    textarea,
    pre {
      font: inherit;
    }
    button {
      appearance: none;
      border: 1px solid var(--line);
      border-radius: 15px;
      background:
        linear-gradient(180deg, rgba(14, 29, 40, 0.92), rgba(8, 18, 26, 0.9)),
        radial-gradient(circle at top right, rgba(123, 243, 232, 0.06), transparent 32%);
      color: var(--ink);
      padding: 9px 13px;
      cursor: pointer;
      transition: transform 160ms ease, border-color 160ms ease, background 160ms ease, box-shadow 160ms ease;
      box-shadow: inset 0 1px 0 rgba(255,255,255,0.03);
    }
    button:hover {
      transform: translateY(-1px);
      border-color: var(--line-strong);
      box-shadow: 0 12px 26px rgba(0, 0, 0, 0.24);
    }
    button:disabled,
    input:disabled,
    textarea:disabled {
      opacity: 0.56;
      cursor: wait;
      transform: none;
      box-shadow: none;
    }
    button.primary {
      border: none;
      color: #041014;
      background: linear-gradient(135deg, var(--accent-blue), var(--accent), var(--accent-warm));
      font-weight: 700;
    }
    button.subtle {
      background: rgba(8, 17, 25, 0.78);
    }
    a.docs-link {
      display: inline-flex;
      align-items: center;
      justify-content: center;
      border: 1px solid var(--line);
      border-radius: 15px;
      background: rgba(8, 17, 25, 0.78);
      color: var(--ink);
      padding: 9px 13px;
      text-decoration: none;
      transition: transform 160ms ease, border-color 160ms ease, background 160ms ease, box-shadow 160ms ease;
      box-shadow: inset 0 1px 0 rgba(255,255,255,0.03);
    }
    a.docs-link:hover {
      transform: translateY(-1px);
      border-color: var(--line-strong);
      box-shadow: 0 14px 30px rgba(0, 0, 0, 0.22);
    }
    button.warn {
      border: none;
      background: linear-gradient(135deg, #ff7e6b, #ffb067);
      color: #240a0a;
      font-weight: 700;
    }
    .toolbar button,
    .toolbar a.docs-link {
      flex: 0 0 auto;
    }
    #saveBtn,
    #runBtn,
    #auditBtn,
    #doctorBtn {
      order: 10;
    }
    #buildVmBtn,
    #buildWasmBtn,
    #buildLlvmBtn,
    #buildNativeBtn {
      order: 20;
    }
    #setEntryBtn,
    #newFileBtn,
    #newFolderBtn,
    #renameBtn,
    #deleteBtn,
    #refreshBtn {
      order: 30;
    }
    #buildVmBtn,
    #setEntryBtn {
      margin-left: 8px;
    }
    .toolbar-spacer {
      order: 80;
    }
    .toolbar-note {
      order: 90;
    }
    .toolbar a.docs-link {
      order: 100;
    }
    #commandPaletteBtn {
      order: 110;
    }
    .toolbar-spacer {
      display: none;
    }
    .toolbar-note {
      color: var(--muted);
      font-size: 0.8rem;
      padding-right: 4px;
      white-space: normal;
      flex: 0 0 auto;
    }
    .workspace {
      display: grid;
      grid-template-columns: minmax(260px, 0.74fr) minmax(660px, 1.92fr) minmax(560px, 1.3fr);
      gap: 14px;
      min-height: 0;
      height: 100%;
      align-items: stretch;
      overflow: hidden;
    }
    .workspace-rail {
      display: none;
      align-items: stretch;
      gap: 10px;
      padding: 12px 14px;
      grid-template-columns: repeat(6, minmax(0, 1fr));
    }
    .workspace-panel {
      min-height: 0;
      min-width: 0;
    }
    .pane-tab {
      width: 100%;
      justify-content: center;
      text-align: center;
      white-space: normal;
      padding: 14px 16px;
      font-size: 0.82rem;
      letter-spacing: 0.08em;
      text-transform: uppercase;
      line-height: 1.25;
      min-height: 60px;
    }
    .pane-tab.active {
      background: linear-gradient(135deg, rgba(24, 244, 255, 0.22), rgba(255, 147, 97, 0.18));
      border-color: rgba(24, 244, 255, 0.32);
    }
    .column {
      display: grid;
      min-height: 0;
      height: 100%;
      gap: 12px;
      min-width: 0;
    }
    .column-left {
      grid-template-rows: minmax(0, 0.98fr) minmax(0, 1.02fr);
    }
    .column-right {
      grid-template-rows: minmax(0, 1.22fr) minmax(0, 0.88fr) minmax(0, 1fr);
    }
    .panel {
      display: flex;
      flex-direction: column;
      min-height: 0;
      min-width: 0;
      overflow: hidden;
      position: relative;
      border-radius: 30px;
      background:
        linear-gradient(180deg, rgba(10, 21, 30, 0.94), rgba(5, 12, 18, 0.92)),
        radial-gradient(circle at top right, rgba(123, 243, 232, 0.08), transparent 30%);
    }
    .panel::before {
      content: "";
      position: absolute;
      inset: 0;
      background:
        linear-gradient(180deg, rgba(61, 184, 255, 0.05), transparent 26%, transparent 72%, rgba(255, 179, 111, 0.05)),
        linear-gradient(90deg, rgba(255,255,255,0.02), transparent 20%);
      pointer-events: none;
    }
    .panel::after {
      content: "";
      position: absolute;
      inset: 1px;
      border-radius: inherit;
      background:
        radial-gradient(circle at top right, rgba(123, 243, 232, 0.08), transparent 22%),
        linear-gradient(125deg, rgba(255,255,255,0.03), transparent 18%, transparent 82%, rgba(255,255,255,0.02));
      pointer-events: none;
      opacity: 0.9;
    }
    .panel-head {
      position: relative;
      z-index: 1;
      display: flex;
      justify-content: space-between;
      align-items: center;
      flex-wrap: wrap;
      gap: 12px;
      padding: 17px 20px 14px;
      border-bottom: 1px solid rgba(122, 232, 229, 0.1);
      background:
        linear-gradient(180deg, rgba(12, 28, 39, 0.96), rgba(9, 20, 29, 0.6)),
        radial-gradient(circle at top right, rgba(123, 243, 232, 0.08), transparent 30%);
    }
    .panel-head strong {
      font-size: 0.82rem;
      letter-spacing: 0.18em;
      text-transform: uppercase;
    }
    .meta {
      color: var(--muted);
      font-size: 0.82rem;
      white-space: normal;
      overflow-wrap: anywhere;
      max-width: 100%;
      min-width: 0;
    }
    .panel-body {
      position: relative;
      z-index: 1;
      min-height: 0;
      display: flex;
      flex-direction: column;
      flex: 1 1 auto;
      overflow: hidden;
    }
    .navigator-top {
      padding: 14px 16px 0;
      display: grid;
      gap: 12px;
    }
    .input-shell,
    textarea.editor,
    textarea.input,
    input.palette,
    pre.output {
      width: 100%;
      border: none;
      outline: none;
      color: var(--ink);
      font-family: var(--mono);
      background: transparent;
      resize: none;
      min-width: 0;
    }
    .input-shell,
    .navigator-input,
    input.palette {
      padding: 14px 16px;
      border-radius: 16px;
      border: 1px solid rgba(122, 232, 229, 0.14);
      background: rgba(8, 18, 27, 0.88);
      color: var(--ink);
      box-shadow: inset 0 1px 0 rgba(255,255,255,0.03);
    }
    .navigator-actions,
    .permission-row,
    .snippet-grid {
      display: flex;
      gap: 8px;
      flex-wrap: wrap;
    }
    .tree,
    .results-list,
    .outline-list,
    pre.output {
      overflow: auto;
      min-height: 0;
      scrollbar-width: thin;
      scrollbar-color: rgba(100, 216, 218, 0.4) transparent;
    }
    .tree {
      padding: 10px 14px 16px;
      font-size: 0.92rem;
    }
    .tree-item,
    .result-card,
    .outline-item,
    .snippet-card {
      width: 100%;
      text-align: left;
      border-radius: 22px;
      border: 1px solid rgba(122, 232, 229, 0.08);
      background:
        linear-gradient(180deg, rgba(10, 23, 32, 0.9), rgba(7, 17, 24, 0.82)),
        radial-gradient(circle at top right, rgba(123, 243, 232, 0.05), transparent 32%);
      color: inherit;
    }
    .tree-item {
      display: flex;
      align-items: center;
      gap: 10px;
      padding: 14px 16px;
      margin-bottom: 8px;
      font-family: var(--mono);
      font-size: 0.88rem;
      min-height: 58px;
    }
    .tree-item:hover,
    .tree-item.active,
    .result-card:hover,
    .outline-item:hover,
    .snippet-card:hover {
      border-color: rgba(123, 243, 232, 0.22);
      background: rgba(13, 34, 45, 0.9);
    }
    .tree-children {
      margin-left: 14px;
      padding-left: 10px;
      border-left: 1px solid rgba(117, 209, 222, 0.14);
    }
    .tree-dot {
      color: var(--accent);
      opacity: 0.9;
    }
    .navigator-panel .panel-body {
      padding-bottom: 14px;
    }
    .results-list,
    .outline-list {
      padding: 0 16px 16px;
      display: grid;
      gap: 10px;
      flex: 1 1 auto;
    }
    .result-card,
    .outline-item,
    .snippet-card {
      padding: 16px 18px;
      cursor: pointer;
      transition: transform 160ms ease, border-color 160ms ease, background 160ms ease;
      min-height: 88px;
    }
    .result-card strong,
    .outline-item strong,
    .snippet-card strong {
      display: block;
      font-size: 0.92rem;
      margin-bottom: 6px;
    }
    .result-card small,
    .outline-item small,
    .snippet-card small,
    .muted {
      color: var(--muted);
    }
    .editor-panel {
      overflow: hidden;
      display: grid;
      grid-template-rows: auto auto auto auto minmax(0, 1fr);
      min-height: 0;
      background:
        linear-gradient(180deg, rgba(8, 20, 29, 0.98), rgba(4, 11, 17, 0.98)),
        radial-gradient(circle at top right, rgba(61, 184, 255, 0.09), transparent 30%);
    }
    .editor-tools {
      padding: 10px 18px 0;
      color: var(--muted);
      font-size: 0.84rem;
      display: grid;
      grid-template-columns: minmax(0, 1fr) auto;
      gap: 12px 14px;
      align-items: center;
    }
    .editor-actions,
    .editor-stats {
      display: flex;
      flex-wrap: wrap;
      gap: 8px;
      align-items: center;
    }
    .editor-stats {
      justify-content: flex-end;
    }
    .editor-tools button,
    .editor-tools .hero-chip {
      padding: 10px 14px;
      border-radius: 14px;
      font-size: 0.8rem;
    }
    .editor-stats .hero-chip {
      background: rgba(8, 18, 24, 0.76);
      border-color: rgba(117, 209, 222, 0.16);
    }
    .editor-assist {
      padding: 0 18px 10px;
      color: var(--muted);
      font-size: 0.8rem;
      line-height: 1.5;
    }
    .editor-surface {
      margin: 12px 18px 18px;
      min-height: 0;
      height: 100%;
      display: grid;
      grid-template-columns: auto minmax(0, 1fr);
      border-radius: 28px;
      border: 1px solid rgba(122, 232, 229, 0.14);
      overflow: hidden;
      background:
        linear-gradient(180deg, rgba(255,255,255,0.02), transparent 22%),
        linear-gradient(90deg, rgba(61, 184, 255, 0.1), transparent 16%),
        linear-gradient(180deg, rgba(5, 13, 19, 0.98), rgba(3, 9, 14, 0.98));
      box-shadow: inset 0 1px 0 rgba(255,255,255,0.03), 0 18px 36px rgba(0, 0, 0, 0.24);
      position: relative;
    }
    .editor-surface::before {
      content: "";
      position: absolute;
      inset: 0;
      background:
        radial-gradient(circle at top right, rgba(24, 244, 255, 0.12), transparent 24%),
        linear-gradient(180deg, rgba(255,255,255,0.03), transparent 14%);
      pointer-events: none;
    }
    .code-gutter {
      margin: 0;
      min-width: 78px;
      padding: 20px 12px 20px 18px;
      text-align: right;
      line-height: 1.65;
      font-size: 0.98rem;
      color: rgba(141, 169, 183, 0.72);
      background: rgba(4, 10, 15, 0.9);
      border-right: 1px solid rgba(122, 232, 229, 0.08);
      overflow: hidden;
      user-select: none;
      pointer-events: none;
    }
    textarea.editor {
      display: block;
      flex: 1 1 auto;
      height: 100%;
      min-height: 0;
      padding: 20px 22px 20px 18px;
      line-height: 1.65;
      font-size: 0.98rem;
      background: transparent;
      tab-size: 4;
      caret-color: var(--accent);
      background-image:
        linear-gradient(rgba(255,255,255,0.035) 1px, transparent 1px),
        linear-gradient(90deg, rgba(61, 184, 255, 0.03) 1px, transparent 1px);
      background-size: 100% calc(1.65em);
      background-attachment: local;
      overflow: auto;
    }
    .editor-subhead {
      padding: 12px 18px 0;
      color: var(--muted);
      font-size: 0.84rem;
      display: flex;
      justify-content: space-between;
      flex-wrap: wrap;
      gap: 8px;
    }
    .radar-grid {
      display: grid;
      grid-template-columns: repeat(2, minmax(0, 1fr));
      gap: 12px;
      padding: 14px 16px 10px;
    }
    .mini-card {
      position: relative;
      padding: 18px;
      border-radius: 24px;
      border: 1px solid rgba(122, 232, 229, 0.12);
      background:
        radial-gradient(circle at top right, rgba(61, 184, 255, 0.16), transparent 32%),
        linear-gradient(180deg, rgba(11, 28, 39, 0.92), rgba(7, 18, 26, 0.86));
      overflow: hidden;
      min-height: 124px;
    }
    .mini-card::after {
      content: "";
      position: absolute;
      inset: auto 14px 0;
      height: 2px;
      background: linear-gradient(90deg, rgba(24, 244, 255, 0.42), rgba(255, 147, 97, 0.2), transparent);
      pointer-events: none;
    }
    .mini-card span {
      display: block;
      font-size: 0.72rem;
      text-transform: uppercase;
      letter-spacing: 0.14em;
      color: var(--muted);
      margin-bottom: 8px;
    }
    .mini-card strong {
      display: block;
      font-size: 1.3rem;
    }
    .permission-row {
      padding: 8px 16px 0;
    }
    .power-grid {
      display: grid;
      grid-template-columns: repeat(auto-fit, minmax(240px, 1fr));
      grid-auto-rows: minmax(196px, auto);
      gap: 14px;
      padding: 12px 18px 18px;
      min-height: 0;
      overflow: auto;
      align-content: start;
      scrollbar-width: thin;
      scrollbar-color: rgba(100, 216, 218, 0.4) transparent;
    }
    .power-card {
      position: relative;
      padding: 20px;
      border-radius: 28px;
      border: 1px solid rgba(122, 232, 229, 0.12);
      background:
        radial-gradient(circle at top right, rgba(61, 184, 255, 0.18), transparent 30%),
        radial-gradient(circle at bottom left, rgba(255, 179, 111, 0.1), transparent 30%),
        linear-gradient(180deg, rgba(10, 24, 34, 0.94), rgba(7, 17, 24, 0.9));
      display: grid;
      gap: 12px;
      min-height: 0;
      overflow: hidden;
      align-content: start;
      box-shadow: inset 0 1px 0 rgba(255,255,255,0.03), 0 18px 32px rgba(0, 0, 0, 0.24);
    }
    .power-card::before {
      content: "";
      position: absolute;
      inset: 0;
      background:
        linear-gradient(90deg, rgba(24, 244, 255, 0.08), transparent 24%),
        linear-gradient(180deg, rgba(255,255,255,0.03), transparent 16%);
      pointer-events: none;
    }
    .power-card > * {
      position: relative;
      z-index: 1;
    }
    .power-card-hero,
    .power-card-boosters,
    .power-card-activity {
      grid-column: 1 / -1;
    }
    .power-card-hero {
      min-height: 218px;
    }
    .power-card-control {
      min-height: 206px;
    }
    .power-card-boosters {
      min-height: 244px;
    }
    .power-card-activity {
      min-height: 256px;
    }
    .power-card span {
      display: block;
      font-size: 0.72rem;
      text-transform: uppercase;
      letter-spacing: 0.14em;
      color: var(--muted);
    }
    .power-copy {
      color: var(--muted);
      font-size: 0.88rem;
      line-height: 1.62;
      margin-top: -1px;
    }
    .power-actions,
    .power-form {
      display: flex;
      flex-wrap: wrap;
      gap: 8px;
      align-items: center;
    }
    .power-actions.scroller {
      flex-wrap: wrap;
      max-width: 100%;
      max-height: none;
      overflow: visible;
      align-content: start;
      padding-right: 0;
      scrollbar-width: thin;
      scrollbar-color: rgba(100, 216, 218, 0.4) transparent;
    }
    .power-card-hero .power-actions.scroller {
      max-height: none;
    }
    .power-card-boosters .power-actions.scroller,
    .power-card-activity .activity-list {
      max-height: none;
    }
    .power-actions button,
    .power-actions a.docs-link,
    .power-form button {
      padding: 10px 14px;
      border-radius: 14px;
      font-size: 0.82rem;
      flex: 0 0 auto;
    }
    .power-form {
      display: flex;
      flex-wrap: wrap;
      gap: 8px;
      align-items: center;
    }
    .power-form .input-shell {
      flex: 1 1 100%;
      min-width: 0;
    }
    .activity-list {
      display: grid;
      gap: 10px;
      min-height: 0;
      height: 100%;
      overflow: auto;
      scrollbar-width: thin;
      scrollbar-color: rgba(100, 216, 218, 0.4) transparent;
    }
    .activity-list .empty-state {
      margin: 0;
    }
    .activity-item {
      padding: 12px 14px;
      border-radius: 16px;
      border: 1px solid rgba(122, 232, 229, 0.1);
      background: rgba(7, 18, 25, 0.78);
      display: grid;
      gap: 4px;
    }
    .activity-item strong {
      font-size: 0.84rem;
    }
    .activity-item small {
      color: var(--muted);
    }
    button.permission-chip {
      cursor: pointer;
    }
    .permission-chip.on {
      border-color: rgba(99, 223, 159, 0.34);
      background: rgba(11, 34, 25, 0.72);
      color: var(--success);
    }
    .permission-chip.off {
      border-color: rgba(255, 147, 97, 0.18);
      background: rgba(32, 20, 16, 0.72);
      color: #ffc39d;
    }
    .snippet-grid {
      padding: 12px;
      display: grid;
      grid-template-columns: repeat(2, minmax(0, 1fr));
      gap: 10px;
      overflow: auto;
      min-height: 0;
      align-content: start;
    }
    .snippet-card {
      background: linear-gradient(180deg, rgba(13, 28, 35, 0.82), rgba(9, 21, 26, 0.78));
    }
    .subpanel-head {
      margin: 0 16px 0;
      border-radius: 22px 22px 0 0;
      padding: 16px 18px 12px;
      border-bottom-color: rgba(122, 232, 229, 0.1);
      background: linear-gradient(180deg, rgba(13, 30, 41, 0.96), rgba(8, 20, 29, 0.78));
    }
    .radar-panel .outline-list,
    .radar-panel .snippet-grid {
      margin: 0 16px 16px;
      border: 1px solid rgba(122, 232, 229, 0.1);
      border-top: none;
      border-radius: 0 0 20px 20px;
      background: rgba(7, 18, 25, 0.58);
      box-shadow: inset 0 1px 0 rgba(255,255,255,0.02);
    }
    .console-panel {
      min-height: 0;
      display: grid;
      grid-template-rows: auto auto minmax(0, 1fr) auto;
    }
    .console-panel pre.output {
      margin: 12px 18px 0;
      border-radius: 24px;
      border: 1px solid rgba(122, 232, 229, 0.1);
      background:
        linear-gradient(180deg, rgba(8, 18, 27, 0.92), rgba(5, 12, 18, 0.92)),
        radial-gradient(circle at top right, rgba(61, 184, 255, 0.08), transparent 28%);
      box-shadow: inset 0 1px 0 rgba(255,255,255,0.02);
    }
    .tabs {
      display: grid;
      grid-template-columns: repeat(auto-fit, minmax(132px, 1fr));
      gap: 12px;
      padding: 16px 18px 10px;
    }
    .tab {
      width: 100%;
      justify-content: center;
      text-align: center;
      white-space: normal;
      padding: 14px 16px;
      cursor: pointer;
      font-size: 0.82rem;
      letter-spacing: 0.08em;
      text-transform: uppercase;
      border-radius: 16px;
      line-height: 1.25;
      min-height: 64px;
    }
    .tab.active {
      background: linear-gradient(135deg, rgba(24, 244, 255, 0.22), rgba(255, 147, 97, 0.18));
      border-color: rgba(24, 244, 255, 0.32);
    }
    pre.output {
      flex: 1 1 auto;
      min-height: 0;
      margin: 0;
      padding: 18px;
      white-space: pre-wrap;
      word-break: break-word;
      line-height: 1.6;
      font-size: 0.92rem;
    }
    .console-foot {
      padding: 12px 20px 18px;
      color: var(--muted);
      font-size: 0.82rem;
    }
    .terminal-panel textarea.input {
      flex: 1 1 auto;
      min-height: 0;
      min-height: 172px;
      padding: 16px 18px;
      line-height: 1.55;
      background: rgba(8, 18, 27, 0.88);
      border: 1px solid rgba(122, 232, 229, 0.1);
      border-radius: 22px;
      box-shadow: inset 0 1px 0 rgba(255,255,255,0.02);
    }
    .terminal-panel .panel-body {
      display: grid;
      grid-template-rows: minmax(176px, 1fr) auto auto auto;
      min-height: 0;
      gap: 12px;
      padding: 10px 0 14px;
    }
    .agent-panel {
      display: grid;
      gap: 12px;
      padding: 0 14px;
      margin: 0 18px;
      border-radius: 26px;
      background:
        linear-gradient(180deg, rgba(9, 20, 29, 0.92), rgba(6, 14, 20, 0.9)),
        radial-gradient(circle at top right, rgba(123, 243, 232, 0.06), transparent 30%);
      border: 1px solid rgba(122, 232, 229, 0.08);
      padding-top: 14px;
      padding-bottom: 16px;
    }
    .agent-panel textarea {
      min-height: 132px;
      padding: 16px 18px;
      line-height: 1.55;
      background: rgba(8, 18, 27, 0.88);
      border: 1px solid rgba(122, 232, 229, 0.1);
      border-radius: 20px;
    }
    .agent-actions {
      display: flex;
      flex-wrap: wrap;
      gap: 10px;
      align-items: center;
    }
    .agent-actions .input-shell {
      flex: 1 1 260px;
    }
    .tree-panel,
    .navigator-panel,
    .radar-panel,
    .console-panel,
    .terminal-panel {
      min-height: 0;
    }
    .tree,
    .results-list,
    .outline-list {
      flex: 1 1 auto;
    }
    .radar-panel .panel-body {
      display: grid;
      grid-template-rows: auto auto minmax(0, 1.34fr) auto minmax(0, 0.92fr) auto minmax(0, 0.94fr);
      overflow: hidden;
    }
    .terminal-shell {
      display: flex;
      gap: 12px;
      padding: 14px 16px 16px;
      align-items: center;
    }
    .terminal-tools {
      display: flex;
      flex-wrap: wrap;
      gap: 12px;
      padding: 0 16px;
      align-items: center;
    }
    .terminal-tools a.docs-link,
    .terminal-tools button {
      padding: 10px 14px;
      border-radius: 14px;
      font-size: 0.86rem;
    }
    .terminal-shell .input-shell {
      flex: 1;
    }
    .statusbar {
      display: flex;
      justify-content: space-between;
      align-items: center;
      flex-wrap: wrap;
      gap: 16px;
      padding: 12px 16px;
      font-size: 0.84rem;
      min-width: 0;
    }
    .status-strong {
      color: var(--ink);
      font-weight: 700;
    }
    .success {
      color: var(--success);
    }
    .error {
      color: var(--danger);
    }
    .palette-overlay {
      position: fixed;
      inset: 0;
      display: none;
      align-items: flex-start;
      justify-content: center;
      padding: 72px 18px 18px;
      background: rgba(4, 10, 14, 0.58);
      backdrop-filter: blur(10px);
      z-index: 20;
    }
    .palette-overlay.open {
      display: flex;
    }
    .palette-card {
      width: min(780px, calc(100vw - 32px));
      border-radius: 24px;
      border: 1px solid rgba(122, 232, 229, 0.18);
      background: linear-gradient(180deg, rgba(9, 24, 35, 0.98), rgba(5, 14, 20, 0.98));
      box-shadow: 0 30px 80px rgba(0, 0, 0, 0.44);
      overflow: hidden;
    }
    .palette-head {
      padding: 14px;
      border-bottom: 1px solid rgba(117, 209, 222, 0.1);
    }
    .palette-list {
      max-height: 56vh;
      overflow: auto;
      padding: 10px;
      display: grid;
      gap: 8px;
    }
    .palette-action {
      border-radius: 16px;
      border: 1px solid transparent;
      background: rgba(9, 22, 27, 0.8);
      padding: 12px 14px;
      text-align: left;
      cursor: pointer;
    }
    .palette-action:hover {
      border-color: rgba(24, 244, 255, 0.22);
      background: rgba(15, 36, 42, 0.88);
    }
    .palette-action strong {
      display: block;
      margin-bottom: 4px;
    }
    .empty-state {
      padding: 16px;
      color: var(--muted);
      text-align: center;
      border: 1px dashed rgba(117, 209, 222, 0.14);
      border-radius: 16px;
      margin: 0 14px 14px;
    }
    @media (max-width: 1420px) {
      .workspace {
        grid-template-columns: minmax(220px, 0.66fr) minmax(520px, 1.5fr) minmax(420px, 1.08fr);
      }
    }
    @media (max-height: 860px) {
      .hero {
        min-height: 128px;
        padding: 10px 14px;
      }
      .hero h1 {
        font-size: clamp(1.5rem, 2.3vw, 2.1rem);
      }
      .hero p {
        font-size: 0.84rem;
      }
      .metric-card {
        min-height: 84px;
      }
      .toolbar {
        padding: 8px 10px;
      }
      .statusbar {
        padding: 8px 12px;
      }
    }
    @media (max-width: 1180px) {
      html, body {
        overflow: auto;
      }
      .app {
        min-height: 100vh;
        height: auto;
        overflow: visible;
      }
      .toolbar {
        flex-wrap: wrap;
        max-height: none;
        overflow: visible;
      }
      .workspace-rail {
        display: grid;
        grid-template-columns: repeat(3, minmax(0, 1fr));
      }
      .hero,
      .workspace {
        grid-template-columns: 1fr;
      }
      .hero h1 {
        max-width: none;
      }
      .workspace {
        display: block;
        min-height: auto;
        height: auto;
        overflow: visible;
      }
      .column {
        display: contents;
      }
      .workspace-panel {
        display: none;
        min-height: clamp(360px, 68vh, 760px);
      }
      .workspace-panel.active-pane {
        display: flex;
      }
      .editor-panel.workspace-panel.active-pane,
      .console-panel.workspace-panel.active-pane {
        display: grid;
      }
      .radar-panel .panel-body {
        display: grid;
        grid-template-rows: auto auto minmax(140px, auto) auto minmax(120px, 1fr) auto minmax(110px, 0.8fr);
      }
      .editor-tools {
        grid-template-columns: 1fr;
      }
      .editor-stats {
        justify-content: flex-start;
      }
      .tabs {
        grid-template-columns: repeat(3, minmax(132px, 1fr));
      }
      .power-grid {
        grid-template-columns: 1fr;
        grid-auto-rows: minmax(176px, auto);
      }
      .power-card-hero,
      .power-card-control,
      .power-card-boosters,
      .power-card-activity {
        grid-column: 1;
      }
    }
    @media (max-width: 720px) {
      .workspace-rail {
        grid-template-columns: repeat(2, minmax(140px, 1fr));
      }
      .tabs {
        grid-template-columns: repeat(2, minmax(140px, 1fr));
      }
      .pane-tab,
      .tab {
        white-space: normal;
        line-height: 1.25;
      }
    }
  </style>
</head>
<body>
  <div class="app">
    <section class="hero glass">
      <div class="hero-copy">
        <div class="kicker">SNSX Unified Studio</div>
        <h1>Orbital-grade coding for the entire SNSX system.</h1>
        <p>
          A single cinematic control room for writing, running, auditing, scaffolding, and directing SNSX projects.
          File intelligence, strict security, AI-assisted authoring, live terminal input, and build systems stay fused
          into one high-signal workspace instead of scattering across separate tools.
        </p>
        <div class="hero-meta">
          <div class="hero-chip">Command Deck: <strong>Ctrl/Cmd + K</strong></div>
          <div class="hero-chip">Navigator: <strong>Ctrl/Cmd + P</strong></div>
          <div class="hero-chip">Text Search: <strong>Ctrl/Cmd + Shift + F</strong></div>
        </div>
      </div>
      <div class="hero-metrics">
        <div class="mode-pill" id="modePill">SNSX MODE</div>
        <div class="metric-card">
          <span>Security Blockers</span>
          <strong id="metricBlockers">0</strong>
          <small id="metricBlockersNote">Strict audit clean</small>
        </div>
        <div class="metric-card">
          <span>Workspace Files</span>
          <strong id="metricFiles">0</strong>
          <small id="metricSources">0 SNSX source files</small>
        </div>
        <div class="metric-card">
          <span>Dependencies</span>
          <strong id="metricDependencies">0</strong>
          <small id="metricDependencyList">No extra dependencies</small>
        </div>
        <div class="metric-card">
          <span>Entry Flow</span>
          <strong id="metricEntry">src/main.snsx</strong>
          <small id="rootMeta">.</small>
        </div>
      </div>
    </section>

    <section class="toolbar">
      <button class="primary" id="saveBtn">Save</button>
      <button class="primary" id="runBtn">Run</button>
      <button id="auditBtn">Audit</button>
      <button id="doctorBtn">Doctor</button>
      <button id="buildVmBtn">Build VM</button>
      <button id="buildWasmBtn">Build WASM</button>
      <button id="buildLlvmBtn">Build LLVM</button>
      <button id="buildNativeBtn">Build Native</button>
      <button id="setEntryBtn">Set Entry</button>
      <button id="newFileBtn">New File</button>
      <button id="newFolderBtn">New Folder</button>
      <button id="renameBtn">Rename</button>
      <button class="warn" id="deleteBtn">Delete</button>
      <button class="subtle" id="refreshBtn">Refresh</button>
      <div class="toolbar-spacer"></div>
      <div class="toolbar-note">Orbital command dock · strict compiler gate · shared shell backend</div>
      <a class="docs-link" href="/docs/manual" target="_blank" rel="noopener">Manual</a>
      <a class="docs-link" href="/docs/spec" target="_blank" rel="noopener">Spec</a>
      <a class="docs-link" href="/docs/techspec" target="_blank" rel="noopener">Tech Spec</a>
      <a class="docs-link" href="/docs/readme" target="_blank" rel="noopener">Docs</a>
      <button class="subtle" id="commandPaletteBtn">Command Deck</button>
    </section>

    <nav class="workspace-rail glass" id="workspaceRail" aria-label="Workspace panels">
      <button class="pane-tab" data-pane="navigator" aria-pressed="false">Navigator</button>
      <button class="pane-tab" data-pane="tree" aria-pressed="false">Files</button>
      <button class="pane-tab active" data-pane="editor" aria-pressed="true">Editor</button>
      <button class="pane-tab" data-pane="radar" aria-pressed="false">Radar</button>
      <button class="pane-tab" data-pane="console" aria-pressed="false">Output</button>
      <button class="pane-tab" data-pane="terminal" aria-pressed="false">Terminal</button>
    </nav>

    <section class="workspace">
      <div class="column column-left">
        <section class="panel navigator-panel workspace-panel" data-pane="navigator">
          <div class="panel-head">
            <strong>Workspace Navigator</strong>
            <span class="meta" id="entryMeta">Entry: src/main.snsx</span>
          </div>
          <div class="panel-body">
            <div class="navigator-top">
              <input class="navigator-input input-shell" id="navigatorInput" placeholder="Find files or search text across the workspace">
              <div class="navigator-actions">
                <button class="subtle" id="findFilesBtn">Find Files</button>
                <button class="subtle" id="searchTextBtn">Search Text</button>
                <div class="hero-chip" id="navigatorModeLabel">Mode: file finder</div>
              </div>
            </div>
            <div class="results-list" id="navigatorResults"></div>
          </div>
        </section>

        <section class="panel tree-panel workspace-panel" data-pane="tree">
          <div class="panel-head">
            <strong>Workspace Tree</strong>
            <span class="meta" id="treeMeta">Root</span>
          </div>
          <div class="panel-body">
            <div class="tree" id="treeView"></div>
          </div>
        </section>
      </div>

      <section class="panel editor-panel workspace-panel active-pane" data-pane="editor">
        <div class="panel-head">
          <strong id="editorTitle">Editor</strong>
          <span class="meta" id="editorMeta">SNSX source surface</span>
        </div>
        <div class="editor-subhead">
          <span id="editorPath">No file open</span>
          <span>Bring-first imports · strict contracts · text-like SNSX coding</span>
        </div>
        <div class="editor-tools">
          <div class="editor-actions">
            <div class="hero-chip">Smart Editor</div>
            <button class="subtle" id="formatEditorBtn">Format</button>
            <button class="subtle" id="indentSelectionBtn">Indent</button>
            <button class="subtle" id="outdentSelectionBtn">Outdent</button>
          </div>
          <div class="editor-stats">
            <div class="hero-chip" id="editorLanguageStat">SNSX</div>
            <div class="hero-chip" id="editorLineStat">1 line</div>
            <div class="hero-chip" id="editorCursorStat">Ln 1, Col 1</div>
          </div>
        </div>
        <div class="editor-assist" id="editorAssist">Auto-indent on Enter, Tab and Shift+Tab blocks, paired brackets, clean paste, line gutter, cursor telemetry, and Cmd/Ctrl+Shift+F formatting.</div>
        <div class="editor-surface">
          <pre class="code-gutter" id="editorGutter">1</pre>
          <textarea class="editor" id="editor" spellcheck="false"></textarea>
        </div>
      </section>

      <div class="column column-right">
        <section class="panel radar-panel workspace-panel" data-pane="radar">
          <div class="panel-head">
            <strong>Project Radar</strong>
            <span class="meta">Permissions, outline, snippet forge</span>
          </div>
          <div class="panel-body">
            <div class="radar-grid">
              <div class="mini-card">
                <span>Current Entry</span>
                <strong id="overviewEntry">src/main.snsx</strong>
              </div>
              <div class="mini-card">
                <span>Launch URL</span>
                <strong id="overviewUrl">studio</strong>
              </div>
            </div>
            <div class="permission-row" id="permissionList"></div>
            <div class="power-grid">
              <div class="power-card power-card-hero">
                <span>Power Recipes</span>
                <div class="power-copy">Run, build, audit, and inspect the active workspace from one launch lane without leaving the radar.</div>
                <div class="power-actions scroller">
                  <button class="subtle power-control shell-quick" data-command="status" data-label="Loading status">Status</button>
                  <button class="subtle power-control shell-quick" data-command="doctor" data-label="Running doctor">Doctor</button>
                  <button class="subtle power-control shell-quick" data-command="manifest" data-label="Loading manifest">Manifest</button>
                  <button class="subtle power-control shell-quick" data-command="entry" data-label="Loading entry">Entry</button>
                  <button class="subtle power-control shell-quick" data-command="outline" data-label="Loading outline">Outline</button>
                  <button class="subtle power-control shell-quick" data-command="tree" data-label="Loading tree">Tree</button>
                  <button class="subtle power-control shell-quick" data-command="run" data-label="Running program">Run</button>
                  <button class="subtle power-control shell-quick" data-command="audit" data-label="Running audit">Audit</button>
                  <button class="subtle power-control shell-quick" data-command="build vm" data-label="Building VM artifact">VM</button>
                  <button class="subtle power-control shell-quick" data-command="build wasm" data-label="Building WASM artifact">WASM</button>
                  <button class="subtle power-control shell-quick" data-command="build llvm" data-label="Building LLVM artifact">LLVM</button>
                  <button class="subtle power-control shell-quick" data-command="build native-x86_64" data-label="Building native artifact">Native</button>
                </div>
              </div>
              <div class="power-card power-card-hero">
                <span>Workspace Intelligence</span>
                <div class="power-copy">Dependencies, tools, docs, and workspace diagnostics stay grouped as a single intelligence deck.</div>
                <div class="power-actions scroller">
                  <button class="subtle power-control shell-quick" data-command="deps" data-label="Loading dependencies">Deps</button>
                  <button class="subtle power-control shell-quick" data-command="package list" data-label="Listing packages">Packages</button>
                  <button class="subtle power-control shell-quick" data-command="module list" data-label="Listing modules">Modules</button>
                  <button class="subtle power-control shell-quick" data-command="history" data-label="Loading shell history">History</button>
                  <button class="subtle power-control shell-quick" data-command="tools" data-label="Loading tools">Tools</button>
                  <button class="subtle power-control shell-quick" data-command="docs" data-label="Loading docs">Docs</button>
                  <a class="docs-link" href="/docs/manual" target="_blank" rel="noopener">Manual</a>
                  <a class="docs-link" href="/docs/spec" target="_blank" rel="noopener">Spec</a>
                  <a class="docs-link" href="/docs/techspec" target="_blank" rel="noopener">Tech Spec</a>
                </div>
              </div>
              <div class="power-card power-card-control">
                <span>Package Control</span>
                <div class="power-copy">Add, remove, or inspect manifest packages from a focused control bay.</div>
                <div class="power-form">
                  <input class="input-shell power-control" id="packageInput" placeholder="Package name, for example astro.http">
                  <button class="primary power-control" id="packageAddBtn">Add</button>
                  <button class="subtle power-control" id="packageRemoveBtn">Remove</button>
                  <button class="subtle power-control" id="packageListBtn">List</button>
                </div>
              </div>
              <div class="power-card power-card-control">
                <span>Module Control</span>
                <div class="power-copy">Install, scaffold, and inspect modules without breaking your current editor flow.</div>
                <div class="power-form">
                  <input class="input-shell power-control" id="moduleInput" placeholder="Module name, for example std.design">
                  <button class="primary power-control" id="moduleInstallBtn">Install</button>
                  <button class="subtle power-control" id="moduleScaffoldBtn">Scaffold</button>
                  <button class="subtle power-control" id="moduleListBtn">List</button>
                </div>
              </div>
              <div class="power-card power-card-boosters">
                <span>Authoring Boosters</span>
                <div class="power-copy">Spin up starter flows, AI drafts, and reusable surfaces from one authoring bay.</div>
                <div class="power-actions scroller">
                  <button class="subtle power-control snippet-quick" data-snippet="starter">Starter</button>
                  <button class="subtle power-control snippet-quick" data-snippet="mind">Mind</button>
                  <button class="subtle power-control snippet-quick" data-snippet="service">Service</button>
                  <button class="subtle power-control snippet-quick" data-snippet="design">Design</button>
                  <button class="subtle power-control snippet-quick" data-snippet="worker">Worker</button>
                  <button class="subtle power-control snippet-quick" data-snippet="tensor">Tensor</button>
                  <button class="subtle power-control" id="powerAgentPreviewBtn">AI Preview</button>
                  <button class="subtle power-control" id="powerAgentApplyBtn">AI Apply</button>
                </div>
              </div>
              <div class="power-card power-card-activity">
                <span>Recent Activity</span>
                <div class="power-copy">Recent builds, audits, saves, and shell actions stay visible as a live command ledger.</div>
                <div class="activity-list" id="activityFeed"></div>
              </div>
            </div>
            <div class="panel-head subpanel-head">
              <strong>Flow Map</strong>
              <span class="meta">Current file outline</span>
            </div>
            <div class="outline-list" id="outlineView"></div>
            <div class="panel-head subpanel-head">
              <strong>Snippet Forge</strong>
              <span class="meta">Scaffold starter surfaces</span>
            </div>
            <div class="snippet-grid" id="snippetDeck"></div>
          </div>
        </section>

        <section class="panel console-panel workspace-panel" data-pane="console">
          <div class="panel-head">
            <strong>Execution Deck</strong>
            <span class="meta" id="outputMeta">Run output and diagnostics</span>
          </div>
          <div class="tabs">
            <div class="tab active" data-tab="run">Run</div>
            <div class="tab" data-tab="build">Build</div>
            <div class="tab" data-tab="audit">Security</div>
            <div class="tab" data-tab="doctor">Doctor</div>
            <div class="tab" data-tab="shell">Shell</div>
          </div>
          <pre class="output" id="outputPane"></pre>
          <div class="console-foot">Run, build, audit, and shell all stay attached to the same strict SNSX workspace state.</div>
        </section>

        <section class="panel terminal-panel workspace-panel" data-pane="terminal">
          <div class="panel-head">
            <strong>Input + SNSX Terminal</strong>
            <span class="meta" id="hintText">Ctrl/Cmd+S save · Ctrl/Cmd+R run current file · live terminal input on every ask() · Ctrl/Cmd+K command deck</span>
          </div>
          <div class="panel-body">
            <textarea class="input" id="stdinInput" placeholder="Program input. Use one line per ask(), or paste full stdin for read_stdin()."></textarea>
            <div class="agent-panel">
              <div class="panel-head" style="margin: 0; border-radius: 16px; padding: 12px 14px 10px;">
                <strong>SNS AI Coding Agent</strong>
                <span class="meta">Local prompt-to-SNSX generator</span>
              </div>
              <textarea class="input" id="agentPrompt" placeholder="Describe the SNSX program you want. Example: write a grade checker that asks for marks and shows the grade."></textarea>
              <div class="agent-actions">
                <input class="input-shell" id="agentTarget" placeholder="Target file path, for example src/grade_checker.snsx">
                <button class="subtle" id="agentPreviewBtn">Preview Code</button>
                <button class="primary" id="agentApplyBtn">Apply to File</button>
              </div>
            </div>
            <div class="terminal-tools">
              <button class="subtle" id="promptRunBtn">Run Current</button>
              <button class="subtle" id="cancelRunBtn">Cancel Wait</button>
              <button class="subtle" id="historyBtn">History</button>
              <button class="subtle" id="clearInputBtn">Clear Input</button>
              <a class="docs-link" href="/docs/manual" target="_blank" rel="noopener">Manual</a>
              <a class="docs-link" href="/docs/techspec" target="_blank" rel="noopener">Tech Spec</a>
            </div>
            <div class="terminal-shell">
              <input class="input-shell" id="shellInput" placeholder="SNSX terminal: help, tools, status, permissions, deps, doctor, package list, module install std.design, !git status">
              <button id="shellBtn">Run SNSX</button>
            </div>
          </div>
        </section>
      </div>
    </section>

    <section class="statusbar">
      <div id="statusText" class="status-strong">SNSX future studio ready</div>
      <div>All file actions are confined to the active workspace root.</div>
    </section>
  </div>

  <div class="palette-overlay" id="palette">
    <div class="palette-card">
      <div class="palette-head">
        <input class="palette" id="paletteInput" placeholder="Search actions: run, audit, find files, snippet starter, doctor">
      </div>
      <div class="palette-list" id="paletteResults"></div>
    </div>
  </div>

  <script>
    const BOOTSTRAP = __SNSX_BOOTSTRAP__;
    const state = {
      token: BOOTSTRAP.token,
      root: BOOTSTRAP.root,
      entry: BOOTSTRAP.entry,
      currentPath: BOOTSTRAP.entry,
      currentTab: 'run',
      activePane: 'editor',
      dirty: false,
      busy: false,
      tree: null,
      navigatorMode: 'find',
      paletteOpen: false,
      runSession: null,
      summary: null,
      snippets: [],
      activity: [],
      logs: {
        run: 'Run output will appear here.',
        build: 'Build output will appear here.',
        audit: 'Strict security diagnostics will appear here.',
        doctor: 'Workspace doctor output will appear here.',
        shell: 'SNSX terminal output will appear here.'
      }
    };

    const ui = {
      modePill: document.getElementById('modePill'),
      rootMeta: document.getElementById('rootMeta'),
      entryMeta: document.getElementById('entryMeta'),
      treeMeta: document.getElementById('treeMeta'),
      editorTitle: document.getElementById('editorTitle'),
      editorMeta: document.getElementById('editorMeta'),
      editorPath: document.getElementById('editorPath'),
      treeView: document.getElementById('treeView'),
      editor: document.getElementById('editor'),
      editorGutter: document.getElementById('editorGutter'),
      editorAssist: document.getElementById('editorAssist'),
      outputPane: document.getElementById('outputPane'),
      outputMeta: document.getElementById('outputMeta'),
      stdinInput: document.getElementById('stdinInput'),
      shellInput: document.getElementById('shellInput'),
      agentPrompt: document.getElementById('agentPrompt'),
      agentTarget: document.getElementById('agentTarget'),
      statusText: document.getElementById('statusText'),
      hintText: document.getElementById('hintText'),
      shellButton: document.getElementById('shellBtn'),
      navigatorInput: document.getElementById('navigatorInput'),
      navigatorModeLabel: document.getElementById('navigatorModeLabel'),
      navigatorResults: document.getElementById('navigatorResults'),
      outlineView: document.getElementById('outlineView'),
      snippetDeck: document.getElementById('snippetDeck'),
      overviewEntry: document.getElementById('overviewEntry'),
      overviewUrl: document.getElementById('overviewUrl'),
      permissionList: document.getElementById('permissionList'),
      metricBlockers: document.getElementById('metricBlockers'),
      metricBlockersNote: document.getElementById('metricBlockersNote'),
      metricFiles: document.getElementById('metricFiles'),
      metricSources: document.getElementById('metricSources'),
      metricDependencies: document.getElementById('metricDependencies'),
      metricDependencyList: document.getElementById('metricDependencyList'),
      metricEntry: document.getElementById('metricEntry'),
      editorLanguageStat: document.getElementById('editorLanguageStat'),
      editorLineStat: document.getElementById('editorLineStat'),
      editorCursorStat: document.getElementById('editorCursorStat'),
      formatEditorBtn: document.getElementById('formatEditorBtn'),
      indentSelectionBtn: document.getElementById('indentSelectionBtn'),
      outdentSelectionBtn: document.getElementById('outdentSelectionBtn'),
      packageInput: document.getElementById('packageInput'),
      moduleInput: document.getElementById('moduleInput'),
      activityFeed: document.getElementById('activityFeed'),
      palette: document.getElementById('palette'),
      paletteInput: document.getElementById('paletteInput'),
      paletteResults: document.getElementById('paletteResults'),
      paneButtons: Array.from(document.querySelectorAll('.pane-tab')),
      workspacePanels: Array.from(document.querySelectorAll('.workspace-panel')),
      shellQuickButtons: Array.from(document.querySelectorAll('.shell-quick')),
      snippetQuickButtons: Array.from(document.querySelectorAll('.snippet-quick'))
    };

    const tabLabels = {
      run: 'Runtime execution and return state',
      build: 'Compiler, artifact, and backend output',
      audit: 'Strict compiler security diagnostics',
      doctor: 'Workspace doctor, health, and readiness report',
      shell: 'SNSX workspace shell and host shell output'
    };

    const interactiveIds = [
      'saveBtn',
      'runBtn',
      'auditBtn',
      'doctorBtn',
      'buildVmBtn',
      'buildWasmBtn',
      'buildLlvmBtn',
      'buildNativeBtn',
      'setEntryBtn',
      'newFileBtn',
      'newFolderBtn',
      'renameBtn',
      'deleteBtn',
      'refreshBtn',
      'shellBtn',
      'agentPreviewBtn',
      'agentApplyBtn',
      'promptRunBtn',
      'cancelRunBtn',
      'historyBtn',
      'clearInputBtn',
      'findFilesBtn',
      'searchTextBtn',
      'commandPaletteBtn',
      'formatEditorBtn',
      'indentSelectionBtn',
      'outdentSelectionBtn',
      'packageAddBtn',
      'packageRemoveBtn',
      'packageListBtn',
      'moduleInstallBtn',
      'moduleScaffoldBtn',
      'moduleListBtn',
      'powerAgentPreviewBtn',
      'powerAgentApplyBtn'
    ];

    const paletteActions = [
      { id: 'save', label: 'Save current file', detail: 'Write the editor buffer to disk.', run: () => saveCurrent() },
      { id: 'run', label: 'Run current file', detail: 'Save if needed, then execute the SNSX file open in the editor.', run: () => runCurrent() },
      { id: 'audit', label: 'Run strict audit', detail: 'Refresh compiler security blockers and unlock state.', run: () => auditCurrent() },
      { id: 'doctor', label: 'Workspace doctor', detail: 'Show project health, permissions, and audit readiness.', run: () => runDoctor() },
      { id: 'agent-preview', label: 'Preview SNS AI code', detail: 'Generate SNSX from the AI agent prompt panel.', run: () => previewAgent() },
      { id: 'agent-apply', label: 'Apply SNS AI code', detail: 'Write generated SNSX into the target file from the AI panel.', run: () => applyAgent() },
      { id: 'history', label: 'Show shell history', detail: 'Display recent SNSX shell commands for this studio session.', run: async () => runShellCommand('history') },
      { id: 'status', label: 'Show terminal status', detail: 'Display the current workspace, entry, policy, and dependency snapshot.', run: async () => runShellCommand('status') },
      { id: 'permissions', label: 'Show permissions policy', detail: 'Display the active strict permission policy for the current entry.', run: async () => runShellCommand('permissions') },
      { id: 'deps', label: 'Show dependency report', detail: 'Display manifest dependencies and vendored module state.', run: async () => runShellCommand('deps') },
      { id: 'tools', label: 'Show terminal tools', detail: 'List the full SNSX terminal toolset available in this studio.', run: async () => runShellCommand('tools') },
      { id: 'docs', label: 'Show docs catalog', detail: 'List manual, spec, and technical specification locations.', run: async () => runShellCommand('docs') },
      { id: 'build-vm', label: 'Build VM artifact', detail: 'Emit SNSX VM bytecode for the SNSX file open in the editor.', run: () => buildCurrent('vm') },
      { id: 'build-wasm', label: 'Build WASM artifact', detail: 'Emit WebAssembly text for the SNSX file open in the editor.', run: () => buildCurrent('wasm') },
      { id: 'build-llvm', label: 'Build LLVM artifact', detail: 'Emit LLVM IR text for the SNSX file open in the editor.', run: () => buildCurrent('llvm') },
      { id: 'build-native', label: 'Build native-x86_64 artifact', detail: 'Emit native assembly text for the SNSX file open in the editor.', run: () => buildCurrent('native-x86_64') },
      { id: 'find', label: 'Focus file navigator', detail: 'Search workspace paths from the navigator pane.', run: () => focusNavigator('find') },
      { id: 'search', label: 'Focus text search', detail: 'Search source content across the workspace.', run: () => focusNavigator('search') },
      { id: 'outline', label: 'Refresh flow map', detail: 'Rebuild the outline of the current file.', run: () => loadOutline(state.currentPath) },
      { id: 'snippet-starter', label: 'Scaffold starter snippet', detail: 'Create a minimal entry template file.', run: () => createSnippet('starter') },
      { id: 'snippet-mind', label: 'Scaffold AI mind snippet', detail: 'Create a prompt-based AI flow starter.', run: () => createSnippet('mind') },
      { id: 'snippet-service', label: 'Scaffold service snippet', detail: 'Create typed reusable service-style flows.', run: () => createSnippet('service') }
    ];

    async function api(method, path, body) {
      const response = await fetch(path, {
        method,
        headers: {
          'Content-Type': 'application/json',
          'X-SNSX-Token': state.token
        },
        body: body ? JSON.stringify(body) : undefined
      });
      const payload = await response.json();
      if (!response.ok) {
        throw new Error(payload.error || 'Request failed');
      }
      return payload;
    }

    function setInteractiveBusy(active, label = '') {
      state.busy = active;
      interactiveIds.forEach(id => {
        const node = document.getElementById(id);
        if (node) {
          node.disabled = active;
        }
      });
      document.querySelectorAll('.power-control').forEach(node => {
        node.disabled = active;
      });
      ui.shellInput.disabled = active;
      ui.agentPrompt.disabled = active;
      ui.agentTarget.disabled = active;
      ui.navigatorInput.disabled = active;
      ui.stdinInput.disabled = active;
      ui.packageInput.disabled = active;
      ui.moduleInput.disabled = active;
      if (active && label) {
        setStatus(`${label}...`);
      }
    }

    async function runUiAction(label, action) {
      if (state.busy) {
        return;
      }
      setInteractiveBusy(true, label);
      try {
        return await action();
      } catch (error) {
        handleError(error);
        return null;
      } finally {
        setInteractiveBusy(false);
      }
    }

    async function saveIfDirty() {
      if (state.dirty) {
        await saveCurrent();
      }
    }

    function setStatus(message, ok = true) {
      ui.statusText.textContent = message;
      ui.statusText.className = ok ? 'status-strong success' : 'status-strong error';
    }

    function setLog(tab, title, output) {
      state.logs[tab] = (title ? `${title}\n\n` : '') + (output || '');
      if (state.currentTab === tab) {
        ui.outputPane.textContent = state.logs[tab];
      }
    }

    function renderActivity() {
      ui.activityFeed.innerHTML = '';
      if (!state.activity.length) {
        const empty = document.createElement('div');
        empty.className = 'empty-state';
        empty.textContent = 'Actions, builds, audits, and shell work will appear here.';
        ui.activityFeed.appendChild(empty);
        return;
      }
      state.activity.forEach(item => {
        const card = document.createElement('div');
        card.className = 'activity-item';
        card.innerHTML = `<strong>${item.title}</strong><small>${item.detail}</small><small>${item.time} · ${item.ok ? 'ok' : 'attention'}</small>`;
        ui.activityFeed.appendChild(card);
      });
    }

    function pushActivity(title, detail, ok = true) {
      const time = new Date().toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
      state.activity.unshift({
        title,
        detail,
        ok,
        time
      });
      state.activity = state.activity.slice(0, 10);
      renderActivity();
    }

    const EDITOR_INDENT = '    ';
    const PAIR_MAP = {
      '(': ')',
      '[': ']',
      '{': '}',
      '"': '"',
      "'": "'"
    };
    const BLOCK_OPENERS = new Set(['(', '[', '{']);
    const CLOSING_PAIRS = new Set(Object.values(PAIR_MAP));

    function lineStartAt(text, index) {
      return text.lastIndexOf('\n', Math.max(0, index - 1)) + 1;
    }

    function lineEndAt(text, index) {
      const next = text.indexOf('\n', index);
      return next === -1 ? text.length : next;
    }

    function leadingWhitespace(line) {
      const match = line.match(/^\s*/);
      return match ? match[0] : '';
    }

    function renderEditorGutter() {
      const count = Math.max(1, ui.editor.value.split('\n').length);
      ui.editorGutter.textContent = Array.from({ length: count }, (_, index) => index + 1).join('\n');
      ui.editorGutter.scrollTop = ui.editor.scrollTop;
    }

    function cursorLineColumn() {
      const text = ui.editor.value;
      const start = ui.editor.selectionStart || 0;
      const slice = text.slice(0, start);
      const line = slice.split('\n').length;
      const column = start - lineStartAt(text, start) + 1;
      const selection = Math.max(0, (ui.editor.selectionEnd || 0) - start);
      return { line, column, selection };
    }

    function renderEditorTelemetry() {
      const lineCount = Math.max(1, ui.editor.value.split('\n').length);
      const { line, column, selection } = cursorLineColumn();
      const role = !state.currentPath
        ? 'SNSX'
        : state.currentPath === state.entry
          ? 'SNSX ENTRY'
          : 'SNSX FILE';
      ui.editorLanguageStat.textContent = state.dirty ? `${role} · DIRTY` : role;
      ui.editorLineStat.textContent = `${lineCount} ${lineCount === 1 ? 'line' : 'lines'}`;
      ui.editorCursorStat.textContent = selection > 0
        ? `Ln ${line}, Col ${column} · ${selection} sel`
        : `Ln ${line}, Col ${column}`;
    }

    function markEditorDirty() {
      state.dirty = true;
      ui.editorTitle.textContent = `Editor · ${state.currentPath} *`;
      renderEditorGutter();
      renderEditorTelemetry();
    }

    function sanitizeEditorText(text) {
      return text
        .replace(/\r\n?/g, '\n')
        .replace(/\t/g, EDITOR_INDENT)
        .replace(/[ \t]+$/gm, '')
        .replace(/\n{4,}$/g, '\n\n\n');
    }

    function setEditorSelection(start, end = start) {
      ui.editor.focus();
      ui.editor.selectionStart = start;
      ui.editor.selectionEnd = end;
    }

    function replaceEditorSelection(text, caretOffset = text.length) {
      const start = ui.editor.selectionStart;
      const end = ui.editor.selectionEnd;
      const current = ui.editor.value;
      ui.editor.value = current.slice(0, start) + text + current.slice(end);
      const next = start + caretOffset;
      setEditorSelection(next, next);
      markEditorDirty();
    }

    function formatEditorBuffer(options = {}) {
      const formatted = sanitizeEditorText(ui.editor.value);
      if (formatted !== ui.editor.value) {
        const start = ui.editor.selectionStart;
        const end = ui.editor.selectionEnd;
        ui.editor.value = formatted;
        setEditorSelection(Math.min(formatted.length, start), Math.min(formatted.length, end));
        markEditorDirty();
        if (!options.silent) {
          setStatus('Formatted SNSX editor buffer');
        }
      } else if (!options.silent) {
        setStatus('Editor formatting is already clean');
      }
      renderEditorGutter();
    }

    function indentBlock(outdent = false) {
      const value = ui.editor.value;
      const start = ui.editor.selectionStart;
      const end = ui.editor.selectionEnd;
      const from = lineStartAt(value, start);
      const to = lineEndAt(value, end);
      const original = value.slice(from, to);
      const lines = original.split('\n');
      const updated = lines.map(line => {
        if (!outdent) {
          return EDITOR_INDENT + line;
        }
        return line.replace(/^ {1,4}/, '');
      });
      ui.editor.value = value.slice(0, from) + updated.join('\n') + value.slice(to);
      const nextStart = outdent
        ? Math.max(from, start - EDITOR_INDENT.length)
        : start + EDITOR_INDENT.length;
      const delta = updated.join('\n').length - original.length;
      setEditorSelection(nextStart, Math.max(nextStart, end + delta));
      markEditorDirty();
    }

    function insertSoftTab() {
      const start = ui.editor.selectionStart;
      const end = ui.editor.selectionEnd;
      const selection = ui.editor.value.slice(start, end);
      if (start !== end && selection.includes('\n')) {
        indentBlock(false);
        return;
      }
      const column = start - lineStartAt(ui.editor.value, start);
      const size = EDITOR_INDENT.length - (column % EDITOR_INDENT.length || 0);
      replaceEditorSelection(' '.repeat(size === 0 ? EDITOR_INDENT.length : size));
    }

    function outdentSoftTab() {
      const start = ui.editor.selectionStart;
      const end = ui.editor.selectionEnd;
      const selection = ui.editor.value.slice(start, end);
      if (start !== end && selection.includes('\n')) {
        indentBlock(true);
        return;
      }
      const lineStart = lineStartAt(ui.editor.value, start);
      const currentLine = ui.editor.value.slice(lineStart, lineEndAt(ui.editor.value, start));
      const removal = Math.min(EDITOR_INDENT.length, (currentLine.match(/^ +/) || [''])[0].length);
      if (!removal) {
        return;
      }
      ui.editor.value = ui.editor.value.slice(0, lineStart) + currentLine.slice(removal) + ui.editor.value.slice(lineEndAt(ui.editor.value, start));
      setEditorSelection(Math.max(lineStart, start - removal), Math.max(lineStart, end - removal));
      markEditorDirty();
    }

    function insertSmartNewline() {
      const start = ui.editor.selectionStart;
      const before = ui.editor.value[start - 1];
      const after = ui.editor.value[start];
      const lineStart = lineStartAt(ui.editor.value, start);
      const line = ui.editor.value.slice(lineStart, lineEndAt(ui.editor.value, start));
      if (BLOCK_OPENERS.has(before) && PAIR_MAP[before] === after) {
        const indent = leadingWhitespace(line);
        const innerIndent = indent + EDITOR_INDENT;
        replaceEditorSelection(`\n${innerIndent}\n${indent}`, 1 + innerIndent.length);
        return;
      }
      const trimmed = line.trim();
      let indent = leadingWhitespace(line);
      if (trimmed.endsWith(':') || /^(flow\b|entry\b|actor\b)/.test(trimmed)) {
        indent += EDITOR_INDENT;
      }
      replaceEditorSelection(`\n${indent}`, 1 + indent.length);
    }

    function handleEditorAutoPair(key) {
      const close = PAIR_MAP[key];
      if (!close) {
        return false;
      }
      const start = ui.editor.selectionStart;
      const end = ui.editor.selectionEnd;
      const selected = ui.editor.value.slice(start, end);
      if (selected) {
        replaceEditorSelection(`${key}${selected}${close}`, selected.length + 1);
        setEditorSelection(start + 1, start + selected.length + 1);
      } else {
        replaceEditorSelection(`${key}${close}`, 1);
      }
      return true;
    }

    function maybeDeletePair() {
      const start = ui.editor.selectionStart;
      const end = ui.editor.selectionEnd;
      if (start !== end || start === 0) {
        return false;
      }
      const previous = ui.editor.value[start - 1];
      const next = ui.editor.value[start];
      if (PAIR_MAP[previous] !== next) {
        return false;
      }
      ui.editor.value = ui.editor.value.slice(0, start - 1) + ui.editor.value.slice(start + 1);
      setEditorSelection(start - 1);
      markEditorDirty();
      return true;
    }

    function maybeStepOverClosingPair(key) {
      const start = ui.editor.selectionStart;
      const end = ui.editor.selectionEnd;
      if (start !== end) {
        return false;
      }
      if (ui.editor.value[start] !== key) {
        return false;
      }
      setEditorSelection(start + 1);
      renderEditorTelemetry();
      return true;
    }

    async function executeShellCommand(command) {
      const trimmed = (command || '').trim();
      if (!trimmed) {
        setStatus('Shell command is empty', false);
        return null;
      }
      const localCommand = parseLocalStudioCommand(trimmed);
      if (localCommand) {
        if (localCommand.kind === 'run') {
          return startProgramRun(ui.stdinInput.value);
        }
        if (localCommand.kind === 'build') {
          return buildCurrent(localCommand.target);
        }
        if (localCommand.kind === 'audit') {
          return auditCurrent();
        }
        if (localCommand.kind === 'doctor') {
          return runDoctor();
        }
      }
      const payload = await api('POST', '/api/shell', { command: trimmed, stdin: ui.stdinInput.value });
      setLog('shell', payload.title, payload.output);
      if (payload.open) {
        applyOpenPayload(payload.open);
        await loadOutline(payload.open.path);
      }
      ui.shellInput.value = '';
      switchTab('shell');
      await refreshState();
      setStatus(payload.title, !!payload.ok);
      pushActivity(payload.title, trimmed, !!payload.ok);
      return payload;
    }

    async function runShellCommand(command) {
      await saveIfDirty();
      return executeShellCommand(command);
    }

    async function togglePermission(name) {
      if (!state.summary || !state.summary.permissions) {
        throw new Error('Permission summary is not ready yet');
      }
      const enabled = !!state.summary.permissions[name];
      return runShellCommand(`perm ${name} ${enabled ? 'off' : 'on'}`);
    }

    function requireInputValue(node, message) {
      const value = node.value.trim();
      if (!value) {
        throw new Error(message);
      }
      return value;
    }

    function isCompactLayout() {
      return window.matchMedia('(max-width: 1180px)').matches;
    }

    function setWorkspacePane(pane, options = {}) {
      if (!pane) {
        return;
      }
      state.activePane = pane;
      ui.paneButtons.forEach(node => {
        const active = node.dataset.pane === pane;
        node.classList.toggle('active', active);
        node.setAttribute('aria-pressed', active ? 'true' : 'false');
      });
      ui.workspacePanels.forEach(node => {
        node.classList.toggle('active-pane', node.dataset.pane === pane);
      });
      if (isCompactLayout() && options.scroll !== false) {
        const activeButton = ui.paneButtons.find(node => node.dataset.pane === pane);
        if (activeButton) {
          activeButton.scrollIntoView({ block: 'nearest', inline: 'center', behavior: 'smooth' });
        }
      }
    }

    function switchTab(tab) {
      state.currentTab = tab;
      document.querySelectorAll('.tab').forEach(node => {
        node.classList.toggle('active', node.dataset.tab === tab);
      });
      ui.outputPane.textContent = state.logs[tab] || '';
      ui.outputMeta.textContent = tabLabels[tab] || 'SNSX output';
      if (isCompactLayout()) {
        setWorkspacePane('console', { scroll: false });
      }
    }

    function renderTreeNode(node, depth = 0) {
      const wrapper = document.createElement('div');
      const button = document.createElement('button');
      button.className = 'tree-item' + (state.currentPath === node.path ? ' active' : '');
      button.innerHTML = `<span class="tree-dot">${node.is_dir ? '▣' : '◦'}</span><span>${node.name}</span>`;
      button.style.paddingLeft = `${12 + depth * 14}px`;
      button.onclick = () => runUiAction('Opening file', async () => {
        if (!node.is_dir) {
          await openFile(node.path);
        }
      });
      wrapper.appendChild(button);
      if (node.children && node.children.length) {
        const childWrap = document.createElement('div');
        childWrap.className = 'tree-children';
        node.children.forEach(child => childWrap.appendChild(renderTreeNode(child, depth + 1)));
        wrapper.appendChild(childWrap);
      }
      return wrapper;
    }

    function renderSummary(summary) {
      if (!summary) return;
      state.summary = summary;
      ui.metricBlockers.textContent = summary.blockers;
      ui.metricBlockersNote.textContent = summary.blockers === 0 ? 'Strict audit clean' : 'Run/build remain gated';
      ui.metricFiles.textContent = summary.files;
      ui.metricSources.textContent = `${summary.snsx_files} SNSX source files`;
      ui.metricDependencies.textContent = summary.dependency_count;
      ui.metricDependencyList.textContent = summary.dependencies.length
        ? summary.dependencies.join(', ')
        : 'No extra dependencies';
      ui.metricEntry.textContent = summary.entry;
      ui.overviewEntry.textContent = summary.entry;
      ui.permissionList.innerHTML = '';
      [
        ['fs', summary.permissions.fs],
        ['net', summary.permissions.net],
        ['ai', summary.permissions.ai]
      ].forEach(([name, enabled]) => {
        const chip = document.createElement('button');
        chip.className = `permission-chip ${enabled ? 'on' : 'off'}`;
        chip.textContent = `${name.toUpperCase()} ${enabled ? 'ON' : 'OFF'}`;
        chip.title = `Toggle ${name.toUpperCase()} permission`;
        chip.onclick = () => runUiAction(`Updating ${name.toUpperCase()} permission`, () => togglePermission(name));
        ui.permissionList.appendChild(chip);
      });
    }

    function renderNavigatorResults(mode, results) {
      ui.navigatorResults.innerHTML = '';
      if (!results || !results.length) {
        const empty = document.createElement('div');
        empty.className = 'empty-state';
        empty.textContent = mode === 'find' ? 'No matching files in this workspace.' : 'No matching source text found.';
        ui.navigatorResults.appendChild(empty);
        return;
      }
      results.forEach(result => {
        const button = document.createElement('button');
        button.className = 'result-card';
        if (mode === 'find') {
          button.innerHTML = `<strong>${result.path}</strong><small>Open this workspace path</small>`;
          button.onclick = () => runUiAction('Opening file', () => openFile(result.path));
        } else {
          button.innerHTML = `<strong>${result.path}${result.line ? ` · line ${result.line}` : ''}</strong><small>${result.excerpt}</small>`;
          button.onclick = () => runUiAction('Opening file', () => openFile(result.path, { line: result.line }));
        }
        ui.navigatorResults.appendChild(button);
      });
    }

    function renderOutline(items, title) {
      ui.outlineView.innerHTML = '';
      if (!items || !items.length) {
        const empty = document.createElement('div');
        empty.className = 'empty-state';
        empty.textContent = 'No SNSX declarations detected in this file yet.';
        ui.outlineView.appendChild(empty);
        return;
      }
      items.forEach(item => {
        const button = document.createElement('button');
        button.className = 'outline-item';
        button.innerHTML = `<strong>${item.kind} ${item.label || ''}</strong><small>Line ${item.line}</small>`;
        button.onclick = () => focusLine(item.line);
        ui.outlineView.appendChild(button);
      });
    }

    function renderSnippets(snippets) {
      ui.snippetDeck.innerHTML = '';
      snippets.forEach(snippet => {
        const button = document.createElement('button');
        button.className = 'snippet-card';
        button.innerHTML = `<strong>${snippet.name}</strong><small>${snippet.description}</small>`;
        button.onclick = () => runUiAction('Creating snippet', () => createSnippet(snippet.name));
        ui.snippetDeck.appendChild(button);
      });
    }

    async function refreshState() {
      const payload = await api('GET', '/api/state');
      state.tree = payload.tree;
      state.entry = payload.entry;
      ui.modePill.textContent = `SNSX ${payload.mode.toUpperCase()} MODE`;
      ui.rootMeta.textContent = payload.root;
      ui.treeMeta.textContent = payload.root;
      ui.entryMeta.textContent = `Entry: ${payload.entry}`;
      ui.overviewUrl.textContent = BOOTSTRAP.url;
      renderSummary(payload.summary);
      ui.treeView.innerHTML = '';
      ui.treeView.appendChild(renderTreeNode(payload.tree));
    }

    function applyOpenPayload(payload) {
      clearRunSession();
      state.currentPath = payload.path;
      ui.editor.value = payload.content;
      ui.editorTitle.textContent = `Editor · ${payload.path}`;
      ui.editorPath.textContent = payload.path;
      ui.editorMeta.textContent = payload.is_entry ? 'Active SNSX entry file' : 'Workspace source surface';
      state.dirty = false;
      renderEditorGutter();
      renderEditorTelemetry();
      setWorkspacePane('editor', { scroll: false });
    }

    async function openFile(path, options = {}) {
      if (state.dirty && !options.force && !confirm('Discard unsaved editor changes?')) {
        return;
      }
      const payload = await api('POST', '/api/open', { path });
      applyOpenPayload(payload);
      await refreshState();
      await loadOutline(payload.path);
      if (options.line) {
        focusLine(options.line);
      }
      setStatus(`Opened ${payload.path}`);
      pushActivity('Opened file', payload.path);
    }

    async function saveCurrent() {
      if (!state.currentPath) {
        throw new Error('No file is open');
      }
      formatEditorBuffer({ silent: true });
      const payload = await api('POST', '/api/save', {
        path: state.currentPath,
        content: ui.editor.value
      });
      state.dirty = false;
      ui.editorTitle.textContent = `Editor · ${state.currentPath}`;
      setStatus(payload.message);
      renderEditorGutter();
      renderEditorTelemetry();
      await refreshState();
      await loadOutline(state.currentPath);
      pushActivity('Saved file', state.currentPath);
      return payload;
    }

    function splitProgramInput(text) {
      if (!text) {
        return [];
      }
      return text.split(/\r?\n/);
    }

    function shellCommandRunsProgram(command) {
      return /^(?::|snsx\s+)?run$/.test((command || '').trim());
    }

    function parseLocalStudioCommand(command) {
      const trimmed = (command || '').trim();
      if (shellCommandRunsProgram(trimmed)) {
        return { kind: 'run' };
      }
      const buildMatch = trimmed.match(/^(?::|snsx\s+)?build(?:\s+(\S+))?$/);
      if (buildMatch) {
        return { kind: 'build', target: buildMatch[1] || 'vm' };
      }
      if (/^(?::|snsx\s+)?(audit|security)$/.test(trimmed)) {
        return { kind: 'audit' };
      }
      if (/^(?::|snsx\s+)?doctor$/.test(trimmed)) {
        return { kind: 'doctor' };
      }
      return null;
    }

    function isRunnableSource(path) {
      return typeof path === 'string' && path.toLowerCase().endsWith('.snsx');
    }

    async function ensureExecutionTargetCurrent() {
      if (!isRunnableSource(state.currentPath) || state.currentPath === state.entry) {
        return;
      }
      const payload = await api('POST', '/api/set-entry', { path: state.currentPath });
      state.entry = payload.entry;
      await refreshState();
      setStatus(payload.message);
    }

    function clearRunSession() {
      state.runSession = null;
      ui.hintText.textContent = 'Ctrl/Cmd+S save · Ctrl/Cmd+R run current file · live terminal input on every ask() · Ctrl/Cmd+K command deck';
      ui.shellInput.placeholder = 'SNSX terminal: help, tools, status, permissions, deps, doctor, package list, module install std.design, !git status';
      ui.shellButton.textContent = 'Run SNSX';
    }

    function beginRunSession(payload, stdin) {
      const lineIndex = Number(payload.consumed_lines || 0);
      const lines = splitProgramInput(stdin);
      state.runSession = { lineIndex };
      ui.hintText.textContent = `Program is waiting for input line ${lineIndex + 1}. Type it in the SNSX terminal and press Send Input.`;
      ui.shellInput.placeholder = `Program input line ${lineIndex + 1}`;
      ui.shellInput.value = lines[lineIndex] ?? '';
      ui.shellButton.textContent = 'Send Input';
      setWorkspacePane('terminal');
      ui.shellInput.focus();
      ui.shellInput.select();
    }

    async function startProgramRun(initialStdin) {
      const stdin = initialStdin ?? ui.stdinInput.value;
      await ensureExecutionTargetCurrent();
      const payload = await api('POST', '/api/run', { stdin });
      setLog('run', payload.title, payload.output);
      switchTab(payload.ok || payload.needs_input ? 'run' : 'audit');
      setStatus(payload.title, !!payload.ok || !!payload.needs_input);
      await refreshState();
      if (payload.needs_input) {
        beginRunSession(payload, stdin);
      } else {
        clearRunSession();
      }
      return payload;
    }

    async function continueRunSession() {
      if (!state.runSession) {
        return null;
      }
      await saveIfDirty();
      const lines = splitProgramInput(ui.stdinInput.value);
      while (lines.length < state.runSession.lineIndex) {
        lines.push('');
      }
      if (lines.length === state.runSession.lineIndex) {
        lines.push(ui.shellInput.value);
      } else {
        lines[state.runSession.lineIndex] = ui.shellInput.value;
      }
      ui.stdinInput.value = lines.join('\n');
      ui.shellInput.value = '';
      return startProgramRun(ui.stdinInput.value);
    }

    function clearProgramInput() {
      ui.stdinInput.value = '';
      clearRunSession();
      setStatus('Program input cleared');
    }

    async function previewAgent() {
      const prompt = ui.agentPrompt.value.trim();
      if (!prompt) {
        throw new Error('SNS AI prompt is empty');
      }
      const payload = await api('POST', '/api/agent-preview', { prompt });
      if (!ui.agentTarget.value.trim()) {
        ui.agentTarget.value = payload.suggested_path;
      }
      const fileSections = (payload.files || []).length > 1
        ? payload.files.flatMap(file => ['', `File: ${file.path}`, `Purpose: ${file.summary || 'SNSX source file'}`, '', file.code])
        : ['', payload.code];
      const output = [
        `Summary: ${payload.summary}`,
        `Recipe: ${payload.recipe}`,
        `Suggested path: ${payload.suggested_path}`,
        `Generated files: ${payload.file_count || ((payload.files || []).length || 1)}`,
        `Dependencies: ${(payload.dependencies || []).join(', ') || 'std.io'}`,
        ...(payload.notes && payload.notes.length ? ['', 'Notes:', ...payload.notes.map(note => `- ${note}`)] : []),
        ...fileSections
      ].join('\n');
      setLog('shell', payload.title, output);
      switchTab('shell');
      setStatus(payload.title);
      pushActivity(payload.title, payload.suggested_path || 'preview');
    }

    async function applyAgent() {
      const prompt = ui.agentPrompt.value.trim();
      if (!prompt) {
        throw new Error('SNS AI prompt is empty');
      }
      await saveIfDirty();
      const path = ui.agentTarget.value.trim() || 'src/agent_output.snsx';
      const payload = await api('POST', '/api/agent-apply', { prompt, path });
      setLog('shell', payload.title, payload.message);
      if (payload.open) {
        applyOpenPayload(payload.open);
        await loadOutline(payload.open.path);
      }
      await refreshState();
      switchTab('shell');
      setStatus(payload.title, !!payload.ok);
      pushActivity(payload.title, path, !!payload.ok);
    }

    async function runCurrent() {
      await saveIfDirty();
      const payload = await startProgramRun(ui.stdinInput.value);
      if (!payload.ok && !payload.needs_input) {
        setLog('audit', 'Security gate', payload.output);
        switchTab('audit');
      }
      pushActivity(payload.title, state.currentPath, !!payload.ok || !!payload.needs_input);
    }

    async function buildCurrent(target) {
      await saveIfDirty();
      await ensureExecutionTargetCurrent();
      const payload = await api('POST', '/api/build', { target });
      setLog('build', payload.title, payload.output);
      if (!payload.ok) {
        setLog('audit', 'Security gate', payload.output);
      }
      switchTab(payload.ok ? 'build' : 'audit');
      setStatus(payload.title, !!payload.ok);
      await refreshState();
      pushActivity(payload.title, `${state.currentPath} · ${target}`, !!payload.ok);
    }

    async function auditCurrent() {
      await saveIfDirty();
      await ensureExecutionTargetCurrent();
      const payload = await api('POST', '/api/audit', {});
      setLog('audit', payload.title, payload.output);
      switchTab('audit');
      setStatus(payload.title, !!payload.ok);
      await refreshState();
      pushActivity(payload.title, state.currentPath, !!payload.ok);
    }

    async function runDoctor() {
      await ensureExecutionTargetCurrent();
      const payload = await api('GET', '/api/doctor');
      setLog('doctor', payload.title, payload.output);
      switchTab('doctor');
      renderSummary(payload.summary);
      setStatus(payload.title, !!payload.ok);
      pushActivity(payload.title, state.currentPath, !!payload.ok);
    }

    async function runShell() {
      if (state.runSession) {
        await continueRunSession();
        return;
      }
      await runShellCommand(ui.shellInput.value);
    }

    async function createPath(directory) {
      await saveIfDirty();
      const baseDefault = state.currentPath && state.currentPath.includes('/')
        ? state.currentPath.substring(0, state.currentPath.lastIndexOf('/'))
        : '.';
      const base = prompt(directory ? 'Create folder inside path' : 'Create file inside path', baseDefault || '.');
      if (base === null) return;
      const relative = prompt(
        directory ? 'Folder name or relative path' : 'File name or relative path',
        directory ? 'new_surface' : 'new_flow.snsx'
      );
      if (!relative) return;
      const payload = await api('POST', '/api/create', { base, path: relative, directory });
      await refreshState();
      setStatus(payload.message);
      pushActivity(payload.message, relative);
      if (!directory) {
        await openFile(payload.path, { force: true });
      }
    }

    async function renamePath() {
      await saveIfDirty();
      const from = state.currentPath;
      if (!from) return;
      const to = prompt('Rename or move to workspace path', from);
      if (!to || to === from) return;
      const payload = await api('POST', '/api/rename', { from, to });
      await refreshState();
      setStatus(payload.message);
      pushActivity(payload.message, to);
      await openFile(payload.path, { force: true });
    }

    async function deletePath() {
      await saveIfDirty();
      const path = state.currentPath;
      if (!path) return;
      if (!confirm(`Delete ${path}? This cannot be undone.`)) return;
      const payload = await api('POST', '/api/delete', { path });
      await refreshState();
      setStatus(payload.message);
      pushActivity(payload.message, path, true);
      await openFile(state.entry, { force: true });
    }

    async function setEntry() {
      await saveIfDirty();
      const payload = await api('POST', '/api/set-entry', { path: state.currentPath });
      clearRunSession();
      state.entry = payload.entry;
      await refreshState();
      await loadOutline(state.currentPath);
      setStatus(payload.message);
      pushActivity(payload.message, payload.entry);
    }

    async function loadOutline(path) {
      const payload = await api('POST', '/api/outline', { path });
      renderOutline(payload.items, payload.title);
    }

    async function loadSnippets() {
      const payload = await api('GET', '/api/snippets');
      state.snippets = payload.snippets || [];
      renderSnippets(state.snippets);
    }

    async function runNavigator() {
      const query = ui.navigatorInput.value.trim();
      if (!query) {
        setStatus('Navigator query is empty', false);
        return;
      }
      if (state.navigatorMode === 'find') {
        const payload = await api('POST', '/api/find', { query });
        renderNavigatorResults('find', payload.results);
        setStatus(payload.title);
      } else {
        const payload = await api('POST', '/api/search', { query });
        renderNavigatorResults('search', payload.results);
        setStatus(payload.title);
      }
    }

    function focusNavigator(mode, options = {}) {
      const focus = options.focus !== false;
      const showPane = options.showPane !== false;
      state.navigatorMode = mode;
      ui.navigatorModeLabel.textContent = `Mode: ${mode === 'find' ? 'file finder' : 'text search'}`;
      ui.navigatorInput.placeholder = mode === 'find'
        ? 'Type a file or folder fragment and press Enter'
        : 'Type source text to search and press Enter';
      if (showPane) {
        setWorkspacePane('navigator');
      }
      if (focus) {
        ui.navigatorInput.focus();
        ui.navigatorInput.select();
      }
      closePalette();
    }

    async function createSnippet(kind) {
      await saveIfDirty();
      const path = prompt(`Create ${kind} snippet at workspace path`, `${kind}_${Date.now()}.snsx`);
      if (!path) return;
      const payload = await api('POST', '/api/snippet', { kind, path });
      await refreshState();
      if (payload.open) {
        applyOpenPayload(payload.open);
        await loadOutline(payload.open.path);
      }
      setStatus(payload.title);
      pushActivity(payload.title, path);
    }

    async function runPackageAction(action) {
      if (action === 'list') {
        return runShellCommand('package list');
      }
      const spec = requireInputValue(ui.packageInput, 'Package name is empty');
      return runShellCommand(`package ${action} ${spec}`);
    }

    async function runModuleAction(action) {
      if (action === 'list') {
        return runShellCommand('module list');
      }
      const spec = requireInputValue(ui.moduleInput, 'Module name is empty');
      return runShellCommand(`module ${action} ${spec}`);
    }

    function focusLine(line) {
      const lines = ui.editor.value.split('\n');
      const target = Math.max(1, Math.min(line || 1, lines.length));
      let offset = 0;
      for (let index = 0; index < target - 1; index += 1) {
        offset += lines[index].length + 1;
      }
      setWorkspacePane('editor');
      ui.editor.focus();
      ui.editor.selectionStart = offset;
      ui.editor.selectionEnd = offset + (lines[target - 1] || '').length;
      const lineHeight = 22;
      ui.editor.scrollTop = Math.max(0, (target - 3) * lineHeight);
      setStatus(`Focused line ${target}`);
    }

    function openPalette() {
      state.paletteOpen = true;
      ui.palette.classList.add('open');
      ui.paletteInput.value = '';
      renderPaletteActions('');
      window.requestAnimationFrame(() => ui.paletteInput.focus());
    }

    function closePalette() {
      state.paletteOpen = false;
      ui.palette.classList.remove('open');
    }

    function renderPaletteActions(filter) {
      const needle = (filter || '').trim().toLowerCase();
      const matches = paletteActions.filter(action => {
        if (!needle) return true;
        return action.label.toLowerCase().includes(needle) || action.detail.toLowerCase().includes(needle);
      });
      ui.paletteResults.innerHTML = '';
      if (!matches.length) {
        const empty = document.createElement('div');
        empty.className = 'empty-state';
        empty.textContent = 'No command deck actions match that search.';
        ui.paletteResults.appendChild(empty);
        return;
      }
      matches.forEach(action => {
        const button = document.createElement('button');
        button.className = 'palette-action';
        button.innerHTML = `<strong>${action.label}</strong><small class="muted">${action.detail}</small>`;
        button.onclick = () => {
          closePalette();
          runUiAction(action.label, action.run);
        };
        ui.paletteResults.appendChild(button);
      });
    }

    function handleError(error) {
      const message = error.message || String(error);
      setStatus(message, false);
      setLog('shell', 'Error', message);
      switchTab('shell');
      pushActivity('Error', message, false);
    }

    function applyParallax() {
      document.documentElement.style.setProperty('--scroll', window.scrollY || 0);
    }

    document.querySelectorAll('.tab').forEach(node => {
      node.addEventListener('click', () => switchTab(node.dataset.tab));
    });

    ui.paneButtons.forEach(node => {
      node.addEventListener('click', () => {
        const pane = node.dataset.pane;
        if (pane === 'navigator') {
          focusNavigator(state.navigatorMode);
          return;
        }
        setWorkspacePane(pane);
        if (pane === 'editor') {
          ui.editor.focus();
        } else if (pane === 'terminal') {
          ui.shellInput.focus();
          ui.shellInput.select();
        }
      });
    });

    ui.shellQuickButtons.forEach(node => {
      node.addEventListener('click', () => {
        runUiAction(node.dataset.label || 'Running shell command', () => runShellCommand(node.dataset.command));
      });
    });

    ui.snippetQuickButtons.forEach(node => {
      node.addEventListener('click', () => {
        runUiAction('Creating snippet', () => createSnippet(node.dataset.snippet));
      });
    });

    ui.editor.addEventListener('input', () => {
      markEditorDirty();
    });

    ui.editor.addEventListener('scroll', () => {
      ui.editorGutter.scrollTop = ui.editor.scrollTop;
    });

    ['click', 'keyup', 'mouseup', 'select'].forEach(eventName => {
      ui.editor.addEventListener(eventName, () => {
        renderEditorTelemetry();
      });
    });

    ui.editor.addEventListener('keydown', event => {
      const accel = event.ctrlKey || event.metaKey;
      if (accel && event.shiftKey && event.key.toLowerCase() === 'f') {
        event.preventDefault();
        formatEditorBuffer();
        return;
      }
      if (event.key === 'Tab') {
        event.preventDefault();
        if (event.shiftKey) {
          outdentSoftTab();
        } else {
          insertSoftTab();
        }
        return;
      }
      if (event.key === 'Enter') {
        event.preventDefault();
        insertSmartNewline();
        return;
      }
      if (event.key === 'Backspace' && maybeDeletePair()) {
        event.preventDefault();
        return;
      }
      if (CLOSING_PAIRS.has(event.key) && maybeStepOverClosingPair(event.key)) {
        event.preventDefault();
        return;
      }
      if (PAIR_MAP[event.key]) {
        event.preventDefault();
        handleEditorAutoPair(event.key);
      }
    });

    ui.editor.addEventListener('paste', event => {
      const text = (event.clipboardData || window.clipboardData).getData('text');
      if (!text) {
        return;
      }
      event.preventDefault();
      replaceEditorSelection(sanitizeEditorText(text));
    });

    ui.navigatorInput.addEventListener('keydown', event => {
      if (event.key === 'Enter') {
        event.preventDefault();
        runUiAction('Running navigator', runNavigator);
      }
    });

    ui.shellInput.addEventListener('keydown', event => {
      if (event.key === 'Enter') {
        event.preventDefault();
        runUiAction('Running shell command', runShell);
      }
    });

    ui.packageInput.addEventListener('keydown', event => {
      if (event.key === 'Enter') {
        event.preventDefault();
        runUiAction('Adding package', () => runPackageAction('add'));
      }
    });

    ui.moduleInput.addEventListener('keydown', event => {
      if (event.key === 'Enter') {
        event.preventDefault();
        runUiAction('Installing module', () => runModuleAction('install'));
      }
    });

    ui.paletteInput.addEventListener('input', () => {
      renderPaletteActions(ui.paletteInput.value);
    });

    ui.palette.addEventListener('click', event => {
      if (event.target === ui.palette) {
        closePalette();
      }
    });

    document.getElementById('saveBtn').onclick = () => runUiAction('Saving file', saveCurrent);
    document.getElementById('runBtn').onclick = () => runUiAction('Running program', runCurrent);
    document.getElementById('auditBtn').onclick = () => runUiAction('Running audit', auditCurrent);
    document.getElementById('doctorBtn').onclick = () => runUiAction('Running doctor', runDoctor);
    document.getElementById('buildVmBtn').onclick = () => runUiAction('Building VM artifact', () => buildCurrent('vm'));
    document.getElementById('buildWasmBtn').onclick = () => runUiAction('Building WASM artifact', () => buildCurrent('wasm'));
    document.getElementById('buildLlvmBtn').onclick = () => runUiAction('Building LLVM artifact', () => buildCurrent('llvm'));
    document.getElementById('buildNativeBtn').onclick = () => runUiAction('Building native artifact', () => buildCurrent('native-x86_64'));
    document.getElementById('setEntryBtn').onclick = () => runUiAction('Setting entry file', setEntry);
    document.getElementById('newFileBtn').onclick = () => runUiAction('Creating file', () => createPath(false));
    document.getElementById('newFolderBtn').onclick = () => runUiAction('Creating folder', () => createPath(true));
    document.getElementById('renameBtn').onclick = () => runUiAction('Renaming path', renamePath);
    document.getElementById('deleteBtn').onclick = () => runUiAction('Deleting path', deletePath);
    document.getElementById('refreshBtn').onclick = () => runUiAction('Refreshing workspace', refreshState);
    document.getElementById('shellBtn').onclick = () => runUiAction('Running shell command', runShell);
    document.getElementById('agentPreviewBtn').onclick = () => runUiAction('Previewing SNS AI code', previewAgent);
    document.getElementById('agentApplyBtn').onclick = () => runUiAction('Applying SNS AI code', applyAgent);
    document.getElementById('powerAgentPreviewBtn').onclick = () => runUiAction('Previewing SNS AI code', previewAgent);
    document.getElementById('powerAgentApplyBtn').onclick = () => runUiAction('Applying SNS AI code', applyAgent);
    document.getElementById('promptRunBtn').onclick = () => runUiAction('Running program', runCurrent);
    document.getElementById('formatEditorBtn').onclick = () => formatEditorBuffer();
    document.getElementById('indentSelectionBtn').onclick = () => indentBlock(false);
    document.getElementById('outdentSelectionBtn').onclick = () => indentBlock(true);
    document.getElementById('packageAddBtn').onclick = () => runUiAction('Adding package', () => runPackageAction('add'));
    document.getElementById('packageRemoveBtn').onclick = () => runUiAction('Removing package', () => runPackageAction('remove'));
    document.getElementById('packageListBtn').onclick = () => runUiAction('Listing packages', () => runPackageAction('list'));
    document.getElementById('moduleInstallBtn').onclick = () => runUiAction('Installing module', () => runModuleAction('install'));
    document.getElementById('moduleScaffoldBtn').onclick = () => runUiAction('Scaffolding module', () => runModuleAction('scaffold'));
    document.getElementById('moduleListBtn').onclick = () => runUiAction('Listing modules', () => runModuleAction('list'));
    document.getElementById('cancelRunBtn').onclick = () => {
      clearRunSession();
      setStatus('Interactive program wait cancelled', false);
    };
    document.getElementById('historyBtn').onclick = () => runUiAction('Loading shell history', () => runShellCommand('history'));
    document.getElementById('clearInputBtn').onclick = () => clearProgramInput();
    document.getElementById('findFilesBtn').onclick = () => {
      focusNavigator('find');
      if (ui.navigatorInput.value.trim()) {
        runUiAction('Running navigator', runNavigator);
      }
    };
    document.getElementById('searchTextBtn').onclick = () => {
      focusNavigator('search');
      if (ui.navigatorInput.value.trim()) {
        runUiAction('Running navigator', runNavigator);
      }
    };
    document.getElementById('commandPaletteBtn').onclick = () => openPalette();

    window.addEventListener('mousemove', event => {
      document.documentElement.style.setProperty('--mx', (event.clientX / window.innerWidth).toFixed(3));
      document.documentElement.style.setProperty('--my', (event.clientY / window.innerHeight).toFixed(3));
    });
    window.addEventListener('scroll', applyParallax, { passive: true });
    window.addEventListener('resize', () => setWorkspacePane(state.activePane, { scroll: false }));

    window.addEventListener('keydown', event => {
      const key = event.key.toLowerCase();
      const accel = event.ctrlKey || event.metaKey;
      if (accel && key === 'k') {
        event.preventDefault();
        openPalette();
        return;
      }
      if (event.key === 'Escape' && state.paletteOpen) {
        event.preventDefault();
        closePalette();
        return;
      }
      if (accel && key === 's') {
        event.preventDefault();
        runUiAction('Saving file', saveCurrent);
      } else if (accel && key === 'r') {
        event.preventDefault();
        runUiAction('Running program', runCurrent);
      } else if (accel && key === 'b') {
        event.preventDefault();
        runUiAction('Building VM artifact', () => buildCurrent('vm'));
      } else if (accel && key === 't') {
        event.preventDefault();
        runUiAction('Running audit', auditCurrent);
      } else if (accel && key === 'p') {
        event.preventDefault();
        focusNavigator('find');
      } else if (accel && event.shiftKey && key === 'f') {
        event.preventDefault();
        focusNavigator('search');
      }
    });

    (async () => {
      try {
        switchTab('run');
        setWorkspacePane('editor', { scroll: false });
        focusNavigator('find', { focus: false, showPane: false });
        renderActivity();
        renderEditorGutter();
        renderEditorTelemetry();
        await refreshState();
        await loadSnippets();
        await openFile(state.entry, { force: true });
        await auditCurrent();
        applyParallax();
      } catch (error) {
        handleError(error);
      }
    })();
  </script>
</body>
</html>
"#;
