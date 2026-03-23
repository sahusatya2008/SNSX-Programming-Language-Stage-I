# SNSX Technical Specification

Version: Stage-0 bootstrap specification  
Status: Implemented bootstrap with VM-first execution, LLVM/WASM/native text emitters, strict security gate, terminal studio, and web/app studio

## 1. Purpose

SNSX, Smart Neural Syntax eXtended, is a programming language and toolchain designed to combine:

- text-like readability
- static structure
- strict security gating
- AI-native language constructs
- multi-target compilation
- integrated development tooling

This document describes the current implemented system in the repository, the execution model, the language surface, the security model, and the operational behavior of the compiler, runtime, virtual machine, shell, and IDE modes.

This is a technical specification for the software as implemented today. It is not a claim that the system is already self-hosted or kernel-native. The present SNSX stack is a hosted bootstrap system with a defined path for future reduction of the trusted bootstrap base.

## 2. Design Principles

SNSX is built around the following rules:

1. Source code should read more like structured instructions than conventional systems code.
2. Security blockers should stop execution before runtime whenever the compiler can prove risk.
3. The same workspace state should drive CLI, terminal studio, app studio, and web studio.
4. Input, output, and shell behavior should be explicit and inspectable.
5. File actions must remain confined to the active workspace root.
6. AI features must be first-class but still pass through the same compiler and security pipeline.

## 3. Implemented Scope

The current repository contains the following major SNSX subsystems:

- `compiler/`: lexer, parser, AST, semantic analysis, diagnostics, strict audit, IR lowering, target compilation
- `runtime/`: runtime context, sandbox policy, I/O state, deterministic clock support
- `vm/`: executable VM backend for the current bootstrap stage
- `stdlib/`: built-in module registry and vendorable stdlib source modules
- `ide/`: terminal studio, web/app studio, SNSX shell, workspace tools
- `cli/`: `snsx` command-line interface
- `network_stack/`: SNSP protocol-oriented networking skeleton
- `distributed_engine/`: distributed orchestration skeleton
- `ai_engine/`: AI-native execution abstraction layer
- `examples/`: runnable language examples

## 4. Trust Boundary

### 4.1 Current Reality

SNSX is currently hosted. The compiler, runtime, IDE, and VM are implemented in Rust for bootstrap purposes.

### 4.2 What Is Native Today

Native SNSX concerns that are already part of the language/toolchain contract:

- SNSX source syntax
- SNSX diagnostics
- SNSX strict security audit rules
- SNSX VM bytecode-style execution path
- SNSX package and module workflow
- SNSX shell commands and workspace confinement rules

### 4.3 What Is Not Yet Native

The following are not yet fully SNSX-written:

- the compiler implementation
- the runtime implementation
- the IDE frontend/backend implementation
- a kernel or bare-metal execution environment
- a self-hosting bootstrap chain

## 5. Repository-Level Architecture

### 5.1 Primary Source Directories

- `compiler/src/lexer.rs`: tokenization
- `compiler/src/parser.rs`: syntax parsing
- `compiler/src/ast.rs`: AST definitions
- `compiler/src/semantic.rs`: builtin typing, type checks, callable resolution
- `compiler/src/security.rs`: strict static security rules
- `compiler/src/diag.rs`: diagnostics formatting
- `compiler/src/ir.rs`: IR definitions and lowering helpers
- `runtime/src/lib.rs`: runtime state, stdout, stdin, sandbox policy, deterministic support
- `vm/src/lib.rs`: VM execution engine and builtin implementations
- `ide/src/studio.rs`: terminal studio
- `ide/src/web.rs`: web/app studio
- `ide/src/shell.rs`: SNSX shell parsing and workspace operations
- `cli/src/main.rs`: CLI command dispatch

### 5.2 Shared Execution Contract

Every SNSX execution mode shares the same high-level sequence:

1. resolve entry file
2. resolve workspace security policy
3. load source
4. run strict audit
5. compile for requested target or compile-to-VM
6. execute if allowed
7. present output, diagnostics, artifacts, or blockers

## 6. Language Surface

### 6.1 Source Files

- SNSX source files use the `.snsx` extension.
- A project entry file is usually `src/main.snsx`.
- Workspace metadata lives in `snsx.toml`.

### 6.2 Import Style

The preferred import syntax is:

```snsx
bring <module/std.io> as io
bring <module/std.ai> as ai
bring <lib/std.design> as design
bring <package/std.error> as errors
```

Compatibility parsing still accepts some older bootstrap forms, but the bring-first style is the primary language surface.

### 6.3 Program Entry

The program root is declared with `entry`.

```snsx
entry
    show "Hello"
    0
```

The final expression in `entry` becomes the return value.

### 6.4 Value Binding

SNSX uses `hold` for immutable-like value introduction.

```snsx
hold name = ask()
hold count = 10
```

Mutation-oriented forms exist elsewhere in the language, but the current visible bootstrap surface emphasizes `hold`.

### 6.5 Output

SNSX uses `show`.

```snsx
show "Ready"
show count
```

### 6.6 Input

SNSX uses `ask()`.

```snsx
hold name = ask()
```

Operational behavior:

- CLI `snsx run` reads from real terminal stdin when no explicit stdin override is given.
- terminal studio prompts for one line of input every time a source file containing `ask()` is run
- web/app studio prompts for one line of input every time a source file containing `ask()` is run
- entered input is reflected back into the program input panel for inspection and reuse

### 6.7 Conditional Control

SNSX uses `gate` and `otherwise`.

```snsx
gate age >= 18:
    show "Eligible"
otherwise:
    show "Not eligible"
```

### 6.8 Validation

SNSX uses `guard` for contract-style assertions.

```snsx
guard name != "", "Name is required"
```

`guard` is a hard runtime stop. When user-facing recovery is preferred, `gate` should be used instead.

### 6.9 Reusable Flow Blocks

SNSX uses `flow` for callable reusable logic.

```snsx
flow add
    takes a: Int, b: Int
    gives Int
    a + b
```

### 6.10 AI-Native Blocks

SNSX uses `mind` for prompt-bound AI declarations.

```snsx
mind summarize
    takes text: String
    gives String
    from "Summarize this text"
```

### 6.11 Pipeline Operator

SNSX supports `|>` for readable chained flow.

```snsx
"Some text" |> summarize |> show
```

## 7. Lexical and Parsing Model

### 7.1 Lexer Responsibilities

The lexer recognizes:

- identifiers
- keywords
- string literals
- numeric literals
- punctuation and delimiters
- operators, including pipeline forms

### 7.2 Parser Responsibilities

The parser builds the structured AST for:

- import declarations
- entry blocks
- flow blocks
- mind blocks
- variable bindings
- expressions
- conditional branches
- calls
- pipeline expressions

### 7.3 Parsing Style

The parser accepts the SNSX structured style rather than copying C-family grammar directly. The current syntax is indentation-friendly but still token-driven internally.

## 8. Semantic Layer

### 8.1 Responsibilities

The semantic layer performs:

- symbol resolution
- builtin recognition
- callable validation
- type inference for supported bootstrap constructs
- return and declaration validation
- compiler-facing shape validation before IR lowering

### 8.2 Current Builtins

Implemented bootstrap builtins include language-facing utilities such as:

- `show`
- `ask`
- `int`
- `float`
- `is_int`
- design and formatting helpers added by the SNSX power-style surface

Builtin availability is enforced by the semantic layer and executed by the VM layer.

## 9. Strict Security Gate

### 9.1 Goal

SNSX strict mode blocks execution and build output when statically detectable unsafe patterns are present.

### 9.2 Current Enforced Areas

The current strict audit checks include patterns such as:

- direct unsafe handling of raw input
- undeclared AI usage
- undeclared filesystem usage
- undeclared networking usage
- unbounded obvious loop hazards
- contract gaps on callable surfaces

### 9.3 Output Form

Diagnostics are emitted with:

- a rule code
- a plain-language error
- `why:` explanation
- `fix:` guidance

### 9.4 Runtime Policy Label

The standard policy label is:

`strict | fs <on/off> | net <on/off> | ai <on/off>`

### 9.5 Execution Contract

If strict audit reports blockers:

- `run` is blocked
- `build` is blocked
- IDE status changes to a blocked state
- the Security console becomes the primary output target

## 10. Runtime Model

### 10.1 Runtime Context

The runtime context manages:

- sandbox policy
- stdout accumulation
- stdin queue snapshot
- optional host stdin passthrough
- deterministic clock state

### 10.2 Standard Input Behavior

Runtime input works in this order:

1. consume queued stdin lines if present
2. if allowed, read a line from host terminal stdin
3. otherwise return empty input

This is how CLI mode and IDE modes share the same runtime while still handling input differently.

### 10.3 Standard Output Behavior

Output is accumulated in-memory as line-oriented text and returned with the VM execution result.

### 10.4 Deterministic Support

A deterministic clock is available to make certain runtime behaviors reproducible during testing or restricted execution modes.

## 11. VM Execution Model

### 11.1 Role of the VM

The SNSX VM is the primary executable backend in the current bootstrap stage.

### 11.2 VM Responsibilities

- execute lowered SNSX functions
- invoke builtins
- manage runtime context
- produce stdout
- return final program value
- optionally emit trace output

### 11.3 Backend Truth

The VM is the currently real runtime backend. Other targets are emitted artifacts, but the VM is the present production execution path in this repository.

## 12. Compiler Pipeline

### 12.1 Implemented Pipeline

The compiler runs the following stages:

1. lexing
2. parsing
3. AST build
4. semantic analysis
5. strict audit
6. IR construction
7. target artifact emission

### 12.2 Compilation Targets

Supported target labels include:

- `vm`
- `wasm`
- `llvm`
- `native-x86_64`
- `native-aarch64`

### 12.3 Artifact Expectations

Current artifact forms are bootstrap-stage textual or structured outputs:

- VM artifact
- WebAssembly text
- LLVM IR text
- native assembly text

These targets are integrated into the toolchain and build flow, but the VM remains the authoritative execution backend.

## 13. IR Model

### 13.1 Design Direction

SNSX uses an SSA-oriented internal compilation model.

### 13.2 Current Responsibilities

The IR layer exists to:

- decouple parsing from backends
- support artifact emission
- provide counts and summaries in the build output
- create a target-independent lowering stage

### 13.3 Output Observability

Build output reports function, block, and value counts so developers can inspect the compilation shape without opening raw IR files manually.

## 14. Standard Library

### 14.1 Delivery Model

The SNSX stdlib is a vendorable registry-backed source library set.

### 14.2 Current Categories

Implemented registry surfaces include modules such as:

- `std.io`
- `std.ai`
- `std.math`
- `std.design`
- `std.error`
- `std.sentinel`
- `std.net`
- `std.web`

### 14.3 Installation Model

Modules can be:

- listed
- vendored into the current workspace
- tracked in `snsx.toml`
- scaffolded into workspace-owned source paths

## 15. Package and Manifest Model

### 15.1 Manifest File

Workspace metadata is stored in `snsx.toml`.

### 15.2 Manifest Concerns

The manifest stores:

- package metadata
- entry path
- permissions
- dependencies

### 15.3 Permission Model

Permissions are explicit and per-workspace:

- filesystem
- network
- AI

They are surfaced both to the compiler audit and to the IDE.

## 16. SNSX Shell

### 16.1 Purpose

The SNSX shell is a workspace-aware control surface embedded in the terminal studio and the web/app studio.

### 16.2 Core Commands

