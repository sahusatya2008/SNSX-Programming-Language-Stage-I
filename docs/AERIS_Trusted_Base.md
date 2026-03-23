# AERIS Trusted Computing Base

## Current Truth

Right now the AERIS trusted base is still hosted. That means these pieces are trusted:

- the Rust compiler used to build stage 0
- Cargo as the stage-0 build orchestrator
- the host OS process loader
- the AERIS stage-0 crates in this repository

## What Changed In This Step

The repository now contains a concrete low-level substrate in:

- `aeris_kernel_abi/`
- `aeris_stage1/`

That does not remove the hosted bootstrap yet, but it does reduce ambiguity about the target architecture.

## End-State Target

The long-term trusted base should shrink toward:

- AERIS-written compiler stages
- AERIS-written standard library
- AERIS kernel and runtime ABI
- a tiny auditable machine bootstrap/loader

## Why I Am Not Claiming More

No serious compiler engineer should pretend a hosted bootstrap has disappeared just because a new language exists in the repo. Self-hosting is earned by staged replacement and reproducible rebuilds, not by wording.
