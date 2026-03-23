#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUT_FILE="${ROOT_DIR}/docs/SNSX_Training_Manual.md"

mkdir -p "$(dirname "${OUT_FILE}")"

cat > "${OUT_FILE}" <<'EOF'
# SNSX Training Manual

This manual teaches a new developer how to code in SNSX from the ground up.

It is written as a course, not as a marketing document.

The goal is simple:

- understand how SNSX thinks
- write real `.snsx` files
- use modules, libraries, and frameworks correctly
- pass the strict compiler and security audit
- build professional projects with confidence

---

## 1. How To Use This Manual

Read this manual in order the first time.

After that, treat it like a working handbook:

- return to the syntax chapters when writing code
- return to the security chapter when the compiler blocks a run
- return to the studio and shell chapter when working inside the IDE
- return to the project chapters when building a real SNSX app

You do not need to memorize everything.

You do need to understand the model behind the language.

SNSX is easy to read, but it is strict on purpose.

---

## 2. What SNSX Is

SNSX stands for Smart Neural Syntax eXtended.

SNSX is built around a few core ideas:

1. source code should read like intention
2. security should be part of compilation
3. AI should be a native feature, not a bolt-on
4. modules and permissions should stay visible
5. simple code should stay simple, but advanced code should remain possible

SNSX is not designed to look like C, Rust, or JavaScript.

That is why the core words are different:

- `entry` instead of a traditional `main`
- `flow` instead of a classic function keyword
- `mind` for AI-backed flows
- `hold` and `set` for state
- `gate`, `unless`, `pick` for control logic
- `guard` and `fail` for contract-driven safety

SNSX uses indentation. Four spaces per level is the standard.

Comments begin with `#`.

Example:

```snsx
entry
    show "SNSX started"
    0
```

---

## 3. What A Beginner Needs Before Coding SNSX

Before you begin a real SNSX project, make sure you understand these five things:

### 3.1 Source layout

You should know where the main file lives and where the manifest lives.

### 3.2 Imports

You should know how to bring modules into a file.

### 3.3 Contracts

You should know that reusable flows need clear `takes` and `gives` sections.

### 3.4 Permissions

You should know that file access, network access, and AI access are controlled by `snsx.toml`.

### 3.5 Audit-first workflow

You should expect the compiler to stop you when the code crosses a security rule.

That is not friction.

That is the language doing its job.

---

## 4. The Shape Of An SNSX Project

A normal SNSX workspace looks like this:

```text
my_app/
├── snsx.toml
├── src/
│   └── main.snsx
├── vendor/
│   └── modules/
└── dist/
```

### 4.1 `snsx.toml`

This is the project manifest.

It tells SNSX:

- the package name
- the entry file
- the version
- the project permissions
- dependencies installed into the workspace

Example:

```toml
[dependencies]
std = "builtin"

[package]
entry = "src/main.snsx"
name = "hello_snsx"
version = "0.1.0"

[permissions]
ai = true
fs = false
net = false
```

### 4.2 `src/main.snsx`

This is the default entry file.

If you start with `snsx init my_app`, this is where you begin coding.

### 4.3 `vendor/modules`

Installed modules can be vendored here.

This is useful when your project uses SNSX shell commands such as:

- `module install std.design`
- `module scaffold ui/card`

### 4.4 `dist`

This is where build output can be written.

Examples:

- VM artifacts
- WAT output for WebAssembly
- deploy payloads

---

## 5. Your First SNSX Program

Start with the smallest useful program.

```snsx
need std.io

entry
    show "hello from snsx"
    0
```

### 5.1 What this does

- `need std.io` imports the standard I/O module
- `entry` declares the app root
- `show` prints a value
- `0` is the program result

### 5.2 How to run it

From the CLI:

```bash
snsx run
```

Or with an explicit file:

```bash
snsx run --file src/main.snsx
```

### 5.3 Beginner lesson

An SNSX program usually reads top-to-bottom:

1. imports
2. reusable flows
3. entry logic

That structure is worth keeping even in larger projects.

---

## 6. Reading SNSX Syntax

SNSX is easier to learn when you see it as a sentence-like language.

### 6.1 Import styles

#### `need`

Use `need` when you want a direct module import.

```snsx
need std.io
need std.ai
```

#### `bring ... as ...`

Use `bring` when you want a shorter alias.

```snsx
bring std.io as io
```

#### `bundle`

