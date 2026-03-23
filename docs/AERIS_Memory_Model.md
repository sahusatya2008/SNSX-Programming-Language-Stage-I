# AERIS Memory Model

## Objective

AERIS is targeting a memory discipline stronger than today's mainstream systems languages by combining:

- ownership
- borrowing
- regions
- optional audited escape hatches

## Bootstrap Reality

The stage-0 compiler already enforces the structural side of the model through:

- explicit capabilities
- no implicit globals
- no raw pointer syntax
- no ambient mutable shared state

## Planned Static Rules

### Ownership

- every value has one owning path
- ownership may move but not silently copy for linear resources

### Borrowing

- immutable borrows may alias
- mutable borrows are exclusive
- lifetime inference is structural and local-first

### Regions

- short-lived allocations may be attached to lexical regions
- region teardown is deterministic

### Unsafe Boundary

- unsafe operations live in auditable capability-scoped modules only
- unsafe blocks require explanatory annotations and lints

## Runtime Goal

The long-term runtime should allow:

- no undefined behavior from ordinary AERIS code
- deterministic teardown
- predictable embedded and OS-level deployment
