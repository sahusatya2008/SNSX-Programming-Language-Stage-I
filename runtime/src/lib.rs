use serde::{Deserialize, Serialize};
use sha3::{Digest, Sha3_256};
use std::collections::{HashMap, VecDeque};
use std::fmt;
use std::io;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct HandleId(pub usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TaskId(pub usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ActorId(pub usize);

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Tensor {
    pub shape: Vec<usize>,
    pub data: Vec<f64>,
}

impl Tensor {
    pub fn new(shape: Vec<usize>, data: Vec<f64>) -> Self {
        Self { shape, data }
    }

    pub fn zeros(shape: Vec<usize>) -> Self {
        let len = shape.iter().product();
        Self {
            shape,
            data: vec![0.0; len],
        }
    }

    pub fn add(&self, rhs: &Tensor) -> Tensor {
        let data = self
            .data
            .iter()
            .zip(&rhs.data)
            .map(|(a, b)| a + b)
            .collect();
        Tensor {
            shape: self.shape.clone(),
            data,
        }
    }

    pub fn relu(&self) -> Tensor {
        Tensor {
            shape: self.shape.clone(),
            data: self.data.iter().map(|v| v.max(0.0)).collect(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Value {
    Int(i64),
    Float(f64),
    Bool(bool),
    String(String),
    Bytes(Vec<u8>),
    Array(Vec<Value>),
    Tensor(Tensor),
    Function(usize),
    Task(TaskId),
    Unit,
}

impl Value {
    pub fn truthy(&self) -> bool {
        match self {
            Value::Bool(v) => *v,
            Value::Int(v) => *v != 0,
            Value::Float(v) => *v != 0.0,
            Value::String(v) => !v.is_empty(),
            Value::Bytes(v) => !v.is_empty(),
            Value::Array(v) => !v.is_empty(),
            Value::Tensor(v) => !v.data.is_empty(),
            Value::Function(_) => true,
            Value::Task(_) => true,
            Value::Unit => false,
        }
    }

    pub fn type_name(&self) -> &'static str {
        match self {
            Value::Int(_) => "Int",
            Value::Float(_) => "Float",
            Value::Bool(_) => "Bool",
            Value::String(_) => "String",
            Value::Bytes(_) => "Bytes",
            Value::Array(_) => "Array",
            Value::Tensor(_) => "Tensor",
            Value::Function(_) => "Function",
            Value::Task(_) => "Task",
            Value::Unit => "Unit",
        }
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Int(v) => write!(f, "{v}"),
            Value::Float(v) => write!(f, "{v}"),
            Value::Bool(v) => write!(f, "{v}"),
            Value::String(v) => write!(f, "{v}"),
            Value::Bytes(v) => write!(f, "0x{}", hex(v)),
            Value::Array(v) => {
                write!(f, "[")?;
                for (index, item) in v.iter().enumerate() {
                    if index > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{item}")?;
                }
                write!(f, "]")
            }
            Value::Tensor(tensor) => write!(f, "Tensor{:?}", tensor.shape),
            Value::Function(index) => write!(f, "<fn:{index}>"),
            Value::Task(id) => write!(f, "<task:{}>", id.0),
            Value::Unit => write!(f, "()"),
        }
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect::<String>()
}

#[derive(Debug, Clone)]
pub struct HeapCell {
    pub refs: usize,
    pub value: Value,
}

#[derive(Debug, Default)]
pub struct HybridHeap {
    next: usize,
    pub cells: HashMap<HandleId, HeapCell>,
}

impl HybridHeap {
    pub fn alloc(&mut self, value: Value) -> HandleId {
        let id = HandleId(self.next);
        self.next += 1;
        self.cells.insert(id, HeapCell { refs: 1, value });
        id
    }

    pub fn retain(&mut self, id: HandleId) {
        if let Some(cell) = self.cells.get_mut(&id) {
            cell.refs += 1;
        }
    }

    pub fn release(&mut self, id: HandleId) {
        if let Some(cell) = self.cells.get_mut(&id) {
            if cell.refs > 1 {
                cell.refs -= 1;
                return;
            }
        }
        self.cells.remove(&id);
    }

    pub fn stats(&self) -> HeapStats {
        HeapStats {
            objects: self.cells.len(),
            bytes_estimate: self
                .cells
                .values()
                .map(|cell| match &cell.value {
                    Value::String(s) => s.len(),
                    Value::Bytes(b) => b.len(),
                    Value::Array(a) => a.len() * 16,
                    Value::Tensor(t) => t.data.len() * 8,
                    _ => 16,
                })
                .sum(),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct HeapStats {
    pub objects: usize,
    pub bytes_estimate: usize,
}

#[derive(Debug, Clone)]
pub enum TaskState<T> {
    Ready,
    Running,
    Completed(T),
    Failed(String),
}

#[derive(Debug, Clone)]
pub struct TaskRecord<T> {
    pub label: String,
    pub state: TaskState<T>,
}

#[derive(Debug, Clone)]
pub struct TaskScheduler<T> {
    next: usize,
    queue: VecDeque<TaskId>,
    tasks: HashMap<TaskId, TaskRecord<T>>,
    deterministic: bool,
}

impl<T: Clone> Default for TaskScheduler<T> {
    fn default() -> Self {
        Self::new(false)
    }
}

impl<T: Clone> TaskScheduler<T> {
    pub fn new(deterministic: bool) -> Self {
        Self {
            next: 0,
            queue: VecDeque::new(),
            tasks: HashMap::new(),
            deterministic,
        }
    }

    pub fn spawn(&mut self, label: impl Into<String>) -> TaskId {
        let id = TaskId(self.next);
        self.next += 1;
        self.tasks.insert(
            id,
            TaskRecord {
                label: label.into(),
                state: TaskState::Ready,
            },
        );
        self.queue.push_back(id);
        id
    }

    pub fn start(&mut self, id: TaskId) {
        if let Some(record) = self.tasks.get_mut(&id) {
            record.state = TaskState::Running;
        }
    }

    pub fn complete(&mut self, id: TaskId, value: T) {
        if let Some(record) = self.tasks.get_mut(&id) {
            record.state = TaskState::Completed(value);
        }
    }

    pub fn fail(&mut self, id: TaskId, message: impl Into<String>) {
        if let Some(record) = self.tasks.get_mut(&id) {
            record.state = TaskState::Failed(message.into());
        }
    }

    pub fn take_next(&mut self) -> Option<TaskId> {
        if self.deterministic {
            let mut ids = self.queue.drain(..).collect::<Vec<_>>();
            ids.sort_by_key(|id| id.0);
            self.queue.extend(ids);
        }
        self.queue.pop_front()
    }

    pub fn get(&self, id: TaskId) -> Option<&TaskRecord<T>> {
        self.tasks.get(&id)
    }
}

#[derive(Debug, Clone)]
pub struct Mailbox<M> {
    queue: VecDeque<M>,
}

impl<M> Default for Mailbox<M> {
    fn default() -> Self {
        Self {
            queue: VecDeque::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ActorSystem<M> {
    next: usize,
    mailboxes: HashMap<ActorId, Mailbox<M>>,
}

impl<M> Default for ActorSystem<M> {
    fn default() -> Self {
        Self {
            next: 0,
            mailboxes: HashMap::new(),
        }
    }
}

impl<M> ActorSystem<M> {
    pub fn create_actor(&mut self) -> ActorId {
        let id = ActorId(self.next);
        self.next += 1;
        self.mailboxes.insert(id, Mailbox::default());
        id
    }

    pub fn send(&mut self, id: ActorId, message: M) {
        if let Some(mailbox) = self.mailboxes.get_mut(&id) {
            mailbox.queue.push_back(message);
        }
    }

    pub fn recv(&mut self, id: ActorId) -> Option<M> {
        self.mailboxes
            .get_mut(&id)
            .and_then(|mailbox| mailbox.queue.pop_front())
    }
}

#[derive(Debug, Clone)]
pub struct SandboxPolicy {
    pub allow_fs: bool,
    pub allow_network: bool,
    pub allow_ai: bool,
    pub deterministic: bool,
    pub max_memory_bytes: usize,
}

impl Default for SandboxPolicy {
    fn default() -> Self {
        Self {
            allow_fs: false,
            allow_network: false,
            allow_ai: true,
            deterministic: false,
            max_memory_bytes: 64 * 1024 * 1024,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SandboxError {
    #[error("filesystem access denied")]
    FilesystemDenied,
    #[error("network access denied")]
    NetworkDenied,
    #[error("ai execution denied")]
    AiDenied,
    #[error("memory limit exceeded")]
    MemoryExceeded,
}

impl SandboxPolicy {
    pub fn check_fs(&self) -> Result<(), SandboxError> {
        if self.allow_fs {
            Ok(())
        } else {
            Err(SandboxError::FilesystemDenied)
        }
    }

    pub fn check_network(&self) -> Result<(), SandboxError> {
        if self.allow_network {
            Ok(())
        } else {
            Err(SandboxError::NetworkDenied)
        }
    }

    pub fn check_ai(&self) -> Result<(), SandboxError> {
        if self.allow_ai {
            Ok(())
        } else {
            Err(SandboxError::AiDenied)
        }
    }
}

#[derive(Debug, Clone)]
pub struct DeterministicClock {
    pub seed: u64,
    tick: u64,
}

impl DeterministicClock {
    pub fn new(seed: u64) -> Self {
        Self { seed, tick: 0 }
    }

    pub fn tick(&mut self) -> u64 {
        self.tick += 1;
        self.tick
    }

    pub fn digest_bytes(&mut self, bytes: &[u8]) -> [u8; 32] {
        let mut hasher = Sha3_256::new();
        hasher.update(self.seed.to_le_bytes());
        hasher.update(self.tick().to_le_bytes());
        hasher.update(bytes);
        hasher.finalize().into()
    }
}

#[derive(Debug)]
pub struct RuntimeContext {
    pub heap: HybridHeap,
    pub tasks: TaskScheduler<Value>,
    pub actors: ActorSystem<Vec<u8>>,
    pub sandbox: SandboxPolicy,
    pub clock: DeterministicClock,
    pub stdout: String,
    pub stdin: VecDeque<String>,
    pub stdin_snapshot: String,
    pub allow_host_stdin: bool,
    pub consumed_stdin_lines: usize,
}

impl RuntimeContext {
    pub fn new(policy: SandboxPolicy, stdin: impl Into<String>, allow_host_stdin: bool) -> Self {
        let stdin_snapshot = stdin.into();
        let seed = if policy.deterministic { 1 } else { 0x5EED_C0DE };
        Self {
            heap: HybridHeap::default(),
            tasks: TaskScheduler::new(policy.deterministic),
            actors: ActorSystem::default(),
            sandbox: policy,
            clock: DeterministicClock::new(seed),
            stdout: String::new(),
            stdin: stdin_snapshot
                .lines()
                .map(ToString::to_string)
                .collect::<VecDeque<_>>(),
            stdin_snapshot,
            allow_host_stdin,
            consumed_stdin_lines: 0,
        }
    }

    pub fn print_line(&mut self, value: &Value) {
        self.stdout.push_str(&value.to_string());
        self.stdout.push('\n');
    }

    pub fn try_read_line(&mut self) -> Option<String> {
        if let Some(line) = self.stdin.pop_front() {
            self.consumed_stdin_lines += 1;
            return Some(line);
        }
        if self.allow_host_stdin {
            let mut line = String::new();
            if io::stdin().read_line(&mut line).is_ok() {
                self.consumed_stdin_lines += 1;
                return Some(line.trim_end_matches(&['\r', '\n'][..]).to_string());
            }
        }
        None
    }

    pub fn read_line(&mut self) -> String {
        self.try_read_line().unwrap_or_default()
    }

    pub fn read_all(&self) -> String {
        self.stdin_snapshot.clone()
    }
}
