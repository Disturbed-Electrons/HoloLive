#!/usr/bin/env bash

set -euo pipefail

PROJECT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$PROJECT_DIR"

RED='\033[0;31m'
GREEN='\033[0;32m'
BLUE='\033[0;34m'
NC='\033[0m'

info() { printf "%b[INFO]%b %s\n" "$GREEN" "$NC" "$1"; }
fail() { printf "%b[ERROR]%b %s\n" "$RED" "$NC" "$1" >&2; exit 1; }
step() { printf "%b==>%b %s\n" "$BLUE" "$NC" "$1"; }

RELEASE=true
NATIVE=false

cpu_jobs() {
    getconf _NPROCESSORS_ONLN 2>/dev/null || sysctl -n hw.logicalcpu 2>/dev/null || echo 4
}

cargo_cmd() {
    local cmd="$1"
    shift
    if [[ "$NATIVE" == true ]]; then
        RUSTFLAGS="${RUSTFLAGS:+$RUSTFLAGS }-C target-cpu=native" cargo "$cmd" --jobs "$(cpu_jobs)" "$@"
    else
        cargo "$cmd" --jobs "$(cpu_jobs)" "$@"
    fi
}

check_tools() {
    command -v cargo >/dev/null 2>&1 || fail "Cargo Not Found. Install Rust From https://rustup.rs/"
    info "Rust: $(rustc --version)"
    info "Cargo: $(cargo --version)"
    if [[ "$NATIVE" == true ]]; then
        info "target-cpu: native ($(rustc --print target-cpus | sed -n 's/.*currently \([^)]*\).*/\1/p' | head -1))"
    fi
}

build() {
    step "Building HoloLive"
    check_tools
    if [[ "$RELEASE" == true ]]; then
        cargo_cmd build --workspace --release
    else
        cargo_cmd build --workspace
    fi
}

run() {
    step "Running HoloLive"
    check_tools
    if [[ "$RELEASE" == true ]]; then
        cargo_cmd run -p hololive-app --bin HoloLive --release -- "$@"
    else
        cargo_cmd run -p hololive-app --bin HoloLive -- "$@"
    fi
}

check_all() {
    step "Checking Formatting And Rust Code"
    check_tools
    cargo fmt --all -- --check
    cargo_cmd check --workspace --all-targets --all-features
    cargo_cmd clippy --workspace --all-targets --all-features -- -D warnings
    RUSTDOCFLAGS="-D warnings" cargo_cmd doc --workspace --no-deps
}

format_all() {
    step "Formatting Rust"
    check_tools
    cargo fmt --all
}

clean() {
    step "Cleaning Cargo Build Artifacts And Exports"
    check_tools
    cargo clean
    rm -rf -- "$PROJECT_DIR/exports"
}

usage() {
    cat <<'EOF'
HoloLive Native Rust Application

Usage: ./build.sh [--release|--debug] [--native] [COMMAND ...]

Options may appear before or after commands, including after `run`.
Tokens after `run` that are not `--release`, `--debug`, or `--native` are
passed to the application. `run -- ...` forwards the rest unchanged.
Without a command, `build` runs.

Options:
  --release   Optimized build (default)
  --debug     Unoptimized incremental build
  --native    Compile for this machine's CPU (not a portable binary)

Commands:
  build       Build HoloLive (default)
  run         Run HoloLive; non-option arguments after `run` go to the app
  check       Check formatting, compilation, Clippy, and documentation
  format      Format Rust sources with standard rustfmt settings
  clean       Remove Cargo build artifacts and exported data
  help        Show this help
EOF
}

main() {
    local -a commands=()
    local -a run_args=()
    local saw_run=false
    while (($#)); do
        if [[ "$saw_run" == true ]]; then
            case "$1" in
                --)
                    shift
                    while (($#)); do
                        run_args+=("$1")
                        shift
                    done
                    continue
                    ;;
                --release) RELEASE=true ;;
                --debug) RELEASE=false ;;
                --native) NATIVE=true ;;
                *) run_args+=("$1") ;;
            esac
            shift
            continue
        fi
        case "$1" in
            --release) RELEASE=true ;;
            --debug) RELEASE=false ;;
            --native) NATIVE=true ;;
            run) saw_run=true; commands+=("run") ;;
            build|check|format|clean) commands+=("$1") ;;
            help|-h|--help) usage; return ;;
            *) fail "Unknown Option Or Command: $1" ;;
        esac
        shift
    done
    if ((${#commands[@]} == 0)); then
        build
        return
    fi
    local command
    for command in "${commands[@]}"; do
        case "$command" in
            build) build ;;
            run)
                if ((${#run_args[@]})); then
                    run "${run_args[@]}"
                else
                    run
                fi
                ;;
            check) check_all ;;
            format) format_all ;;
            clean) clean ;;
        esac
    done
}

main "$@"
