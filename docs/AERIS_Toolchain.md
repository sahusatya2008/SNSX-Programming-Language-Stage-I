# AERIS Toolchain

## Binaries and Crates

- `aeris_compiler`: frontend, semantic checks, IR lowering
- `aeris_runtime`: interpreter and deterministic actor runtime
- `aeris_pkg`: manifest and project scaffolding
- `aeris_fmt`: source formatter
- `aeris_lint`: style and safety linting
- `aeris_cli`: user-facing command line
- `aeris_kernel_abi`: `no_std` kernel and bare-metal ABI definitions

## CLI Commands

- `aeris init <path>`
- `aeris check --file path/to/main.ae`
- `aeris run --file path/to/main.ae`
- `aeris ir --file path/to/main.ae`
- `aeris build --file path/to/main.ae --out-dir build`
- `aeris fmt --file path/to/main.ae`
- `aeris lint --file path/to/main.ae`
- `aeris pkg manifest --path Aeris.toml`

## Manifest

`Aeris.toml`

- package metadata
- entry file
- default capabilities
- dependency coordinates
