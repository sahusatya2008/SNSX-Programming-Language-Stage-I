# AERIS Language Specification (Bootstrap Stage)

## Overview

AERIS is a capability-secure systems language designed around:

- explicit effects
- explicit capabilities
- algebraic data types
- deterministic actors
- compile-time refinement checks
- SSA-oriented compilation

The bootstrap implementation is stage-0: it is hosted in Rust today so AERIS can grow into a self-hosted stage-1 compiler without pretending that goal is already complete.

## Source Files

- File extension: `.ae`
- Package manifest: `Aeris.toml`
- Entry point: `fn main(...) -> Int !effect { ... }`

## Core Syntax

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

## Design Rules

- Every function declares an effect.
- IO requires explicit capability tokens.
- Actor primitives are isolated to `!actor` functions.
- Refinements may be attached to parameters with `where`.
- No implicit globals exist in the language.

## Built-in Effects

- `pure`
- `io`
- `actor`
- `state`

## Built-in Capabilities

- `Console`
- `Clock`
- `FileSystem`
- `Network`

## Built-in Types

- `Int`
- `Bool`
- `Text`
- `Unit`
- `ActorHandle`

## Built-in Functions

- `print(console, value)` -> `Unit`
- `read_line(console)` -> `Text`
- `assert(condition, message)` -> `Unit`
- `spawn(actor_factory)` -> `ActorHandle`
- `emit(handle, message)` -> `Unit`
- `drain()` -> `Int`

## Type System Direction

The bootstrap compiler already models:

- explicit capabilities in types
- effect tracking
- algebraic data types
- basic refinement clauses

The full AERIS roadmap extends this into:

- ownership and borrowing
- linear resources
- effect polymorphism
- richer refinement solving
- verified capability propagation

## Security Model

- Capabilities must be passed explicitly.
- Built-in IO requires capability tokens.
- No ambient authority is granted by default.
- Actor execution is deterministic FIFO.

## Compilation Pipeline

1. lex
2. parse
3. semantic analysis
4. refinement/effect/capability validation
5. SSA-style IR lowering
6. interpretation in the stage-0 runtime

## Current Bootstrap Boundaries

- ownership/borrow checking is specified but not fully implemented yet
- refinement solving is intentionally narrow in stage 0
- deterministic actors are implemented as a single-process FIFO runtime
- native machine code generation is planned after stage-0 stabilization
