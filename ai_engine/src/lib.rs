use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use snsx_compiler::security::SecurityPolicy;
use snsx_compiler::{compile_source_with_policy, CompileOptions, Target};
use snsx_runtime::{Tensor, Value};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

pub trait Model: Send + Sync {
    fn name(&self) -> &str;
    fn complete(&self, prompt: &str, input: &[Value]) -> Result<Value>;
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ExecutionPlan {
    pub chunks: usize,
    pub use_gpu: bool,
}

#[derive(Debug, Clone, Default)]
pub struct CpuBackend;

impl CpuBackend {
    pub fn matmul(&self, left: &Tensor, right: &Tensor) -> Result<Tensor> {
        if left.shape.len() != 2 || right.shape.len() != 2 {
            return Err(anyhow!("matmul expects rank-2 tensors"));
        }
        let (m, k) = (left.shape[0], left.shape[1]);
        let (rk, n) = (right.shape[0], right.shape[1]);
        if k != rk {
            return Err(anyhow!("matmul shape mismatch"));
        }
        let mut out = vec![0.0; m * n];
        for row in 0..m {
            for col in 0..n {
                let mut sum = 0.0;
                for inner in 0..k {
                    sum += left.data[row * k + inner] * right.data[inner * n + col];
                }
                out[row * n + col] = sum;
            }
        }
        Ok(Tensor {
            shape: vec![m, n],
            data: out,
        })
    }
}

#[derive(Debug, Clone)]
pub struct GpuHook {
    pub backend_name: String,
}

impl GpuHook {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            backend_name: name.into(),
        }
    }

    pub fn describe(&self) -> String {
        format!("gpu://{}", self.backend_name)
    }
}

#[derive(Debug)]
pub struct MockModel;

impl Model for MockModel {
    fn name(&self) -> &str {
        "local:mock"
    }

    fn complete(&self, prompt: &str, input: &[Value]) -> Result<Value> {
        let text = input
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(" ");
        let summary = if prompt.to_lowercase().contains("summarize") {
            format!("summary: {}", truncate(&text, 72))
        } else if prompt.to_lowercase().contains("classify") {
            format!(
                "classification: {}",
                if text.len() % 2 == 0 {
                    "balanced"
                } else {
                    "spiky"
                }
            )
        } else {
            format!("{prompt}: {}", truncate(&text, 72))
        };
        Ok(Value::String(summary))
    }
}

#[derive(Default)]
pub struct ModelRegistry {
    models: HashMap<String, Arc<dyn Model>>,
    pub gpu: Option<GpuHook>,
    pub cpu: CpuBackend,
}

impl ModelRegistry {
    pub fn new() -> Self {
        let mut registry = Self::default();
        registry.register(MockModel);
        registry
    }

    pub fn register<M: Model + 'static>(&mut self, model: M) {
        self.models
            .insert(model.name().to_string(), Arc::new(model));
    }

    pub fn resolve(&self, name: &str) -> Option<Arc<dyn Model>> {
        self.models.get(name).cloned()
    }

    pub fn invoke_prompt_function(
        &self,
        prompt: &str,
        input: &[Value],
        model_name: Option<&str>,
    ) -> Result<Value> {
        let model = model_name
            .and_then(|name| self.resolve(name))
            .or_else(|| self.resolve("local:mock"))
            .ok_or_else(|| anyhow!("no model registered"))?;
        model.complete(prompt, input)
    }

    pub fn plan_parallel(&self, work_items: usize, preferred_chunk: usize) -> ExecutionPlan {
        let chunk = preferred_chunk.max(1);
        ExecutionPlan {
            chunks: (work_items + chunk - 1) / chunk,
            use_gpu: self.gpu.is_some() && work_items > chunk,
        }
    }
}

pub fn tensor_from_values(data: &[Value], shape: &[usize]) -> Result<Value> {
    let mut dense = Vec::with_capacity(data.len());
    for value in data {
        match value {
            Value::Float(v) => dense.push(*v),
            Value::Int(v) => dense.push(*v as f64),
            other => {
                return Err(anyhow!(
                    "tensor element must be numeric, found {}",
                    other.type_name()
                ))
            }
        }
    }
    Ok(Value::Tensor(Tensor::new(shape.to_vec(), dense)))
}

