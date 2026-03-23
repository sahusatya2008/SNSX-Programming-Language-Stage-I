#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

cargo build --release -p snsx --manifest-path "${ROOT_DIR}/Cargo.toml"
mkdir -p "${HOME}/.local/bin"
cp "${ROOT_DIR}/target/release/snsx" "${HOME}/.local/bin/snsx"
echo "Installed snsx to ${HOME}/.local/bin/snsx"
