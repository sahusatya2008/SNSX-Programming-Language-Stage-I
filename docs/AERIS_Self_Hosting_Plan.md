# AERIS Self-Hosting Plan

## Stage 0

- bootstrap compiler in Rust
- define source language, effect/capability system, interpreter, IR
- stabilize package and formatting workflow
- land a `no_std` kernel ABI crate for the future bare-metal boundary

## Stage 1

- implement `aeris_std`
- implement text, file, collections, and test libraries in AERIS
- write parser combinators and AST definitions in AERIS
- seed the `aeris_stage1/` source tree inside the repository

## Stage 2

- reimplement lexer/parser in AERIS
- validate output equivalence against the stage-0 compiler

## Stage 3

- reimplement semantic analysis and IR lowering in AERIS
- self-compile the compiler frontend

## Stage 4

- introduce native backend work in AERIS plus auditable low-level runtime shims
- shrink the trusted Rust bootstrap to a verifier/loader role

## Stage 5

- AERIS compiler builds AERIS compiler
- stage-0 compiler retained only as recovery bootstrap

## Honest Boundary

True "from no other language at all" implementation is not something we can honestly claim at bootstrap. The correct path is staged replacement until the trusted base is mostly AERIS itself.
