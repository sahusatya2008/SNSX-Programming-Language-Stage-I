#!/usr/bin/env bash
set -Eeuo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DEFAULT_SNSX_PROJECT="${ROOT_DIR}/user_workspace"
DEFAULT_AERIS_PROJECT="${ROOT_DIR}/aeris_workspace"
LOG_DIR="${ROOT_DIR}/logs"
TIMESTAMP="$(date +"%Y%m%d-%H%M%S")"
LOG_FILE="${LOG_DIR}/master-${TIMESTAMP}.log"

MODE="terminal"
LANGUAGE="auto"
PROJECT_DIR=""
PROJECT_SET=0
ENTRY_FILE=""
DO_SMOKE=1
DO_INSTALL=0
KILL_STALE=1
FORCE_REBUILD=0
TARGET_BIN=""
PACKAGE_NAME=""
PROJECT_MANIFEST=""
ENTRY_BASENAME=""

mkdir -p "${LOG_DIR}"
touch "${LOG_FILE}"

log() {
    printf '[master] %s\n' "$*" | tee -a "${LOG_FILE}"
}

run_cmd() {
    log "running: $*"
    "$@" 2>&1 | tee -a "${LOG_FILE}"
}

run_interactive() {
    log "launching interactive command: $*"
    "$@"
}

retry_cmd() {
    local label="$1"
    shift

    if run_cmd "$@"; then
        return 0
    fi

    log "${label} failed. attempting automatic recovery with cargo clean"
    run_cmd cargo clean
    rebuild_package
    run_cmd "$@"
}

rebuild_package() {
    if [[ -z "${PACKAGE_NAME}" ]]; then
        return 0
    fi

    log "rebuilding ${PACKAGE_NAME} after recovery"
    if run_cmd cargo build -p "${PACKAGE_NAME}" --offline; then
        return 0
    fi
    maybe_fetch
    run_cmd cargo build -p "${PACKAGE_NAME}"
}

usage() {
    cat <<EOF
SNSX + AERIS master runner

Usage:
  ./master.sh [options]

Options:
  --lang <auto|snsx|aeris>     Language system to launch. Default: auto
  --mode <terminal|app|web|studio|run|ide|build|check|ir|fmt|lint>  Launch mode after bootstrap. Default: terminal
  --project <path>            Project directory to create/use
  --file <path>               Direct source file (.snsx or .ae) to launch instead of a project
  --skip-smoke                Skip example smoke tests
  --install-local-bin         Install snsx into ~/.local/bin using install.sh
  --no-kill                   Do not stop stale SNSX-related processes first
  --force-rebuild             Always clean before building
  --help                      Show this help
EOF
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --lang)
            LANGUAGE="${2:-}"
            shift 2
            ;;
        --mode)
            MODE="${2:-}"
            shift 2
            ;;
        --project)
            PROJECT_DIR="${2:-}"
            PROJECT_SET=1
            shift 2
            ;;
        --file)
            ENTRY_FILE="${2:-}"
            shift 2
            ;;
        --skip-smoke)
            DO_SMOKE=0
            shift
            ;;
        --install-local-bin)
            DO_INSTALL=1
            shift
            ;;
        --no-kill)
            KILL_STALE=0
            shift
            ;;
        --force-rebuild)
            FORCE_REBUILD=1
            shift
            ;;
        --help|-h)
            usage
            exit 0
            ;;
        *)
            log "unknown argument: $1"
            usage
            exit 1
            ;;
    esac
done

require_cmd() {
    if ! command -v "$1" >/dev/null 2>&1; then
        log "missing required command: $1"
        exit 1
    fi
}

resolve_language() {
    if [[ "${LANGUAGE}" == "auto" ]]; then
        if [[ -n "${ENTRY_FILE}" && "${ENTRY_FILE}" == *.ae ]]; then
            LANGUAGE="aeris"
        elif [[ -n "${PROJECT_DIR}" && -f "${PROJECT_DIR}/Aeris.toml" ]]; then
            LANGUAGE="aeris"
        elif [[ -n "${PROJECT_DIR}" && -f "${PROJECT_DIR}/snsx.toml" ]]; then
            LANGUAGE="snsx"
        else
            LANGUAGE="snsx"
        fi
    fi

    case "${LANGUAGE}" in
        snsx)
            TARGET_BIN="${ROOT_DIR}/target/debug/snsx"
            PACKAGE_NAME="snsx"
            PROJECT_MANIFEST="snsx.toml"
            ENTRY_BASENAME="src/main.snsx"
            if [[ "${PROJECT_SET}" -eq 0 ]]; then
                PROJECT_DIR="${DEFAULT_SNSX_PROJECT}"
            fi
            ;;
        aeris)
            TARGET_BIN="${ROOT_DIR}/target/debug/aeris"
            PACKAGE_NAME="aeris_cli"
            PROJECT_MANIFEST="Aeris.toml"
            ENTRY_BASENAME="src/main.ae"
            if [[ "${PROJECT_SET}" -eq 0 ]]; then
                PROJECT_DIR="${DEFAULT_AERIS_PROJECT}"
            fi
            ;;
        *)
            log "unsupported language: ${LANGUAGE}"
            exit 1
            ;;
    esac

    if [[ "${LANGUAGE}" == "aeris" ]]; then
        case "${MODE}" in
            terminal|app|web|studio|ide)
                log "mode '${MODE}' is SNSX-only today; falling back to AERIS run mode"
                MODE="run"
                ;;
        esac
    fi
}