fn truncate(input: &str, max: usize) -> String {
    if input.len() <= max {
        input.to_string()
    } else {
        format!("{}...", &input[..max])
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AgentPermissions {
    pub fs: bool,
    pub net: bool,
    pub ai: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneratedSnsxFile {
    pub path: String,
    pub summary: String,
    pub code: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneratedSnsxProgram {
    pub title: String,
    pub prompt: String,
    pub recipe: String,
    pub summary: String,
    pub suggested_path: String,
    pub dependencies: Vec<String>,
    pub permissions: AgentPermissions,
    pub notes: Vec<String>,
    pub code: String,
    #[serde(default)]
    pub files: Vec<GeneratedSnsxFile>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AgentRecipe {
    RegistrationSuite,
    VoteEligibility,
    GradeChecker,
    Calculator,
    NumberComparison,
    DataRecordStore,
    TextSummary,
    DesignBoard,
    AccessGuard,
    WorkerTask,
    InputEcho,
    GenericStarter,
}

#[derive(Debug, Clone, Default)]
pub struct SnsxCodingAgent;

impl SnsxCodingAgent {
    pub fn generate(prompt: &str) -> Result<GeneratedSnsxProgram> {
        let prompt = prompt.trim();
        if prompt.is_empty() {
            return Err(anyhow!("SNS AI coding agent prompt cannot be empty"));
        }
        let profile = PromptProfile::analyze(prompt);
        let ranked = rank_recipes(&profile);
        let mut failures = Vec::new();

        for recipe in ranked {
            let mut artifact = render_recipe(recipe, &profile);
            if artifact.files.is_empty() {
                artifact.files.push(GeneratedSnsxFile {
                    path: artifact.suggested_path.clone(),
                    summary: "Primary SNSX source file".to_string(),
                    code: artifact.code.clone(),
                });
            }
            artifact.notes.push(format!(
                "Planner signals: {}",
                profile.signal_notes().join(", ")
            ));
            match validate_generated_program(&artifact) {
                Ok(validation) => {
                    artifact.notes.push(validation);
                    artifact.notes.push(format!(
                        "Planner selected '{}' after scoring the prompt against {} recipe families.",
                        artifact.recipe,
                        profile.recipe_family_count()
                    ));
                    return Ok(artifact);
                }
                Err(error) => failures.push(format!(
                    "{} failed validation: {}",
                    recipe_name(recipe),
                    error
                )),
            }
        }

        Err(anyhow!(
            "SNS AI coding agent could not synthesize a valid SNSX program for this prompt.\n{}",
            failures.join("\n")
        ))
    }
}

#[derive(Debug, Clone)]
struct PromptProfile {
    prompt: String,
    lower: String,
    slug: String,
    numbers: Vec<i64>,
    wants_crud: bool,
    wants_login: bool,
    wants_report_export: bool,
    wants_multifile_app: bool,
    wants_storage: bool,
    wants_database: bool,
    wants_contact_record: bool,
    wants_input: bool,
    wants_multiple_inputs: bool,
    wants_ai: bool,
    wants_design: bool,
    wants_security: bool,
    wants_concurrency: bool,
    wants_numeric_validation: bool,
    wants_comparison: bool,
}

impl PromptProfile {
    fn analyze(prompt: &str) -> Self {
        let trimmed = prompt.trim();
        let lower = trimmed.to_lowercase();
        let wants_crud = contains_any(
            &lower,
            &[
                "crud",
                "create",
                "search",
                "update",
                "delete",
                "remove",
                "edit",
                "find",
            ],
        );
        let wants_login = contains_any(
            &lower,
            &["login", "sign in", "signin", "authentication", "password"],
        );
        let wants_report_export = contains_any(
            &lower,
            &[
                "report",
                "export",
                "download",
                "summary report",
                "export report",
            ],
        );
        let wants_multifile_app = contains_any(
            &lower,
            &[
                "app",
                "application",
                "system",
                "project",
                "dashboard",
                "registration system",
            ],
        ) || (wants_crud && wants_login);
        let wants_storage = contains_any(
            &lower,
            &[
                "store",
                "save",
                "database",
                "db",
                "record",
                "details",
                "persist",
                "write to file",
                "write into",
            ],
        );
        let wants_database =
            contains_any(&lower, &["database", "db", "table", "records", "store in"]);
        let wants_contact_record = contains_any(
            &lower,
            &[
                "name",
                "mobile",
                "phone",
                "email",
                "contact",
                "user detail",
                "user details",
                "registration",
            ],
        );
        let wants_ai = contains_any(
            &lower,
            &[
                "ai",
                "summary",
                "summarize",
                "summarise",
                "classify",
                "headline",
                "model",
            ],
        );
        let wants_design = contains_any(
            &lower,
            &[
                "dashboard",
                "panel",
                "board",
                "card",
                "design",
                "ui",
                "layout",
            ],
        );
        let wants_security = contains_any(
            &lower,
            &[
                "secure", "security", "login", "password", "auth", "access", "token", "otp",
            ],
        );
        let wants_concurrency = contains_any(
            &lower,
            &[
                "async",
                "parallel",
                "concurrent",
                "background",
                "worker",
                "queue",
                "task",
            ],
        );
        let wants_comparison = contains_any(
            &lower,
            &[
                "largest", "greatest", "smallest", "minimum", "maximum", "compare", "bigger",
            ],
        );
        let wants_numeric_validation = contains_any(
            &lower,
            &[
                "age",
                "marks",
                "score",
                "number",
                "numbers",
                "vote",
                "grade",
                "calculate",
                "calculator",
                "sum",
                "subtract",
                "multiply",
                "divide",
                "compare",
            ],
        );
        let wants_multiple_inputs = contains_any(
            &lower,
            &[
                "two numbers",
                "three numbers",
                "compare",
                "calculator",
                "username and password",
                "login form",
                "multiple inputs",
                "name and",
                "email and",
                "mobile and",
            ],
        );
        let wants_input = wants_multiple_inputs
            || contains_any(
                &lower,
                &[
                    "input",
                    "ask",
                    "enter",
                    "user input",
                    "form",
                    "read",
                    "age",
                    "marks",
                    "score",
                    "vote",
                    "calculator",
                    "compare",
                    "login",
                    "password",
                    "email",
                    "mobile",
                    "phone",
                    "name",
                    "database",
                    "store",
                ],
            );
        Self {
            prompt: trimmed.to_string(),
            lower,
            slug: slugify(trimmed),
            numbers: extract_numbers(trimmed),
            wants_crud,
            wants_login,
            wants_report_export,
            wants_multifile_app,
            wants_storage,
            wants_database,
            wants_contact_record,
            wants_input,
            wants_multiple_inputs,
            wants_ai,
            wants_design,
            wants_security,
            wants_concurrency,
            wants_numeric_validation,
            wants_comparison,
        }
    }

    fn signal_notes(&self) -> Vec<String> {
        let mut notes = Vec::new();
        if self.wants_crud {
            notes.push("CRUD workflow".to_string());
        }
        if self.wants_login {
            notes.push("login/auth flow".to_string());
        }
        if self.wants_report_export {
            notes.push("report export".to_string());
        }
        if self.wants_multifile_app {
            notes.push("multi-file app scaffold".to_string());
        }
        if self.wants_storage {
            notes.push("persistent storage".to_string());
        }
        if self.wants_database {
            notes.push("database/local record store".to_string());
        }
        if self.wants_contact_record {
            notes.push("structured contact fields".to_string());
        }
        if self.wants_input {
            notes.push("user input".to_string());
        }
        if self.wants_multiple_inputs {
            notes.push("multi-step prompts".to_string());
        }
        if self.wants_numeric_validation {
            notes.push("numeric validation".to_string());
        }
        if self.wants_ai {
            notes.push("AI/native prompt execution".to_string());
        }
        if self.wants_design {
            notes.push("design composition".to_string());
        }
        if self.wants_security {
            notes.push("access/security rules".to_string());
        }
        if self.wants_concurrency {
            notes.push("deterministic concurrency".to_string());
        }
        if self.wants_comparison {
            notes.push("decision/comparison logic".to_string());
        }
        if notes.is_empty() {
            notes.push("general starter scaffold".to_string());
        }
        notes
    }

    fn vote_threshold(&self) -> i64 {
        self.numbers
            .iter()
            .copied()
            .find(|value| (16..=30).contains(value))
            .unwrap_or(18)
    }

    fn comparison_arity(&self) -> usize {
        if self.lower.contains("three") || self.numbers.contains(&3) {
            3
        } else {
            2
        }
    }

    fn worker_formula(&self) -> (&'static str, &'static str) {
        if contains_any(&self.lower, &["square", "squared", "squares"]) {
            ("value * value", "squares the worker input")
        } else if contains_any(&self.lower, &["double", "doubles", "twice"]) {
            ("value * 2", "doubles the worker input")
        } else if contains_any(&self.lower, &["triple", "triples"]) {
            ("value * 3", "triples the worker input")
        } else {
            ("value * 10", "scales the worker input by ten")
        }
    }

    fn ai_instruction(&self) -> String {
        if self.lower.contains("headline") {
            "Turn this text into a short headline".to_string()
        } else if contains_any(&self.lower, &["classify", "category", "tag"]) {
            "Classify this text into one short label".to_string()
        } else if self.lower.contains("bullet") {
            "Summarize this text into three compact clauses".to_string()
        } else {
            "Summarize this text in one sentence".to_string()
        }
    }

    fn requested_fields(&self) -> Vec<&'static str> {
        let mut fields = Vec::new();
        if contains_any(&self.lower, &["name", "full name"]) {
            fields.push("name");
        }
        if contains_any(
            &self.lower,
            &["mobile", "phone", "phone number", "mobile number"],
        ) {
            fields.push("mobile");
        }
        if contains_any(&self.lower, &["email", "email id", "mail"]) {
            fields.push("email");
        }
        if contains_any(&self.lower, &["address", "location"]) {
            fields.push("address");
        }
        if contains_any(&self.lower, &["city"]) {
            fields.push("city");
        }
        if contains_any(&self.lower, &["age"]) {
            fields.push("age");
        }
        if fields.is_empty() {
            fields.extend(["name", "email"]);
        }
        fields
    }

    fn database_slug(&self) -> String {
        extract_database_name(&self.prompt)
            .filter(|name| !name.is_empty())
            .unwrap_or_else(|| {
                if self.wants_contact_record {
                    "user_detail".to_string()
                } else {
                    non_empty_slug(&self.slug, "snsx_store")
                }
            })
    }

    fn recipe_family_count(&self) -> usize {
        12
    }
}

fn rank_recipes(profile: &PromptProfile) -> Vec<AgentRecipe> {
    let mut scored = vec![
        (
            score_recipe(profile, AgentRecipe::RegistrationSuite),
            AgentRecipe::RegistrationSuite,
        ),
        (
            score_recipe(profile, AgentRecipe::VoteEligibility),
            AgentRecipe::VoteEligibility,
        ),
        (
            score_recipe(profile, AgentRecipe::GradeChecker),
            AgentRecipe::GradeChecker,
        ),
        (
            score_recipe(profile, AgentRecipe::Calculator),
            AgentRecipe::Calculator,
        ),
        (
            score_recipe(profile, AgentRecipe::NumberComparison),
            AgentRecipe::NumberComparison,
        ),
        (
            score_recipe(profile, AgentRecipe::DataRecordStore),
            AgentRecipe::DataRecordStore,
        ),
        (
            score_recipe(profile, AgentRecipe::TextSummary),
            AgentRecipe::TextSummary,
        ),
        (
            score_recipe(profile, AgentRecipe::DesignBoard),
            AgentRecipe::DesignBoard,
        ),
        (
            score_recipe(profile, AgentRecipe::AccessGuard),
            AgentRecipe::AccessGuard,
        ),
        (
            score_recipe(profile, AgentRecipe::WorkerTask),
            AgentRecipe::WorkerTask,
        ),
        (
            score_recipe(profile, AgentRecipe::InputEcho),
            AgentRecipe::InputEcho,
        ),
        (
            score_recipe(profile, AgentRecipe::GenericStarter),
            AgentRecipe::GenericStarter,
        ),
    ];
    scored.sort_by(|(left_score, left_recipe), (right_score, right_recipe)| {
        right_score
            .cmp(left_score)
            .then_with(|| recipe_priority(*left_recipe).cmp(&recipe_priority(*right_recipe)))
    });
    let mut ranked = scored
        .into_iter()
        .filter(|(score, _)| *score > 0)
        .map(|(_, recipe)| recipe)
        .collect::<Vec<_>>();
    if !ranked.contains(&AgentRecipe::GenericStarter) {
        ranked.push(AgentRecipe::GenericStarter);
    }
    ranked
}

fn score_recipe(profile: &PromptProfile, recipe: AgentRecipe) -> i32 {
    match recipe {
        AgentRecipe::RegistrationSuite => {
            score_keywords(
                &profile.lower,
                &[
                    ("registration", 10),
                    ("register", 10),
                    ("user registration", 10),
                    ("crud", 10),
                    ("create", 6),
                    ("search", 6),
                    ("update", 6),
                    ("delete", 6),
                    ("login", 7),
                    ("export", 6),
                    ("report", 5),
                    ("system", 4),
                ],
            ) + if profile.wants_crud { 6 } else { 0 }
                + if profile.wants_login { 5 } else { 0 }
                + if profile.wants_report_export { 4 } else { 0 }
                + if profile.wants_multifile_app { 3 } else { 0 }
                + if profile.wants_storage { 3 } else { 0 }
        }
        AgentRecipe::VoteEligibility => {
            score_keywords(
                &profile.lower,
                &[("vote", 7), ("eligibility", 6), ("eligible", 6), ("age", 4)],
            ) + if profile.wants_numeric_validation {
                1
            } else {
                0
            }
        }
        AgentRecipe::GradeChecker => {
            score_keywords(
                &profile.lower,
                &[
                    ("grade", 8),
                    ("marks", 7),
                    ("score", 6),
                    ("result", 3),
                    ("report card", 3),
                ],
            ) + if profile.wants_numeric_validation {
                1
            } else {
                0
            }
        }
        AgentRecipe::Calculator => {
            score_keywords(
                &profile.lower,
                &[
                    ("calculator", 9),
                    ("calculate", 8),
                    ("arithmetic", 7),
                    ("add", 3),
                    ("subtract", 3),
                    ("multiply", 3),
                    ("divide", 3),
                ],
            ) + if profile.wants_multiple_inputs { 2 } else { 0 }
        }
        AgentRecipe::NumberComparison => {
            score_keywords(
                &profile.lower,
                &[
                    ("largest", 8),
                    ("greatest", 8),
                    ("smallest", 8),
                    ("maximum", 6),
                    ("minimum", 6),
                    ("compare", 6),
                ],
            ) + if profile.wants_comparison { 3 } else { 0 }
        }
        AgentRecipe::DataRecordStore => {
            score_keywords(
                &profile.lower,
                &[
                    ("database", 10),
                    ("store", 9),
                    ("save", 8),
                    ("record", 8),
                    ("details", 6),
                    ("email", 6),
                    ("mobile", 6),
                    ("phone", 6),
                    ("name", 4),
                    ("registration", 4),
                ],
            ) + if profile.wants_storage { 4 } else { 0 }
                + if profile.wants_contact_record { 4 } else { 0 }
                + if profile.wants_database { 4 } else { 0 }
        }
        AgentRecipe::TextSummary => {
            score_keywords(
                &profile.lower,
                &[
                    ("summary", 8),
                    ("summarize", 8),
                    ("summarise", 8),
                    ("headline", 6),
                    ("classify", 6),
                    ("text", 2),
                ],
            ) + if profile.wants_ai { 4 } else { 0 }
        }
        AgentRecipe::DesignBoard => {
            score_keywords(
                &profile.lower,
                &[
                    ("dashboard", 8),
                    ("board", 7),
                    ("panel", 7),
                    ("card", 6),
                    ("design", 6),
                    ("ui", 5),
                    ("layout", 4),
                ],
            ) + if profile.wants_design { 4 } else { 0 }
        }
        AgentRecipe::AccessGuard => {
            score_keywords(
                &profile.lower,
                &[
                    ("login", 8),
                    ("password", 8),
                    ("secure", 7),
                    ("security", 7),
                    ("access", 6),
                    ("auth", 6),
                    ("token", 5),
                ],
            ) + if profile.wants_security { 3 } else { 0 }
        }
        AgentRecipe::WorkerTask => {
            score_keywords(
                &profile.lower,
                &[
                    ("async", 8),
                    ("parallel", 8),
                    ("concurrent", 8),
                    ("worker", 7),
                    ("background", 6),
                    ("task", 5),
                    ("queue", 5),
                ],
            ) + if profile.wants_concurrency { 4 } else { 0 }
        }
        AgentRecipe::InputEcho => {
            score_keywords(
                &profile.lower,
                &[
                    ("input", 5),
                    ("ask", 5),
                    ("enter", 3),
                    ("user input", 5),
                    ("form", 4),
                ],
            ) + if profile.wants_input { 2 } else { 0 }
        }
        AgentRecipe::GenericStarter => 1,
    }
}

fn recipe_priority(recipe: AgentRecipe) -> usize {
    match recipe {
        AgentRecipe::RegistrationSuite => 0,
        AgentRecipe::VoteEligibility => 1,
        AgentRecipe::GradeChecker => 2,
        AgentRecipe::Calculator => 3,
        AgentRecipe::NumberComparison => 4,
        AgentRecipe::DataRecordStore => 5,
        AgentRecipe::TextSummary => 6,
        AgentRecipe::DesignBoard => 7,
        AgentRecipe::AccessGuard => 8,
        AgentRecipe::WorkerTask => 9,
        AgentRecipe::InputEcho => 10,
        AgentRecipe::GenericStarter => 11,
    }
}

fn recipe_name(recipe: AgentRecipe) -> &'static str {
    match recipe {
        AgentRecipe::RegistrationSuite => "registration_suite",
        AgentRecipe::VoteEligibility => "vote_eligibility",
        AgentRecipe::GradeChecker => "grade_checker",
        AgentRecipe::Calculator => "calculator",
        AgentRecipe::NumberComparison => "number_comparison",
        AgentRecipe::DataRecordStore => "data_record_store",
        AgentRecipe::TextSummary => "text_summary",
        AgentRecipe::DesignBoard => "design_board",
        AgentRecipe::AccessGuard => "access_guard",
        AgentRecipe::WorkerTask => "worker_task",
        AgentRecipe::InputEcho => "input_echo",
        AgentRecipe::GenericStarter => "generic_starter",
    }
}

fn score_keywords(input: &str, weights: &[(&str, i32)]) -> i32 {
    weights
        .iter()
        .map(|(keyword, weight)| {
            if contains_term(input, keyword) {
                *weight
            } else {
                0
            }
        })
        .sum()
}

fn validate_generated_program(artifact: &GeneratedSnsxProgram) -> Result<String> {
    let policy = SecurityPolicy {
        strict: true,
        allow_fs: artifact.permissions.fs,
        allow_network: artifact.permissions.net,
        allow_ai: artifact.permissions.ai,
    };
    let options = CompileOptions {
        target: Target::Vm,
        ..CompileOptions::default()
    };
    let workspace = write_validation_workspace(artifact)?;
    let entry = workspace.join(&artifact.suggested_path);
    let source = fs::read_to_string(&entry)?;
    let result = compile_source_with_policy(&entry.display().to_string(), &source, &options, &policy)
        .map(|_| {
            format!(
                "Compiler-validated in strict mode with permissions fs={} net={} ai={}.",
                on_off(artifact.permissions.fs),
                on_off(artifact.permissions.net),
                on_off(artifact.permissions.ai)
            )
        })
        .map_err(|diagnostics| {
            let details = diagnostics
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("\n");
            anyhow!(details)
        });
    let _ = fs::remove_dir_all(&workspace);
    result
}

fn write_validation_workspace(artifact: &GeneratedSnsxProgram) -> Result<PathBuf> {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    let root = std::env::temp_dir().join(format!(
        "snsx_agent_validation_{}_{}",
        std::process::id(),
        nonce
    ));
    fs::create_dir_all(&root)?;

    let manifest = format!(
        "[package]\nname = \"snsx_agent_validation\"\nversion = \"0.1.0\"\nentry = \"{}\"\n\n[permissions]\nfs = {}\nnet = {}\nai = {}\n\n[dependencies]\nstd = \"builtin\"\n",
        artifact.suggested_path,
        artifact.permissions.fs,
        artifact.permissions.net,
        artifact.permissions.ai
    );
    fs::write(root.join("snsx.toml"), manifest)?;

    for file in &artifact.files {
        write_generated_file(&root, &file.path, &file.code)?;
    }

    if !artifact.files.iter().any(|file| file.path == artifact.suggested_path) {
        write_generated_file(&root, &artifact.suggested_path, &artifact.code)?;
    }

    Ok(root)
}

fn write_generated_file(root: &Path, relative: &str, code: &str) -> Result<()> {
    let path = root.join(relative);
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

fn on_off(value: bool) -> &'static str {
    if value {
        "on"
    } else {
        "off"
    }
}

fn render_recipe(recipe: AgentRecipe, profile: &PromptProfile) -> GeneratedSnsxProgram {
    match recipe {
        AgentRecipe::RegistrationSuite => registration_suite_program(profile),
        AgentRecipe::VoteEligibility => vote_eligibility_program(profile),
        AgentRecipe::GradeChecker => grade_checker_program(profile),
        AgentRecipe::Calculator => calculator_program(profile),
        AgentRecipe::NumberComparison => number_comparison_program(profile),
        AgentRecipe::DataRecordStore => data_record_store_program(profile),
        AgentRecipe::TextSummary => text_summary_program(profile),
        AgentRecipe::DesignBoard => design_board_program(profile),
        AgentRecipe::AccessGuard => access_guard_program(profile),
        AgentRecipe::WorkerTask => worker_task_program(profile),
        AgentRecipe::InputEcho => input_echo_program(profile),
        AgentRecipe::GenericStarter => generic_starter_program(profile),
    }
}

fn registration_suite_program(profile: &PromptProfile) -> GeneratedSnsxProgram {
    let title = agent_title(&profile.prompt, "User Registration System");
    let slug = if profile.slug.is_empty() {
        "user_registration".to_string()
    } else {
        profile.slug.clone()
    };
    let database_name = format!("data/{}.snsxdb", slug);
    let export_name = format!("reports/{}_export.txt", slug);

    let main_code = format!(
        "bring <module/std.io> as io\nbring <module/app.store> as store\nbring <module/app.validators> as validators\nbring <module/app.reports> as reports\n\nflow action_help\n    gives String\n    \"Choose one of: create | search | update | delete | login | export\"\n\nentry\n    show \"{}\"\n    show action_help()\n    hold action = ask()\n\n    gate action == \"create\":\n        show \"Enter name:\"\n        hold name = ask()\n        show \"Enter mobile number:\"\n        hold mobile = ask()\n        show \"Enter email address:\"\n        hold email = ask()\n        show \"Create a password:\"\n        hold password = ask()\n\n        gate name_missing(name):\n            show \"Please enter the name.\"\n        otherwise:\n            gate mobile_missing(mobile):\n                show \"Please enter the mobile number.\"\n            otherwise:\n                gate mobile_digits(mobile):\n                    gate email_missing(email):\n                        show \"Please enter the email address.\"\n                    otherwise:\n                        gate email_valid(email):\n                            gate password_missing(password):\n                                show \"Please enter the password.\"\n                            otherwise:\n                                gate password_long_enough(password):\n                                    hold saved_to = append_text(user_database_path(), build_user_record(name, mobile, email, password) + \"\\n\")\n                                    show \"User saved to:\"\n                                    show sanitize(saved_to)\n                                otherwise:\n                                    show \"Password must be at least 6 characters.\"\n                        otherwise:\n                            show \"Please enter a valid email address.\"\n                otherwise:\n                    show \"Please enter a valid mobile number using digits only.\"\n    otherwise:\n        gate action == \"search\":\n            show \"Enter email address to search:\"\n            hold email = ask()\n            gate contains(read_text(user_database_path()), \"email=\" + sanitize(email)):\n                show \"User record found for:\"\n                show sanitize(email)\n            otherwise:\n                show \"User record not found.\"\n        otherwise:\n            gate action == \"update\":\n                show \"Enter email address to update:\"\n                hold email = ask()\n                show \"Enter new mobile number:\"\n                hold mobile = ask()\n                gate email_missing(email):\n                    show \"Please enter the email address.\"\n                otherwise:\n                    gate mobile_missing(mobile):\n                        show \"Please enter the mobile number.\"\n                    otherwise:\n                        gate mobile_digits(mobile):\n                            gate contains(read_text(user_database_path()), \"email=\" + sanitize(email)):\n                                hold saved_to = append_text(user_database_path(), build_update_record(email, mobile) + \"\\n\")\n                                show \"Update recorded in:\"\n                                show sanitize(saved_to)\n                            otherwise:\n                                show \"User record not found.\"\n                        otherwise:\n                            show \"Please enter a valid mobile number using digits only.\"\n            otherwise:\n                gate action == \"delete\":\n                    show \"Enter email address to delete:\"\n                    hold email = ask()\n                    gate email_missing(email):\n                        show \"Please enter the email address.\"\n                    otherwise:\n                        gate contains(read_text(user_database_path()), \"email=\" + sanitize(email)):\n                            hold saved_to = append_text(user_database_path(), build_delete_record(email) + \"\\n\")\n                            show \"Delete request recorded in:\"\n                            show sanitize(saved_to)\n                        otherwise:\n                            show \"User record not found.\"\n                otherwise:\n                    gate action == \"login\":\n                        show \"Enter email address:\"\n                        hold email = ask()\n                        show \"Enter password:\"\n                        hold password = ask()\n                        gate email_missing(email):\n                            show \"Please enter the email address.\"\n                        otherwise:\n                            gate password_missing(password):\n                                show \"Please enter the password.\"\n                            otherwise:\n                                gate contains(read_text(user_database_path()), \"email=\" + sanitize(email)):\n                                    gate contains(read_text(user_database_path()), \"password=\" + seal(password)):\n                                        show \"Login accepted for:\"\n                                        show sanitize(email)\n                                    otherwise:\n                                        show \"Invalid email or password.\"\n                                otherwise:\n                                    show \"Invalid email or password.\"\n                    otherwise:\n                        gate action == \"export\":\n                            hold saved_to = export_user_report()\n                            show \"Report exported to:\"\n                            show saved_to\n                        otherwise:\n                            show action_help()\n\n    0\n",
        escape_string(&title)
    );

    let store_code = format!(
        "bring <module/std.io> as io\n\nflow user_database_path\n    gives String\n    \"{database_name}\"\n\nflow build_user_record\n    takes name: String, mobile: String, email: String, password: String\n    gives String\n    \"action=create|status=active|name=\" + sanitize(name) + \"|mobile=\" + sanitize(mobile) + \"|email=\" + sanitize(email) + \"|password=\" + seal(password)\n\nflow build_update_record\n    takes email: String, mobile: String\n    gives String\n    \"action=update|email=\" + sanitize(email) + \"|mobile=\" + sanitize(mobile)\n\nflow build_delete_record\n    takes email: String\n    gives String\n    \"action=delete|email=\" + sanitize(email)\n\nflow login_signature\n    takes email: String, password: String\n    gives String\n    \"email=\" + sanitize(email) + \"|password=\" + seal(password)\n"
    );

    let validators_code = "flow name_missing\n    takes name: String\n    gives Bool\n    name == \"\"\n\nflow mobile_missing\n    takes mobile: String\n    gives Bool\n    mobile == \"\"\n\nflow mobile_digits\n    takes mobile: String\n    gives Bool\n    is_digits(mobile)\n\nflow email_missing\n    takes email: String\n    gives Bool\n    email == \"\"\n\nflow email_valid\n    takes email: String\n    gives Bool\n    is_email(email)\n\nflow password_missing\n    takes password: String\n    gives Bool\n    password == \"\"\n\nflow password_long_enough\n    takes password: String\n    gives Bool\n    len(password) >= 6\n"
        .to_string();

    let reports_code = format!(
        "bring <module/std.io> as io\n\nflow user_report_path\n    gives String\n    \"{export_name}\"\n\nflow export_report_payload\n    gives String\n    \"SNSX User Report\\n\" + read_text(user_database_path())\n\nflow export_user_report\n    gives String\n    write_text(user_report_path(), export_report_payload())\n"
    );

    GeneratedSnsxProgram {
        title: title.clone(),
        prompt: profile.prompt.clone(),
        recipe: "registration_suite".to_string(),
        summary: "Generates a multi-file SNSX registration app with create, search, update, delete, login, and export flows backed by a local event-log database.".to_string(),
        suggested_path: "src/main.snsx".to_string(),
        dependencies: vec!["std.io".to_string()],
        permissions: AgentPermissions {
            fs: true,
            net: false,
            ai: false,
        },
        notes: vec![
            "The agent scaffold writes multiple SNSX source files under src/app so the project is structured rather than dumped into one entry file.".to_string(),
            "Local `bring <module/app...>` imports are used for the generated app modules.".to_string(),
            "CRUD operations use an append-only SNSX database log so the app stays deterministic and auditable.".to_string(),
        ],
        code: main_code.clone(),
        files: vec![
            GeneratedSnsxFile {
                path: "src/main.snsx".to_string(),
                summary: "Entry point with the command menu and workflow routing.".to_string(),
                code: main_code,
            },
            GeneratedSnsxFile {
                path: "src/app/store.snsx".to_string(),
                summary: "Local database flows for create/search/update/delete/login.".to_string(),
                code: store_code,
            },
            GeneratedSnsxFile {
                path: "src/app/validators.snsx".to_string(),
                summary: "Reusable validation flows for fields and registration input.".to_string(),
                code: validators_code,
            },
            GeneratedSnsxFile {
                path: "src/app/reports.snsx".to_string(),
                summary: "Report export flow that writes a project report file.".to_string(),
                code: reports_code,
            },
        ],
    }
}

fn vote_eligibility_program(profile: &PromptProfile) -> GeneratedSnsxProgram {
    let threshold = profile.vote_threshold();
    GeneratedSnsxProgram {
        title: "Vote Eligibility Checker".to_string(),
        prompt: profile.prompt.clone(),
        recipe: "vote_eligibility".to_string(),
        summary: format!(
            "Generates a validated input flow that checks whether the user meets the voting threshold of {}.",
            threshold
        ),
        suggested_path: "src/vote_eligibility.snsx".to_string(),
        dependencies: vec!["std.io".to_string()],
        permissions: AgentPermissions {
            ai: false,
            ..AgentPermissions::default()
        },
        notes: vec![
            "This program uses one ask() prompt and validates empty or non-numeric input before conversion.".to_string(),
            format!("The voting threshold was set to {} from the prompt or default policy.", threshold),
        ],
        code: format!(
            "bring <module/std.io> as io\n\nflow can_vote\n    takes age: Int\n    gives Int\n    gate age >= {threshold}:\n        give 1\n    otherwise:\n        give 0\n\nentry\n    show \"Enter your age:\"\n    hold raw_age = ask()\n\n    gate raw_age == \"\":\n        show \"Please enter your age.\"\n    otherwise:\n        gate is_int(raw_age):\n            hold age = int(raw_age)\n\n            gate can_vote(age) == 1:\n                show \"You are eligible to vote.\"\n            otherwise:\n                show \"You are not eligible to vote.\"\n        otherwise:\n            show \"Please enter a valid numeric age.\"\n\n    0\n"
        ),
        files: Vec::new(),
    }
}

fn grade_checker_program(profile: &PromptProfile) -> GeneratedSnsxProgram {
    GeneratedSnsxProgram {
        title: "Grade Checker".to_string(),
        prompt: profile.prompt.clone(),
        recipe: "grade_checker".to_string(),
        summary: "Generates a numeric marks-to-grade program with range validation and reusable grading flows.".to_string(),
        suggested_path: "src/grade_checker.snsx".to_string(),
        dependencies: vec!["std.io".to_string()],
        permissions: AgentPermissions {
            ai: false,
            ..AgentPermissions::default()
        },
        notes: vec![
            "The generated code checks for empty input, numeric input, and valid marks range.".to_string(),
            "Edit the grade bands inside grade_label() if your institution uses different cutoffs.".to_string(),
        ],
        code: r#"bring <module/std.io> as io

flow marks_in_range
    takes marks: Int
    gives Int
    gate marks >= 0:
        gate marks <= 100:
            give 1
        otherwise:
            give 0
    otherwise:
        give 0

flow grade_label
    takes marks: Int
    gives String
    gate marks >= 90:
        give "Grade A"
    otherwise:
        gate marks >= 75:
            give "Grade B"
        otherwise:
            gate marks >= 60:
                give "Grade C"
            otherwise:
                give "Grade D"

entry
    show "Enter your marks:"
    hold raw_marks = ask()

    gate raw_marks == "":
        show "Please enter your marks."
    otherwise:
        gate is_int(raw_marks):
            hold marks = int(raw_marks)

            gate marks_in_range(marks) == 1:
                show sanitize(grade_label(marks))
            otherwise:
                show "Please enter marks between 0 and 100."
        otherwise:
            show "Please enter valid numeric marks."

    0
"#
        .to_string(),
        files: Vec::new(),
    }
}

fn calculator_program(profile: &PromptProfile) -> GeneratedSnsxProgram {
    GeneratedSnsxProgram {
        title: "Calculator".to_string(),
        prompt: profile.prompt.clone(),
        recipe: "calculator".to_string(),
        summary: "Generates a validated calculator that asks for two numbers and an operation, then computes the result.".to_string(),
        suggested_path: "src/calculator.snsx".to_string(),
        dependencies: vec!["std.io".to_string()],
        permissions: AgentPermissions {
            ai: false,
            ..AgentPermissions::default()
        },
        notes: vec![
            "Supported operations are +, -, *, and /.".to_string(),
            "Division by zero is blocked before divide() is called.".to_string(),
        ],
        code: r#"bring <module/std.io> as io

flow add_values
    takes left: Int, right: Int
    gives Int
    left + right

flow subtract_values
    takes left: Int, right: Int
    gives Int
    left - right

flow multiply_values
    takes left: Int, right: Int
    gives Int
    left * right

flow divide_values
    takes left: Int, right: Int
    gives Int
    left / right

entry
    show "Enter first number:"
    hold raw_left = ask()
    show "Enter second number:"
    hold raw_right = ask()
    show "Choose operation (+, -, *, /):"
    hold op = ask()

    gate raw_left == "":
        show "Please enter the first number."
    otherwise:
        gate raw_right == "":
            show "Please enter the second number."
        otherwise:
            gate is_int(raw_left):
                gate is_int(raw_right):
                    hold left = int(raw_left)
                    hold right = int(raw_right)

                    gate op == "+":
                        show sanitize(add_values(left, right))
                    otherwise:
                        gate op == "-":
                            show sanitize(subtract_values(left, right))
                        otherwise:
                            gate op == "*":
                                show sanitize(multiply_values(left, right))
                            otherwise:
                                gate op == "/":
                                    gate right == 0:
                                        show "Cannot divide by zero."
                                    otherwise:
                                        show sanitize(divide_values(left, right))
                                otherwise:
                                    show "Please choose one of +, -, *, /."
                otherwise:
                    show "Please enter a valid second number."
            otherwise:
                show "Please enter a valid first number."

    0
"#
        .to_string(),
        files: Vec::new(),
    }
}

fn number_comparison_program(profile: &PromptProfile) -> GeneratedSnsxProgram {
    if profile.comparison_arity() == 3 {
        GeneratedSnsxProgram {
            title: "Largest Of Three Numbers".to_string(),
            prompt: profile.prompt.clone(),
            recipe: "number_comparison".to_string(),
            summary: "Generates a validated three-number comparison program that finds the largest value.".to_string(),
            suggested_path: "src/largest_of_three.snsx".to_string(),
            dependencies: vec!["std.io".to_string()],
            permissions: AgentPermissions {
                ai: false,
                ..AgentPermissions::default()
            },
            notes: vec![
                "This recipe asks for three numeric inputs and reuses max_two() to keep the logic clean.".to_string(),
            ],
            code: r#"bring <module/std.io> as io

flow max_two
    takes left: Int, right: Int
    gives Int
    gate left >= right:
        give left
    otherwise:
        give right

entry
    show "Enter first number:"
    hold raw_first = ask()
    show "Enter second number:"
    hold raw_second = ask()
    show "Enter third number:"
    hold raw_third = ask()

    gate raw_first == "":
        show "Please enter the first number."
    otherwise:
        gate raw_second == "":
            show "Please enter the second number."
        otherwise:
            gate raw_third == "":
                show "Please enter the third number."
            otherwise:
                gate is_int(raw_first):
                    gate is_int(raw_second):
                        gate is_int(raw_third):
                            hold first = int(raw_first)
                            hold second = int(raw_second)
                            hold third = int(raw_third)
                            hold largest = max_two(max_two(first, second), third)
                            show "Largest number:"
                            show sanitize(largest)
                        otherwise:
                            show "Please enter a valid third number."
                    otherwise:
                        show "Please enter a valid second number."
                otherwise:
                    show "Please enter a valid first number."

    0
"#
            .to_string(),
            files: Vec::new(),
        }
    } else {
        GeneratedSnsxProgram {
            title: "Number Comparison".to_string(),
            prompt: profile.prompt.clone(),
            recipe: "number_comparison".to_string(),
            summary: "Generates a validated two-number comparison program that finds the larger value.".to_string(),
            suggested_path: "src/number_comparison.snsx".to_string(),
            dependencies: vec!["std.io".to_string()],
            permissions: AgentPermissions {
                ai: false,
                ..AgentPermissions::default()
            },
            notes: vec![
                "This recipe keeps the decision logic in max_two() so the entry block stays easy to read.".to_string(),
            ],
            code: r#"bring <module/std.io> as io

flow max_two
    takes left: Int, right: Int
    gives Int
    gate left >= right:
        give left
    otherwise:
        give right

entry
    show "Enter first number:"
    hold raw_first = ask()
    show "Enter second number:"
    hold raw_second = ask()

    gate raw_first == "":
        show "Please enter the first number."
    otherwise:
        gate raw_second == "":
            show "Please enter the second number."
        otherwise:
            gate is_int(raw_first):
                gate is_int(raw_second):
                    hold first = int(raw_first)
                    hold second = int(raw_second)
                    show "Largest number:"
                    show sanitize(max_two(first, second))
                otherwise:
                    show "Please enter a valid second number."
            otherwise:
                show "Please enter a valid first number."

    0
"#
            .to_string(),
            files: Vec::new(),
        }
    }
}

fn data_record_store_program(profile: &PromptProfile) -> GeneratedSnsxProgram {
    let title = agent_title(&profile.prompt, "User Detail Database");
    let database_slug = profile.database_slug();
    let database_path = format!("data/{}.snsxdb", database_slug);
    let fields = profile.requested_fields();
    let suggested_path = format!(
        "src/{}_capture.snsx",
        non_empty_slug(&database_slug, "data_store")
    );

    let mut code = String::new();
    code.push_str("bring <module/std.io> as io\n");
    code.push_str("bring <module/std.db> as db\n\n");
    code.push_str("flow database_path\n");
    code.push_str("    gives String\n");
    code.push_str(&format!("    \"{}\"\n\n", escape_string(&database_path)));
    code.push_str("flow build_record\n");
    code.push_str("    takes ");
    code.push_str(
        &fields
            .iter()
            .map(|field| format!("{field}: String"))
            .collect::<Vec<_>>()
            .join(", "),
    );
    code.push('\n');
    code.push_str("    gives String\n");
    code.push_str("    ");
    code.push_str(
        &fields
            .iter()
            .map(|field| format!("\"{}=\" + sanitize({})", field, field))
            .collect::<Vec<_>>()
            .join(" + \"|\" + "),
    );
    code.push_str("\n\n");
    code.push_str("entry\n");
    code.push_str(&format!("    show \"{}\"\n", escape_string(&title)));
    for field in &fields {
        code.push_str(&format!(
            "    show \"Enter {}:\"\n",
            escape_string(&field_prompt(field))
        ));
        code.push_str(&format!("    hold {field} = ask()\n"));
    }
    code.push('\n');
    code.push_str(&render_field_validation(&fields, 0, 1));
    code.push_str("\n    0\n");

    GeneratedSnsxProgram {
        title: title.clone(),
        prompt: profile.prompt.clone(),
        recipe: "data_record_store".to_string(),
        summary: format!(
            "Generates a validated input form that stores structured records in the local SNSX database file {}.",
            database_path
        ),
        suggested_path,
        dependencies: vec!["std.io".to_string(), "std.db".to_string()],
        permissions: AgentPermissions {
            fs: true,
            ai: false,
            net: false,
        },
        notes: vec![
            "This recipe uses the SNSX local file-backed database surface, not an external SQL engine.".to_string(),
            format!("Records append into {} as line-based SNSX database entries.", database_path),
            "Email validation checks for '@' and '.', while mobile and age-like fields are constrained to digits.".to_string(),
        ],
        code,
        files: Vec::new(),
    }
}

fn text_summary_program(profile: &PromptProfile) -> GeneratedSnsxProgram {
    let title = agent_title(&profile.prompt, "Text Summary Agent");
    let instruction = profile.ai_instruction();
    GeneratedSnsxProgram {
        title: title.clone(),
        prompt: profile.prompt.clone(),
        recipe: "text_summary".to_string(),
        summary: "Generates an AI-native SNSX program that accepts one line of text and transforms it with a prompt-defined mind flow.".to_string(),
        suggested_path: "src/text_summary.snsx".to_string(),
        dependencies: vec!["std.io".to_string(), "std.ai".to_string()],
        permissions: AgentPermissions {
            ai: true,
            ..AgentPermissions::default()
        },
        notes: vec![
            "This recipe keeps input to one line so it works cleanly in CLI and IDE prompt mode.".to_string(),
            format!("The AI instruction was derived from the prompt: {}.", instruction),
        ],
        code: format!(
            "bring <module/std.io> as io\nbring <module/std.ai> as ai\n\nmind transform_text\n    takes text: String\n    gives String\n    from \"{}\"\n\nentry\n    show \"{}\"\n    show \"Enter text:\"\n    hold raw = ask()\n\n    gate raw == \"\":\n        show \"Please enter text to process.\"\n    otherwise:\n        hold result = sanitize(raw |> transform_text)\n        show result\n\n    0\n",
            escape_string(&instruction),
            escape_string(&title),
        ),
        files: Vec::new(),
    }
}

fn design_board_program(profile: &PromptProfile) -> GeneratedSnsxProgram {
    let title = agent_title(&profile.prompt, "Design Board");
    let body = truncate(profile.prompt.trim(), 88);
    let uses_ai = profile.wants_ai;
    let mut dependencies = vec![
        "std.io".to_string(),
        "std.design".to_string(),
        "std.sentinel".to_string(),
    ];
    if uses_ai {
        dependencies.push("std.ai".to_string());
    }
    let code = if uses_ai {
        format!(
            "bring <module/std.io> as io\nbring <module/std.ai> as ai\nbring <lib/std.design> as design\nbring <lib/std.sentinel> as sentry\n\nmind condense_board_note\n    takes text: String\n    gives String\n    from \"Summarize this product or system idea into one crisp board description\"\n\nentry\n    hold request = \"{}\"\n    hold summary = request |> condense_board_note\n    hold board = draft(\"{}\", stack([\n        panel(\"Prompt\", request),\n        panel(\"Summary\", summary),\n        field(\"shape\", shape(summary)),\n        field(\"seal\", seal(summary))\n    ]))\n    show board\n    0\n",
            escape_string(&body),
            escape_string(&title)
        )
    } else {
        format!(
            "bring <module/std.io> as io\nbring <lib/std.design> as design\nbring <lib/std.sentinel> as sentry\n\nentry\n    hold summary = \"{}\"\n    hold board = draft(\"{}\", stack([\n        panel(\"Prompt\", summary),\n        field(\"shape\", shape(summary)),\n        field(\"seal\", seal(summary))\n    ]))\n    show board\n    0\n",
            escape_string(&body),
            escape_string(&title)
        )
    };
    GeneratedSnsxProgram {
        title: title.clone(),
        prompt: profile.prompt.clone(),
        recipe: "design_board".to_string(),
        summary: "Generates a design-oriented SNSX board using panel, field, and draft helpers, with optional AI condensation.".to_string(),
        suggested_path: "src/design_board.snsx".to_string(),
        dependencies,
        permissions: AgentPermissions {
            ai: uses_ai,
            ..AgentPermissions::default()
        },
        notes: vec![
            "This recipe is useful for prompt-driven UI, reports, or presentation scaffolds.".to_string(),
            if uses_ai {
                "The planner enabled AI so the board includes a condensed description panel.".to_string()
            } else {
                "The board stays deterministic because the prompt did not require AI output.".to_string()
            },
        ],
        code,
        files: Vec::new(),
    }
}

fn access_guard_program(profile: &PromptProfile) -> GeneratedSnsxProgram {
    let title = agent_title(&profile.prompt, "Access Guard");
    GeneratedSnsxProgram {
        title: title.clone(),
        prompt: profile.prompt.clone(),
        recipe: "access_guard".to_string(),
        summary: "Generates a security-focused input gate for access-code style programs.".to_string(),
        suggested_path: "src/access_guard.snsx".to_string(),
        dependencies: vec!["std.io".to_string(), "std.error".to_string()],
        permissions: AgentPermissions {
            ai: false,
            ..AgentPermissions::default()
        },
        notes: vec![
            "Replace SNSX-OPEN with your real access token or validation rule.".to_string(),
            "This is a starter validation surface, not a full authentication framework.".to_string(),
        ],
        code: format!(
            "bring <module/std.io> as io\nbring <package/std.error> as errors\n\nentry\n    show \"{}\"\n    show \"Enter access code:\"\n    hold raw_code = ask()\n\n    gate raw_code == \"\":\n        show \"Please enter an access code.\"\n    otherwise:\n        gate raw_code == \"SNSX-OPEN\":\n            show \"Access granted.\"\n        otherwise:\n            show \"Access denied.\"\n\n    0\n",
            escape_string(&title)
        ),
        files: Vec::new(),
    }
}

fn worker_task_program(profile: &PromptProfile) -> GeneratedSnsxProgram {
    let title = agent_title(&profile.prompt, "Worker Task");
    let (formula, description) = profile.worker_formula();
    GeneratedSnsxProgram {
        title: title.clone(),
        prompt: profile.prompt.clone(),
        recipe: "worker_task".to_string(),
        summary: "Generates a deterministic launch/wait worker program with validated numeric input.".to_string(),
        suggested_path: "src/worker_task.snsx".to_string(),
        dependencies: vec!["std.io".to_string()],
        permissions: AgentPermissions {
            ai: false,
            ..AgentPermissions::default()
        },
        notes: vec![
            "This recipe uses SNSX launch/wait so the prompt becomes a real concurrency example.".to_string(),
            format!("The worker currently {}.", description),
        ],
        code: format!(
            "bring <module/std.io> as io\n\nflow work_unit\n    takes value: Int\n    gives Int\n    {formula}\n\nentry\n    show \"{}\"\n    show \"Enter a job value:\"\n    hold raw = ask()\n\n    gate raw == \"\":\n        show \"Please enter a job value.\"\n    otherwise:\n        gate is_int(raw):\n            hold value = int(raw)\n            hold task = launch work_unit(value)\n            hold result = sanitize(wait task)\n            show \"Worker result:\"\n            show result\n        otherwise:\n            show \"Please enter a valid numeric job value.\"\n\n    0\n",
            escape_string(&title)
        ),
        files: Vec::new(),
    }
}

fn input_echo_program(profile: &PromptProfile) -> GeneratedSnsxProgram {
    let title = agent_title(&profile.prompt, "Input Echo");
    GeneratedSnsxProgram {
        title: title.clone(),
        prompt: profile.prompt.clone(),
        recipe: "input_echo".to_string(),
        summary: "Generates a simple one-input starter that validates and echoes a user value.".to_string(),
        suggested_path: "src/input_echo.snsx".to_string(),
        dependencies: vec!["std.io".to_string()],
        permissions: AgentPermissions {
            ai: false,
            ..AgentPermissions::default()
        },
        notes: vec![
            "This is the safest default scaffold when the prompt asks for user input but does not define the full business rule yet.".to_string(),
        ],
        code: format!(
            "bring <module/std.io> as io\n\nentry\n    show \"{}\"\n    show \"Enter value:\"\n    hold raw = ask()\n\n    gate raw == \"\":\n        show \"Please enter a value.\"\n    otherwise:\n        show \"Received:\"\n        show sanitize(raw)\n\n    0\n",
            escape_string(&title)
        ),
        files: Vec::new(),
    }
}

fn generic_starter_program(profile: &PromptProfile) -> GeneratedSnsxProgram {
    let title = agent_title(&profile.prompt, "SNS AI Starter");
    let prompt_label = truncate(profile.prompt.trim(), 96);
    GeneratedSnsxProgram {
        title: title.clone(),
        prompt: profile.prompt.clone(),
        recipe: "generic_starter".to_string(),
        summary: "Generates a safe SNSX starter scaffold when the prompt does not strongly match a specialized recipe.".to_string(),
        suggested_path: format!("src/{}.snsx", non_empty_slug(&profile.slug, &title)),
        dependencies: vec!["std.io".to_string()],
        permissions: AgentPermissions {
            ai: false,
            ..AgentPermissions::default()
        },
        notes: vec![
            "The prompt was converted into a clean starter title and description scaffold.".to_string(),
            "Extend the entry block or add flows once you know the exact business rules.".to_string(),
        ],
        code: format!(
            "bring <module/std.io> as io\n\nflow describe_request\n    gives String\n    \"{}\"\n\nflow next_step\n    gives String\n    \"Replace describe_request() with your project logic or use the SNS AI agent again with a more specific prompt.\"\n\nentry\n    show \"{}\"\n    show describe_request()\n    show next_step()\n    0\n",
            escape_string(&prompt_label),
            escape_string(&title)
        ),
        files: Vec::new(),
    }
}

fn render_field_validation(fields: &[&str], index: usize, depth: usize) -> String {
    if index >= fields.len() {
        let indent = "    ".repeat(depth);
        let record_args = fields.join(", ");
        return format!(
            "{indent}hold record = build_record({record_args})\n{indent}hold saved_to = append_text(database_path(), record + \"\\n\")\n{indent}show \"Record saved to:\"\n{indent}show saved_to\n"
        );
    }

    let field = fields[index];
    let indent = "    ".repeat(depth);
    let mut out = String::new();
    out.push_str(&format!("{indent}gate {field} == \"\":\n"));
    out.push_str(&format!(
        "{indent}    show \"{}\"\n",
        escape_string(&field_missing_message(field))
    ));
    out.push_str(&format!("{indent}otherwise:\n"));

    if let Some(condition) = field_validation_condition(field) {
        out.push_str(&format!("{indent}    gate {condition}:\n"));
        out.push_str(&render_field_validation(fields, index + 1, depth + 2));
        out.push_str(&format!("{indent}    otherwise:\n"));
        out.push_str(&format!(
            "{indent}        show \"{}\"\n",
            escape_string(&field_invalid_message(field))
        ));
    } else {
        out.push_str(&render_field_validation(fields, index + 1, depth + 1));
    }
    out
}

fn field_prompt(field: &str) -> String {
    match field {
        "name" => "name".to_string(),
        "mobile" => "mobile number".to_string(),
        "email" => "email address".to_string(),
        "address" => "address".to_string(),
        "city" => "city".to_string(),
        "age" => "age".to_string(),
        _ => field.to_string(),
    }
}

fn field_missing_message(field: &str) -> String {
    match field {
        "name" => "Please enter the name.".to_string(),
        "mobile" => "Please enter the mobile number.".to_string(),
        "email" => "Please enter the email address.".to_string(),
        "address" => "Please enter the address.".to_string(),
        "city" => "Please enter the city.".to_string(),
        "age" => "Please enter the age.".to_string(),
        _ => "Please enter the value.".to_string(),
    }
}

fn field_validation_condition(field: &str) -> Option<&'static str> {
    match field {
        "mobile" => Some("is_digits(mobile)"),
        "age" => Some("is_digits(age)"),
        "email" => Some("is_email(email)"),
        _ => None,
    }
}

fn field_invalid_message(field: &str) -> String {
    match field {
        "mobile" => "Please enter a valid mobile number using digits only.".to_string(),
        "age" => "Please enter a valid numeric age.".to_string(),
        "email" => "Please enter a valid email address.".to_string(),
        _ => "Please enter a valid value.".to_string(),
    }
}

fn agent_title(prompt: &str, fallback: &str) -> String {
    let trimmed = prompt.trim();
    if trimmed.is_empty() {
        return fallback.to_string();
    }
    let first = trimmed
        .split(['.', '!', '?', '\n'])
        .next()
        .unwrap_or(trimmed)
        .trim();
    let short = first
        .split_whitespace()
        .take(6)
        .collect::<Vec<_>>()
        .join(" ");
    if short.is_empty() {
        fallback.to_string()
    } else {
        short
    }
}

fn contains_any(input: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| contains_term(input, needle))
}

fn contains_term(input: &str, term: &str) -> bool {
    if term.contains(' ') {
        input.contains(term)
    } else {
        input
            .split(|ch: char| !ch.is_ascii_alphanumeric() && ch != '_')
            .any(|token| token == term)
    }
}

fn extract_numbers(input: &str) -> Vec<i64> {
    let mut numbers = Vec::new();
    let mut current = String::new();
    for ch in input.chars() {
        if ch.is_ascii_digit() || (current.is_empty() && ch == '-') {
            current.push(ch);
        } else if !current.is_empty() {
            if let Ok(value) = current.parse::<i64>() {
                numbers.push(value);
            }
            current.clear();
        }
    }
    if !current.is_empty() {
        if let Ok(value) = current.parse::<i64>() {
            numbers.push(value);
        }
    }
    numbers
}

fn escape_string(input: &str) -> String {
    input.replace('\\', "\\\\").replace('"', "\\\"")
}

fn non_empty_slug(slug: &str, fallback: &str) -> String {
    if slug.is_empty() {
        slugify(fallback)
    } else {
        slug.to_string()
    }
}

fn slugify(input: &str) -> String {
    let mut out = String::new();
    let mut last_was_sep = false;
    for ch in input.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            last_was_sep = false;
        } else if !last_was_sep {
            out.push('_');
            last_was_sep = true;
        }
    }
    out.trim_matches('_').to_string().chars().take(32).collect()
}

