# Security Policy

## Supported Versions

T-meet is pre-1.0. During this period, only the latest tagged release is supported.

| Version | Supported |
|---------|-----------|
| Latest tag on `main` | Yes |
| Older tags | No |

Once 1.0 ships, the latest minor on the current major plus the prior major's last minor will be supported.

## Reporting a Vulnerability

**Do not open a public GitHub issue for security problems.**

Send a report to: `security@t-meet.invalid` (placeholder — replace before public launch).

Encrypt with the project's age key (published in the repo once the maintainer key is generated; see Phase 09).

Please include:

- A description of the issue and its impact.
- Steps to reproduce, or a proof-of-concept.
- The commit hash you tested against.
- Your name and affiliation if you want public credit; "anonymous" if you do not.

You will receive an acknowledgement within 72 hours. We aim to ship a fix within 90 days; severe issues sooner. A coordinated disclosure window will be agreed before any public write-up.

## Scope

**In scope:**

- The Rust server crates under `crates/`.
- The frontend under `frontend/`.
- The build pipeline (`justfile`, `.github/workflows/`, `Dockerfile` if present).
- The default configuration shipped in the release tarball.

**Out of scope:**

- Site-specific deployment configurations.
- Misuse stemming from running with `--insecure-*` flags (these are explicitly unsupported in production).
- Third-party reverse proxies in front of T-meet — report those upstream.
- Denial of service via raw bandwidth exhaustion against a single-binary, single-host deployment.

## Security Model Summary

See [prompt.txt](prompt.txt) §4 for the full threat model. Short version:

- No outbound network calls. No telemetry.
- TLS via `rustls`; no OpenSSL anywhere in the dependency graph.
- Argon2id for password hashing; PASETO v4 local for session tokens.
- End-to-end encrypted chat sidechannel (Phase 08). Media is SRTP between peers via the SFU; the SFU does not see media plaintext content beyond what DTLS-SRTP requires for routing.
- Single-binary deploy on air-gapped hosts is a first-class target.

## Hall of Fame

Reporters who follow this policy and verify a real issue will be listed here (with permission) after the fix ships.