Use `bundle` when you want to import several modules in one line.

```snsx
bundle std.ai, std.design, std.sentinel, std.error
```

### 6.2 Program roots

Use `entry` for the application root.

```snsx
entry
    show "booted"
    0
```

### 6.3 Bindings

Use `hold` to create a new binding.

```snsx
hold name = "SNSX"
```

Use `set` to update an existing binding.

```snsx
set counter = counter + 1
```

### 6.4 Output and input

Use `show` to display output.

```snsx
show "ready"
```

Use `ask()` to read one line of input.

```snsx
hold name = ask()
```

### 6.5 Flow declarations

Use `flow` for reusable deterministic logic.

```snsx
flow add
    takes a: Int, b: Int
    gives Int
    a + b
```

### 6.6 AI declarations

Use `mind` for an AI-native flow.

```snsx
mind summarize
    takes text: String
    gives String
    from "Summarize this text"
```

### 6.7 Piping

Use `|>` to pass data through stages.

```snsx
"hello" |> summarize |> show
```

This style is important in SNSX.

It helps keep transformation pipelines readable.

---

## 7. Values And Types

SNSX is strongly typed.

Even when the language feels simple, the compiler expects clear type boundaries.

Common types:

- `Int`
- `Float`
- `Bool`
- `String`
- `Bytes`
- `Unit`
- `Tensor`
- `Task[T]`
- `Array[T]`

### 7.1 Basic values

```snsx
hold a = 10
hold b = 3.14
hold c = true
hold d = "snsx"
hold e = [1, 2, 3]
```

### 7.2 Explicit typing

You can make type intent visible:

```snsx
hold title: String = "Course"
hold ready: Bool = true
```

### 7.3 Why type clarity matters in SNSX

In strict SNSX, types are not only about correctness.

They also help with:

- security review
- API boundaries
- ownership expectations
- auditability

### 7.4 Professional guidance

For small local bindings, inference is fine.

For reusable flows and public boundaries, be explicit.

That is the SNSX way.

---

## 8. Working With Variables And State

Use `hold` for creation and `set` for mutation.

Example:

```snsx
entry
    hold count = 0
    set count = count + 1
    show count
    0
```

### 8.1 When to mutate

Mutation is allowed, but use it intentionally.

Prefer a new binding when it makes the data story clearer.

Bad:

```snsx
hold text = ask()
set text = text + "!"
show text
```

This is not always wrong, but it can hide where data changed.

Clearer:

```snsx
hold raw = ask()
hold excited = raw + "!"
show excited
```

### 8.2 Naming guidance

Good SNSX names explain trust and meaning:

- `raw`
- `sanitized`
- `summary`
- `task`
- `board`

Weak names make reviews harder:

- `x`
- `temp`
- `data`

Use short names only when the scope is tiny and obvious.

---

## 9. Conditions And Branching

SNSX offers several branching styles.

Use the one that best matches the intent.

### 9.1 `gate`

Use `gate` for a direct, policy-like branch.

```snsx
entry
    hold score = 72
    gate score > 50:
        show "pass"
    otherwise:
        show "retry"
    0
```

### 9.2 `unless`

Use `unless` when the negative condition is easier to read than its inverse.

```snsx
entry
    hold name = "snsx"
    unless name == "":
        show name
    0
```

### 9.3 `pick` and `case`

Use `pick` when one value drives several outcomes.

```snsx
flow mood
    takes score: Int
    gives String
    pick score
        case 0
            give "cold"
        case 1
            give "warm"
        else
            give "electric"
```

### 9.4 Choosing the right style

Use:

- `gate` for yes/no policy
- `unless` for readable inversion
- `pick` for value dispatch

This is an important SNSX style choice.

Good developers choose the form that makes review easy.

---

## 10. Loops

Use `during` for looping.

```snsx
entry
    hold count = 0
    during count < 3:
        show count
        set count = count + 1
    0
```

### 10.1 Strict compiler rule

Do not write unbounded `during true`.

In strict mode, the compiler blocks it with `SNSX-SEC-020`.

Reason:

- an always-true loop can hide denial-of-service behavior
- it is hard to audit safely

Good:

```snsx
during count < 10:
    set count = count + 1
```

Bad:

```snsx
during true:
    show "never stops"
```

### 10.2 Loop advice

Every loop should answer two review questions:

1. what makes it continue?
2. what makes it stop?

If those answers are not obvious, rewrite it.

---

