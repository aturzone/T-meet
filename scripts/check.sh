#!/usr/bin/env bash
# Local mirror of CI quality gates. Run via `just check`.

set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

step() {
    printf "\n\033[1;34m==> %s\033[0m\n" "$*"
}

step "cargo fmt --all -- --check"
cargo fmt --all -- --check

step "cargo clippy --workspace --all-targets -- -D warnings"
cargo clippy --workspace --all-targets -- -D warnings

step "cargo test --workspace"
cargo test --workspace

if command -v cargo-deny >/dev/null 2>&1; then
    step "cargo deny check"
    cargo deny check
else
    echo
    echo "(skipping cargo deny — install with 'cargo install --locked cargo-deny')"
fi

if [ -d frontend/node_modules ]; then
    step "pnpm -C frontend lint"
    pnpm -C frontend lint

    step "pnpm -C frontend typecheck"
    pnpm -C frontend typecheck

    step "pnpm -C frontend test -- --run"
    pnpm -C frontend test -- --run
else
    echo
    echo "(skipping frontend gates — run 'pnpm -C frontend install' first)"
fi

step "all gates green"