maybe_fetch() {
    log "attempting dependency fetch if network is available"
    if run_cmd cargo fetch --locked; then
        return 0
    fi
    log "network fetch unavailable or unnecessary; continuing with local cargo cache"
    return 0
}

kill_stale_processes() {
    local patterns=(
        "cargo run -p snsx"
        "cargo run -p aeris_cli"
        "snsx run --watch"
        "snsx ide"
        "${ROOT_DIR}/target/debug/snsx"
        "${ROOT_DIR}/target/debug/aeris"
    )

    if ! ps -ax -o pid= -o command= >/dev/null 2>&1; then
        log "process inspection is unavailable here; skipping stale-process cleanup"
        return 0
    fi

    for pattern in "${patterns[@]}"; do
        while IFS= read -r line; do
            local pid command
            pid="$(printf '%s\n' "${line}" | awk '{print $1}')"
            command="${line#${pid} }"
            if [[ -z "${pid}" || "${pid}" == "$$" ]]; then
                continue
            fi
            if [[ "${command}" == *"${pattern}"* ]]; then
                log "stopping stale process ${pid} (${pattern})"
                kill "${pid}" 2>/dev/null || true
            fi
        done < <(ps -ax -o pid= -o command=)
    done

    sleep 1

    for pattern in "${patterns[@]}"; do
        while IFS= read -r line; do
            local pid command
            pid="$(printf '%s\n' "${line}" | awk '{print $1}')"
            command="${line#${pid} }"
            if [[ -z "${pid}" || "${pid}" == "$$" ]]; then
                continue
            fi
            if [[ "${command}" == *"${pattern}"* ]]; then
                log "force stopping process ${pid} (${pattern})"
                kill -9 "${pid}" 2>/dev/null || true
            fi
        done < <(ps -ax -o pid= -o command=)
    done
}

ensure_project() {
    if [[ -n "${ENTRY_FILE}" ]]; then
        return 0
    fi

    if [[ -f "${PROJECT_DIR}/${PROJECT_MANIFEST}" ]]; then
        return 0
    fi

    log "initializing ${LANGUAGE} project at ${PROJECT_DIR}"
    run_cmd "${TARGET_BIN}" init "${PROJECT_DIR}"
}

resolve_entry() {
    if [[ -n "${ENTRY_FILE}" ]]; then
        printf '%s\n' "${ENTRY_FILE}"
        return 0
    fi
    printf '%s\n' "${PROJECT_DIR}/${ENTRY_BASENAME}"
}

run_smoke_tests() {
    if [[ "${LANGUAGE}" == "snsx" ]]; then
        log "running full SNSX smoke tests"
        retry_cmd "hello example" "${TARGET_BIN}" run --file "${ROOT_DIR}/examples/hello.snsx"
        retry_cmd "concurrency example" "${TARGET_BIN}" run --file "${ROOT_DIR}/examples/concurrency.snsx"
        retry_cmd "tensor example" "${TARGET_BIN}" run --file "${ROOT_DIR}/examples/tensor.snsx"
        retry_cmd "wasm build" "${TARGET_BIN}" build --file "${ROOT_DIR}/examples/hello.snsx" --target wasm
        retry_cmd "deploy bundle" "${TARGET_BIN}" deploy --file "${ROOT_DIR}/examples/hello.snsx" --cluster local
    else
        log "running full AERIS smoke tests"
        retry_cmd "aeris hello check" "${TARGET_BIN}" check --file "${ROOT_DIR}/aeris_examples/hello.ae"
        retry_cmd "aeris hello run" "${TARGET_BIN}" run --file "${ROOT_DIR}/aeris_examples/hello.ae"
        retry_cmd "aeris refinement run" "${TARGET_BIN}" run --file "${ROOT_DIR}/aeris_examples/refinement.ae"
        retry_cmd "aeris actor run" "${TARGET_BIN}" run --file "${ROOT_DIR}/aeris_examples/actors.ae"
        retry_cmd "aeris ir build" "${TARGET_BIN}" build --file "${ROOT_DIR}/aeris_examples/hello.ae" --out-dir "${ROOT_DIR}/aeris_examples/build"
    fi
}

