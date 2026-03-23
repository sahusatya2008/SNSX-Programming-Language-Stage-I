#![no_std]

pub const ABI_MAJOR: u16 = 0;
pub const ABI_MINOR: u16 = 1;

#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CapabilityId(pub u32);

#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Handle(pub u32);

#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RegionId(pub u32);

#[repr(u16)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CapabilityKind {
    Console = 1,
    FileSystem = 2,
    Network = 3,
    Clock = 4,
    Memory = 5,
    Scheduler = 6,
    Device = 7,
    Crypto = 8,
    Audit = 9,
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PagePolicy {
    ReadOnly = 1,
    ReadWrite = 2,
    ReadExecute = 3,
    Device = 4,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PageFlags {
    bits: u32,
}

impl PageFlags {
    pub const READ: u32 = 1 << 0;
    pub const WRITE: u32 = 1 << 1;
    pub const EXECUTE: u32 = 1 << 2;
    pub const USER: u32 = 1 << 3;
    pub const DEVICE: u32 = 1 << 4;
    pub const GUARDED: u32 = 1 << 5;

    pub const fn new(bits: u32) -> Self {
        Self { bits }
    }

    pub const fn bits(self) -> u32 {
        self.bits
    }

    pub const fn contains(self, other: u32) -> bool {
        (self.bits & other) == other
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MemoryRegion {
    pub id: RegionId,
    pub base: u64,
    pub length: u64,
    pub flags: PageFlags,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CapabilityToken {
    pub id: CapabilityId,
    pub kind: CapabilityKind,
    pub rights: u64,
    pub nonce: u64,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ModuleDigest {
    pub words: [u64; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AuditLabel {
    pub subsystem: u16,
    pub rule: u16,
    pub severity: u8,
    pub reserved: u8,
    pub aux: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MessageHeader {
    pub kind: u32,
    pub flags: u32,
    pub sender: Handle,
    pub capability: CapabilityId,
    pub bytes: u32,
    pub words: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Message {
    pub header: MessageHeader,
    pub payload_words: [u64; 6],
}

impl Message {
    pub const fn empty(kind: u32, sender: Handle) -> Self {
        Self {
            header: MessageHeader {
                kind,
                flags: 0,
                sender,
                capability: CapabilityId(0),
                bytes: 0,
                words: 0,
            },
            payload_words: [0; 6],
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SchedulerHint {
    pub priority: u8,
    pub quantum_ticks: u16,
    pub deterministic: u8,
}

#[repr(u16)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SysOp {
    CapClone = 1,
    CapDrop = 2,
    RegionMap = 10,
    RegionUnmap = 11,
    ChannelSend = 20,
    ChannelRecv = 21,
    TaskSpawn = 30,
    TaskYield = 31,
    ClockRead = 40,
    AuditEmit = 50,
}

#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Ok = 0,
    Denied = -1,
    Invalid = -2,
    Exhausted = -3,
    Fault = -4,
    Busy = -5,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SysCall {
    pub op: SysOp,
    pub arg0: u64,
    pub arg1: u64,
    pub arg2: u64,
    pub arg3: u64,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SysResult {
    pub status: Status,
    pub value0: u64,
    pub value1: u64,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BootInfo {
    pub abi_major: u16,
    pub abi_minor: u16,
    pub cpu_count: u16,
    pub region_count: u16,
    pub memory_map_ptr: u64,
    pub init_cap_ptr: u64,
    pub init_module_digest: ModuleDigest,
}

impl BootInfo {
    pub const fn abi_ok(&self) -> bool {
        self.abi_major == ABI_MAJOR && self.abi_minor >= ABI_MINOR
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TrapFrame {
    pub pc: u64,
    pub sp: u64,
    pub status: u64,
    pub regs: [u64; 16],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TaskDescriptor {
    pub handle: Handle,
    pub entry_pc: u64,
    pub stack_top: u64,
    pub root_cap: CapabilityId,
    pub hint: SchedulerHint,
}

pub const fn syscall(op: SysOp, arg0: u64, arg1: u64, arg2: u64, arg3: u64) -> SysCall {
    SysCall {
        op,
        arg0,
        arg1,
        arg2,
        arg3,
    }
}

pub const fn console_cap() -> CapabilityKind {
    CapabilityKind::Console
}

pub const fn deterministic_hint(priority: u8, quantum_ticks: u16) -> SchedulerHint {
    SchedulerHint {
        priority,
        quantum_ticks,
        deterministic: 1,
    }
}
