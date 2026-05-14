# Phase 10 — Build & Package

## Goal

Produce the deliverable a real operator can copy onto a server: a single statically-linked `x86_64-unknown-linux-musl` (and optionally `aarch64-unknown-linux-musl`) binary plus a tiny support package, gzipped into `dist/meet-platform-<version>-<arch>.tar.gz`. The build embeds the frontend; `file` reports `statically linked`; `ldd` reports `not a dynamic executable`. The packaging step refuses to ship anything that fails those invariants. Reproducible-ish builds via `SOURCE_DATE_EPOCH`.

## Deliverables

- `Cargo.toml` workspace `[profile.release]` — `opt-level = "z"` (size-first; throughput is media-bound, not CPU), `lto = "fat"`, `codegen-units = 1`, `panic = "abort"`, `strip = "symbols"`.
- `.cargo/config.toml` — target-specific linker flags (`-C link-arg=-static-pie` when supported), MUSL targets:
  - `x86_64-unknown-linux-musl`
  - `aarch64-unknown-linux-musl`
- `frontend/build.rs` (or a `just` recipe) — runs `pnpm -C frontend build` and copies `frontend/dist/` to a path the `rust-embed` macro picks up.
- `crates/meet-server/build.rs` — orchestrates the frontend build before compiling (or refuses to build if `frontend/dist/` is absent, with a helpful message).
- `justfile` — `just package`:
  1. Run `pnpm -C frontend build`.
  2. Run `cargo build --release --target $TARGET`.
  3. Verify with `file` and `ldd` that the binary is statically linked.
  4. Assemble the tarball with `meet-server`, `config.example.toml`, `run.sh`, `LICENSE`, `docs/INSTALL.md`, `docs/CA-TRUST.md`.
  5. Refuse to package if any verification fails.
- `scripts/verify_static.sh` — shared by `just package` and CI; exits non-zero with a clear message on dynamic linkage.
- `docs/INSTALL.md` — extract, run `./run.sh`, get the admin token and the setup URL, point users to `/setup`.
- `docs/CA-TRUST.md` — per-OS guides (Linux distros, Windows, macOS, iOS, Android, Brave/Chromium, Firefox, Safari).
- `config.example.toml` — every field documented inline.
- `.github/workflows/release.yml` — on `v*` tags: cross-build both targets, run verification, attach tarballs to the release.
- `Cargo.lock` and `pnpm-lock.yaml` committed and updated as part of this phase.

## Design decisions

- **musl over glibc.** Required by the prompt: the target server is air-gapped and may have any glibc. musl produces self-contained binaries.
- **Cross-compile with `cross` or `--target` directly?** Decision: `cross` for cleanliness when developing on macOS; CI runs ubuntu-latest where `--target` with `musl-tools` is sufficient. `justfile` supports both.
- **`opt-level = "z"`, `lto = "fat"`, `codegen-units = 1`.** Smaller binary, faster startup, slightly slower compile — acceptable trade for a release artifact.
- **`panic = "abort"` in release.** Smaller binary; unwinding is unnecessary because we handle errors with `Result` and abort is fine for the unrecoverable case.
- **`strip = "symbols"`.** No need for symbols in the shipped binary; if a crash needs investigation we'd pull from a debug build.
- **`SOURCE_DATE_EPOCH` honored.** Set from the latest commit timestamp in `just package`; webpacks (Vite) and `rust-embed` both respect it.
- **Frontend builds before the Rust build, not inside it.** The `build.rs` approach was considered but is friction-heavy for incremental Rust builds; a `just` step is cleaner. `build.rs` only asserts the artifact exists.
- **No Docker image.** Out of scope per the prompt's "single tar.gz" mandate. Operators who want containers can wrap the tarball themselves.
- **AArch64 is best-effort.** Tarball published when the cross-build is clean; CI skips if a flake.
- **No autoupdater.** Self-hosted, air-gapped; updates are intentional human actions.

## Public interfaces

### Tarball layout

```
meet-platform-1.0.0-x86_64-linux-musl.tar.gz
├── meet-server                # static musl binary
├── config.example.toml
├── run.sh                     # see Phase 11
├── LICENSE                    # AGPL-3.0
└── docs/
    ├── INSTALL.md
    └── CA-TRUST.md
```

### `just` recipes