- `help`
- `docs`
- `manual`
- `spec`
- `techspec`
- `history`
- `agent <prompt>`
- `agent write <path> <prompt>`
- `run`
- `build <target>`
- `audit`
- `doctor`
- `open <path>`
- `pwd`
- `ls`
- `tree`
- `find`
- `search`
- `outline`
- `touch`
- `mkdir`
- `mv`
- `cp`
- `rm`
- `entry`
- `package ...`
- `module ...`
- `snippet ...`
- `manifest`
- `perm ...`

### 16.3 Host Shell Escape

The IDE shell supports guarded host-shell escape using:

- `shell <cmd>`
- `!<cmd>`

### 16.4 Host Shell Guard

The host shell path is intentionally restricted. The current terminal guard blocks obviously destructive host-shell prefixes such as:

- `rm`
- `sudo`
- `dd`
- `mkfs`
- `shutdown`
- `reboot`
- `kill`
- `killall`
- `pkill`

The intended rule is:

- use SNSX workspace commands for workspace mutations
- use guarded host shell only for inspection or non-destructive helper tasks

### 16.5 Shell History

Both terminal studio and web/app studio keep shell history for the current session and expose it via the `history` command.

### 16.6 SNS AI Coding Agent

The SNS AI Coding Agent is a local prompt-to-SNSX generator. It does not require an external model service for baseline generation.

Current behavior:

- classifies the prompt into a supported recipe
- generates valid SNSX source using built-in templates and prompt-aware titles
- suggests a file path
- returns dependency hints
- can write generated code directly into the workspace through shell or UI actions

Current recipe families include:

- vote eligibility
- grade checker
- text summary
- design board
- access guard
- input echo
- generic starter

## 17. Terminal Studio

### 17.1 Layout Model

The terminal studio presents:

- file tree
- editor
- output deck
- program input panel
- SNSX shell panel

### 17.2 Output Tabs

The output deck supports:

- Run
- Build
- Security
- Terminal

### 17.3 Keyboard Focus

Terminal studio focus areas:

- `F1` tree
- `F2` editor
- `F3` input
- `F4` run output
- `F5` build output
- `F6` shell
- `F7` security

### 17.4 Shell and Input Behavior

Terminal studio now prompts for one line of input on every run when the entry source contains `ask()`. The entered value is mirrored into the input panel.

### 17.5 Workspace Safety

File creation, deletion, rename, move, entry switching, and shell file commands are constrained to the active workspace root.

## 18. Web and App Studio

### 18.1 Architecture

The web/app studio is a local HTTP server with a browser-delivered interface and a shared backend that performs file, compile, build, audit, and shell operations.

### 18.2 Shared Backend Rules

The web/app studio shares the same:

- compiler
- audit
- workspace confinement
- package/module operations
- build pipeline
- shell parser

### 18.3 User Interface Areas

The web/app studio includes:

- workspace navigator
- workspace tree
- editor
- project radar
- flow map
- snippet forge
- execution deck
- input and SNSX terminal panel

### 18.4 Input Behavior

If a program contains `ask()`, the web/app studio prompts for input on every run and writes the entered line into the program input panel.

### 18.5 Documentation Surfaces

The web/app studio exposes local documentation routes:

- `/docs/manual`
- `/docs/spec`
- `/docs/techspec`
- `/docs/readme`

### 18.6 Web/App SNS AI Agent Panel

The web/app studio includes a dedicated SNS AI Coding Agent panel with:

- prompt textarea
- target file input
- preview action
- apply action

The preview path returns generated SNSX code and recipe metadata. The apply path writes the generated code into the target file, refreshes the workspace tree, and opens the generated file in the editor.

## 19. CLI

### 19.1 Common Commands

- `snsx init`
- `snsx run`
- `snsx build`
- `snsx deploy`
- `snsx agent --prompt "..."`
- `snsx studio`
- `snsx start --mode terminal`
- `snsx start --mode app`
- `snsx start --mode web`

