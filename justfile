# T-meet task runner. Each recipe is a TODO until the matching phase implements it.
# Convention: any change that touches a recipe must also bump the phase tag in the
# echo string, so the README's quick-start always reflects the current capability.

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
    @echo "TODO phase-00 — cargo build --release (musl target during phase-10)"

build-musl:
    @echo "TODO phase-10 — cargo build --release --target x86_64-unknown-linux-musl"

frontend-build:
    @echo "TODO phase-06 — pnpm --filter frontend build"

# ─── Check / quality gates ───────────────────────────────────────────────────
check: fmt-check lint test audit
    @echo "All gates green."

fmt:
    @echo "TODO phase-00 — cargo fmt --all + pnpm --filter frontend prettier --write"

fmt-check:
    @echo "TODO phase-00 — cargo fmt --all -- --check + pnpm prettier --check"

lint:
    @echo "TODO phase-00 — cargo clippy --all-targets --all-features -- -D warnings + pnpm eslint"

typecheck:
    @echo "TODO phase-06 — pnpm --filter frontend tsc --noEmit"

test:
    @echo "TODO phase-00 — cargo test --workspace + pnpm vitest + pnpm playwright"

test-unit:
    @echo "TODO phase-00 — cargo test --workspace"

test-e2e:
    @echo "TODO phase-07 — pnpm playwright test (Brave via @playwright/mcp)"

audit:
    @echo "TODO phase-09 — cargo deny check + pnpm audit --prod"

# ─── Packaging ───────────────────────────────────────────────────────────────
package:
    @echo "TODO phase-10 — produce dist/meet-platform-<version>-<arch>.tar.gz"

verify-static:
    @echo "TODO phase-10 — file ./target/x86_64-unknown-linux-musl/release/meet-server | grep 'statically linked'"

# ─── Hygiene ─────────────────────────────────────────────────────────────────
clean:
    @echo "TODO phase-00 — cargo clean && rm -rf frontend/dist frontend/node_modules dist/"

# ─── Database (Phase 02) ─────────────────────────────────────────────────────
db-prepare:
    @echo "TODO phase-02 — sqlx migrate run + sqlx prepare (for offline CI)"

migrate:
    @echo "TODO phase-02 — sqlx migrate run against the local sqlite file"

# ─── Release ─────────────────────────────────────────────────────────────────
release VERSION:
    @echo "TODO phase-10 — tag v{{VERSION}}, push, watch release.yml"
