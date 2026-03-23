# AERIS Formal Semantics Plan

## Goal

Formalize the core AERIS calculus in Lean or Coq with proofs for:

- progress
- preservation
- capability non-forgeability
- deterministic actor reduction
- refinement soundness for supported predicates

## Core Judgments

- `Gamma ⊢ e : T ! E`
- `Gamma ; Cap ⊢ stmt ok`
- `Gamma ⊢ p ⇐ T`
- `Sigma ⊢ actor_step -> Sigma'`

## Stage Breakdown

### Stage A

- simply typed core with algebraic data types
- effect annotations
- capability tokens as linear resources in the core model

### Stage B

- actor mailbox semantics
- deterministic scheduling theorem
- message isolation theorem

### Stage C

- refinement predicates over integers and booleans
- proof that verified predicates cannot be violated by literal calls accepted by the checker

### Stage D

- compiler IR correspondence for a straight-line subset
- proof sketch that SSA lowering preserves expression meaning

## Why This Matters

AERIS is intended for mission-critical systems. The bootstrap compiler is practical engineering; the formal model is the path to research-grade assurance.
