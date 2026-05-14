# T-meet task runner. Recipes implemented per phase; TODO markers track what's
# still pending. Convention: when a recipe lands, drop its TODO and bump the
# README quick-start if behavior changes.

set shell := ["bash", "-cu"]
set dotenv-load := false

# Default — list recipes.
default:
    @just --list

# ─── Development ─────────────────────────────────────────────────────────────
dev:
    @echo "TODO phase-02 — start dev server (cargo run + frontend vite dev)"

# ─── Build ───────────────────────────────────────────────────────────────────
build:
    cargo build --workspace

build-release:
    cargo build --workspace --release

build-musl:
    @echo "TODO phase-10 — cargo build --release --target x86_64-unknown-linux-musl"

frontend-build:
    pnpm -C frontend install --frozen-lockfile
    pnpm -C frontend build

# ─── Check / quality gates ───────────────────────────────────────────────────
check:
    @bash scripts/check.sh

fmt:
    cargo fmt --all
    @echo "(frontend prettier not wired until phase-06)"

fmt-check:
    cargo fmt --all -- --check

lint:
    cargo clippy --workspace --all-targets -- -D warnings

typecheck:
    pnpm -C frontend typecheck

test:
    cargo test --workspace

test-unit:
    cargo test --workspace

test-e2e:
    @echo "TODO phase-07 — pnpm -C frontend playwright test (Brave via @playwright/mcp)"

audit:
    @echo "TODO phase-09 — cargo deny check + pnpm audit --prod"

# ─── Packaging ───────────────────────────────────────────────────────────────
package:
    @echo "TODO phase-10 — produce dist/meet-platform-<version>-<arch>.tar.gz"

verify-static:
    @echo "TODO phase-10 — file ./target/x86_64-unknown-linux-musl/release/meet-server | grep 'statically linked'"

# ─── Hygiene ─────────────────────────────────────────────────────────────────
clean:
    cargo clean
    rm -rf frontend/dist frontend/node_modules dist/

# ─── Database (Phase 02) ─────────────────────────────────────────────────────
db-prepare:
    @echo "TODO phase-02 — sqlx migrate run + sqlx prepare (for offline CI)"

migrate:
    @echo "TODO phase-02 — sqlx migrate run against the local sqlite file"

# ─── Release ─────────────────────────────────────────────────────────────────
release VERSION:
    @echo "TODO phase-10 — tag v{{VERSION}}, push, watch release.yml"