## 11. Flows: The Core Of Reusable SNSX Code

Most reusable code in SNSX is written as a `flow`.

Example:

```snsx
flow add
    takes a: Int, b: Int
    gives Int
    a + b
```

### 11.1 The role of `takes`

`takes` describes inputs.

This matters for both readability and security review.

### 11.2 The role of `gives`

`gives` describes output.

In strict SNSX, reusable flows should declare what they return.

If you skip this, the compiler can block the project with `SNSX-SEC-011`.

### 11.3 Return styles

SNSX supports implicit tail return in structured flows:

```snsx
flow double
    takes x: Int
    gives Int
    x * 2
```

You can also use `give` explicitly:

```snsx
flow label
    takes value: Int
    gives String
    gate value > 10:
        give "big"
    otherwise:
        give "small"
```

### 11.4 What makes a good flow

A good flow:

- has a clear name
- has typed inputs
- has a declared output
- does one job
- is easy to test mentally

### 11.5 Beginner practice

Write these three flows on your own:

1. `square`
2. `is_even`
3. `greet`

If you can write those comfortably, you understand the basic SNSX flow model.

---

## 12. AI With `mind`

`mind` is one of the biggest differences in SNSX.

It lets AI-backed behavior live inside the language model instead of pretending AI is just another REST call.

Example:

```snsx
need std.ai

mind summarize
    takes text: String
    gives String
    from "Summarize this text in one sentence"
```

Then use it:

```snsx
entry
    hold summary = "SNSX is readable, safe, and AI-native." |> summarize
    show summary
    0
```

### 12.1 Why `mind` matters

It gives the compiler and runtime a clear signal:

- this flow is AI-backed
- this project needs AI permission
- prompts are part of the source contract

### 12.2 Permission rule

If AI is disabled in `snsx.toml`, the compiler can block:

- importing `std.ai`
- declaring AI flows
- calling AI flows

Relevant rules:

- `SNSX-SEC-002`
- `SNSX-SEC-012`
- `SNSX-SEC-033`

### 12.3 Professional advice for `mind`

Treat prompts like API design.

Good prompt flows:

- have a narrow purpose
- accept clear typed input
- return one predictable shape

Weak prompt flows:

- do too many jobs
- hide business rules in vague prompt text
- return content with no validation strategy

### 12.4 Sample AI program

```snsx
need std.io
need std.ai

mind summarize
    takes text: String
    gives String
    from "Summarize this text"

entry
    hold raw = "Smart Neural Syntax eXtended is readable, safe, and AI-native."
    hold summary = raw |> summarize
    show summary
    0
```

---

## 13. Input, Output, And Safer Data Handling

The simplest I/O program is:

```snsx
need std.io

entry
    hold name = ask()
    show "hello " + name
    0
```

But in strict SNSX, direct raw input output can be blocked.

### 13.1 Why the compiler blocks raw input

The rule is `SNSX-SEC-030`.

The compiler blocks direct display of raw user input because:

- secrets can leak
- tokens can leak
- unsafe payloads can be echoed

### 13.2 Safer pattern

Keep raw input separate from reviewed output.

```snsx
need std.io

entry
    hold raw = ask()
    guard raw != "", "name cannot be blank"
    hold safe_name = raw
    show "hello " + safe_name
    0
```

### 13.3 Core lesson

Whenever data enters the system:

1. name it honestly
2. validate it
3. transform it if needed
4. only then display or send it

This pattern appears again and again in professional SNSX code.

---

## 14. Guarding And Failing

SNSX uses `guard` and `fail` to make contracts visible.

### 14.1 `guard`

Use `guard` when a condition must stay true.

```snsx
guard raw != "", "raw input cannot be blank"
```

This is better than silently continuing with broken state.

### 14.2 `fail`

Use `fail` when execution must stop with an explicit reason.

```snsx
fail "board rendering failed"
```

### 14.3 When to use which

Use `guard` when:

- a precondition should be checked
- a result must meet a condition
- a branch would otherwise become messy

Use `fail` when:

- the program has reached an invalid state
- there is no meaningful recovery path
- you want the stop to be explicit and reviewable

### 14.4 Beginner principle

Do not hide errors.

Make the boundary visible.

That is one of the deepest SNSX habits.

---

## 15. Modules, Libraries, Frameworks, And Packages

A new SNSX developer should know the difference between these words.

### 15.1 Module

A module is an importable source unit.

Examples:

