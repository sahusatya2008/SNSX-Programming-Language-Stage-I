use anyhow::{anyhow, bail, Result};
use sha3::{Digest, Sha3_256};
use snsx_ai_engine::{tensor_from_values, ModelRegistry};
use snsx_compiler::codegen::vm::{BytecodeFunction, BytecodeModule, Instruction};
use snsx_runtime::{RuntimeContext, SandboxPolicy, TaskId, Value};
use std::collections::HashMap;
use std::error::Error as StdError;
use std::fmt;
use std::fs;
use std::io::Write;

#[derive(Debug, Clone)]
pub struct VmOptions {
    pub deterministic: bool,
    pub trace: bool,
    pub sandbox: SandboxPolicy,
    pub stdin: String,
    pub allow_host_stdin: bool,
}

impl Default for VmOptions {
    fn default() -> Self {
        Self {
            deterministic: false,
            trace: false,
            sandbox: SandboxPolicy::default(),
            stdin: String::new(),
            allow_host_stdin: false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ExecutionResult {
    pub value: Value,
    pub stdout: String,
    pub trace: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct InputRequest {
    pub stdout: String,
    pub trace: Vec<String>,
    pub consumed_lines: usize,
}

#[derive(Debug, Clone)]
pub enum VmTrap {
    InputRequested(InputRequest),
}

impl fmt::Display for VmTrap {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VmTrap::InputRequested(request) => write!(
                f,
                "SNSX program is waiting for input line {}",
                request.consumed_lines + 1
            ),
        }
    }
}

impl StdError for VmTrap {}

#[derive(Debug, Clone)]
struct PendingTask {
    symbol_index: usize,
    args: Vec<Value>,
}

#[derive(Debug, Clone)]
struct Frame {
    function_index: usize,
    ip: usize,
    locals: Vec<Value>,
    stack: Vec<Value>,
}

pub struct VirtualMachine {
    module: BytecodeModule,
    runtime: RuntimeContext,
    models: ModelRegistry,
    pending_tasks: HashMap<TaskId, PendingTask>,
    trace: Vec<String>,
    trace_enabled: bool,
}

impl VirtualMachine {
    pub fn new(module: BytecodeModule, options: VmOptions) -> Self {
        let sandbox = SandboxPolicy {
            deterministic: options.deterministic || options.sandbox.deterministic,
            ..options.sandbox
        };
        Self {
            module,
            runtime: RuntimeContext::new(sandbox, options.stdin, options.allow_host_stdin),
            models: ModelRegistry::new(),
            pending_tasks: HashMap::new(),
            trace: Vec::new(),
            trace_enabled: options.trace,
        }
    }

    pub fn execute_main(&mut self, args: Vec<Value>) -> Result<ExecutionResult> {
        let value = self.invoke_symbol(self.module.entry, args)?;
        Ok(ExecutionResult {
            value,
            stdout: self.runtime.stdout.clone(),
            trace: self.trace.clone(),
        })
    }

    pub fn invoke_symbol(&mut self, symbol_index: usize, args: Vec<Value>) -> Result<Value> {
        let Some(name) = self.module.symbols.get(symbol_index).cloned() else {
            bail!("unknown symbol index {}", symbol_index);
        };
        if let Some(function_index) = self.module.user_functions.get(&name).copied() {
            let function = self
                .module
                .functions
                .get(function_index)
                .cloned()
                .ok_or_else(|| anyhow!("missing function"))?;
            self.invoke_function(function_index, &function, args)
        } else {
            self.call_builtin(&name, args)
        }
    }

    fn invoke_function(
        &mut self,
        function_index: usize,
        function: &BytecodeFunction,
        args: Vec<Value>,
    ) -> Result<Value> {
        if function.is_ai {
            self.runtime.sandbox.check_ai()?;
            let prompt = function
                .prompt
                .clone()
                .unwrap_or_else(|| "AI prompt".to_string());
            return self.models.invoke_prompt_function(&prompt, &args, None);
        }

        let mut frame = Frame {
            function_index,
            ip: 0,
            locals: vec![Value::Unit; function.locals.max(function.params)],
            stack: Vec::new(),
        };
        for (index, arg) in args.into_iter().enumerate() {
            if index < frame.locals.len() {
                frame.locals[index] = arg;
            }
        }
        loop {
            let Some(instruction) = function.code.get(frame.ip).cloned() else {
                return Ok(Value::Unit);
            };
            if self.trace_enabled {
                self.trace.push(format!(
                    "{}#{} ip={} {:?}",
                    function.name, frame.function_index, frame.ip, instruction
                ));
            }
            frame.ip += 1;
            match instruction {
                Instruction::PushConst(index) => {
                    let value = function
                        .constants
                        .get(index)
                        .cloned()
                        .ok_or_else(|| anyhow!("constant index {} out of range", index))?;
                    frame.stack.push(constant_to_value(value));
                }
                Instruction::LoadLocal(slot) => {
                    frame
                        .stack
                        .push(frame.locals.get(slot).cloned().unwrap_or(Value::Unit));
                }
                Instruction::StoreLocal(slot) => {
                    let value = frame
                        .stack
                        .pop()
                        .ok_or_else(|| anyhow!("stack underflow on store"))?;
                    if slot >= frame.locals.len() {
                        frame.locals.resize(slot + 1, Value::Unit);
                    }
                    frame.locals[slot] = value;
                }
                Instruction::LoadFunction(index) => frame.stack.push(Value::Function(index)),
                Instruction::Add => binop(&mut frame.stack, add_values)?,
                Instruction::Sub => binop(&mut frame.stack, sub_values)?,
                Instruction::Mul => binop(&mut frame.stack, mul_values)?,
                Instruction::Div => binop(&mut frame.stack, div_values)?,
                Instruction::Mod => binop(&mut frame.stack, mod_values)?,
                Instruction::Eq => binop(&mut frame.stack, |a, b| Ok(Value::Bool(a == b)))?,
                Instruction::Ne => binop(&mut frame.stack, |a, b| Ok(Value::Bool(a != b)))?,
                Instruction::Lt => binop(&mut frame.stack, cmp_lt)?,
                Instruction::Le => binop(&mut frame.stack, cmp_le)?,
                Instruction::Gt => binop(&mut frame.stack, cmp_gt)?,
                Instruction::Ge => binop(&mut frame.stack, cmp_ge)?,
                Instruction::Neg => unary(&mut frame.stack, neg_value)?,
                Instruction::MakeArray(len) => {
                    let mut items = Vec::with_capacity(len);
                    for _ in 0..len {
                        items.push(
                            frame
                                .stack
                                .pop()
                                .ok_or_else(|| anyhow!("stack underflow on array"))?,
                        );
                    }
                    items.reverse();
                    frame.stack.push(Value::Array(items));
                }
                Instruction::Index => {
                    let index = frame
                        .stack
                        .pop()
                        .ok_or_else(|| anyhow!("stack underflow on index"))?;
                    let target = frame
                        .stack
                        .pop()
                        .ok_or_else(|| anyhow!("stack underflow on index"))?;
                    frame.stack.push(index_value(target, index)?);
                }
                Instruction::Call(argc) => {
                    let mut args = pop_args(&mut frame.stack, argc)?;
                    let callee = frame.stack.pop().ok_or_else(|| anyhow!("missing callee"))?;
                    let symbol = function_from_value(callee)?;
                    frame
                        .stack
                        .push(self.invoke_symbol(symbol, std::mem::take(&mut args))?);
                }
                Instruction::Spawn(argc) => {
                    let args = pop_args(&mut frame.stack, argc)?;
                    let callee = frame.stack.pop().ok_or_else(|| anyhow!("missing callee"))?;
                    let symbol_index = function_from_value(callee)?;
                    let id = self
                        .runtime
                        .tasks
                        .spawn(self.module.symbols[symbol_index].clone());
                    self.pending_tasks
                        .insert(id, PendingTask { symbol_index, args });
                    frame.stack.push(Value::Task(id));
                }
                Instruction::Await => {
                    let task_value = frame
                        .stack
                        .pop()
                        .ok_or_else(|| anyhow!("stack underflow on await"))?;
                    let task = match task_value {
                        Value::Task(task) => task,
                        other => {
                            return Err(anyhow!("await expected task, found {}", other.type_name()))
                        }
                    };
                    frame.stack.push(self.await_task(task)?);
                }
                Instruction::Pop => {
                    let _ = frame.stack.pop();
                }
                Instruction::Jump(target) => {
                    frame.ip = target;
                }
                Instruction::JumpIfFalse(target) => {
                    let condition = frame
                        .stack
                        .pop()
                        .ok_or_else(|| anyhow!("stack underflow on branch"))?;
                    if !condition.truthy() {
                        frame.ip = target;
                    }
                }
                Instruction::Return => {
                    return Ok(frame.stack.pop().unwrap_or(Value::Unit));
                }
            }
        }
    }

    fn await_task(&mut self, task: TaskId) -> Result<Value> {
        if let Some(record) = self.runtime.tasks.get(task) {
            if let snsx_runtime::TaskState::Completed(value) = &record.state {
                return Ok(value.clone());
            }
        }
        let pending = self
            .pending_tasks
            .remove(&task)
            .ok_or_else(|| anyhow!("unknown task {}", task.0))?;
        self.runtime.tasks.start(task);
        let result = self.invoke_symbol(pending.symbol_index, pending.args)?;
        self.runtime.tasks.complete(task, result.clone());
        Ok(result)
    }

    fn call_builtin(&mut self, name: &str, args: Vec<Value>) -> Result<Value> {
        match name {
            "print" | "show" => {
                if let Some(value) = args.first() {
                    self.runtime.print_line(value);
                }
                Ok(Value::Unit)
            }
            "read_line" | "input" | "ask" => match self.runtime.try_read_line() {
                Some(line) => Ok(Value::String(line)),
                None => Err(VmTrap::InputRequested(InputRequest {
                    stdout: self.runtime.stdout.clone(),
                    trace: self.trace.clone(),
                    consumed_lines: self.runtime.consumed_stdin_lines,
                })
                .into()),
            },
            "read_stdin" => Ok(Value::String(self.runtime.read_all())),
            "read_text" => {
                self.runtime.sandbox.check_fs()?;
                let Some(Value::String(path)) = args.first() else {
                    bail!("read_text expects path string")
                };
                let text = match fs::read_to_string(path) {
                    Ok(text) => text,
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
                    Err(error) => return Err(error.into()),
                };
                Ok(Value::String(text))
            }
            "write_text" => {
                self.runtime.sandbox.check_fs()?;
                let Some(Value::String(path)) = args.first() else {
                    bail!("write_text expects path string")
                };
                let Some(Value::String(text)) = args.get(1) else {
                    bail!("write_text expects text string")
                };
                if let Some(parent) = std::path::Path::new(path).parent() {
                    if !parent.as_os_str().is_empty() {
                        fs::create_dir_all(parent)?;
                    }
                }
                fs::write(path, text)?;
                Ok(Value::String(path.clone()))
            }
            "append_text" => {
                self.runtime.sandbox.check_fs()?;
                let Some(Value::String(path)) = args.first() else {
                    bail!("append_text expects path string")
                };
                let Some(Value::String(text)) = args.get(1) else {
                    bail!("append_text expects text string")
                };
                if let Some(parent) = std::path::Path::new(path).parent() {
                    if !parent.as_os_str().is_empty() {
                        fs::create_dir_all(parent)?;
                    }
                }
                let mut file = fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(path)?;
                file.write_all(text.as_bytes())?;
                Ok(Value::String(path.clone()))
            }
            "len" => match args.first() {
                Some(Value::Array(items)) => Ok(Value::Int(items.len() as i64)),
                Some(Value::String(text)) => Ok(Value::Int(text.len() as i64)),
                Some(other) => Err(anyhow!("len unsupported for {}", other.type_name())),
                None => Err(anyhow!("len expects one argument")),
            },
            "contains" => match (args.first(), args.get(1)) {
                (Some(Value::String(text)), Some(Value::String(needle))) => {
                    Ok(Value::Bool(text.contains(needle)))
                }
                (Some(other), _) => Err(anyhow!(
                    "contains expects String, found {}",
                    other.type_name()
                )),
                _ => Err(anyhow!("contains expects two string arguments")),
            },
            "int" => match args.first() {
                Some(Value::String(text)) => {
                    let trimmed = text.trim();
                    if trimmed.is_empty() {
                        bail!("int expects a non-empty numeric string");
                    }
                    let value = trimmed
                        .parse::<i64>()
                        .map_err(|_| anyhow!("int could not parse '{}'", trimmed))?;
                    Ok(Value::Int(value))
                }
                Some(other) => Err(anyhow!("int expects String, found {}", other.type_name())),
                None => Err(anyhow!("int expects one argument")),
            },
            "is_int" => match args.first() {
                Some(Value::String(text)) => {
                    let trimmed = text.trim();
                    if trimmed.is_empty() {
                        Ok(Value::Bool(false))
                    } else {
                        Ok(Value::Bool(trimmed.parse::<i64>().is_ok()))
                    }
                }
                Some(other) => Err(anyhow!(
                    "is_int expects String, found {}",
                    other.type_name()
                )),
                None => Err(anyhow!("is_int expects one argument")),
            },
            "is_digits" => match args.first() {
                Some(Value::String(text)) => {
                    let trimmed = text.trim();
                    Ok(Value::Bool(
                        !trimmed.is_empty() && trimmed.chars().all(|ch| ch.is_ascii_digit()),
                    ))
                }
                Some(other) => Err(anyhow!(
                    "is_digits expects String, found {}",
                    other.type_name()
                )),
                None => Err(anyhow!("is_digits expects one argument")),
            },
            "is_email" => match args.first() {
                Some(Value::String(text)) => {
                    let trimmed = text.trim();
                    Ok(Value::Bool(
                        !trimmed.is_empty()
                            && trimmed.contains('@')
                            && trimmed.contains('.')
                            && !trimmed.starts_with('@')
                            && !trimmed.ends_with('@')
                            && !trimmed.starts_with('.')
                            && !trimmed.ends_with('.'),
                    ))
                }
                Some(other) => Err(anyhow!(
                    "is_email expects String, found {}",
                    other.type_name()
                )),
                None => Err(anyhow!("is_email expects one argument")),
            },
            "float" => match args.first() {
                Some(Value::String(text)) => {
                    let trimmed = text.trim();
                    if trimmed.is_empty() {
                        bail!("float expects a non-empty numeric string");
                    }
                    let value = trimmed
                        .parse::<f64>()
                        .map_err(|_| anyhow!("float could not parse '{}'", trimmed))?;
                    Ok(Value::Float(value))
                }
                Some(other) => Err(anyhow!("float expects String, found {}", other.type_name())),
                None => Err(anyhow!("float expects one argument")),
            },
            "sanitize" => match args.first() {
                Some(value) => Ok(value.clone()),
                None => Err(anyhow!("sanitize expects one argument")),
            },
            "tensor" => {
                let data = match args.first() {
                    Some(Value::Array(items)) => items.clone(),
                    _ => bail!("tensor expects data array"),
                };
                let shape = match args.get(1) {
                    Some(Value::Array(items)) => items
                        .iter()
                        .map(|value| match value {
                            Value::Int(v) => Ok(*v as usize),
                            _ => Err(anyhow!("tensor shape values must be Int")),
                        })
                        .collect::<Result<Vec<_>>>()?,
                    _ => bail!("tensor expects shape array"),
                };
                tensor_from_values(&data, &shape)
            }
            "sha3" => {
                let Some(Value::String(text)) = args.first() else {
                    bail!("sha3 expects a string")
                };
                let mut hasher = Sha3_256::new();
                hasher.update(text.as_bytes());
                let digest = hasher.finalize();
                Ok(Value::String(hex(&digest)))
            }
            "seal" => {
                let Some(Value::String(text)) = args.first() else {
                    bail!("seal expects a string")
                };
                let mut hasher = Sha3_256::new();
                hasher.update(text.as_bytes());
                let digest = hasher.finalize();
                Ok(Value::String(hex(&digest)))
            }
            "shape" => {
                let Some(value) = args.first() else {
                    bail!("shape expects a value")
                };
                Ok(Value::String(value_shape(value)))
            }
            "watch" => {
                let Some(Value::String(label)) = args.first() else {
                    bail!("watch expects a string label")
                };
                let Some(value) = args.get(1) else {
                    bail!("watch expects a value")
                };
                self.runtime
                    .print_line(&Value::String(format!("[watch] {} = {}", label, value)));
                Ok(value.clone())
            }
            "guard" => {
                let Some(condition) = args.first() else {
                    bail!("guard expects a condition")
                };
                let message = args
                    .get(1)
                    .map(ToString::to_string)
                    .unwrap_or_else(|| "guard failed".to_string());
                if condition.truthy() {
                    Ok(Value::Unit)
                } else {
                    Err(anyhow!("guard blocked execution: {}", message))
                }
            }
            "fail" => {
                let message = args
                    .first()
                    .map(ToString::to_string)
                    .unwrap_or_else(|| "execution failed".to_string());
                Err(anyhow!("{}", message))
            }
            "repair" => {
                let Some(primary) = args.first() else {
                    bail!("repair expects a primary value")
                };
                let Some(fallback) = args.get(1) else {
                    bail!("repair expects a fallback value")
                };
                if value_is_blank(primary) {
                    Ok(fallback.clone())
                } else {
                    Ok(primary.clone())
                }
            }
            "blend" => {
                let Some(Value::Array(items)) = args.first() else {
                    bail!("blend expects an array")
                };
                let separator = match args.get(1) {
                    Some(Value::String(text)) => text.as_str(),
                    None => "",
                    Some(other) => bail!(
                        "blend separator must be String, found {}",
                        other.type_name()
                    ),
                };
                Ok(Value::String(
                    items
                        .iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join(separator),
                ))
            }
            "draft" => {
                let Some(Value::String(title)) = args.first() else {
                    bail!("draft expects a title string")
                };
                let Some(Value::String(body)) = args.get(1) else {
                    bail!("draft expects a body string")
                };
                Ok(Value::String(format!(
                    "<draft title=\"{}\">\n{}\n</draft>",
                    title, body
                )))
            }
            "panel" => {
                let Some(Value::String(title)) = args.first() else {
                    bail!("panel expects a title string")
                };
                let Some(Value::String(body)) = args.get(1) else {
                    bail!("panel expects a body string")
                };
                Ok(Value::String(format!("[{}]\n{}", title, body)))
            }
            "stack" => {
                let Some(Value::Array(items)) = args.first() else {
                    bail!("stack expects an array")
                };
                Ok(Value::String(
                    items
                        .iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join("\n"),
                ))
            }
            "field" => {
                let Some(Value::String(label)) = args.first() else {
                    bail!("field expects a label string")
                };
                let Some(value) = args.get(1) else {
                    bail!("field expects a value")
                };
                Ok(Value::String(format!("{}: {}", label, value)))
            }
            "matmul" => {
                let (left, right) = match (args.first(), args.get(1)) {
                    (Some(Value::Tensor(left)), Some(Value::Tensor(right))) => (left, right),
                    _ => bail!("matmul expects two tensors"),
                };
                let output = self.models.cpu.matmul(left, right)?;
                Ok(Value::Tensor(output))
            }
            "relu" => {
                let Some(Value::Tensor(tensor)) = args.first() else {
                    bail!("relu expects a tensor")
                };
                Ok(Value::Tensor(tensor.relu()))
            }
            "sleep_ticks" => {
                let ticks = match args.first() {
                    Some(Value::Int(v)) => *v,
                    _ => 1,
                };
                for _ in 0..ticks {
                    self.runtime.clock.tick();
                }
                Ok(Value::Unit)
            }
            "pulse" => {
                let ticks = match args.first() {
                    Some(Value::Int(v)) => *v,
                    _ => 1,
                };
                for _ in 0..ticks {
                    self.runtime.clock.tick();
                }
                Ok(Value::Unit)
            }
            other => Err(anyhow!("unknown builtin '{}'", other)),
        }
    }
}

fn constant_to_value(constant: snsx_compiler::ir::Constant) -> Value {
    match constant {
        snsx_compiler::ir::Constant::Int(v) => Value::Int(v),
        snsx_compiler::ir::Constant::Float(v) => Value::Float(v),
        snsx_compiler::ir::Constant::Bool(v) => Value::Bool(v),
        snsx_compiler::ir::Constant::String(v) => Value::String(v),
    }
}

fn pop_args(stack: &mut Vec<Value>, argc: usize) -> Result<Vec<Value>> {
    let mut args = Vec::with_capacity(argc);
    for _ in 0..argc {
        args.push(
            stack
                .pop()
                .ok_or_else(|| anyhow!("stack underflow on call"))?,
        );
    }
    args.reverse();
    Ok(args)
}

fn value_shape(value: &Value) -> String {
    match value {
        Value::Int(_) => "Int".to_string(),
        Value::Float(_) => "Float".to_string(),
        Value::Bool(_) => "Bool".to_string(),
        Value::String(text) => format!("String({} bytes)", text.len()),
        Value::Bytes(bytes) => format!("Bytes({} bytes)", bytes.len()),
        Value::Array(items) => format!("Array(len={})", items.len()),
        Value::Tensor(tensor) => format!("Tensor{:?}", tensor.shape),
        Value::Function(index) => format!("Function({})", index),
        Value::Task(id) => format!("Task({})", id.0),
        Value::Unit => "Unit".to_string(),
    }
}

fn value_is_blank(value: &Value) -> bool {
    match value {
        Value::Unit => true,
        Value::String(text) => text.trim().is_empty(),
        Value::Bytes(bytes) => bytes.is_empty(),
        Value::Array(items) => items.is_empty(),
        _ => false,
    }
}

fn function_from_value(value: Value) -> Result<usize> {
    match value {
        Value::Function(index) => Ok(index),
        other => Err(anyhow!(
            "expected function value, found {}",
            other.type_name()
        )),
    }
}

fn unary(stack: &mut Vec<Value>, op: fn(Value) -> Result<Value>) -> Result<()> {
    let value = stack.pop().ok_or_else(|| anyhow!("stack underflow"))?;
    stack.push(op(value)?);
    Ok(())
}

fn binop(stack: &mut Vec<Value>, op: fn(Value, Value) -> Result<Value>) -> Result<()> {
    let rhs = stack.pop().ok_or_else(|| anyhow!("stack underflow"))?;
    let lhs = stack.pop().ok_or_else(|| anyhow!("stack underflow"))?;
    stack.push(op(lhs, rhs)?);
    Ok(())
}

fn add_values(lhs: Value, rhs: Value) -> Result<Value> {
    match (lhs, rhs) {
        (Value::Int(a), Value::Int(b)) => Ok(Value::Int(a + b)),
        (Value::Float(a), Value::Float(b)) => Ok(Value::Float(a + b)),
        (Value::String(a), Value::String(b)) => Ok(Value::String(format!("{a}{b}"))),
        (Value::Array(mut a), Value::Array(b)) => {
            a.extend(b);
            Ok(Value::Array(a))
        }
        (Value::Tensor(a), Value::Tensor(b)) => Ok(Value::Tensor(a.add(&b))),
        (a, b) => Err(anyhow!(
            "cannot add {} and {}",
            a.type_name(),
            b.type_name()
        )),
    }
}

fn sub_values(lhs: Value, rhs: Value) -> Result<Value> {
    match (lhs, rhs) {
        (Value::Int(a), Value::Int(b)) => Ok(Value::Int(a - b)),
        (Value::Float(a), Value::Float(b)) => Ok(Value::Float(a - b)),
        (a, b) => Err(anyhow!(
            "cannot subtract {} and {}",
            a.type_name(),
            b.type_name()
        )),
    }
}

fn mul_values(lhs: Value, rhs: Value) -> Result<Value> {
    match (lhs, rhs) {
        (Value::Int(a), Value::Int(b)) => Ok(Value::Int(a * b)),
        (Value::Float(a), Value::Float(b)) => Ok(Value::Float(a * b)),
        (a, b) => Err(anyhow!(
            "cannot multiply {} and {}",
            a.type_name(),
            b.type_name()
        )),
    }
}

fn div_values(lhs: Value, rhs: Value) -> Result<Value> {
    match (lhs, rhs) {
        (Value::Int(a), Value::Int(b)) => Ok(Value::Int(a / b)),
        (Value::Float(a), Value::Float(b)) => Ok(Value::Float(a / b)),
        (a, b) => Err(anyhow!(
            "cannot divide {} and {}",
            a.type_name(),
            b.type_name()
        )),
    }
}

fn mod_values(lhs: Value, rhs: Value) -> Result<Value> {
    match (lhs, rhs) {
        (Value::Int(a), Value::Int(b)) => Ok(Value::Int(a % b)),
        (a, b) => Err(anyhow!(
            "cannot mod {} and {}",
            a.type_name(),
            b.type_name()
        )),
    }
}

fn cmp_lt(lhs: Value, rhs: Value) -> Result<Value> {
    match (lhs, rhs) {
        (Value::Int(a), Value::Int(b)) => Ok(Value::Bool(a < b)),
        (Value::Float(a), Value::Float(b)) => Ok(Value::Bool(a < b)),
        (a, b) => Err(anyhow!(
            "cannot compare {} and {}",
            a.type_name(),
            b.type_name()
        )),
    }
}

fn cmp_le(lhs: Value, rhs: Value) -> Result<Value> {
    match (lhs, rhs) {
        (Value::Int(a), Value::Int(b)) => Ok(Value::Bool(a <= b)),
        (Value::Float(a), Value::Float(b)) => Ok(Value::Bool(a <= b)),
        (a, b) => Err(anyhow!(
            "cannot compare {} and {}",
            a.type_name(),
            b.type_name()
        )),
    }
}

fn cmp_gt(lhs: Value, rhs: Value) -> Result<Value> {
    match (lhs, rhs) {
        (Value::Int(a), Value::Int(b)) => Ok(Value::Bool(a > b)),
        (Value::Float(a), Value::Float(b)) => Ok(Value::Bool(a > b)),
        (a, b) => Err(anyhow!(
            "cannot compare {} and {}",
            a.type_name(),
            b.type_name()
        )),
    }
}

fn cmp_ge(lhs: Value, rhs: Value) -> Result<Value> {
    match (lhs, rhs) {
        (Value::Int(a), Value::Int(b)) => Ok(Value::Bool(a >= b)),
        (Value::Float(a), Value::Float(b)) => Ok(Value::Bool(a >= b)),
        (a, b) => Err(anyhow!(
            "cannot compare {} and {}",
            a.type_name(),
            b.type_name()
        )),
    }
}

fn neg_value(value: Value) -> Result<Value> {
    match value {
        Value::Int(v) => Ok(Value::Int(-v)),
        Value::Float(v) => Ok(Value::Float(-v)),
        other => Err(anyhow!("cannot negate {}", other.type_name())),
    }
}

fn index_value(target: Value, index: Value) -> Result<Value> {
    match (target, index) {
        (Value::Array(items), Value::Int(index)) => items
            .get(index as usize)
            .cloned()
            .ok_or_else(|| anyhow!("array index out of bounds")),
        (Value::String(text), Value::Int(index)) => text
            .chars()
            .nth(index as usize)
            .map(|ch| Value::String(ch.to_string()))
            .ok_or_else(|| anyhow!("string index out of bounds")),
        (Value::Tensor(tensor), Value::Int(index)) => tensor
            .data
            .get(index as usize)
            .copied()
            .map(Value::Float)
            .ok_or_else(|| anyhow!("tensor index out of bounds")),
        (value, _) => Err(anyhow!("cannot index {}", value.type_name())),
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect::<String>()
}

#[cfg(test)]
mod tests {
    use super::*;
    use snsx_compiler::{compile_vm_with_policy, security::SecurityPolicy};
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn compile_sample(source: &str) -> BytecodeModule {
        compile_vm_with_policy("test.snsx", source, &SecurityPolicy::permissive()).unwrap()
    }

    #[test]
    fn requests_more_input_when_program_needs_second_line() {
        let source = r#"bring <module/std.io> as io

entry
    show "First"
    hold left = ask()
    show "Second"
    hold right = ask()
    show left
    show right
    0
"#;
        let module = compile_sample(source);
        let mut vm = VirtualMachine::new(
            module,
            VmOptions {
                stdin: "7".to_string(),
                allow_host_stdin: false,
                ..VmOptions::default()
            },
        );
        let error = vm.execute_main(Vec::new()).unwrap_err();
        let request = error
            .downcast_ref::<VmTrap>()
            .and_then(|trap| match trap {
                VmTrap::InputRequested(request) => Some(request),
            })
            .expect("expected input request");
        assert_eq!(request.consumed_lines, 1);
        assert!(request.stdout.contains("First"));
        assert!(request.stdout.contains("Second"));
    }

    #[test]
    fn completes_program_when_all_input_lines_are_available() {
        let source = r#"bring <module/std.io> as io

entry
    hold left = ask()
    hold right = ask()
    show int(left) + int(right)
    0
"#;
        let module = compile_sample(source);
        let mut vm = VirtualMachine::new(
            module,
            VmOptions {
                stdin: "7\n3".to_string(),
                allow_host_stdin: false,
                ..VmOptions::default()
            },
        );
        let result = vm.execute_main(Vec::new()).unwrap();
        assert!(result.stdout.contains("10"));
    }

    #[test]
    fn preserves_distinct_branch_locals_with_reused_names() {
        let source = r#"bring <module/std.io> as io

entry
    hold action = ask()

    gate action == "login":
        hold email = ask()
        hold password = ask()
        show email
        show password
    otherwise:
        hold email = ask()
        show email

    0
"#;
        let module = compile_sample(source);
        let mut vm = VirtualMachine::new(
            module,
            VmOptions {
                stdin: "login\nsatya@example.com\nsecret12".to_string(),
                allow_host_stdin: false,
                ..VmOptions::default()
            },
        );
        let result = vm.execute_main(Vec::new()).unwrap();
        assert!(result.stdout.contains("satya@example.com"));
        assert!(result.stdout.contains("secret12"));
    }

    #[test]
    fn supports_local_storage_builtins_with_fs_permission() {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("snsx_store_{stamp}.txt"));
        let path_text = path.display().to_string().replace('\\', "\\\\");
        let source = format!(
            "entry\n    hold target = write_text(\"{path_text}\", \"name=Satya\")\n    hold saved = append_text(target, \"\\nemail=satya@example.com\")\n    show saved\n    0\n"
        );
        let module = compile_sample(&source);
        let mut vm = VirtualMachine::new(
            module,
            VmOptions {
                sandbox: SandboxPolicy {
                    allow_fs: true,
                    ..SandboxPolicy::default()
                },
                ..VmOptions::default()
            },
        );
        let result = vm.execute_main(Vec::new()).unwrap();
        assert!(result.stdout.contains(&path.display().to_string()));
        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("name=Satya"));
        assert!(content.contains("email=satya@example.com"));
        let _ = fs::remove_file(&path);
    }
}
