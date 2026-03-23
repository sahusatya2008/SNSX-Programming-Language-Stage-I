use std::collections::{HashMap, VecDeque};
use std::rc::Rc;

use aeris_compiler::ast::{ActorDecl, Block, Expr, FunctionDecl, MatchArm, Pattern, Stmt};
use aeris_compiler::{compile_source, CompileOptions, CompiledUnit};
use anyhow::{anyhow, bail, Context, Result};

#[derive(Clone, Debug)]
pub struct RunOptions {
    pub entry: String,
    pub stdin_text: Option<String>,
}

impl Default for RunOptions {
    fn default() -> Self {
        Self {
            entry: "main".to_string(),
            stdin_text: None,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Execution {
    pub exit_code: i64,
    pub stdout: String,
    pub actor_trace: Vec<String>,
}

pub fn compile_and_run(source_name: &str, source: &str, options: RunOptions) -> Result<Execution> {
    let compiled = compile_source(source_name, source, CompileOptions::default())
        .map_err(|diag| anyhow!(render_diagnostics(&diag)))?;
    run(&compiled, options)
}

pub fn run(compiled: &CompiledUnit, options: RunOptions) -> Result<Execution> {
    let mut runtime = Runtime::new(compiled.checked.program.clone(), options.stdin_text);
    let result = runtime.call_function(&options.entry, runtime.seed_entry_args(&options.entry)?)?;
    let exit_code = match result {
        Value::Int(value) => value,
        Value::Unit => 0,
        other => bail!(
            "entry function returned non-integer value: {}",
            other.describe()
        ),
    };
    Ok(Execution {
        exit_code,
        stdout: runtime.stdout,
        actor_trace: runtime.actor_trace,
    })
}

fn render_diagnostics(diag: &[aeris_compiler::diag::Diagnostic]) -> String {
    let mut out = String::new();
    for item in diag {
        out.push_str(&format!("[{}] {}\n", item.code, item.message));
        if let Some(help) = &item.help {
            out.push_str(&format!("help: {help}\n"));
        }
        if let Some(detail) = &item.detail {
            out.push_str(&format!("detail: {detail}\n"));
        }
    }
    out
}

#[derive(Clone)]
enum Value {
    Int(i64),
    Bool(bool),
    Text(String),
    Unit,
    Variant { tag: String, values: Vec<Value> },
    FunctionRef(String),
    Closure(Rc<Closure>),
    ActorFactory(String),
    ActorHandle(usize),
    Capability(String),
}

impl Value {
    fn describe(&self) -> String {
        match self {
            Value::Int(value) => value.to_string(),
            Value::Bool(value) => value.to_string(),
            Value::Text(value) => value.clone(),
            Value::Unit => "unit".to_string(),
            Value::Variant { tag, values } => {
                let parts = values.iter().map(Value::describe).collect::<Vec<_>>();
                format!("{tag}({})", parts.join(", "))
            }
            Value::FunctionRef(name) => format!("<fn {name}>"),
            Value::Closure(_) => "<closure>".to_string(),
            Value::ActorFactory(name) => format!("<actor {name}>"),
            Value::ActorHandle(id) => format!("<actor#{id}>"),
            Value::Capability(name) => format!("<cap {name}>"),
        }
    }
}

#[derive(Clone)]
struct Closure {
    params: Vec<String>,
    body: Expr,
    env: Env,
}

type Env = HashMap<String, Value>;

struct Runtime {
    functions: HashMap<String, FunctionDecl>,
    actors: HashMap<String, ActorDecl>,
    constructors: HashMap<String, usize>,
    stdout: String,
    stdin_text: Option<String>,
    next_actor_id: usize,
    actor_instances: HashMap<usize, String>,
    mailbox: VecDeque<(usize, Value)>,
    actor_trace: Vec<String>,
}

impl Runtime {
    fn new(program: aeris_compiler::ast::Program, stdin_text: Option<String>) -> Self {
        let functions = program
            .functions
            .iter()
            .map(|function| (function.name.clone(), function.clone()))
            .collect::<HashMap<_, _>>();
        let actors = program
            .actors
            .iter()
            .map(|actor| (actor.name.clone(), actor.clone()))
            .collect::<HashMap<_, _>>();
        let constructors = program
            .types
            .iter()
            .flat_map(|item| {
                item.variants
                    .iter()
                    .map(|variant| (variant.name.clone(), variant.fields.len()))
            })
            .collect::<HashMap<_, _>>();
        Self {
            functions,
            actors,
            constructors,
            stdout: String::new(),
            stdin_text,
            next_actor_id: 1,
            actor_instances: HashMap::new(),
            mailbox: VecDeque::new(),
            actor_trace: Vec::new(),
        }
    }

    fn seed_entry_args(&self, entry: &str) -> Result<Vec<Value>> {
        let function = self
            .functions
            .get(entry)
            .with_context(|| format!("entry function '{entry}' not found"))?;
        let mut args = Vec::new();
        for param in &function.params {
            let value = match param.ty.name() {
                "Int" => Value::Int(0),
                "Bool" => Value::Bool(false),
                "Text" => Value::Text(String::new()),
                capability => Value::Capability(capability.to_string()),
            };
            args.push(value);
        }
        Ok(args)
    }

    fn call_function(&mut self, name: &str, args: Vec<Value>) -> Result<Value> {
        let function = self
            .functions
            .get(name)
            .cloned()
            .with_context(|| format!("function '{name}' not found"))?;
        if function.params.len() != args.len() {
            bail!(
                "function '{}' expects {} argument(s), got {}",
                name,
                function.params.len(),
                args.len()
            );
        }
        let mut env = Env::new();
        for (param, arg) in function.params.iter().zip(args.into_iter()) {
            env.insert(param.name.clone(), arg);
        }
        match self.eval_block(&function.body, &mut env)? {
            Flow::Value(value) | Flow::Return(value) => Ok(value),
        }
    }

    fn eval_block(&mut self, block: &Block, env: &mut Env) -> Result<Flow> {
        for stmt in &block.statements {
            match stmt {
                Stmt::Let { name, value, .. } => {
                    let value = match value {
                        Some(expr) => self.eval_expr(expr, env)?,
                        None => Value::Unit,
                    };
                    env.insert(name.clone(), value);
                }
                Stmt::Mut { name, value, .. } => {
                    let value = self.eval_expr(value, env)?;
                    env.insert(name.clone(), value);
                }
                Stmt::Require {
                    condition, message, ..
                } => {
                    let passed = self.eval_expr(condition, env)?;
                    if !matches!(passed, Value::Bool(true)) {
                        bail!(
                            "{}",
                            message
                                .clone()
                                .unwrap_or_else(|| "require condition failed".to_string())
                        );
                    }
                }
                Stmt::Return { value, .. } => {
                    let value = self.eval_expr(value, env)?;
                    return Ok(Flow::Return(value));
                }
                Stmt::Expr { value, .. } => {
                    let _ = self.eval_expr(value, env)?;
                }
            }
        }

        if let Some(expr) = &block.tail {
            Ok(Flow::Value(self.eval_expr(expr, env)?))
        } else {
            Ok(Flow::Value(Value::Unit))
        }
    }

    fn eval_expr(&mut self, expr: &Expr, env: &mut Env) -> Result<Value> {
        match expr {
            Expr::Int(value) => Ok(Value::Int(*value)),
            Expr::Bool(value) => Ok(Value::Bool(*value)),
            Expr::Text(value) => Ok(Value::Text(value.clone())),
            Expr::Variable(name) => env
                .get(name)
                .cloned()
                .or_else(|| {
                    self.functions
                        .get(name)
                        .map(|_| Value::FunctionRef(name.clone()))
                })
                .or_else(|| {
                    self.actors
                        .get(name)
                        .map(|_| Value::ActorFactory(name.clone()))
                })
                .ok_or_else(|| anyhow!("unbound identifier '{}'", name)),
            Expr::Unary { op, value } => {
                let value = self.eval_expr(value, env)?;
                match (op, value) {
                    (aeris_compiler::ast::UnaryOp::Neg, Value::Int(value)) => {
                        Ok(Value::Int(-value))
                    }
                    (aeris_compiler::ast::UnaryOp::Not, Value::Bool(value)) => {
                        Ok(Value::Bool(!value))
                    }
                    _ => bail!("invalid unary operand"),
                }
            }
            Expr::Binary { op, left, right } => {
                let left = self.eval_expr(left, env)?;
                let right = self.eval_expr(right, env)?;
                self.eval_binary(op, left, right)
            }
            Expr::If {
                condition,
                then_branch,
                else_branch,
            } => {
                let condition = self.eval_expr(condition, env)?;
                if matches!(condition, Value::Bool(true)) {
                    let mut branch_env = env.clone();
                    match self.eval_block(then_branch, &mut branch_env)? {
                        Flow::Value(value) | Flow::Return(value) => Ok(value),
                    }
                } else {
                    let mut branch_env = env.clone();
                    match self.eval_block(else_branch, &mut branch_env)? {
                        Flow::Value(value) | Flow::Return(value) => Ok(value),
                    }
                }
            }
            Expr::Match { scrutinee, arms } => {
                let value = self.eval_expr(scrutinee, env)?;
                self.eval_match(value, arms, env)
            }
            Expr::Lambda { params, body } => Ok(Value::Closure(Rc::new(Closure {
                params: params.clone(),
                body: body.as_ref().clone(),
                env: env.clone(),
            }))),
            Expr::Call { callee, args } => {
                if let Expr::Variable(name) = &**callee {
                    if let Some(result) = self.try_builtin(name, args, env)? {
                        return Ok(result);
                    }
                    if self.constructors.contains_key(name) {
                        let values = args
                            .iter()
                            .map(|arg| self.eval_expr(arg, env))
                            .collect::<Result<Vec<_>>>()?;
                        return Ok(Value::Variant {
                            tag: name.clone(),
                            values,
                        });
                    }
                }

                let callable = self.eval_expr(callee, env)?;
                let values = args
                    .iter()
                    .map(|arg| self.eval_expr(arg, env))
                    .collect::<Result<Vec<_>>>()?;
                match callable {
                    Value::FunctionRef(name) => self.call_function(&name, values),
                    Value::Closure(closure) => {
                        if closure.params.len() != values.len() {
                            bail!("closure expected {} args", closure.params.len());
                        }
                        let mut nested = closure.env.clone();
                        for (param, arg) in closure.params.iter().zip(values.into_iter()) {
                            nested.insert(param.clone(), arg);
                        }
                        match self.eval_expr(&closure.body, &mut nested)? {
                            value => Ok(value),
                        }
                    }
                    other => bail!("value '{}' is not callable", other.describe()),
                }
            }
        }
    }

    fn eval_binary(
        &self,
        op: &aeris_compiler::ast::BinaryOp,
        left: Value,
        right: Value,
    ) -> Result<Value> {
        use aeris_compiler::ast::BinaryOp;
        match op {
            BinaryOp::Add => match (left, right) {
                (Value::Int(left), Value::Int(right)) => Ok(Value::Int(left + right)),
                (Value::Text(left), Value::Text(right)) => {
                    Ok(Value::Text(format!("{left}{right}")))
                }
                (Value::Text(left), other) => {
                    Ok(Value::Text(format!("{left}{}", other.describe())))
                }
                (other, Value::Text(right)) => {
                    Ok(Value::Text(format!("{}{right}", other.describe())))
                }
                _ => bail!("addition requires Int or Text operands"),
            },
            BinaryOp::Sub => int_bin(left, right, |l, r| l - r),
            BinaryOp::Mul => int_bin(left, right, |l, r| l * r),
            BinaryOp::Div => match (left, right) {
                (Value::Int(_), Value::Int(0)) => bail!("division by zero"),
                (Value::Int(left), Value::Int(right)) => Ok(Value::Int(left / right)),
                _ => bail!("division requires Int operands"),
            },
            BinaryOp::Mod => match (left, right) {
                (Value::Int(_), Value::Int(0)) => bail!("modulo by zero"),
                (Value::Int(left), Value::Int(right)) => Ok(Value::Int(left % right)),
                _ => bail!("modulo requires Int operands"),
            },
            BinaryOp::Eq => Ok(Value::Bool(eq_values(&left, &right))),
            BinaryOp::Ne => Ok(Value::Bool(!eq_values(&left, &right))),
            BinaryOp::Lt => int_cmp(left, right, |l, r| l < r),
            BinaryOp::Le => int_cmp(left, right, |l, r| l <= r),
            BinaryOp::Gt => int_cmp(left, right, |l, r| l > r),
            BinaryOp::Ge => int_cmp(left, right, |l, r| l >= r),
            BinaryOp::And => match (left, right) {
                (Value::Bool(left), Value::Bool(right)) => Ok(Value::Bool(left && right)),
                _ => bail!("&& requires Bool operands"),
            },
            BinaryOp::Or => match (left, right) {
                (Value::Bool(left), Value::Bool(right)) => Ok(Value::Bool(left || right)),
                _ => bail!("|| requires Bool operands"),
            },
        }
    }

    fn eval_match(&mut self, value: Value, arms: &[MatchArm], env: &mut Env) -> Result<Value> {
        for arm in arms {
            if let Some(bindings) = pattern_bind(&arm.pattern, &value) {
                let mut local = env.clone();
                for (name, value) in bindings {
                    local.insert(name, value);
                }
                return match self.eval_block(&arm.body, &mut local)? {
                    Flow::Value(value) | Flow::Return(value) => Ok(value),
                };
            }
        }
        bail!("match was not exhaustive")
    }

    fn try_builtin(&mut self, name: &str, args: &[Expr], env: &mut Env) -> Result<Option<Value>> {
        match name {
            "print" => {
                if args.len() < 2 {
                    bail!("print expects (Console, value)");
                }
                let cap = self.eval_expr(&args[0], env)?;
                ensure_capability(&cap, "Console")?;
                let value = self.eval_expr(&args[1], env)?;
                self.stdout.push_str(&value.describe());
                self.stdout.push('\n');
                Ok(Some(Value::Unit))
            }
            "read_line" => {
                if args.len() != 1 {
                    bail!("read_line expects (Console)");
                }
                let cap = self.eval_expr(&args[0], env)?;
                ensure_capability(&cap, "Console")?;
                let line = self.stdin_text.take().unwrap_or_default();
                Ok(Some(Value::Text(line)))
            }
            "assert" => {
                if args.len() < 2 {
                    bail!("assert expects (Bool, Text)");
                }
                let cond = self.eval_expr(&args[0], env)?;
                let msg = self.eval_expr(&args[1], env)?;
                match (cond, msg) {
                    (Value::Bool(true), _) => Ok(Some(Value::Unit)),
                    (Value::Bool(false), Value::Text(text)) => bail!("{text}"),
                    _ => bail!("assert expects (Bool, Text)"),
                }
            }
            "spawn" => {
                if args.len() != 1 {
                    bail!("spawn expects one actor factory");
                }
                let actor = self.eval_expr(&args[0], env)?;
                let name = match actor {
                    Value::ActorFactory(name) => name,
                    Value::Text(name) => name,
                    other => bail!("spawn expected actor factory, got {}", other.describe()),
                };
                let id = self.next_actor_id;
                self.next_actor_id += 1;
                self.actor_instances.insert(id, name.clone());
                self.actor_trace.push(format!("spawn actor#{id} {name}"));
                Ok(Some(Value::ActorHandle(id)))
            }
            "emit" => {
                if args.len() != 2 {
                    bail!("emit expects (ActorHandle, Message)");
                }
                let handle = self.eval_expr(&args[0], env)?;
                let message = self.eval_expr(&args[1], env)?;
                let id = match handle {
                    Value::ActorHandle(id) => id,
                    other => bail!("emit expected actor handle, got {}", other.describe()),
                };
                self.mailbox.push_back((id, message.clone()));
                self.actor_trace
                    .push(format!("queue actor#{id} {}", message.describe()));
                Ok(Some(Value::Unit))
            }
            "drain" => {
                let mut processed = 0;
                while let Some((id, message)) = self.mailbox.pop_front() {
                    processed += 1;
                    self.process_actor_message(id, message)?;
                }
                Ok(Some(Value::Int(processed)))
            }
            _ => Ok(None),
        }
    }

    fn process_actor_message(&mut self, id: usize, message: Value) -> Result<()> {
        let actor_name = self
            .actor_instances
            .get(&id)
            .cloned()
            .with_context(|| format!("unknown actor handle {id}"))?;
        let actor = self
            .actors
            .get(&actor_name)
            .cloned()
            .with_context(|| format!("actor '{actor_name}' not found"))?;
        for handler in &actor.handlers {
            if let Some(bindings) = pattern_bind(&handler.pattern, &message) {
                let mut env = Env::new();
                for (name, value) in bindings {
                    env.insert(name, value);
                }
                let value = match self.eval_block(&handler.body, &mut env)? {
                    Flow::Value(value) | Flow::Return(value) => value,
                };
                self.actor_trace.push(format!(
                    "actor#{id} handled {} => {}",
                    message.describe(),
                    value.describe()
                ));
                return Ok(());
            }
        }
        bail!("no actor handler matched {}", message.describe())
    }
}

enum Flow {
    Value(Value),
    Return(Value),
}

fn ensure_capability(value: &Value, expected: &str) -> Result<()> {
    match value {
        Value::Capability(name) if name == expected => Ok(()),
        Value::Capability(name) => bail!("expected capability {expected}, got {name}"),
        other => bail!("expected capability {expected}, got {}", other.describe()),
    }
}

fn int_bin(left: Value, right: Value, op: impl FnOnce(i64, i64) -> i64) -> Result<Value> {
    match (left, right) {
        (Value::Int(left), Value::Int(right)) => Ok(Value::Int(op(left, right))),
        _ => bail!("arithmetic requires Int operands"),
    }
}

fn int_cmp(left: Value, right: Value, op: impl FnOnce(i64, i64) -> bool) -> Result<Value> {
    match (left, right) {
        (Value::Int(left), Value::Int(right)) => Ok(Value::Bool(op(left, right))),
        _ => bail!("comparison requires Int operands"),
    }
}

fn eq_values(left: &Value, right: &Value) -> bool {
    match (left, right) {
        (Value::Int(left), Value::Int(right)) => left == right,
        (Value::Bool(left), Value::Bool(right)) => left == right,
        (Value::Text(left), Value::Text(right)) => left == right,
        (Value::Unit, Value::Unit) => true,
        _ => false,
    }
}

fn pattern_bind(pattern: &Pattern, value: &Value) -> Option<Vec<(String, Value)>> {
    match (pattern, value) {
        (Pattern::Wildcard, _) => Some(Vec::new()),
        (Pattern::Identifier(name), value) => Some(vec![(name.clone(), value.clone())]),
        (Pattern::Int(left), Value::Int(right)) if left == right => Some(Vec::new()),
        (Pattern::Bool(left), Value::Bool(right)) if left == right => Some(Vec::new()),
        (Pattern::Text(left), Value::Text(right)) if left == right => Some(Vec::new()),
        (Pattern::Variant { name, bindings }, Value::Variant { tag, values })
            if name == tag && bindings.len() == values.len() =>
        {
            Some(
                bindings
                    .iter()
                    .cloned()
                    .zip(values.iter().cloned())
                    .collect(),
            )
        }
        _ => None,
    }
}