- `std.io`
- `std.ai`
- `std.design`
- `std.sentinel`

### 15.2 Library

A library is a collection of reusable modules.

In practice, the standard library is the first library you will use.

### 15.3 Framework

In SNSX, a framework is a more opinionated set of modules that guide a style of building.

Examples:

- `std.design` for UI-like composition
- `std.error` for contract-style failure patterns
- `std.sentinel` for tracing, sealing, and inspection

### 15.4 Package

A package is a project dependency tracked in the manifest and often installed into the workspace.

You manage packages with the SNSX shell.

### 15.5 Import examples

```snsx
need std.io
bring std.io as io
bundle std.ai, std.design, std.sentinel
```

### 15.6 Shell examples

```text
module list
module install std.design
package list
package add secure-core
```

### 15.7 How to think about reuse in SNSX

Ask these questions:

1. is this just a helper flow?
2. should this become a module?
3. does this belong in a framework layer?
4. does it change permissions?

That final question matters more in SNSX than in many other ecosystems.

---

## 16. Built-In Functions And Framework-Style Helpers

SNSX includes built-ins that are meant to support AI-native and design-oriented workflows.

The most important ones right now are:

- `watch`
- `shape`
- `seal`
- `repair`
- `blend`
- `draft`
- `panel`
- `stack`
- `field`
- `pulse`

### 16.1 `watch`

Use `watch` when you want a value to stay visible in the reasoning path.

```snsx
hold seen = watch("raw", raw)
```

### 16.2 `shape`

Use `shape` when you want structural information about a value.

```snsx
hold kind = shape(summary)
```

### 16.3 `seal`

Use `seal` when you want a value represented as a sealed or reviewable signature.

```snsx
hold mark = seal(summary)
```

### 16.4 `repair`

Use `repair` when you want a fallback strategy.

```snsx
hold stable = repair(primary, fallback)
```

### 16.5 `blend`

Use `blend` to join values into one composed form.

```snsx
hold line = blend(["alpha", "beta", "gamma"], " / ")
```

### 16.6 `panel`, `stack`, `field`, `draft`

These helpers make structured design composition possible.

```snsx
hold card = panel("Summary", summary)
hold meta = stack([field("shape", shape(summary)), field("seal", seal(summary))])
hold board = draft("SNSX Board", stack([card, meta]))
```

This is one of the distinctive styles in SNSX.

The code reads more like a design description than a low-level widget construction sequence.

---

## 17. A Real Framework-Style SNSX Program

Study this program carefully.

It combines imports, guard rails, AI, and design composition.

```snsx
bring std.io as io
bundle std.ai, std.design, std.sentinel, std.error

mind summarize
    takes text: String
    gives String
    from "Summarize this text in one sentence"

entry
    hold raw = "SNSX now supports its own import style, guard rails, and design composition."
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
```

### 17.1 What to learn from it

- imports stay visible
- raw input is named honestly
- contracts are explicit
- AI is isolated in a `mind`
- composition reads like intention
- failure states are visible

This is a good model for professional SNSX style.

---

## 18. Concurrency And Parallel Work

SNSX supports concurrent work without forcing low-level thread handling into every program.

The simple model uses `launch` and `wait`.

Example:

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

### 18.1 What is happening

- `launch` starts async work
- the result becomes a task
- `wait` joins the task and gets the value

### 18.2 When to use it

Use this pattern when:

- the work can happen independently
- the result is needed later
- the code stays readable with task boundaries

### 18.3 Beginner advice

Do not start with concurrency everywhere.

First, write the logic clearly.

Then introduce tasks where they actually help.

---

## 19. Tensor And AI Data Work

SNSX treats tensor operations as first-class language features.

Example:

```snsx
entry
    hold a = tensor([1, 2, 3, 4], [2, 2])
    hold b = tensor([1, 0, 0, 1], [2, 2])
    hold c = matmul(a, b)
    show c
    0
```

### 19.1 What to learn here

- tensors are direct values
- tensor shapes are explicit
- matrix operations can live next to normal flow code

### 19.2 When to use this

Use tensor-native code when:

- writing AI preprocessing
- shaping model input
- expressing math-heavy data paths

Keep tensor code isolated in small flows when possible.

That keeps the rest of the project readable.

---

## 20. Strict Compiler And Security Audit

This is one of the most important chapters in the entire manual.

The SNSX compiler is not only checking syntax.

It is also enforcing project safety rules.

