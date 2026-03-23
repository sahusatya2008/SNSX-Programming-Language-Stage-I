# SNSX Architecture

The SNSX bootstrap pipeline is:

1. Source loader and manifest resolver
2. Indentation-aware lexer
3. Recursive descent parser
4. Semantic analysis with type inference and ownership-aware diagnostics
5. SSA-style IR lowering
6. Optimization passes
7. Target lowering:
   - SNSX-VM bytecode
   - LLVM IR text
   - WebAssembly text
   - Native assembly text
8. Runtime execution or package emission

Runtime layers:

- Hybrid memory manager with owned/reference-counted values
- Cooperative task scheduler
- Actor mailboxes
- Deterministic clock and scheduler mode
- Sandboxed capability checks
- AI model bridge and tensor runtime
- SNSP secure networking stack
- Distributed cluster planner