fn extract_database_name(prompt: &str) -> Option<String> {
    let lower = prompt.to_lowercase();
    for marker in [
        "database named",
        "database name",
        "db named",
        "table named",
        "named",
    ] {
        if let Some(index) = lower.find(marker) {
            let tail = prompt.get(index + marker.len()..)?.trim();
            let mut words = Vec::new();
            for raw in
                tail.split(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_' || ch == '-'))
            {
                let token = raw.trim();
                if token.is_empty() {
                    continue;
                }
                let token_lower = token.to_lowercase();
                if !words.is_empty()
                    && matches!(
                        token_lower.as_str(),
                        "with"
                            | "that"
                            | "which"
                            | "where"
                            | "using"
                            | "for"
                            | "from"
                            | "to"
                            | "into"
                            | "and"
                            | "then"
                    )
                {
                    break;
                }
                words.push(token);
                if words.len() >= 4 {
                    break;
                }
            }
            if !words.is_empty() {
                return Some(slugify(&words.join(" ")));
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picks_vote_recipe_for_vote_prompt() {
        let artifact =
            SnsxCodingAgent::generate("Write a program to check vote eligibility by age").unwrap();
        assert_eq!(artifact.recipe, "vote_eligibility");
        assert!(artifact.code.contains("You are eligible to vote."));
    }

    #[test]
    fn picks_vote_recipe_for_common_vote_eligibility_phrase() {
        let artifact = SnsxCodingAgent::generate("write a vote eligibility checker").unwrap();
        assert_eq!(artifact.recipe, "vote_eligibility");
    }

    #[test]
    fn picks_summary_recipe_for_summary_prompt() {
        let artifact =
            SnsxCodingAgent::generate("Build an AI summary tool for one line of text").unwrap();
        assert_eq!(artifact.recipe, "text_summary");
        assert!(artifact.code.contains("mind transform_text"));
    }

    #[test]
    fn builds_calculator_recipe_for_arithmetic_prompt() {
        let artifact =
            SnsxCodingAgent::generate("Write a calculator that asks two numbers and an operator")
                .unwrap();
        assert_eq!(artifact.recipe, "calculator");
        assert!(artifact.code.contains("flow add_values"));
    }

    #[test]
    fn builds_worker_recipe_for_parallel_prompt() {
        let artifact =
            SnsxCodingAgent::generate("Create a parallel worker that doubles the input").unwrap();
        assert_eq!(artifact.recipe, "worker_task");
        assert!(artifact.code.contains("launch work_unit"));
    }

    #[test]
    fn annotates_validation_in_notes() {
        let artifact = SnsxCodingAgent::generate("write a grade checker").unwrap();
        assert!(artifact
            .notes
            .iter()
            .any(|note| note.contains("Compiler-validated in strict mode")));
    }

    #[test]
    fn builds_data_store_recipe_for_contact_database_prompt() {
        let artifact = SnsxCodingAgent::generate(
            "write a program to take input from the user about the name, mobile number, email id and store it in a database named user detail",
        )
        .unwrap();
        assert_eq!(artifact.recipe, "data_record_store");
        assert!(artifact.permissions.fs);
        assert!(artifact
            .code
            .contains("append_text(database_path(), record + \"\\n\")"));
        assert!(artifact.code.contains("is_email(email)"));
        assert!(artifact.code.contains("data/user_detail.snsxdb"));
    }

    #[test]
    fn builds_registration_suite_for_multifile_system_prompt() {
        let artifact = SnsxCodingAgent::generate(
            "build a user registration system with create, search, update, delete, login, and report export",
        )
        .unwrap();
        assert_eq!(artifact.recipe, "registration_suite");
        assert!(artifact.permissions.fs);
        assert_eq!(artifact.suggested_path, "src/main.snsx");
        assert!(artifact.files.len() >= 4);
        assert!(artifact
            .files
            .iter()
            .any(|file| file.path == "src/app/store.snsx"));
        assert!(artifact
            .files
            .iter()
            .any(|file| file.path == "src/app/validators.snsx"));
        assert!(artifact
            .files
            .iter()
            .any(|file| file.code.contains("export_user_report")));
    }
}