### 20.1 The main idea

A project does not run just because it parses.

It runs only when:

- the code compiles
- the contracts are clear enough
- the security audit passes

### 20.2 Common security rules

#### `SNSX-SEC-001`

Network module imported without network permission.

Fix:

- set `[permissions].net = true` in `snsx.toml`
- or remove `need std.net`

#### `SNSX-SEC-002`

AI module imported without AI permission.

Fix:

- set `[permissions].ai = true`
- or remove the AI import

#### `SNSX-SEC-010`

A reusable flow parameter is missing a type.

Fix:

- add the missing type in `takes`

#### `SNSX-SEC-011`

A reusable flow is missing an explicit output contract.

Fix:

- add `gives Type`

#### `SNSX-SEC-020`

An unbounded `during true` loop is blocked.

Fix:

- use a real stopping condition

#### `SNSX-SEC-030`

Raw input is being displayed without validation or redaction.

Fix:

- store input in a variable
- validate it
- display a reviewed or sanitized value

#### `SNSX-SEC-031`

Filesystem read attempted without filesystem permission.

Fix:

- enable `[permissions].fs = true`
- or remove file access

#### `SNSX-SEC-032`

Network call attempted without network permission.

Fix:

- enable `[permissions].net = true`
- or remove the call

#### `SNSX-SEC-033`

AI call blocked by project policy.

Fix:

- enable `[permissions].ai = true`
- or replace the AI call

### 20.3 The developer mindset

When SNSX blocks your code, do not only ask:

"How do I silence this error?"

Ask:

"What boundary is the compiler telling me to make more honest?"

That question leads to better code.

---

## 21. Writing Code That Passes The Audit

Here is the simplest reliable procedure for a new developer.

### 21.1 The safe coding procedure

1. define the outcome of the program
2. choose the minimal modules you need
3. write `flow` contracts first
4. name external data honestly as `raw`, `input`, `payload`, or another truthful name
5. validate with `guard`
6. only then transform, display, or send data
7. keep permissions minimal in `snsx.toml`
8. run `audit`
9. fix every blocker before adding more complexity

### 21.2 A bad workflow

- turn on every permission
- write the feature quickly
- hope the compiler stops complaining

This defeats the point of SNSX.

### 21.3 A good workflow

- start small
- keep the trust boundary visible
- let the compiler shape the design early

---

## 22. Studio, IDE, And SNSX Shell Workflow

SNSX can be used through terminal, app, and web modes.

Starter commands:

```bash
snsx start --mode terminal
snsx start --mode app
snsx start --mode web
```

### 22.1 What the studio gives you

- file tree
- editor
- run output
- build output
- security output
- input panel
- SNSX shell

### 22.2 Core terminal studio controls

- `F1` focus file tree
- `F2` focus editor
- `F3` focus program input
- `F4` focus run output
- `F5` focus build output
- `F6` focus terminal
- `F7` focus security output
- `Ctrl-S` save and run
- `Ctrl-B` build
- `Ctrl-T` audit
- `Ctrl-N` new file
- `Ctrl-G` new folder
- `Ctrl-W` rename or move
- `Ctrl-D` delete
- `Ctrl-Q` quit

### 22.3 SNSX shell commands every beginner should know

```text
help
run
audit
build wasm
ls
tree
open src/main.snsx
touch src/new_flow.snsx
mkdir src/ui
entry src/main.snsx
manifest
perm ai on
package list
module list
module install std.design
```

### 22.4 Why the shell matters

The SNSX shell is not just a generic terminal.

It understands the workspace.

That means file operations, package operations, and entry management stay aligned with the project model.

---

## 23. Libraries And Framework Layers You Will Use Early

A beginner usually starts with these modules.

### 23.1 `std.io`

Use it for:

- `show`
- `ask()`
- simple console interaction

### 23.2 `std.ai`

Use it when:

- declaring `mind` flows
- performing AI-backed summarization or generation

Remember:

- AI permission must be enabled

### 23.3 `std.design`

Use it when:

- composing structured output
- building design-oriented responses
- assembling boards, panels, and fields

### 23.4 `std.error`

Use it when:

- shaping an explicit failure model
- reinforcing boundary-driven code

### 23.5 `std.sentinel`

Use it when:

- tracking values
- sealing content
- inspecting structure

### 23.6 Choosing a layer

If your code is:

