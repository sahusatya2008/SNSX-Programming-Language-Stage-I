<div align="center">

# 🚀 SNSX + AERIS — Next-Generation Programming Language Ecosystem

**Smart Neural Syntax eXtended (SNSX)** & **Advanced Execution & Runtime Isolation System (AERIS)**

![Rust](https://img.shields.io/badge/Language-Rust-DEA584?style=for-the-badge&logo=rust&logoColor=white)
![License](https://img.shields.io/badge/License-Apache--2.0-blue?style=for-the-badge)
![Stage](https://img.shields.io/badge/Stage-Bootstrap%20v0.1.0-green?style=for-the-badge)
![Build](https://img.shields.io/badge/Build-Passing-brightgreen?style=for-the-badge)

---

**Developer:** Satya Narayan Sahu  
**Structural Developer:** Tathoi Mondal

---

</div>

## 📑 Table of Contents

- [1. Introduction](#1-introduction)
- [2. System Overview](#2-system-overview)
- [3. Architecture](#3-architecture)
- [4. Repository Structure](#4-repository-structure)
- [5. SNSX Language Specification](#5-snsx-language-specification)
- [6. AERIS Language Specification](#6-aeris-language-specification)
- [7. Compiler Pipeline](#7-compiler-pipeline)
- [8. Virtual Machine (VM) Engine](#8-virtual-machine-vm-engine)
- [9. Runtime System](#9-runtime-system)
- [10. AI Engine](#10-ai-engine)
- [11. Network Stack (SNSP)](#11-network-stack-snsp)
- [12. Distributed Engine](#12-distributed-engine)
- [13. Standard Library](#13-standard-library)
- [14. Security Architecture](#14-security-architecture)
- [15. IDE & Development Tools](#15-ide--development-tools)
- [16. CLI Reference](#16-cli-reference)
- [17. Package Management](#17-package-management)
- [18. Examples & Tutorials](#18-examples--tutorials)
- [19. Algorithms & Data Structures](#19-algorithms--data-structures)
- [20. Formal Semantics](#20-formal-semantics)
- [21. Build System](#21-build-system)
- [22. Future Roadmap](#22-future-roadmap)
- [23. Contributing](#23-contributing)
- [24. License](#24-license)

---

## 1. Introduction

### 1.1 What is SNSX?

**SNSX** (Smart Neural Syntax eXtended) is a revolutionary programming language designed from the ground up to be:

- **AI-Native**: First-class support for AI prompt functions (`mind` blocks)
- **Security-First**: Strict compile-time security auditing with zero ambient authority
- **Human-Readable**: Intent-driven syntax that reads like structured instructions
- **Multi-Target**: Compile to VM bytecode, LLVM IR, WebAssembly, or native assembly
- **Distributed**: Built-in support for cluster scheduling and fault recovery

### 1.2 What is AERIS?

**AERIS** (Advanced Execution & Runtime Isolation System) is a capability-secure systems language designed around:

- Explicit effects and capabilities
- Algebraic data types
- Deterministic actors
- Compile-time refinement checks
- SSA-oriented compilation

### 1.3 Design Philosophy

```
┌─────────────────────────────────────────────────────────────────┐
│                    SNSX/AERIS Design Pillars                     │
├─────────────────────────────────────────────────────────────────┤
│  🔒 Security      → Zero-trust, explicit capabilities           │
│  🤖 AI-Native     → First-class prompt functions                │
│  📖 Readability   → Intent-driven, ceremony-free syntax         │
│  🎯 Multi-Target  → VM, LLVM, WASM, Native Assembly            │
│  🔄 Concurrency   → Deterministic tasks and actors              │
│  📦 Modularity    → Workspace-aware package management          │
└─────────────────────────────────────────────────────────────────┘
```

---

## 2. System Overview

### 2.1 High-Level System Architecture

```
┌──────────────────────────────────────────────────────────────────────┐
│                         SNSX/AERIS Ecosystem                         │
├──────────────────────────────────────────────────────────────────────┤
│                                                                      │
│  ┌─────────────┐    ┌─────────────┐    ┌─────────────────────────┐  │
│  │   SNSX CLI  │    │  SNSX IDE   │    │   SNSX Web/App Studio   │  │
│  │  (Terminal) │    │ (Terminal)  │    │      (Browser)          │  │
│  └──────┬──────┘    └──────┬──────┘    └───────────┬─────────────┘  │
│         │                  │                       │                 │
│         └──────────────────┼───────────────────────┘                 │
│                            │                                         │
│                     ┌──────▼──────┐                                  │
│                     │   SNSX      │                                  │
│                     │  Compiler   │                                  │
│                     └──────┬──────┘                                  │
│                            │                                         │
│         ┌──────────────────┼──────────────────┐                      │
│         │                  │                  │                      │
│  ┌──────▼──────┐   ┌──────▼──────┐   ┌──────▼──────┐              │
│  │  SNSX-VM    │   │  LLVM IR    │   │  WASM/Native│              │
│  │  Bytecode   │   │   Text      │   │   Assembly  │              │
│  └──────┬──────┘   └─────────────┘   └─────────────┘              │
│         │                                                           │
│  ┌──────▼──────────────────────────────────────────────────────┐   │
│  │                    SNSX Runtime System                       │   │
│  │  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐   │   │
│  │  │  Hybrid   │  │  Task    │  │  Actor   │  │ Sandbox  │   │   │
│  │  │  Heap     │  │ Scheduler│  │  System  │  │ Policy   │   │   │
│  │  └──────────┘  └──────────┘  └──────────┘  └──────────┘   │   │
│  └──────────────────────────────────────────────────────────────┘   │
│                                                                      │
│  ┌──────────────────────────────────────────────────────────────┐   │
│  │                    Supporting Subsystems                      │   │
│  │  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐   │   │
│  │  │AI Engine │  │ Network  │  │Distributed│  │  StdLib  │   │   │
│  │  │          │  │  Stack   │  │  Engine   │  │          │   │   │
│  │  └──────────┘  └──────────┘  └──────────┘  └──────────┘   │   │
│  └──────────────────────────────────────────────────────────────┘   │
└──────────────────────────────────────────────────────────────────────┘
```

### 2.2 Execution Flow Diagram

```
┌──────────────┐
│  .snsx Source │
└──────┬───────┘
       │
       ▼
┌──────────────┐     ┌──────────────┐
│    Lexer     │────▶│   Tokens     │
└──────┬───────┘     └──────────────┘
       │
       ▼
┌──────────────┐     ┌──────────────┐
│    Parser    │────▶│     AST      │
└──────┬───────┘     └──────────────┘
       │
       ▼
┌──────────────┐     ┌──────────────┐
│  Semantic    │────▶│   Typed      │
│  Analysis    │     │   Module     │
└──────┬───────┘     └──────────────┘
       │
       ▼
┌──────────────┐     ┌─────────────────────┐
│  Security    │────▶│  Audit Report       │
│  Audit       │     │  (Block if strict)  │
└──────┬───────┘     └─────────────────────┘
       │
       ▼
┌──────────────┐     ┌──────────────────────────┐
│  IR Lowering │────▶│  SSA-style IR Program    │
└──────┬───────┘     └──────────────────────────┘
       │
       ▼
┌──────────────┐
│  Optimization│
└──────┬───────┘
       │
       ▼
┌──────────────────────────────────────────────────┐
│              Target Emission                      │
│  ┌────────┐ ┌────────┐ ┌────────┐ ┌──────────┐ │
│  │   VM   │ │  LLVM  │ │  WASM  │ │  Native  │ │
│  │Bytecode│ │  IR    │ │  Text  │ │Assembly  │ │
│  └────┬───┘ └────────┘ └────────┘ └──────────┘ │
│       │                                          │
│  ┌────▼──────────────────────────────┐          │
│  │    SNSX Virtual Machine           │          │
│  │    (Primary Execution Backend)    │          │
│  └────┬──────────────────────────────┘          │
│       │                                          │
│  ┌────▼──────────────────────────────┐          │
│  │       Runtime Context             │          │
│  │  • Heap  • Tasks  • Actors       │          │
│  │  • Sandbox  • Clock  • I/O       │          │
│  └───────────────────────────────────┘          │
└──────────────────────────────────────────────────┘
```

---

## 3. Architecture

### 3.1 Component Architecture

The SNSX/AERIS ecosystem is organized into the following major components:

| Component | Purpose | Language | Status |
|-----------|---------|----------|--------|
| `compiler/` | SNSX language compiler | Rust | ✅ Bootstrap v0.1.0 |
| `runtime/` | Runtime context & sandbox | Rust | ✅ Bootstrap v0.1.0 |
| `vm/` | Virtual machine execution engine | Rust | ✅ Bootstrap v0.1.0 |
| `stdlib/` | Standard library modules | SNSX/Rust | ✅ Bootstrap v0.1.0 |
| `ide/` | Terminal & web IDE | Rust | ✅ Bootstrap v0.1.0 |
| `cli/` | Command-line interface | Rust | ✅ Bootstrap v0.1.0 |
| `ai_engine/` | AI model bridge | Rust | ✅ Bootstrap v0.1.0 |
| `network_stack/` | SNSP secure networking | Rust | ✅ Bootstrap v0.1.0 |
| `distributed_engine/` | Cluster orchestration | Rust | ✅ Bootstrap v0.1.0 |
| `aeris_compiler/` | AERIS language compiler | Rust | ✅ Bootstrap Stage-0 |
| `aeris_runtime/` | AERIS runtime interpreter | Rust | ✅ Bootstrap Stage-0 |

### 3.2 Data Flow Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                     Data Flow Through SNSX                       │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  Source (.snsx)                                                  │
│       │                                                          │
│       ▼                                                          │
│  ┌─────────────────────────────────────────┐                    │
│  │  LEXER                                  │                    │
│  │  Input: UTF-8 source string             │                    │
│  │  Output: Vec<Token>                     │                    │
│  │  Algorithm: State-machine tokenization  │                    │
│  │  • Keyword recognition                  │                    │
│  │  • String literal parsing               │                    │
│  │  • Numeric literal parsing              │                    │
│  │  • Indentation tracking                 │                    │
│  └─────────────────────────────────────────┘                    │
│       │                                                          │
│       ▼                                                          │
│  ┌─────────────────────────────────────────┐                    │
│  │  PARSER                                 │                    │
│  │  Input: Vec<Token>                      │                    │
│  │  Output: Module (AST)                   │                    │
│  │  Algorithm: Recursive descent           │                    │
│  │  • Import resolution                    │                    │
│  │  • Entry/flow/mind parsing              │                    │
│  │  • Expression precedence climbing       │                    │
│  │  • Statement block assembly             │                    │
│  └─────────────────────────────────────────┘                    │
│       │                                                          │
│       ▼                                                          │
│  ┌─────────────────────────────────────────┐                    │
│  │  SEMANTIC ANALYZER                      │                    │
│  │  Input: Module (AST)                    │                    │
│  │  Output: SemanticModel                  │                    │
│  │  Algorithm:                             │                    │
│  │  • Symbol table construction            │                    │
│  │  • Type inference & checking            │                    │
│  │  • Builtin recognition                  │                    │
│  │  • Callable validation                  │                    │
│  └─────────────────────────────────────────┘                    │
│       │                                                          │
│       ▼                                                          │
│  ┌─────────────────────────────────────────┐                    │
│  │  SECURITY AUDIT                         │                    │
│  │  Input: Module + SemanticModel          │                    │
│  │  Output: Vec<Diagnostic>                │                    │
│  │  Algorithm:                             │                    │
│  │  • Permission manifest check            │                    │
│  │  • Unsafe pattern detection             │                    │
│  │  • Contract gap analysis                │                    │
│  │  • Loop hazard detection                │                    │
│  └─────────────────────────────────────────┘                    │
│       │                                                          │
│       ▼                                                          │
│  ┌─────────────────────────────────────────┐                    │
│  │  IR LOWERING                            │                    │
│  │  Input: Module + SemanticModel          │                    │
│  │  Output: Program (SSA IR)              │                    │
│  │  Algorithm:                             │                    │
│  │  • SSA construction                     │                    │
│  │  • Control flow graph building          │                    │
│  │  • Constant propagation                 │                    │
│  │  • Dead code elimination                │                    │
│  └─────────────────────────────────────────┘                    │
│       │                                                          │
│       ▼                                                          │
│  ┌─────────────────────────────────────────┐                    │
│  │  CODE GENERATION                        │                    │
│  │  Input: Program (SSA IR)               │                    │
│  │  Output: Artifact                       │                    │
│  │  Targets:                               │                    │
│  │  • VM Bytecode (primary)                │                    │
│  │  • LLVM IR text                         │                    │
│  │  • WebAssembly text (WAT)               │                    │
│  │  • Native x86_64/aarch64 assembly       │                    │
│  └─────────────────────────────────────────┘                    │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

### 3.3 Memory Model

```
┌─────────────────────────────────────────────────────────────────┐
│                     SNSX Memory Model                            │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │                    HybridHeap                            │   │
│  │  ┌──────────┐  ┌──────────┐  ┌──────────┐             │   │
│  │  │ HandleId │  │ HandleId │  │ HandleId │             │   │
│  │  │  refs:3  │  │  refs:1  │  │  refs:2  │             │   │
│  │  │  Value   │  │  Value   │  │  Value   │             │   │
│  │  └──────────┘  └──────────┘  └──────────┘             │   │
│  │                                                         │   │
│  │  Allocation: alloc(value) → HandleId                   │   │
│  │  Retention:  retain(id)   → refs += 1                  │   │
│  │  Release:    release(id)  → refs -= 1 or remove        │   │
│  └─────────────────────────────────────────────────────────┘   │
│                                                                  │
│  Value Types:                                                    │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │  Copy Types (stack-allocated, no heap needed):         │   │
│  │  • Int(i64)                                            │   │
│  │  • Float(f64)                                          │   │
│  │  • Bool(bool)                                          │   │
│  │  • Unit                                                │   │
│  └─────────────────────────────────────────────────────────┘   │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │  Non-Copy Types (heap-allocated, reference-counted):   │   │
│  │  • String(String)                                      │   │
│  │  • Bytes(Vec<u8>)                                      │   │
│  │  • Array(Vec<Value>)                                   │   │
│  │  • Tensor(Tensor)                                      │   │
│  │  • Function(usize)                                     │   │
│  │  • Task(TaskId)                                        │   │
│  └─────────────────────────────────────────────────────────┘   │
│                                                                  │
│  Ownership Model:                                                │
│  • Values are safe by default                                    │
│  • Copy types: Int, Float, Bool, Unit                            │
│  • Non-copy values: borrowed for reads                           │
│  • Explicit transfer with `move expr`                            │
│  • Deterministic reference counting                              │
│  • Optional cycle collection hooks                               │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

---

## 4. Repository Structure

```
Programming Lang/
├── Cargo.toml                          # Workspace root manifest
├── snsx.toml                           # SNSX project manifest
├── README.md                           # This file
├── install.sh                          # Installation script
├── master.sh                           # Master build script
│
├── compiler/                           # SNSX Compiler
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs                      # Compiler entry point
│       ├── lexer.rs                    # Tokenization
│       ├── parser.rs                   # Syntax parsing
│       ├── ast.rs                      # AST definitions
│       ├── semantic.rs                 # Type checking & analysis
│       ├── security.rs                 # Security audit
│       ├── diag.rs                     # Diagnostics
│       ├── ir.rs                       # IR definitions & lowering
│       ├── codegen.rs                  # Code generation
│       └── analysis/                   # Static analysis passes
│
├── runtime/                            # SNSX Runtime
│   ├── Cargo.toml
│   └── src/
│       └── lib.rs                      # Runtime context, heap, tasks, actors, sandbox
│
├── vm/                                 # SNSX Virtual Machine
│   ├── Cargo.toml
│   └── src/
│       └── lib.rs                      # VM execution engine
│
├── stdlib/                             # Standard Library
│   ├── Cargo.toml
│   ├── src/
│   │   └── lib.rs                      # Module registry
│   └── modules/
│       ├── io.snsx                     # I/O module
│       ├── ai.snsx                     # AI module
│       ├── math.snsx                   # Math module
│       ├── design.snsx                 # Design/UI module
│       ├── error.snsx                  # Error handling module
│       ├── sentinel.snsx               # Sentinel/security module
│       ├── db.snsx                     # Database module
│       ├── net.snsx                    # Network module
│       └── web.snsx                    # Web module
│
├── ide/                                # IDE (Terminal & Web)
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs                      # IDE entry point
│       ├── studio.rs                   # Terminal studio
│       ├── web.rs                      # Web/App studio
│       └── shell.rs                    # SNSX shell parser
│
├── cli/                                # Command-Line Interface
│   ├── Cargo.toml
│   └── src/
│       └── main.rs                     # CLI entry point
│
├── ai_engine/                          # AI Engine
│   ├── Cargo.toml
│   └── src/
│       └── lib.rs                      # Model registry, coding agent
│
├── network_stack/                      # SNSP Network Stack
│   ├── Cargo.toml
│   └── src/
│       └── lib.rs                      # Secure channels, frames, RPC
│
├── distributed_engine/                 # Distributed Engine
│   ├── Cargo.toml
│   └── src/
│       └── lib.rs                      # Cluster scheduling & recovery
│
├── aeris_compiler/                     # AERIS Compiler
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs                      # AERIS compiler entry
│       ├── ast.rs                      # AERIS AST
│       ├── lexer.rs                    # AERIS lexer
│       ├── parser.rs                   # AERIS parser
│       ├── semantic.rs                 # AERIS semantic analysis
│       ├── ir.rs                       # AERIS IR
│       └── diag.rs                     # AERIS diagnostics
│
├── aeris_runtime/                      # AERIS Runtime
│   ├── Cargo.toml
│   └── src/
│       └── lib.rs                      # AERIS interpreter
│
├── aeris_cli/                          # AERIS CLI
├── aeris_fmt/                          # AERIS Formatter
├── aeris_lint/                         # AERIS Linter
├── aeris_pkg/                          # AERIS Package Manager
├── aeris_kernel_abi/                   # AERIS Kernel ABI
│
├── aeris_examples/                     # AERIS Examples
│   ├── hello.ae
│   ├── actors.ae
│   └── refinement.ae
│
├── aeris_std/                          # AERIS Standard Library
│   ├── core/
│   ├── io/
│   ├── concurrency/
│   └── security/
│
├── examples/                           # SNSX Examples
│   ├── hello.snsx
│   ├── concurrency.snsx
│   ├── vote_eligibility.snsx
│   ├── calculator_terminal.snsx
│   ├── user_detail_database.snsx
│   ├── tensor.snsx
│   └── match.snsx
│
├── docs/                               # Documentation
│   ├── architecture.md
│   ├── AERIS_Formal_Semantics.md
│   ├── AERIS_IR.md
│   ├── AERIS_Kernel_Roadmap.md
│   ├── AERIS_Memory_Model.md
│   ├── AERIS_Security_Architecture.md
│   ├── AERIS_Self_Hosting_Plan.md
│   ├── AERIS_Toolchain.md
│   ├── AERIS_Trusted_Base.md
│   ├── SNSX_Technical_Specification.md
│   └── SNSX_Training_Manual.md
│
├── spec/                               # Language Specifications
│   ├── SNSX.md                         # SNSX language spec
│   ├── AERIS.md                        # AERIS language spec
│   ├── AERIS_Grammar.ebnf              # AERIS formal grammar
│   └── SNSP.md                         # SNSP protocol spec
│
├── src/                                # Project source
│   └── main.snsx                       # Main entry file
│
├── data/                               # Data files
│   ├── build_a_user_registration_system.snsxdb
│   └── user_detail.snsxdb
│
├── logs/                               # Build logs
└── target/                             # Build artifacts
```

---

## 5. SNSX Language Specification

### 5.1 Source File Format

- **Extension**: `.snsx`
- **Encoding**: UTF-8
- **Indentation**: 4 spaces per level (significant)
- **Comments**: Begin with `#`

### 5.2 Primitive Types

| Type | Description | Copy Type |
|------|-------------|-----------|
| `Int` | 64-bit signed integer | ✅ |
| `Float` | 64-bit IEEE 754 float | ✅ |
| `Bool` | Boolean true/false | ✅ |
| `String` | UTF-8 string | ❌ |
| `Bytes` | Raw byte array | ❌ |
| `Unit` | Unit/null type | ✅ |
| `Tensor` | N-dimensional tensor | ❌ |
| `Task[T]` | Async task handle | ❌ |
| `Array[T]` | Dynamic array | ❌ |
| `Fn[(A, B) -> R]` | Function type | ❌ |

### 5.3 Keywords Reference

```
┌─────────────────────────────────────────────────────────────────┐
│                    SNSX Keyword Reference                        │
├──────────────┬──────────────────────────────────────────────────┤
│  Keyword     │  Purpose                                        │
├──────────────┼──────────────────────────────────────────────────┤
│  bring       │  Import a module/library/package                │
│  as          │  Alias for imports                              │
│  entry       │  Application root (compiles to main)            │
│  flow        │  Callable function definition                   │
│  mind        │  AI prompt function definition                  │
│  takes       │  Function parameters                            │
│  gives       │  Function return type                           │
│  later       │  Async function definition                      │
│  agent       │  Actor state machine                            │
│  memory      │  Actor state field                              │
│  hold        │  Immutable value binding                        │
│  set         │  Value reassignment                             │
│  show        │  Direct output                                  │
│  ask         │  Interactive input                              │
│  when        │  Conditional branching (alias for gate)         │
│  gate        │  Conditional branching                          │
│  otherwise   │  Else branch                                    │
│  unless      │  Inverted condition                             │
│  during      │  Loop construct                                 │
│  pick        │  Pattern matching                               │
│  case        │  Pattern match arm                              │
│  send        │  Explicit return                                │
│  give        │  Explicit return in structured flows            │
│  guard       │  Contract assertion                             │
│  fail        │  Explicit failure                               │
│  from        │  AI prompt metadata                             │
│  move        │  Explicit value transfer                        │
│  launch      │  Spawn async task                               │
│  wait        │  Await async task                               │
│  parallel    │  Parallel execution hint                        │
│  |>          │  Dataflow pipeline operator                     │
└──────────────┴──────────────────────────────────────────────────┘
```

### 5.4 Declaration Syntax

#### 5.4.1 Imports

```snsx
# Standard library modules
bring <module/std.io> as io
bring <module/std.ai> as ai
bring <module/std.db> as db

# Library surfaces
bring <lib/std.design> as design
bring <lib/std.sentinel> as sentry

# Package surfaces
bring <package/std.error> as errors
```

#### 5.4.2 Application Entry

```snsx
entry
    show "SNSX Application Started"
    hold message = "Hello, World!"
    show message
    0  # Return value
```

#### 5.4.3 Flow Functions

```snsx
# Simple flow
flow greet
    takes name: String
    gives String
    "Hello, " + name

# Flow with multiple parameters
flow add
    takes a: Int, b: Int
    gives Int
    a + b

# Async flow
later fetch_data
    takes url: String
    gives String
    "fetched: " + url
```

#### 5.4.4 Mind Functions (AI-Native)

```snsx
mind summarize
    takes text: String
    gives String
    from "Summarize this text in two sentences"

mind classify
    takes text: String
    gives String
    from "Classify this text into a category"
```

#### 5.4.5 Actor Definitions

```snsx
agent Counter:
    memory value: Int = 0

    flow inc
        takes step: Int
        gives Int
        set value = value + step
        value

    flow get
        gives Int
        value
```

### 5.5 Statement Syntax

#### 5.5.1 Bindings

```snsx
# Immutable binding
hold x = 10
hold name: String = "SNSX"

# Reassignment
set x = x + 1
```

#### 5.5.2 Conditional Control

```snsx
# Gate/otherwise
gate x > 10:
    show "large"
otherwise:
    show "small"

# Unless (inverted)
unless name == "":
    show name

# Guard (contract check)
guard x > 0, "x must be positive"
```

#### 5.5.3 Loops

```snsx
# During loop
hold i = 0
during i < 10:
    show i
    set i = i + 1
```

#### 5.5.4 Pattern Matching

```snsx
pick value
    case 0
        show "zero"
    case 1
        show "one"
    else
        show "many"
```

### 5.6 Expression Syntax

```snsx
# Arithmetic
hold result = 1 + 2 * 3

# Function calls
hold sum = add(10, 20)

# Array literals
hold items = [1, 2, 3, 4, 5]

# String operations
hold greeting = "Hello" + ", " + "World!"

# Pipeline operator
"Some text" |> summarize |> show

# Async operations
hold task = launch work(10)
hold value = wait task

# Parallel execution
hold result = parallel compute_heavy(99)

# Tensor operations
hold t = tensor([1.0, 2.0, 3.0], [3])
```

### 5.7 Built-in Functions

```snsx
# I/O
show(value)              # Output value
ask()                    # Read line from stdin

# Type conversion
int(string)              # String to Int
float(string)            # String to Float
is_int(string)           # Check if string is valid Int
is_digits(string)        # Check if string is all digits
is_email(string)         # Basic email validation

# String operations
len(string)              # String/array length
contains(string, needle) # Substring check
sanitize(value)          # Safe string conversion

# Cryptography
seal(value)              # SHA3-256 hash
sha3(value)              # SHA3-256 hash (alias)

# Debugging
shape(value)             # Value shape/type info
watch(label, value)      # Debug watcher

# Error handling
guard(condition, message) # Contract assertion
fail(message)            # Explicit failure
repair(primary, fallback) # Fallback on blank

# Design/UI
panel(title, body)       # Create panel
field(label, value)      # Create field
draft(title, content)    # Create draft
stack(items)             # Stack layout
blend(items, separator)  # Join items

# File I/O (requires fs permission)
read_text(path)          # Read file
write_text(path, text)   # Write file
append_text(path, text)  # Append to file
```

---

## 6. AERIS Language Specification

### 6.1 Overview

AERIS is a capability-secure systems language designed around:

- **Explicit effects**: Every function declares its effects
- **Explicit capabilities**: IO requires capability tokens
- **Algebraic data types**: Rich type system with variants
- **Deterministic actors**: FIFO message processing
- **Compile-time refinement checks**: Type-level constraints
- **SSA-oriented compilation**: Clean IR lowering

### 6.2 Source File Format

- **Extension**: `.ae`
- **Package manifest**: `Aeris.toml`
- **Entry point**: `fn main(...) -> Int !effect { ... }`

### 6.3 Core Syntax

```aeris
module demo.hello;

effect io;
cap Console;

type Option<T> = Some(value: T) | None;

fn greet(name: Text) -> Text !pure {
    "Hello, " + name
}

fn main(console: Console) -> Int !io {
    let message = greet("AERIS");
    print(console, message);
    0
}
```

### 6.4 Design Rules

```
┌─────────────────────────────────────────────────────────────────┐
│                    AERIS Design Rules                            │
├─────────────────────────────────────────────────────────────────┤
│  1. Every function MUST declare an effect                       │
│  2. IO requires explicit capability tokens                      │
│  3. Actor primitives isolated to !actor functions               │
│  4. Refinements attached with `where` clauses                   │
│  5. No implicit globals                                         │
│  6. Capabilities must be passed explicitly                      │
│  7. No ambient authority granted by default                     │
│  8. Actor execution is deterministic FIFO                       │
└─────────────────────────────────────────────────────────────────┘
```

### 6.5 Built-in Effects & Capabilities

```aeris
# Effects
effect pure;     # No side effects
effect io;       # Input/output
effect actor;    # Actor operations
effect state;    # State mutation

# Capabilities
cap Console;     # Console I/O
cap Clock;       # Time access
cap FileSystem;  # File system
cap Network;     # Network access
```

### 6.6 Type System

```aeris
# Built-in types
Int              # 64-bit integer
Bool             # Boolean
Text             # UTF-8 string
Unit             # Unit type
ActorHandle      # Actor reference

# Algebraic data types
type Option<T> = Some(value: T) | None;
type Result<T, E> = Ok(value: T) | Err(error: E);

# Refinement types
type Positive = Int where x > 0;
type NonEmpty = String where len(x) > 0;
```

### 6.7 Actor System

```aeris
actor Counter:
    var count: Int = 0

    handler Increment(step: Int) {
        count = count + step
    }

    handler GetCount() -> Int {
        count
    }

fn main(console: Console) -> Int !io {
    let counter = spawn(Counter);
    emit(counter, Increment(5));
    emit(counter, GetCount());
    drain();
    0
}
```

---

## 7. Compiler Pipeline

### 7.1 Compilation Stages

```rust
// compiler/src/lib.rs - Main compilation pipeline

pub fn compile_source(
    path: &str,
    source: &str,
    options: &CompileOptions,
) -> std::result::Result<Compilation, Vec<Diagnostic>> {
    // Stage 1: Parse source graph (handles multi-file modules)
    let ast = parse_source_graph(path, source)?;
    
    // Stage 2: Semantic analysis
    let semantic = analyze_module(path, &ast)?;
    
    // Stage 3: Security audit
    let security_diagnostics = audit_module(&ast, &semantic, policy);
    if !security_diagnostics.is_empty() {
        return Err(security_diagnostics);
    }
    
    // Stage 4: IR lowering
    let mut ir = lower_module(&ast, &semantic)?;
    
    // Stage 5: Optimization
    if options.optimize {
        optimize_program(&mut ir);
    }
    
    // Stage 6: Target code generation
    let artifact = match options.target {
        Target::Vm => Artifact::Vm(vm::emit_bytecode(&ir, &semantic)),
        Target::Llvm => Artifact::Text(llvm::emit_llvm(&ir, &semantic)),
        Target::Wasm => Artifact::Text(wasm::emit_wat(&ir, &semantic)),
        Target::NativeX86_64 => Artifact::Text(native::emit_asm(&ir, &semantic, Arch::X86_64)),
        Target::NativeAarch64 => Artifact::Text(native::emit_asm(&ir, &semantic, Arch::Aarch64)),
    };
    
    Ok(Compilation { ast, semantic, ir, artifact })
}
```

### 7.2 Lexer Algorithm

The SNSX lexer implements a state-machine tokenizer:

```
┌─────────────────────────────────────────────────────────────────┐
│                    Lexer State Machine                           │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  ┌──────────┐                                                   │
│  │  START   │                                                   │
│  └────┬─────┘                                                   │
│       │                                                          │
│       ├──[a-zA-Z_]──▶──────────────────────────────────┐        │
│       │               │  IDENTIFIER/KEYWORD STATE      │        │
│       │               │  • Read alphanumeric + _       │        │
│       │               │  • Check keyword table         │        │
│       │               │  • Emit Token                  │        │
│       │               └────────────────────────────────┘        │
│       │                                                          │
│       ├──[0-9]─────▶──────────────────────────────────┐         │
│       │               │  NUMBER STATE                  │         │
│       │               │  • Read digits                 │         │
│       │               │  • Check for decimal point     │         │
│       │               │  • Emit Int/Float Token        │         │
│       │               └────────────────────────────────┘         │
│       │                                                          │
│       ├──["]────────▶──────────────────────────────────┐         │
│       │               │  STRING STATE                  │         │
│       │               │  • Read until closing "       │         │
│       │               │  • Handle escape sequences     │         │
│       │               │  • Emit String Token           │         │
│       │               └────────────────────────────────┘         │
│       │                                                          │
│       ├──[#]────────▶──────────────────────────────────┐         │
│       │               │  COMMENT STATE                 │         │
│       │               │  • Skip until newline          │         │
│       │               │  • No token emitted            │         │
│       │               └────────────────────────────────┘         │
│       │                                                          │
│       ├──[ ]────────▶──────────────────────────────────┐         │
│       │               │  WHITESPACE STATE              │         │
│       │               │  • Count spaces (4 = indent)  │         │
│       │               │  • Emit Indent/Dedent tokens   │         │
│       │               └────────────────────────────────┘         │
│       │                                                          │
│       └──[EOF]──────▶──────────────────────────────────┐         │
│                       │  Emit EOF Token                │         │
│                       └────────────────────────────────┘         │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

### 7.3 Parser Algorithm

The parser uses **recursive descent** with **operator precedence climbing** for expressions:

```
┌─────────────────────────────────────────────────────────────────┐
│                 Parser Grammar (Simplified)                      │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  Module      ::= Import* Item*                                  │
│  Import      ::= 'bring' ImportPath 'as' IDENT                  │
│  Item        ::= EntryBlock | FlowBlock | MindBlock | AgentDecl │
│                                                                  │
│  EntryBlock  ::= 'entry' NEWLINE INDENT Statement+ DEDENT      │
│  FlowBlock   ::= 'flow' IDENT ParamList? ReturnType? Body      │
│  MindBlock   ::= 'mind' IDENT ParamList? ReturnType? Prompt    │
│  AgentDecl   ::= 'agent' IDENT ':' NEWLINE INDENT Field+ Flow* │
│                                                                  │
│  Statement   ::= Binding | Reassign | Conditional | Loop       │
│                | Match | Guard | Fail | Expression              │
│                                                                  │
│  Binding     ::= 'hold' IDENT (':' Type)? '=' Expression       │
│  Reassign    ::= 'set' IDENT '=' Expression                     │
│  Conditional ::= 'gate' Expression ':' Block ('otherwise' Block)?│
│  Loop        ::= 'during' Expression ':' Block                  │
│  Match       ::= 'pick' Expression NEWLINE INDENT CaseArm+ DEDENT│
│                                                                  │
│  Expression  ::= Pipeline                                        │
│  Pipeline    ::= Comparison ('|>' Comparison)*                  │
│  Comparison  ::= Addition (('==' | '!=' | '<' | '>' | ...) Addition)*│
│  Addition    ::= Multiplication (('+' | '-') Multiplication)*   │
│  Multiplication ::= Unary (('*' | '/') Unary)*                 │
│  Unary       ::= ('-' | '!')? Call                              │
│  Call         ::= Primary ('(' ArgList ')' | '[' Expression ']')*│
│  Primary     ::= Literal | IDENT | '(' Expression ')' | Array   │
│                                                                  │
│  Literal     ::= INT | FLOAT | STRING | 'true' | 'false'        │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

### 7.4 IR Lowering

The compiler lowers AST to **SSA-style IR**:

```rust
// compiler/src/ir.rs - IR structures

#[derive(Debug, Clone)]
pub struct Program {
    pub functions: Vec<Function>,
}

#[derive(Debug, Clone)]
pub struct Function {
    pub name: String,
    pub params: usize,
    pub locals: usize,
    pub blocks: Vec<Block>,
    pub is_ai: bool,
    pub prompt: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Block {
    pub id: usize,
    pub instructions: Vec<Instruction>,
    pub terminator: Terminator,
}

#[derive(Debug, Clone)]
pub enum Instruction {
    Const(Constant),
    Add(ValueId, ValueId),
    Sub(ValueId, ValueId),
    Mul(ValueId, ValueId),
    Div(ValueId, ValueId),
    Eq(ValueId, ValueId),
    Lt(ValueId, ValueId),
    Call(FunctionId, Vec<ValueId>),
    LoadLocal(usize),
    StoreLocal(usize, ValueId),
    MakeArray(Vec<ValueId>),
    Index(ValueId, ValueId),
}

#[derive(Debug, Clone)]
pub enum Terminator {
    Return(ValueId),
    Jump(BlockId),
    Branch(ValueId, BlockId, BlockId),
    Unreachable,
}
```

### 7.5 Target Code Generation

#### 7.5.1 VM Bytecode (Primary Target)

```rust
// compiler/src/codegen/vm.rs

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Instruction {
    PushConst(usize),       // Push constant from constant pool
    LoadLocal(usize),       // Load local variable
    StoreLocal(usize),      // Store to local variable
    LoadFunction(usize),    // Load function reference
    Add,                    // Add top two stack values
    Sub,                    // Subtract
    Mul,                    // Multiply
    Div,                    // Divide
    Mod,                    // Modulo
    Eq,                     // Equal comparison
    Ne,                     // Not equal
    Lt, Less than
    Le, Less or equal
    Gt, Greater than
    Ge, Greater or equal
    Neg,                    // Negate
    MakeArray(usize),       // Create array from N stack values
    Index,                  // Index into array/string/tensor
    Call(usize),            // Call function with N args
    Spawn(usize),           // Spawn async task
    Await,                  // Wait for task
    Pop,                    // Pop stack
    Jump(usize),            // Unconditional jump
    JumpIfFalse(usize),     // Conditional jump
    Return,                 // Return from function
}
```

#### 7.5.2 LLVM IR Emission

```llvm
; Generated LLVM IR for: flow add takes a: Int, b: Int gives Int a + b

define i64 @add(i64 %a, i64 %b) {
entry:
  %result = add i64 %a, %b
  ret i64 %result
}
```

#### 7.5.3 WebAssembly Text (WAT)

```wat
(module
  (func $add (param $a i64) (param $b i64) (result i64)
    local.get $a
    local.get $b
    i64.add)
  (export "add" (func $add)))
```

#### 7.5.4 Native Assembly (x86_64)

```asm
; x86_64 assembly for add function
add:
    mov rax, rdi
    add rax, rsi
    ret
```

---

## 8. Virtual Machine (VM) Engine

### 8.1 VM Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                    SNSX Virtual Machine                          │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │                    Execution Loop                        │   │
│  │                                                         │   │
│  │  loop {                                                 │   │
│  │      let instruction = code[ip];                        │   │
│  │      ip += 1;                                           │   │
│  │      match instruction {                                │   │
│  │          PushConst(idx) => stack.push(constants[idx]),  │   │
│  │          Add => {                                       │   │
│  │              let rhs = stack.pop();                     │   │
│  │              let lhs = stack.pop();                     │   │
│  │              stack.push(lhs + rhs);                     │   │
│  │          }                                              │   │
│  │          Call(argc) => invoke(argc),                    │   │
│  │          Return => return stack.pop(),                  │   │
│  │          ...                                            │   │
│  │      }                                                  │   │
│  │  }                                                      │   │
│  └─────────────────────────────────────────────────────────┘   │
│                                                                  │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │                    VM State                              │   │
│  │                                                         │   │
│  │  • module: BytecodeModule    // Loaded bytecode         │   │
│  │  • runtime: RuntimeContext   // Runtime state           │   │
│  │  • models: ModelRegistry     // AI models               │   │
│  │  • pending_tasks: HashMap    // Async task queue        │   │
│  │  • trace: Vec<String>        // Execution trace         │   │
│  └─────────────────────────────────────────────────────────┘   │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

### 8.2 VM Execution Algorithm

```rust
// vm/src/lib.rs - Core VM execution

impl VirtualMachine {
    fn invoke_function(
        &mut self,
        function_index: usize,
        function: &BytecodeFunction,
        args: Vec<Value>,
    ) -> Result<Value> {
        // Handle AI functions
        if function.is_ai {
            self.runtime.sandbox.check_ai()?;
            let prompt = function.prompt.clone().unwrap_or("AI prompt".to_string());
            return self.models.invoke_prompt_function(&prompt, &args, None);
        }

        // Create execution frame
        let mut frame = Frame {
            function_index,
            ip: 0,
            locals: vec![Value::Unit; function.locals.max(function.params)],
            stack: Vec::new(),
        };
        
        // Load arguments into locals
        for (index, arg) in args.into_iter().enumerate() {
            if index < frame.locals.len() {
                frame.locals[index] = arg;
            }
        }

        // Main execution loop
        loop {
            let instruction = function.code.get(frame.ip).cloned()?;
            frame.ip += 1;
            
            match instruction {
                Instruction::PushConst(index) => {
                    frame.stack.push(constant_to_value(function.constants[index]));
                }
                Instruction::Add => {
                    let rhs = frame.stack.pop()?;
                    let lhs = frame.stack.pop()?;
                    frame.stack.push(add_values(lhs, rhs)?);
                }
                Instruction::Call(argc) => {
                    let args = pop_args(&mut frame.stack, argc)?;
                    let callee = frame.stack.pop()?;
                    let symbol = function_from_value(callee)?;
                    frame.stack.push(self.invoke_symbol(symbol, args)?);
                }
                Instruction::Return => {
                    return Ok(frame.stack.pop().unwrap_or(Value::Unit));
                }
                // ... other instructions
            }
        }
    }
}
```

### 8.3 Built-in Function Implementations

```rust
// vm/src/lib.rs - Builtin implementations

fn call_builtin(&mut self, name: &str, args: Vec<Value>) -> Result<Value> {
    match name {
        "show" | "print" => {
            if let Some(value) = args.first() {
                self.runtime.print_line(value);
            }
            Ok(Value::Unit)
        }
        "ask" | "read_line" => {
            match self.runtime.try_read_line() {
                Some(line) => Ok(Value::String(line)),
                None => Err(VmTrap::InputRequested(...).into()),
            }
        }
        "int" => {
            let text = args.first()?.as_string()?;
            Ok(Value::Int(text.trim().parse::<i64>()?))
        }
        "is_int" => {
            let text = args.first()?.as_string()?;
            Ok(Value::Bool(text.trim().parse::<i64>().is_ok()))
        }
        "seal" | "sha3" => {
            let text = args.first()?.as_string()?;
            let hash = Sha3_256::digest(text.as_bytes());
            Ok(Value::String(hex(&hash)))
        }
        "guard" => {
            let condition = args.first()?;
            let message = args.get(1).map(ToString::to_string)
                .unwrap_or("guard failed".to_string());
            if condition.truthy() {
                Ok(Value::Unit)
            } else {
                Err(anyhow!("guard blocked execution: {}", message))
            }
        }
        // ... 50+ more builtins
    }
}
```

---

## 9. Runtime System

### 9.1 Runtime Context

```rust
// runtime/src/lib.rs

#[derive(Debug)]
pub struct RuntimeContext {
    pub heap: HybridHeap,              // Memory management
    pub tasks: TaskScheduler<Value>,   // Async task scheduler
    pub actors: ActorSystem<Vec<u8>>,  // Actor message system
    pub sandbox: SandboxPolicy,        // Security sandbox
    pub clock: DeterministicClock,     // Deterministic time
    pub stdout: String,                // Output buffer
    pub stdin: VecDeque<String>,       // Input queue
    pub stdin_snapshot: String,        // Full input snapshot
    pub allow_host_stdin: bool,        // Host stdin permission
    pub consumed_stdin_lines: usize,   // Lines consumed
}
```

### 9.2 Task Scheduler

```
┌─────────────────────────────────────────────────────────────────┐
│                    Task Scheduler                                │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │  Task States:                                           │   │
│  │  • Ready     → Task is queued for execution             │   │
│  │  • Running   → Task is currently executing              │   │
│  │  • Completed → Task finished with result                │   │
│  │  • Failed    → Task failed with error                   │   │
│  └─────────────────────────────────────────────────────────┘   │
│                                                                  │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │  Scheduler Algorithm:                                   │   │
│  │                                                         │   │
│  │  fn spawn(label) -> TaskId {                            │   │
│  │      let id = next_id++;                                │   │
│  │      tasks.insert(id, TaskRecord { label, Ready });    │   │
│  │      queue.push_back(id);                               │   │
│  │      id                                                 │   │
│  │  }                                                      │   │
│  │                                                         │   │
│  │  fn take_next() -> Option<TaskId> {                     │   │
│  │      if deterministic {                                 │   │
│  │          queue.sort_by_key(|id| id.0);                 │   │
│  │      }                                                  │   │
│  │      queue.pop_front()                                  │   │
│  │  }                                                      │   │
│  └─────────────────────────────────────────────────────────┘   │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

### 9.3 Actor System

```
┌─────────────────────────────────────────────────────────────────┐
│                    Actor System                                  │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  ┌──────────────┐     ┌──────────────┐     ┌──────────────┐   │
│  │   Actor #1   │     │   Actor #2   │     │   Actor #3   │   │
│  │              │     │              │     │              │   │
│  │ ┌──────────┐│     │ ┌──────────┐│     │ ┌──────────┐│   │
│  │ │ Mailbox  ││     │ │ Mailbox  ││     │ │ Mailbox  ││   │
│  │ │ [msg1]   ││     │ │ [msg2]   ││     │ │ []       ││   │
│  │ │ [msg2]   ││     │ │ [msg3]   ││     │ │          ││   │
│  │ └──────────┘│     │ └──────────┘│     │ └──────────┘│   │
│  └──────────────┘     └──────────────┘     └──────────────┘   │
│                                                                  │
│  Operations:                                                     │
│  • create_actor() → ActorId                                     │
│  • send(id, message) → Queue message to actor                   │
│  • recv(id) → Option<Message> → Dequeue message                 │
│                                                                  │
│  Guarantees:                                                     │
│  • FIFO message ordering                                        │
│  • No shared mutable state                                      │
│  • Deterministic execution                                      │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

### 9.4 Sandbox Policy

```rust
#[derive(Debug, Clone)]
pub struct SandboxPolicy {
    pub allow_fs: bool,          // Filesystem access
    pub allow_network: bool,     // Network access
    pub allow_ai: bool,          // AI model access
    pub deterministic: bool,     // Deterministic mode
    pub max_memory_bytes: usize, // Memory limit (64MB default)
}

impl SandboxPolicy {
    pub fn check_fs(&self) -> Result<(), SandboxError> {
        if self.allow_fs { Ok(()) } else { Err(SandboxError::FilesystemDenied) }
    }
    
    pub fn check_network(&self) -> Result<(), SandboxError> {
        if self.allow_network { Ok(()) } else { Err(SandboxError::NetworkDenied) }
    }
    
    pub fn check_ai(&self) -> Result<(), SandboxError> {
        if self.allow_ai { Ok(()) } else { Err(SandboxError::AiDenied) }
    }
}
```

---

## 10. AI Engine

### 10.1 AI Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                    SNSX AI Engine                                │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │  Model Registry                                         │   │
│  │  • MockModel (default)                                  │   │
│  │  • Custom model registration                            │   │
│  │  • GPU backend hooks                                    │   │
│  └─────────────────────────────────────────────────────────┘   │
│                                                                  │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │  AI-Native Functions                                    │   │
│  │                                                         │   │
│  │  mind summarize                                         │   │
│  │      takes text: String                                 │   │
│  │      gives String                                       │   │
│  │      from "Summarize this text"                        │   │
│  │                                                         │   │
│  │  Compilation:                                           │   │
│  │  1. Parse mind declaration                              │   │
│  │  2. Extract prompt metadata                             │   │
│  │  3. Generate callable with embedded prompt              │   │
│  │  4. Route to model at runtime                           │   │
│  └─────────────────────────────────────────────────────────┘   │
│                                                                  │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │  Tensor Runtime                                         │   │
│  │  • CPU backend (matmul, relu)                           │   │
│  │  • GPU hook (extensible)                                │   │
│  │  • Shape tracking and validation                        │   │
│  └─────────────────────────────────────────────────────────┘   │
│                                                                  │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │  SNS AI Coding Agent                                    │   │
│  │  • Prompt analysis & recipe scoring                     │   │
│  │  • 12 recipe families                                   │   │
│  │  • Compiler-validated output                            │   │
│  │  • Multi-file generation                                │   │
│  └─────────────────────────────────────────────────────────┘   │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

### 10.2 Coding Agent Recipes

| Recipe | Trigger Keywords | Description |
|--------|-----------------|-------------|
| `registration_suite` | registration, CRUD, system | Multi-file registration app |
| `vote_eligibility` | vote, eligibility, age | Age-based voting checker |
| `grade_checker` | grade, marks, score | Numeric grade calculator |
| `calculator` | calculator, arithmetic | Two-number calculator |
| `number_comparison` | largest, compare, maximum | Number comparison |
| `data_record_store` | database, store, record | File-backed data store |
| `text_summary` | summary, summarize, AI | AI text summarization |
| `design_board` | dashboard, panel, design | UI layout generator |
| `access_guard` | login, password, security | Access control |
| `worker_task` | async, parallel, worker | Concurrent task |
| `input_echo` | input, ask, form | Input validator |
| `generic_starter` | (fallback) | Generic starter scaffold |

---

## 11. Network Stack (SNSP)

### 11.1 Protocol Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                    SNSP Secure Network Stack                     │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │  Frame Format                                           │   │
│  │  ┌──────┬──────┬──────────┬────────┬────────┬────────┐ │   │
│  │  │ ver  │flags │stream_id │ method │ length │payload │ │   │
│  │  │ 1B   │ 1B   │  4B      │  2B    │  4B    │  nB    │ │   │
│  │  └──────┴──────┴──────────┴────────┴────────┴────────┘ │   │
│  └─────────────────────────────────────────────────────────┘   │
│                                                                  │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │  Frame Kinds                                            │   │
│  │  • Open  (0x01) → Connection establishment              │   │
│  │  • Data  (0x02) → Data transfer                         │   │
│  │  • Close (0x04) → Connection termination                │   │
│  │  • Rpc   (0x08) → Remote procedure call                 │   │
│  │  • Ack   (0x10) → Acknowledgment                       │   │
│  └─────────────────────────────────────────────────────────┘   │
│                                                                  │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │  Security                                               │   │
│  │  • Ed25519 identity keys                                │   │
│  │  • X25519 ephemeral key exchange                        │   │
│  │  • AES-256-GCM-SIV encryption                           │   │
│  │  • SHA3-256 key derivation                              │   │
│  │  • Sequence-based nonce                                 │   │
│  └─────────────────────────────────────────────────────────┘   │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

### 11.2 Handshake Protocol

```
┌──────────┐                                    ┌──────────┐
│  Client  │                                    │  Server  │
└────┬─────┘                                    └────┬─────┘
     │                                               │
     │  hello(identity, ephemeral_key, signature)    │
     │──────────────────────────────────────────────▶│
     │                                               │
     │  verify(identity, ephemeral_key, signature)   │
     │◀──────────────────────────────────────────────│
     │                                               │
     │  accept(identity, ephemeral_key, signature)   │
     │◀──────────────────────────────────────────────│
     │                                               │
     │  shared_secret = X25519(local, remote)        │
     │  key = SHA3-256(shared_secret)                │
     │                                               │
     │  ◀═══════════════════════════════════════════▶│
     │           Encrypted Channel Active             │
     │                                               │
```

### 11.3 Secure Channel

```rust
pub struct SecureChannel {
    key: [u8; 32],
}

impl SecureChannel {
    pub fn derive(local_secret: &StaticSecret, remote_public: [u8; 32]) -> Self {
        let shared = local_secret.diffie_hellman(&remote_public);
        let key: [u8; 32] = Sha3_256::digest(shared.as_bytes()).into();
        Self { key }
    }
    
    pub fn encrypt(&self, sequence: u64, plaintext: &[u8]) -> Result<Vec<u8>> {
        let cipher = Aes256GcmSiv::new_from_slice(&self.key)?;
        let nonce = nonce_from_sequence(sequence);
        cipher.encrypt(&nonce, plaintext)
    }
    
    pub fn decrypt(&self, sequence: u64, ciphertext: &[u8]) -> Result<Vec<u8>> {
        let cipher = Aes256GcmSiv::new_from_slice(&self.key)?;
        let nonce = nonce_from_sequence(sequence);
        cipher.decrypt(&nonce, ciphertext)
    }
}
```

---

## 12. Distributed Engine

### 12.1 Cluster Scheduling

```
┌─────────────────────────────────────────────────────────────────┐
│                    Cluster Scheduler                             │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  Input:                                                          │
│  • ClusterState (nodes with resources)                          │
│  • TaskSpec (tasks with requirements)                           │
│                                                                  │
│  Algorithm:                                                      │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │  fn schedule(state, tasks) -> ClusterPlan {             │   │
│  │      scores = calculate_node_scores(state.nodes)        │   │
│  │      assignments = []                                   │   │
│  │                                                         │   │
│  │      for task in tasks {                                │   │
│  │          for replica in 0..task.replicas {              │   │
│  │              candidates = filter_nodes(                 │   │
│  │                  state.nodes,                           │   │
│  │                  gpu_ok = task.requires_gpu             │   │
│  │              )                                          │   │
│  │              candidates.sort_by_score()                 │   │
│  │              best = candidates.first()                  │   │
│  │              assignments.add(task, best, replica)      │   │
│  │              update_scores(scores, best, task)         │   │
│  │          }                                              │   │
│  │      }                                                  │   │
│  │      ClusterPlan { assignments, replication_map }      │   │
│  │  }                                                      │   │
│  └─────────────────────────────────────────────────────────┘   │
│                                                                  │
│  Node Scoring:                                                   │
│  score = cpu_cores * 4 + memory_mb / 256 + gpu ? 64 : 0        │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

### 12.2 Fault Recovery

```rust
pub fn recover(
    state: &ClusterState,
    plan: &ClusterPlan,
    fault: &FaultEvent,
) -> RecoveryPlan {
    // 1. Identify surviving nodes
    let survivors = state.nodes.iter()
        .filter(|node| node.id != fault.node_id)
        .collect();
    
    // 2. Find evacuated tasks
    let evacuated = plan.assignments.iter()
        .filter(|a| a.node_id == fault.node_id)
        .collect();
    
    // 3. Reschedule to survivors
    let survivor_state = ClusterState { nodes: survivors };
    let new_plan = survivor_state.schedule(&evacuated);
    
    RecoveryPlan { evacuate_to: new_plan.assignments }
}
```

---

## 13. Standard Library

### 13.1 Module Registry

```rust
// stdlib/src/lib.rs

impl ModuleRegistry {
    pub fn new() -> Self {
        Self {
            modules: HashMap::from([
                ("std.io",      include_str!("../modules/io.snsx")),
                ("std.math",    include_str!("../modules/math.snsx")),
                ("std.ai",      include_str!("../modules/ai.snsx")),
                ("std.db",      include_str!("../modules/db.snsx")),
                ("std.net",     include_str!("../modules/net.snsx")),
                ("std.gfx",     include_str!("../modules/gfx.snsx")),
                ("std.web",     include_str!("../modules/web.snsx")),
                ("std.design",  include_str!("../modules/design.snsx")),
                ("std.error",   include_str!("../modules/error.snsx")),
                ("std.sentinel",include_str!("../modules/sentinel.snsx")),
            ]),
        }
    }
}
```

### 13.2 Available Modules

| Module | Purpose | Key Functions |
|--------|---------|---------------|
| `std.io` | Input/Output | `show`, `ask`, `print` |
| `std.ai` | AI Functions | `summarize`, `classify`, `transform` |
| `std.math` | Mathematics | `abs`, `sqrt`, `pow`, `min`, `max` |
| `std.db` | Database | `read_text`, `write_text`, `append_text` |
| `std.net` | Networking | `fetch`, `send`, `recv` |
| `std.gfx` | Graphics | `canvas`, `line`, `put` |
| `std.web` | Web | `html`, `css`, `render` |
| `std.design` | UI Design | `panel`, `field`, `draft`, `stack` |
| `std.error` | Error Handling | `guard`, `fail`, `repair` |
| `std.sentinel` | Security | `seal`, `verify`, `token` |

---

## 14. Security Architecture

### 14.1 Multi-Layer Security

```
┌─────────────────────────────────────────────────────────────────┐
│                    SNSX Security Layers                          │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  Layer 1: Syntax & Semantic Validation                          │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │  • Type checking                                        │   │
│  │  • Symbol resolution                                    │   │
│  │  • Builtin recognition                                  │   │
│  └─────────────────────────────────────────────────────────┘   │
│                                                                  │
│  Layer 2: Manifest Permissions                                  │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │  [permissions]                                          │   │
│  │  fs = false    # Filesystem access                      │   │
│  │  net = false   # Network access                         │   │
│  │  ai = true     # AI model access                        │   │
│  └─────────────────────────────────────────────────────────┘   │
│                                                                  │
│  Layer 3: Strict Security Audit                                 │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │  • Unsafe pattern detection                             │   │
│  │  • Undeclared resource usage                            │   │
│  │  • Contract gap analysis                                │   │
│  │  • Loop hazard detection                                │   │
│  └─────────────────────────────────────────────────────────┘   │
│                                                                  │
│  Layer 4: Workspace Confinement                                 │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │  • All file operations confined to workspace root       │   │
│  │  • Path traversal prevention                            │   │
│  │  • Host shell guard (blocks destructive commands)       │   │
│  └─────────────────────────────────────────────────────────┘   │
│                                                                  │
│  Layer 5: Runtime Sandbox                                       │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │  • Capability-based access control                      │   │
│  │  • Memory limits                                        │   │
│  │  • Deterministic mode support                           │   │
│  └─────────────────────────────────────────────────────────┘   │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

### 14.2 Security Audit Rules

```
┌─────────────────────────────────────────────────────────────────┐
│                    Security Audit Rules                          │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  Rule: BLOCKED_UNDECLARED_FS                                    │
│  When: Code uses read_text/write_text but fs = false            │
│  Fix:  Set fs = true in snsx.toml [permissions]                 │
│                                                                  │
│  Rule: BLOCKED_UNDECLARED_NET                                   │
│  When: Code uses network functions but net = false              │
│  Fix:  Set net = true in snsx.toml [permissions]                │
│                                                                  │
│  Rule: BLOCKED_UNDECLARED_AI                                    │
│  When: Code uses mind blocks but ai = false                     │
│  Fix:  Set ai = true in snsx.toml [permissions]                 │
│                                                                  │
│  Rule: BLOCKED_UNSAFE_INPUT                                     │
│  When: Raw input used without validation                        │
│  Fix:  Use guard/is_int/is_email for validation                 │
│                                                                  │
│  Rule: BLOCKED_LOOP_HAZARD                                      │
│  When: Unbounded loop detected                                  │
│  Fix:  Add explicit bounds or use during with clear exit        │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

---

## 15. IDE & Development Tools

### 15.1 Terminal Studio

```
┌─────────────────────────────────────────────────────────────────┐
│                    SNSX Terminal Studio                          │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  ┌──────────────┬────────────────────────────────────────────┐ │
│  │   F1: Tree   │   F2: Editor                               │ │
│  │              │                                            │ │
│  │  src/        │   bring <module/std.io> as io              │ │
│  │  ├─ main.snsx│                                            │ │
│  │  └─ app/     │   entry                                    │ │
│  │    └─ ...    │       show "Hello"                         │ │
│  │              │       0                                    │ │
│  ├──────────────┼────────────────────────────────────────────┤ │
│  │  F3: Input   │   F4: Run Output                          │ │
│  │              │                                            │ │
│  │  > input     │   Hello                                    │ │
│  │              │                                            │ │
│  ├──────────────┼────────────────────────────────────────────┤ │
│  │  F5: Build   │   F6: Shell                               │ │
│  │              │                                            │ │
│  │  Compiled    │   snsx> run                                │ │
│  │  successfully│   snsx> build vm                           │ │
│  │              │                                            │ │
│  └──────────────┴────────────────────────────────────────────┘ │
│                                                                  │
│  Keyboard Shortcuts:                                            │
│  F1-F7: Focus areas  |  Ctrl+R: Run  |  Ctrl+B: Build          │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

### 15.2 Web/App Studio

The web/app studio provides:

- **Workspace Navigator**: Browse and manage files
- **Editor**: Syntax-highlighted code editing
- **Project Radar**: Visual project overview
- **Flow Map**: Function dependency visualization
- **Snippet Forge**: Code snippet management
- **Execution Deck**: Run/Build/Audit controls
- **Input & Terminal Panel**: Interactive I/O

---

## 16. CLI Reference

### 16.1 Commands

```bash
# Initialize a new SNSX project
snsx init my_project

# Run an SNSX program
snsx run
snsx run --file path/to/program.snsx
snsx run --trace
snsx run --deterministic
snsx run --stdin-text "input line"

# Build SNSX to various targets
snsx build --target vm
snsx build --target llvm
snsx build --target wasm
snsx build --target native-x86_64
snsx build --target native-aarch64

# Launch IDE
snsx studio
snsx start --mode terminal
snsx start --mode app
snsx start --mode web

# AI Coding Agent
snsx agent --prompt "write a vote eligibility checker"
snsx agent --prompt "build a calculator" --apply

# Deploy
snsx deploy --cluster local
```

### 16.2 SNSX Shell Commands

```
# Documentation
help, docs, manual, spec, techspec

# Agent
agent <prompt>
agent write <path> <prompt>

# Execution
run
build <target>
audit
doctor

# File Operations
open <path>
pwd, ls, tree, find, search, outline
touch, mkdir, mv, cp, rm

# Package & Module
package init/add/remove/list
module list/vendor/install/scaffold

# Manifest & Permissions
manifest
perm show/grant/revoke
```

---

## 17. Package Management

### 17.1 Project Manifest (snsx.toml)

```toml
[package]
name = "my_snsx_app"
version = "0.1.0"
entry = "src/main.snsx"

[permissions]
fs = false
net = false
ai = true

[dependencies]
std = "builtin"
"std.io" = "builtin"
"std.ai" = "builtin"
"std.design" = "builtin"
```

### 17.2 Dependency Resolution

```
┌─────────────────────────────────────────────────────────────────┐
│                    Dependency Resolution                         │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  1. Read snsx.toml manifest                                     │
│  2. Resolve "builtin" dependencies from stdlib registry         │
│  3. Resolve local module paths:                                 │
│     • bring <module/app.xxx> → src/app/xxx.snsx                │
│  4. Validate permissions against manifest                       │
│  5. Load and compile dependency graph                           │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

---

## 18. Examples & Tutorials

### 18.1 Hello World

```snsx
bring <module/std.io> as io

entry
    show "Hello, SNSX!"
    0
```

### 18.2 Vote Eligibility Checker

```snsx
bring <module/std.io> as io

flow can_vote
    takes age: Int
    gives Int
    gate age >= 18:
        give 1
    otherwise:
        give 0

entry
    show "Enter your age:"
    hold raw_age = ask()

    gate raw_age == "":
        show "Please enter your age."
    otherwise:
        gate is_int(raw_age):
            hold age = int(raw_age)
            gate can_vote(age) == 1:
                show "You are eligible to vote."
            otherwise:
                show "You are not eligible to vote."
        otherwise:
            show "Please enter a valid numeric age."

    0
```

### 18.3 Async Concurrency

```snsx
flow work
    takes x: Int
    gives Int
    x * 10

entry
    hold task = launch work(7)
    hold value = wait task
    show value
    0
```

### 18.4 AI-Native Function

```snsx
bring <module/std.io> as io
bring <module/std.ai> as ai

mind summarize
    takes text: String
    gives String
    from "Summarize this text"

entry
    hold text = "SNSX is a revolutionary programming language."
    hold summary = text |> summarize
    show summary
    0
```

### 18.5 User Detail Database

```snsx
bring <module/std.io> as io
bring <module/std.db> as db

flow database_path
    gives String
    "data/user_detail.snsxdb"

flow build_record
    takes name: String, mobile: String, email: String
    gives String
    "name=" + sanitize(name) + "|mobile=" + sanitize(mobile) + "|email=" + sanitize(email)

entry
    show "User Detail Database"
    show "Enter name:"
    hold name = ask()
    show "Enter mobile number:"
    hold mobile = ask()
    show "Enter email address:"
    hold email = ask()

    gate name == "":
        show "Please enter the name."
    otherwise:
        gate mobile == "":
            show "Please enter the mobile number."
        otherwise:
            gate is_digits(mobile):
                gate email == "":
                    show "Please enter the email address."
                otherwise:
                    gate is_email(email):
                        hold record = build_record(name, mobile, email)
                        hold saved_to = append_text(database_path(), record + "\n")
                        show "Record saved to:"
                        show saved_to
                    otherwise:
                        show "Please enter a valid email address."
            otherwise:
                show "Please enter a valid mobile number using digits only."

    0
```

---

## 19. Algorithms & Data Structures

### 19.1 Compilation Complexity

| Stage | Time Complexity | Space Complexity |
|-------|----------------|------------------|
| Lexer | O(n) | O(n) |
| Parser | O(n) | O(n) |
| Semantic Analysis | O(n log n) | O(n) |
| Security Audit | O(n) | O(n) |
| IR Lowering | O(n) | O(n) |
| Optimization | O(n log n) | O(n) |
| Code Generation | O(n) | O(n) |

Where n = source code length in tokens.

### 19.2 VM Execution Complexity

| Operation | Time Complexity |
|-----------|----------------|
| Push/Pop | O(1) |
| Arithmetic | O(1) |
| Function Call | O(f) where f = function body |
| Array Index | O(1) |
| String Concat | O(s) where s = string length |
| Task Spawn | O(1) |
| Task Await | O(t) where t = task execution |

### 19.3 Memory Management Algorithm

```
┌─────────────────────────────────────────────────────────────────┐
│                 Deterministic Reference Counting                │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  alloc(value):                                                  │
│      id = next_id++                                             │
│      cells[id] = HeapCell { refs: 1, value }                   │
│      return id                                                  │
│                                                                  │
│  retain(id):                                                    │
│      cells[id].refs += 1                                        │
│                                                                  │
│  release(id):                                                   │
│      if cells[id].refs > 1:                                     │
│          cells[id].refs -= 1                                    │
│      else:                                                      │
│          cells.remove(id)                                       │
│                                                                  │
│  Benefits:                                                       │
│  • Deterministic deallocation                                   │
│  • No GC pauses                                                 │
│  • Predictable memory usage                                     │
│  • Optional cycle collection hooks                              │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

---

## 20. Formal Semantics

### 20.1 AERIS Core Judgments

```
┌─────────────────────────────────────────────────────────────────┐
│                 AERIS Formal Judgments                           │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  Typing: Γ ⊢ e : T ! E                                         │
│  "In context Γ, expression e has type T with effect E"         │
│                                                                  │
│  Capability: Γ ; Cap ⊢ stmt ok                                 │
│  "In context Γ with capabilities Cap, statement is safe"       │
│                                                                  │
│  Refinement: Γ ⊢ p ⇐ T                                        │
│  "Pattern p matches type T in context Γ"                       │
│                                                                  │
│  Actor Step: Σ ⊢ actor_step → Σ'                               │
│  "Actor execution transforms state Σ to Σ'"                    │
│                                                                  │
│  Security Goals:                                                 │
│  • Progress: Well-typed programs don't get stuck               │
│  • Preservation: Well-typed programs stay well-typed           │
│  • Non-forgeability: Capabilities can't be forged              │
│  • Determinism: Actor reduction is deterministic               │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

### 20.2 Stage Breakdown

| Stage | Proofs | Status |
|-------|--------|--------|
| A | Simply typed core, effects, capabilities | Planned |
| B | Actor semantics, scheduling, isolation | Planned |
| C | Refinement predicates, verification | Planned |
| D | IR correspondence, SSA preservation | Planned |

---

## 21. Build System

### 21.1 Build Configuration

```toml
# Cargo.toml (workspace root)

[workspace]
members = [
    "compiler",
    "runtime",
    "vm",
    "stdlib",
    "ai_engine",
    "network_stack",
    "distributed_engine",
    "cli",
    "ide",
    "aeris_compiler",
    "aeris_runtime",
    "aeris_pkg",
    "aeris_fmt",
    "aeris_lint",
    "aeris_cli",
    "aeris_kernel_abi",
]
resolver = "2"

[workspace.package]
edition = "2021"
version = "0.1.0"
authors = ["Codex"]
license = "Apache-2.0"
```

### 21.2 Build Commands

```bash
# Build entire workspace
cargo build

# Build release
cargo build --release

# Run tests
cargo test

# Run specific package
cargo run -p snsx_cli -- run --file examples/hello.snsx

# Build with master script
./master.sh build
```

### 21.3 Dependencies

| Dependency | Version | Purpose |
|-----------|---------|---------|
| `serde` | 1.0 | Serialization |
| `serde_json` | 1.0 | JSON support |
| `clap` | 4.5 | CLI parsing |
| `anyhow` | 1.0 | Error handling |
| `thiserror` | 1.0 | Error types |
| `sha3` | 0.10 | Cryptographic hashing |
| `ed25519-dalek` | 2.1 | Digital signatures |
| `x25519-dalek` | 2.0 | Key exchange |
| `aes-gcm-siv` | 0.11 | Encryption |
| `crossterm` | 0.27 | Terminal I/O |
| `notify` | 6.1 | File watching |
| `toml` | 0.8 | TOML parsing |
| `walkdir` | 2.5 | Directory traversal |
| `base64` | 0.22 | Base64 encoding |
| `rand` | 0.8 | Random number generation |

---

## 22. Future Roadmap

### 22.1 SNSX Roadmap

```
┌─────────────────────────────────────────────────────────────────┐
│                    SNSX Development Roadmap                      │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  Stage 0 (Current - Bootstrap v0.1.0):                          │
│  ✅ VM-based execution                                          │
│  ✅ Multi-target compilation (LLVM, WASM, Native)               │
│  ✅ Strict security audit                                       │
│  ✅ Terminal & Web IDE                                          │
│  ✅ AI-native functions                                         │
│  ✅ Distributed engine skeleton                                 │
│                                                                  │
│  Stage 1 (Stabilization):                                       │
│  ◻ Enhanced type inference                                      │
│  ◻ Richer standard library                                      │
│  ◻ Package registry                                             │
│  ◻ IDE improvements                                             │
│  ◻ Performance optimization                                     │
│                                                                  │
│  Stage 2 (Self-Hosting):                                        │
│  ◻ SNSX-written compiler                                        │
│  ◻ SNSX-written runtime                                         │
│  ◻ Reduced trusted bootstrap base                               │
│                                                                  │
│  Stage 3 (Kernel-Native):                                       │
│  ◻ SNSX kernel ABI                                              │
│  ◻ Bare-metal execution                                         │
│  ◻ Hardware abstraction layer                                   │
│  ◻ Self-hosting OS foundation                                   │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

### 22.2 AERIS Roadmap

```
┌─────────────────────────────────────────────────────────────────┐
│                    AERIS Development Roadmap                     │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  Stage 0 (Current - Bootstrap):                                 │
│  ✅ Rust-hosted compiler                                        │
│  ✅ Basic type system with effects                              │
│  ✅ Algebraic data types                                        │
│  ✅ Actor primitives                                            │
│  ✅ Interpreter runtime                                         │
│                                                                  │
│  Stage 1 (Stabilization):                                       │
│  ◻ Ownership and borrowing                                     │
│  ◻ Linear resources                                            │
│  ◻ Effect polymorphism                                         │
│  ◻ Richer refinement solving                                   │
│                                                                  │
│  Stage 2 (Formal Verification):                                 │
│  ◻ Lean/Coq formalization                                      │
│  ◻ Progress & preservation proofs                              │
│  ◻ Capability non-forgeability proofs                          │
│  ◻ Verified compiler passes                                    │
│                                                                  │
│  Stage 3 (Self-Hosting):                                        │
│  ◻ AERIS-written compiler                                      │
│  ◻ Native code generation                                      │
│  ◻ Kernel integration                                          │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

---

## 23. Contributing

### 23.1 Development Setup

```bash
# Clone the repository
git clone <repository-url>
cd "Programming Lang"

# Install Rust (if not already installed)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Build the project
cargo build

# Run tests
cargo test

# Run an example
cargo run -p snsx_cli -- run --file examples/hello.snsx
```

### 23.2 Code Structure Guidelines

1. **Workspace Safety**: All file operations must be confined to workspace root
2. **Security First**: Route every run/build path through strict audit
3. **Clear Diagnostics**: Explain both cause and repair
4. **No Silent Permissions**: All permission changes must be explicit
5. **Aligned Behavior**: CLI, terminal studio, and web/app studio must behave consistently

### 23.3 Adding New Builtins

```rust
// In vm/src/lib.rs, add to call_builtin:

"my_builtin" => {
    let arg = args.first().ok_or_else(|| anyhow!("my_builtin expects one argument"))?;
    // Implementation
    Ok(Value::String(result))
}
```

### 23.4 Adding New Standard Library Modules

```
# Create stdlib/modules/mymodule.snsx

flow my_function
    takes x: Int
    gives Int
    x * 2
```

```rust
// In stdlib/src/lib.rs, register:
("std.mymodule", include_str!("../modules/mymodule.snsx")),
```

---

## 24. License

Copyright 2026 Satya Narayan Sahu, Tathoi Mondal

Licensed under the Apache License, Version 2.0 (the "License");
you may not use this file except in compliance with the License.
You may obtain a copy of the License at

    http://www.apache.org/licenses/LICENSE-2.0

Unless required by applicable law or agreed to in writing, software
distributed under the License is distributed on an "AS IS" BASIS,
WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
See the License for the specific language governing permissions and
limitations under the License.

---

<div align="center">

**Built with ❤️ by Satya Narayan Sahu & Tathoi Mondal**

*SNSX/AERIS — Where AI meets Systems Programming*

</div>