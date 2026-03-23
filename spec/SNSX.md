# SNSX Language Specification

## Source files

- Extension: `.snsx`
- Encoding: UTF-8
- Significant indentation: 4 spaces per level
- Comments begin with `#`

## Primitive types

- `Int`
- `Float`
- `Bool`
- `String`
- `Bytes`
- `Unit`
- `Tensor`
- `Task[T]`
- `Array[T]`
- `Fn[(A, B) -> R]`

## Ownership model

- Values are safe by default.
- Copy types: `Int`, `Float`, `Bool`, `Unit`
- Non-copy values are borrowed for reads and explicitly transferred with `move expr`.
- Runtime storage uses deterministic reference counting with optional cycle collection hooks.

## Native surface syntax

SNSX supports a native, non-C family surface syntax designed to read like intent instead of ceremony.

- `bring <module/...>` for imports
- `bring <lib/...> as alias` for library aliases
- `bring <package/...>` for package-level surfaces
- `entry` for the application root
- `flow` for callable flows
- `mind` for AI prompt flows
- `takes` / `gives` for strict function contracts
- `later` for async flows
- `agent` / `memory` for actor state machines
- `hold` for bindings
- `set` for reassignment
- `show` for direct output
- `ask` for interactive input
- `when` / `otherwise` for branching
- `gate` / `otherwise` for explicit policy-style branching
- `unless` for inverted conditions
- `during` for loops
- `pick` / `case` for pattern dispatch
- `send` for explicit return
- `give` for explicit return in structured flows
- `guard` for contract checks
- `fail` for explicit failure
- `from "..."` for AI prompt metadata
- `|>` for dataflow piping

Legacy bootstrap forms such as `use`, `fn`, `ai fn`, `let`, `return`, `if`, and `match` remain accepted for compatibility.

## Declarations

```snsx
bring <module/std.io> as io
bring <module/std.ai> as ai
bring <lib/std.design> as design
bring <lib/std.sentinel> as sentry
bring <package/std.error> as errors

flow add
    takes a: Int, b: Int
    gives Int
    a + b

later fetch_score
    takes name: String
    gives Int
    42

mind summarize
    takes text: String
    gives String
    from "Summarize the input text in two sentences"

agent Counter:
    memory value: Int = 0

    flow inc
        takes step: Int
        gives Int
        set value = value + step
        value
```

Application roots can be written directly with `entry` and compile to `main` automatically:

```snsx
entry
    show "SNSX booted"
    0
```

Canonical imports:

```snsx
bring <module/std.io> as io
bring <module/std.ai> as ai
bring <lib/std.design> as design
bring <package/std.error> as errors
```

## Statements

```snsx
hold x = 10
hold label: String = "snsx"
set x = x + 1

gate x > 10:
    show label
otherwise:
    show "small"

unless label == "":
    show "missing"

during x < 20:
    set x = x + 1

pick x
    case 0
        show "zero"
    case 1
        show "one"
    else
        show "many"

x
```

Contract-style errors:

```snsx
guard x > 0, "x must stay positive"
fail "manual stop"
```

## Expressions

```snsx
1 + 2 * 3
foo(bar, 10)
[1, 2, 3]
wait launch work(10)
parallel compute_heavy(99)
move tensor([1.0, 2.0, 3.0], [3])
"SNSX is readable" |> summarize |> show
```

New framework-oriented built-ins:

```snsx
shape(summary)
seal(summary)
watch("summary", summary)
repair(primary, fallback)
blend(["a", "b", "c"], " / ")
draft("Page", stack([panel("Info", "SNSX"), field("mode", "strict")]))
```

## AI-native functions

```snsx
mind generate_summary
    takes text: String
    gives String
    from "Summarize this text"

entry
    hold summary = generate_summary("SNSX is AI-native.")
    show summary
    0
```

Semantics:

- `ai fn` compiles to a callable function with embedded prompt metadata.
- Runtime model selection is capability-driven and configurable.
- Tensor operations are first-class values executed by CPU or GPU backends.

## Modules and packages

- Project manifest: `snsx.toml`
- Entry file defaults to `src/main.snsx`
- `bring <module/std.io> as io` loads built-in stdlib modules
- `bring <lib/std.design> as design` loads library surfaces
- `bring <package/std.error> as errors` loads package surfaces
- `use mypkg.util.math` resolves to `src/mypkg/util/math.snsx`