- reading or writing user-facing text: start with `std.io`
- using model-backed logic: add `std.ai`
- composing structured output: add `std.design`
- tracing or review-protecting values: add `std.sentinel`

---

## 24. How To Build A Feature In SNSX

This chapter gives you a practical procedure.

Suppose you want to build a small AI summary board.

### 24.1 Step 1: define the outcome

You want:

- input text
- an AI summary
- a structured display board

### 24.2 Step 2: choose modules

You probably need:

```snsx
need std.io
bundle std.ai, std.design, std.sentinel
```

### 24.3 Step 3: write the AI flow

```snsx
mind summarize
    takes text: String
    gives String
    from "Summarize this text in one sentence"
```

### 24.4 Step 4: write the entry path safely

```snsx
entry
    hold raw = ask()
    guard raw != "", "input cannot be blank"
    hold seen = watch("raw", raw)
    hold summary = seen |> summarize
    guard len(summary) > 0, "summary cannot be blank"
    hold board = draft("Summary", panel("Result", summary))
    show board
    0
```

### 24.5 Step 5: review permissions

The project needs AI permission.

If later it needs filesystem or network access, enable those only after deciding that the feature truly needs them.

### 24.6 Step 6: run audit before polishing

This is a major SNSX habit.

Do not wait until the end to discover a policy issue.

---

## 25. Sample Programs To Study

These are the best kinds of examples for a beginner to master.

### 25.1 Console hello

```snsx
need std.io

entry
    show "hello from snsx"
    0
```

### 25.2 Input echo with validation

```snsx
need std.io

entry
    hold raw = ask()
    guard raw != "", "name cannot be blank"
    hold name = raw
    show "hello " + name
    0
```

### 25.3 Typed deterministic flow

```snsx
flow multiply
    takes a: Int, b: Int
    gives Int
    a * b

entry
    show multiply(6, 7)
    0
```

### 25.4 Value dispatch

```snsx
flow grade
    takes score: Int
    gives String
    pick score
        case 0
            give "zero"
        case 1
            give "one"
        else
            give "many"
```

### 25.5 AI summary

```snsx
need std.ai

mind summarize
    takes text: String
    gives String
    from "Summarize this text"
```

### 25.6 Task concurrency

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

### 25.7 Tensor math

```snsx
entry
    hold a = tensor([1, 2, 3, 4], [2, 2])
    hold b = tensor([1, 0, 0, 1], [2, 2])
    hold c = matmul(a, b)
    show c
    0
```

A new developer should be able to read and explain every line above.

That is a good milestone.

---

## 26. Professional Style Rules For SNSX

These rules will make your code stronger immediately.

### 26.1 Keep imports small

Only import what the file really uses.

### 26.2 Make trust boundaries visible

If data comes from a user, a file, a network call, or an AI result, name that clearly.

### 26.3 Always type reusable flows

If a flow can be called from somewhere else, give it a full contract.

### 26.4 Prefer readability over cleverness

A shorter line is not always a better line.

### 26.5 Use `guard` early

Validate inputs and assumptions close to where they appear.

### 26.6 Keep `entry` clean

`entry` should orchestrate.

Large chunks of logic should move into named flows.

### 26.7 Use the compiler as a partner

If the compiler blocks a pattern, it is usually because the boundary is under-specified.

### 26.8 Review permissions as architecture, not as configuration

Turning on `fs`, `net`, or `ai` changes the trust story of the project.

Treat that seriously.

---

## 27. Common Beginner Mistakes

### 27.1 Writing flows without contracts

Bad:

```snsx
flow build_name
    first + " " + last
```

Better:

```snsx
flow build_name
    takes first: String, last: String
    gives String
    first + " " + last
```

### 27.2 Displaying raw input directly

Bad:

```snsx
show ask()
```

Better:

```snsx
hold raw = ask()
guard raw != "", "input cannot be blank"
hold safe_value = raw
show safe_value
```

### 27.3 Enabling permissions too early

Bad habit:

- turn on `fs`, `net`, and `ai` at project start

Better habit:

- start with the minimum
- enable only what the feature needs

### 27.4 Making `entry` do everything

Bad:

- business rules
- validation
- formatting
- concurrency
- AI calls

all mixed into one long block

Better:

- let `entry` orchestrate
- move stable logic into flows

---

## 28. A Simple Learning Path For A New Developer

If you are completely new to SNSX, follow this sequence.

### Week 1

- learn `need`, `entry`, `show`, `hold`
- write three tiny console programs
- learn `gate`, `unless`, `pick`

