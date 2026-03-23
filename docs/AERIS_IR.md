# AERIS IR Design

## Form

AERIS lowers source functions into an SSA-style control-flow IR with:

- explicit basic blocks
- register-like temporaries
- phi nodes at merges
- explicit terminators
- effect-preserving call sites

## Instructions

- `Param`
- `ConstInt`
- `ConstBool`
- `ConstText`
- `Unary`
- `Binary`
- `Call`
- `Phi`
- `Guard`

## Terminators

- `Return`
- `Jump`
- `Branch`
- `Trap`

## Why SSA

SSA is the right bridge from a safe source language into:

- optimization passes
- data-flow analysis
- borrow/effect reasoning
- future native/WASM backends

## Near-Term Roadmap

- constant folding
- dead code elimination
- effect-aware inlining
- actor-message escape analysis
