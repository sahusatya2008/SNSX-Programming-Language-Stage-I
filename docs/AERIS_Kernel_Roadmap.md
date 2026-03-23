# AERIS Kernel-Level Roadmap

## Goal

Move AERIS from a hosted language toolchain into a full-stack environment with its own runtime contract down to the kernel boundary.

## K0: ABI Foundation

Delivered in this step:

- `aeris_kernel_abi` as a `no_std` crate
- capability identifiers
- memory region descriptors
- page policies and flags
- scheduler/task descriptors
- deterministic messaging ABI
- syscall/result structures

## K1: Boot Chain

Next:

- boot image format for AERIS modules
- bootloader handoff into `BootInfo`
- early allocator and console bring-up

## K2: Microkernel Core

Then:

- capability space manager
- region mapper
- deterministic scheduler
- channel/message kernel objects
- audit/log subsystem

## K3: Native AERIS Runtime

Then:

- AERIS process loader
- AERIS standard library over the kernel ABI
- native actor runtime over channels

## K4: Self-Hosted Compiler

Then:

- compile AERIS-on-AERIS
- cross-build the kernel/runtime with AERIS itself
- retain the hosted stage-0 compiler only as an emergency bootstrap