### Week 2

- learn `flow`, `takes`, `gives`, `give`
- rewrite beginner programs into reusable flows
- practice reading compiler errors

### Week 3

- learn `mind`
- learn `watch`, `shape`, `seal`
- build one safe AI summary tool

### Week 4

- learn the SNSX shell
- install a module
- manage permissions in `snsx.toml`
- use the audit console inside the studio

### Week 5

- use `launch` and `wait`
- read tensor examples
- build a small multi-flow project

If you can complete those steps, you are no longer a beginner.

---

## 29. A Full Beginner Project Walkthrough

Let us build a small professional-grade beginner project.

The goal:

- ask the user for text
- validate the input
- summarize it with AI
- present the result as a board

### 29.1 Imports

```snsx
need std.io
bundle std.ai, std.design, std.sentinel
```

### 29.2 AI flow

```snsx
mind summarize
    takes text: String
    gives String
    from "Summarize this text in one sentence"
```

### 29.3 App entry

```snsx
entry
    hold raw = ask()
    guard raw != "", "text cannot be blank"
    hold seen = watch("raw", raw)
    hold summary = seen |> summarize
    guard len(summary) > 0, "summary cannot be blank"
    hold card = panel("Summary", summary)
    hold meta = stack([field("shape", shape(summary)), field("seal", seal(summary))])
    hold board = draft("SNSX Summary", stack([card, meta]))
    show board
    0
```

### 29.4 Why this project is good training

It teaches:

- module imports
- input handling
- contract checks
- AI flow design
- structured composition
- readable naming

### 29.5 What to improve after version one

- move board creation into its own flow
- add clearer user prompts
- review whether AI permission is still the only needed capability

---

## 30. How To Read Compiler Errors In SNSX

SNSX errors are meant to tell you:

- what is wrong
- why it matters
- how to fix it

The diagnostic format includes:

- severity
- rule code
- message
- `why:`
- `fix:`

Example shape:

```text
Error [SNSX-SEC-030]: raw input is being displayed without validation or redaction
  why: strict mode blocks direct printing of user input because secrets, tokens, and unsafe payloads can leak immediately.
  fix: Store the input in a variable, validate it, and only then show a sanitized value.
```

### 30.1 How to respond professionally

1. read the rule code
2. read the `why`
3. apply the `fix`
4. re-run audit
5. only continue once the boundary is clear

Do not skip step 2.

The explanation is where the language teaches you.

---

## 31. What To Keep In Mind While Designing An SNSX Project

Before writing a medium or large feature, decide these things:

### 31.1 What are the trust boundaries?

Examples:

- user input
- file input
- AI output
- network output

### 31.2 Which permissions are actually needed?

Do not answer with habit.

Answer with evidence.

### 31.3 Which flows are reusable?

Those deserve strong contracts.

### 31.4 Which part of the code is orchestration?

That usually belongs near `entry`.

### 31.5 Which part is framework composition?

That may belong in design-focused flows using `panel`, `field`, `stack`, and `draft`.

---

## 32. Quick Reference

### 32.1 Core language words

- `need`
- `bring`
- `bundle`
- `entry`
- `flow`
- `mind`
- `takes`
- `gives`
- `hold`
- `set`
- `show`
- `ask`
- `gate`
- `otherwise`
- `unless`
- `during`
- `pick`
- `case`
- `give`
- `guard`
- `fail`
- `from`
- `|>`

### 32.2 Common build and run commands

```bash
snsx init my_project
snsx run
snsx run --file examples/hello.snsx
snsx build --target wasm
snsx start --mode terminal
snsx start --mode app
snsx start --mode web
```

### 32.3 Common shell commands

```text
help
run
audit
build wasm
manifest
module list
module install std.design
package list
entry src/main.snsx
```

---

## 33. Final Advice To A New SNSX Developer

Do not try to impress the language.

Let the language make your program honest.

The best SNSX code usually has these qualities:

- it reads clearly
- it names risky data honestly
- it keeps permissions small
- it gives reusable flows strong contracts
- it accepts compiler feedback early
- it treats security as part of development, not as cleanup

If you remember only one thing from this manual, remember this:

SNSX wants your code to explain itself.

When your code explains itself, the compiler, the reviewer, and the future maintainer can all trust it more.

That is how you become productive in SNSX.
EOF

echo "Wrote ${OUT_FILE}"
