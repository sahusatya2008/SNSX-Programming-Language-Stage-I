# AERIS Security Architecture

## Zero-Trust Default

AERIS assumes no code should receive ambient authority.

## Capabilities

Capabilities are explicit typed tokens:

- `Console`
- `FileSystem`
- `Network`
- custom domain capabilities

Operations that touch protected resources require those capabilities in both:

- the function signature
- the actual call site

## Compiler Checks

The bootstrap compiler already blocks:

- unknown effects
- mismatched capability arguments
- effect violations such as calling `print` from `!pure`
- literal refinement violations such as passing `0` to `y where y != 0`

## Actor Isolation

- actor code runs through a message queue
- no shared mutable memory is exposed
- scheduling is deterministic FIFO

## Future Security Work

- signed packages
- verified capability delegation
- syscall sandbox profiles
- constant-time crypto annotations
- proof-carrying modules