### 19.2 Standard Input Contract

CLI `run` accepts:

- real terminal stdin
- `--stdin-text`
- `--stdin-file`

Real terminal stdin is enabled when the command is run normally without explicit stdin override and outside watch mode.

## 20. Diagnostics and Error Reporting

### 20.1 Compiler Diagnostics

Compiler diagnostics aim to be:

- explicit
- source-oriented
- fix-oriented

### 20.2 Security Diagnostics

Security diagnostics explain:

- what is blocked
- why it is blocked
- how it should be fixed

### 20.3 IDE Presentation

Errors are shown in:

- run output
- build output
- security output
- shell output
- status banner

depending on the stage that failed.

## 21. Build Artifacts

### 21.1 Output Directory

Artifacts are typically placed under an entry-local `dist/` directory.

### 21.2 Artifact Examples

- `.snsxbc`
- `.wat`
- `.ll`
- `.x86_64.s`
- `.aarch64.s`

### 21.3 Build Blocking Rule

Artifacts are not emitted if the strict security audit fails.

## 22. Networking and Distributed Work

SNSX includes repository-level architecture for:

- SNSP networking
- distributed execution
- AI engine orchestration

The current implemented language runtime path is still VM-first, but the repository and module structure already reserves these concerns as first-class subsystems.

## 23. Security Architecture Summary

SNSX security is layered:

1. syntax and semantic validation
2. explicit permissions in `snsx.toml`
3. strict audit blockers before execution
4. workspace path confinement in IDE and shell actions
5. guarded host shell execution
6. sandbox policy wired into runtime context

## 24. Performance Model

### 24.1 Current Stage

The primary execution backend is the SNSX VM. Performance work today should be evaluated relative to the VM path and compiler frontend responsiveness.

### 24.2 Future Direction

Future work is expected to improve:

- lowering quality
- optimization passes
- native backend completeness
- self-hosting progression

## 25. Technical Limits

The following are important current limits:

- SNSX is not yet self-hosted
- SNSX is not yet kernel-native
- LLVM/WASM/native outputs are bootstrap artifacts rather than full machine-native end-to-end execution backends
- the host language remains part of the trusted bootstrap base
- the host-shell guard is intentionally simple and conservative, not a full formal shell sandbox

## 26. Recommended Developer Workflow

### 26.1 CLI Workflow

1. create or open a workspace
2. edit `src/main.snsx`
3. run `snsx run`
4. fix diagnostics
5. run `snsx build`

### 26.2 Terminal Studio Workflow

1. open terminal studio
2. edit the entry file
3. run audit first
4. run the program
5. use the SNSX shell for file, package, module, and manifest operations

### 26.3 Web/App Studio Workflow

1. start app or web mode
2. edit source
3. use the execution deck for run/build/audit
4. use the input and terminal panel for input, shell commands, and workspace tasks
5. open manual/spec/tech spec links as needed

## 27. Extension Guidelines

Developers extending SNSX should preserve these invariants:

1. keep workspace actions root-confined
2. route every new run/build path through strict audit
3. make diagnostics explain both cause and repair
4. avoid adding silent permissions
5. keep CLI, terminal studio, and web/app studio behavior aligned
6. document new builtins, shell commands, modules, and permissions in both user docs and technical docs

## 28. Required Files for Future Major Milestones

To move SNSX closer to a lower-level and more independent execution stack, the next milestones should produce:

- a defined SNSX object format
- a native executable loader
- a real optimizing native backend
- an SNSX-written compiler stage
- a verified runtime ABI
- a reduced trusted bootstrap chain

## 29. Conclusion

SNSX today is a hosted, strict, VM-first language ecosystem with:

- its own source language surface
- its own security audit model
- its own workspace shell
- multi-mode IDE operation
- explicit input prompting across run modes
- integrated docs, packages, modules, and build targets

This specification should be read as the engineering contract for the present implementation and the base reference for future expansion.