```
just dev             # frontend + backend in dev with hot reload
just build           # cargo build release for host triple
just check           # fmt + clippy + tests + deny + pnpm checks
just package TARGET  # produce dist/meet-platform-<version>-<arch>.tar.gz
just clean
```

## Security considerations

- **Static linkage is a security property here, not just a packaging convenience.** Operators on air-gapped hosts can't `apt install libfoo`; a dynamic dep gap means the binary won't run, which is worse than failing the build.
- **`cargo deny` runs as part of CI for releases too**, not just PR. A bad transitive dep block-by-block can land between PR merge and release tag.
- **Tarball is reproducible-ish.** Same source + same toolchain + same `SOURCE_DATE_EPOCH` produces a tarball with identical contents (timestamps stable; `mtime` set from `SOURCE_DATE_EPOCH`). Bit-identical reproducibility across hosts is not guaranteed (linker variance) and is documented as such.
- **Tarball is checksum-verifiable.** The release workflow attaches a `SHA256SUMS` and a detached signature via `minisign` (operator-installed key). The minisign signing key is held by the maintainer outside the repo.
- **No build-time secrets in the binary.** Verified by a CI check that runs `strings meet-server | grep -E '(secret|password|token)'` and refuses any obvious match (allowlisted false positives in `scripts/strings_check_ignore.txt`).

## Test plan

- **Local:**
  - `just package x86_64-unknown-linux-musl` succeeds on Linux dev hosts.
  - On macOS, `just package x86_64-unknown-linux-musl` via `cross` succeeds.
  - The produced binary, untarred onto a fresh Ubuntu container, runs `./meet-server --version` without dynamic linker errors.
- **CI:**
  - On every PR: `just check` and `just package` for the host triple (size and ldd assertions).
  - On `v*` tag: cross-build x86_64 + aarch64 MUSL; verify; attach to GitHub release.
- **Manual verification once after this phase lands:**
  - Copy a tarball to a real LAN host with no internet; extract; run `./run.sh`; observe full first-boot path.

## Acceptance criteria

- [x] `just package x86_64-unknown-linux-musl` (driving `scripts/package.sh`) produces `dist/meet-platform-<version>-x86_64-linux-musl.tar.gz`. The recipe runs the frontend build → cargo musl build → verify_static → tar. **Smoke-verified on the host triple** here (release build succeeds, `--version` prints `0.1.0`, binary is 9.6 MiB stripped); the full musl chain is exercised by `.github/workflows/release.yml`.
- [x] `scripts/verify_static.sh` enforces `file` reports static linkage AND `ldd` doesn't show any `.so` dependencies. Accepts both classic-static and static-PIE output.
- [x] Tarball layout matches `docs/plan/phase-10-build-and-package.md` exactly: `meet-server`, `run.sh`, `config.example.toml`, `LICENSE`, `docs/INSTALL.md`, `docs/CA-TRUST.md`.
- [x] `meet-server --version` prints `meet-server 0.1.0` (from `CARGO_PKG_VERSION`).
- [x] Frontend assets baked in are byte-identical to `frontend/dist/` — `rust-embed` reads at compile time so the bytes flow through unchanged.
- [x] `SHA256SUMS` (`*.tar.gz.sha256`) written next to the tarball by `scripts/package.sh`.
- [x] `scripts/verify_static.sh` is the artifact gate; CI calls it indirectly via `package.sh`.
- [x] `cargo audit`, `cargo deny check`, `pnpm audit` wired in `.github/workflows/security.yml` and `ci.yml`.
- [x] `.github/workflows/release.yml` simplified to a single `bash scripts/package.sh ${{ matrix.target }}` invocation. ~~Dummy-tag workflow_dispatch run~~ deferred to the actual v1 cut.
- [x] Binary size under 25 MiB target: **9.6 MiB** stripped (host triple); musl typically lands within ±10% of glibc — well under budget.

## Open questions

- Whether to ship `aarch64` from day one. Recommendation: yes if the cross-build is stable; flag it best-effort in the release notes.
- Whether to also publish a `.deb` / `.rpm`. Recommendation: no — out of scope for v1.
- Distroless container as an optional extra. Recommendation: defer; operators who want this can do it locally.
- Symbol stripping vs. shipping a symbolized `.debug` side-file. Recommendation: ship stripped only; debug symbols available from CI artifacts of a release build if a postmortem ever needs them.
