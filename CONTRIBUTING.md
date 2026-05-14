# Contributing to T-meet

Thanks for your interest. This guide explains how we work and what we expect from contributions.

## Code of conduct

By participating you agree to abide by the [Contributor Covenant](CODE_OF_CONDUCT.md).

## Security issues

**Do not open a public issue for security vulnerabilities.** See [SECURITY.md](SECURITY.md) for the private disclosure channel.

## Repository workflow

- Default branch: `main`. Always green (`just check` passes).
- Feature work happens on short-lived branches: `phase-NN-<area>` or `fix/<short-description>` or `feat/<short-description>`.
- Pull requests are squashed by default. Keep them small and focused.
- The phased plan in [docs/plan/](docs/plan/README.md) is the source of truth for what gets built and in what order. Do not implement phase N+1 until phase N has every acceptance-criteria checkbox ticked.

## Commit messages

Per-phase commits follow this convention:

```
phase-NN: <area>: <verb> <thing>

Optional body that explains the *why*, not the *what*.
Reference docs/plan/phase-NN-<topic>.md sections when relevant.
```

Outside phase work, use a conventional prefix: `chore:`, `docs:`, `fix:`, `refactor:`, `test:`.

One logical change per commit. Atomic. Revertable.

## Required local checks before pushing

```bash
just fmt        # rustfmt + prettier
just lint       # clippy -D warnings + eslint
just test       # cargo test + vitest + playwright (when applicable)
just audit      # cargo-deny + pnpm audit
just check      # all of the above
```

CI runs the same matrix. If `just check` is clean locally, CI should pass.

## Code expectations

### Rust

- `#![forbid(unsafe_code)]` at every crate root. New `unsafe` requires a documented justification block and reviewer sign-off.
- No `.unwrap()` / `.expect()` outside tests and the program-startup paths. Even those `.expect("…")` calls must have descriptive messages.
- Errors via `thiserror` enums per module. No stringly-typed errors in libraries. `anyhow` only at the top level.
- Tests live next to the code: `#[cfg(test)] mod tests`.
- Modules over files. Split into a folder once a module crosses ~400 lines.
- `clippy::pedantic` is on; document each `#[allow(...)]`.

### TypeScript / React

- `strict: true` in `tsconfig`. No `any`. Use `unknown` + narrow.
- Components small and single-purpose. Hooks for logic, not for layout.
- Tests in `__tests__/` next to the component (Vitest). Playwright E2E lives under `frontend/e2e/`.
- Tailwind utility classes preferred; no ad-hoc CSS unless justified.

### Cross-cutting

- Comments explain *why*. If a *what* comment seems necessary, the code probably needs rewriting.
- Every public input passes a `zod` schema (frontend) and `serde` + explicit validators (backend).
- No `println!` in Rust outside tests. Use `tracing`. No `console.log` in frontend production code.
- No PII in logs (no IP addresses at info level, ever).

## Submitting a pull request

1. Pick or open an issue describing the change. For phase work, the corresponding `docs/plan/phase-NN-*.md` is the issue.
2. Branch from `main`.
3. Implement, write tests, ensure `just check` is clean.
4. Tick the acceptance criteria in the phase doc with `[x]` in the same PR.
5. Open the PR using the [template](.github/PULL_REQUEST_TEMPLATE.md). Fill out the security-impact note even if the answer is "none".
6. Address review comments. Maintainers may request additional tests or rationale.

## Questions

Open a [GitHub Discussion](https://github.com/aturzone/T-meet/discussions) (once enabled) or a regular issue if it's a clear bug or feature request.