launch_system() {
    local entry
    entry="$(resolve_entry)"

    if [[ "${LANGUAGE}" == "aeris" ]]; then
        case "${MODE}" in
            run)
                log "launching AERIS system in run mode"
                if [[ -n "${ENTRY_FILE}" ]]; then
                    run_cmd "${TARGET_BIN}" run --file "${entry}"
                else
                    run_cmd "${TARGET_BIN}" run --manifest "${PROJECT_DIR}/${PROJECT_MANIFEST}"
                fi
                ;;
            build)
                log "launching AERIS system in build mode"
                run_cmd "${TARGET_BIN}" build --file "${entry}" --out-dir "${PROJECT_DIR}/build"
                ;;
            check)
                log "launching AERIS system in check mode"
                if [[ -n "${ENTRY_FILE}" ]]; then
                    run_cmd "${TARGET_BIN}" check --file "${entry}"
                else
                    run_cmd "${TARGET_BIN}" check --manifest "${PROJECT_DIR}/${PROJECT_MANIFEST}"
                fi
                ;;
            ir)
                log "launching AERIS system in IR mode"
                run_cmd "${TARGET_BIN}" ir --file "${entry}"
                ;;
            fmt)
                log "launching AERIS formatter"
                run_cmd "${TARGET_BIN}" fmt --file "${entry}"
                ;;
            lint)
                log "launching AERIS linter"
                run_cmd "${TARGET_BIN}" lint --file "${entry}"
                ;;
            *)
                log "unsupported AERIS mode: ${MODE}"
                exit 1
                ;;
        esac
        return 0
    fi

    case "${MODE}" in
        studio)
            log "launching user system in studio mode"
            if [[ -n "${ENTRY_FILE}" ]]; then
                run_interactive "${TARGET_BIN}" studio --file "${entry}"
            else
                (cd "${PROJECT_DIR}" && run_interactive "${TARGET_BIN}" studio)
            fi
            ;;
        terminal)
            log "launching user system in terminal studio mode"
            if [[ -n "${ENTRY_FILE}" ]]; then
                run_interactive "${TARGET_BIN}" start --mode terminal --file "${entry}"
            else
                (cd "${PROJECT_DIR}" && run_interactive "${TARGET_BIN}" start --mode terminal)
            fi
            ;;
        app)
            log "launching user system in desktop app mode"
            if [[ -n "${ENTRY_FILE}" ]]; then
                run_interactive "${TARGET_BIN}" start --mode app --file "${entry}"
            else
                (cd "${PROJECT_DIR}" && run_interactive "${TARGET_BIN}" start --mode app)
            fi
            ;;
        web)
            log "launching user system in web studio mode"
            if [[ -n "${ENTRY_FILE}" ]]; then
                run_interactive "${TARGET_BIN}" start --mode web --file "${entry}"
            else
                (cd "${PROJECT_DIR}" && run_interactive "${TARGET_BIN}" start --mode web)
            fi
            ;;
        run)
            log "launching user system in run mode"
            if [[ -n "${ENTRY_FILE}" ]]; then
                run_cmd "${TARGET_BIN}" run --file "${entry}"
            else
                (cd "${PROJECT_DIR}" && run_cmd "${TARGET_BIN}" run)
            fi
            ;;
        ide)
            log "launching user system in ide mode"
            if [[ -n "${ENTRY_FILE}" ]]; then
                run_interactive "${TARGET_BIN}" ide --file "${entry}"
            else
                (cd "${PROJECT_DIR}" && run_interactive "${TARGET_BIN}" ide)
            fi
            ;;
        build)
            log "launching user system in build mode"
            if [[ -n "${ENTRY_FILE}" ]]; then
                run_cmd "${TARGET_BIN}" build --file "${entry}"
            else
                (cd "${PROJECT_DIR}" && run_cmd "${TARGET_BIN}" build)
            fi
            ;;
        *)
            log "unsupported mode: ${MODE}"
            exit 1
            ;;
    esac
}

main() {
    require_cmd cargo
    require_cmd rustc
    resolve_language

    log "master runner started"
    log "workspace: ${ROOT_DIR}"
    log "language: ${LANGUAGE}"
    log "mode: ${MODE}"
    log "project: ${PROJECT_DIR}"
    log "log file: ${LOG_FILE}"

    if [[ "${KILL_STALE}" -eq 1 ]]; then
        kill_stale_processes
    fi

    if [[ "${FORCE_REBUILD}" -eq 1 ]]; then
        run_cmd cargo clean
    fi

    if ! run_cmd cargo check --offline; then
        maybe_fetch
        retry_cmd "workspace check" cargo check
    fi

    if ! run_cmd cargo build -p "${PACKAGE_NAME}" --offline; then
        maybe_fetch
        retry_cmd "${LANGUAGE} build" cargo build -p "${PACKAGE_NAME}"
    fi

    if [[ "${DO_INSTALL}" -eq 1 ]]; then
        if [[ "${LANGUAGE}" == "snsx" ]]; then
            log "installing local snsx binary"
            run_cmd bash "${ROOT_DIR}/install.sh"
        else
            log "local install automation is only implemented for SNSX; skipping"
        fi
    fi

    ensure_project

    if [[ "${DO_SMOKE}" -eq 1 ]]; then
        run_smoke_tests
    fi

    if [[ "${KILL_STALE}" -eq 1 ]]; then
        kill_stale_processes
    fi

    launch_system
    log "master runner finished"
}

main "$@"
