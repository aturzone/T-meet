# Phase 11 — First-Boot & Ops

## Goal

Make the artifact from Phase 10 trivial to operate. `run.sh` is the single entrypoint. On first boot it detects a missing `data/` directory, takes the admin passphrase (from `MEET_ADMIN_PASSPHRASE` or an interactive prompt), runs `meet-server init` (which generates the CA, the first leaf, the admin token, and the SQLite DB) and prints exactly three things to stdout: the admin token (once), the CA download URL, and the setup page URL with copy-paste end-user instructions. On subsequent boots it asks only for the passphrase and starts `meet-server serve`. A systemd unit is documented for users who want it, and a backup guide explains what to copy.

## Deliverables

- `run.sh` — Bash entrypoint following the workflow described in Goal; lives at the tarball root.
- `meet-server init` and `meet-server serve` subcommands — finalize CLI started in Phase 01.
- `meet-server admin token regenerate` — re-mint the admin token if lost (requires the passphrase; the old token is invalidated by rotating the admin secret).
- `meet-server admin status` — prints `data/` location, leaf cert expiry, room count, audit log size. No secrets.
- `docs/INSTALL.md` — step-by-step.
- `docs/OPS.md` — backup/restore, log rotation, systemd unit example, "what to do if the passphrase is lost" (irrecoverable; documented honestly).
- `docs/CA-TRUST.md` — already drafted in Phase 10; expanded with screenshots if useful.
- `examples/meet-platform.service` — systemd unit; uses `EnvironmentFile=/etc/meet/passphrase.env` with mode 0600.
- `examples/logrotate-meet` — logrotate config for the optional file sink.
- `config.example.toml` — fully documented; copy-paste-able.

## Design decisions

- **`run.sh` is intentionally tiny.** It exists so the operator can `./run.sh` and have everything Just Work. Heavy logic lives inside the binary; the script orchestrates passphrase intake and the init-vs-serve branch.
- **Passphrase intake order:**
  1. `MEET_ADMIN_PASSPHRASE` env var (preferred for systemd).
  2. Interactive prompt via `rpassword` (preferred for first-boot humans).
  3. Refuse to start. We never read from a file by default to avoid `chmod`-hostage scripts.
- **`meet-server init` prints once.** Re-running `init` against an existing `data/` is an error. The admin token is never re-displayed; if lost, regenerate.
- **First-boot stdout is the contract.** Operators copy three values: admin token, CA URL, setup URL. Everything else goes to logs.
- **Systemd unit is an example, not a requirement.** Some users prefer `nohup ./run.sh &`; documented as supported.
- **Backup = "stop the server, copy `data/`, restart".** SQLite WAL means a hot copy can be inconsistent; cold copy is the safe default. Operators who want hot backups can use `sqlite3 .backup`; documented in `docs/OPS.md`.
- **Logs are stdout-by-default.** Operators pipe to journald (systemd) or a file (with logrotate). No built-in file logger.
- **Health checks via `/healthz`.** Already in Phase 02; documented here for ops.
- **No auto-update.** Operator runs `./run.sh` against a new tarball; passphrase carries forward (encrypted CA in `data/`).

## Public interfaces

### `run.sh` (extracted)

```bash
#!/usr/bin/env bash
set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
cd "$HERE"

if [ -z "${MEET_ADMIN_PASSPHRASE:-}" ]; then
    read -srp "Admin passphrase: " MEET_ADMIN_PASSPHRASE; echo
    export MEET_ADMIN_PASSPHRASE
fi

if [ ! -d data ]; then
    ./meet-server init
fi

exec ./meet-server serve
```

### CLI

```
meet-server init                       # one-time: generate CA, leaf, admin token, DB
meet-server serve                      # listen
meet-server admin token regenerate     # rotate admin secret; prints new admin token once
meet-server admin status               # human-readable status (no secrets)
meet-server --version
meet-server --config path/to/config.toml ...   # override default ./config.toml
```

### First-boot stdout (copy this contract; never break it)

```
================================================================
T-meet — first-boot setup complete

  Admin token (save this — it is shown ONCE):
    v4.local.xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx

  Download the CA cert and trust it on every device that will join:
    https://<host>:<port>/ca.crt

  Send users this setup page:
    https://<host>:<port>/setup
================================================================
```

### `examples/meet-platform.service`

```ini
[Unit]
Description=T-meet self-hosted meeting platform
After=network.target

[Service]
Type=simple
User=meet
Group=meet
WorkingDirectory=/opt/meet-platform
EnvironmentFile=/etc/meet/passphrase.env
ExecStart=/opt/meet-platform/meet-server serve
Restart=on-failure
RestartSec=5
NoNewPrivileges=true
ProtectSystem=strict
ProtectHome=true
PrivateTmp=true
ReadWritePaths=/opt/meet-platform/data
CapabilityBoundingSet=
AmbientCapabilities=

[Install]
WantedBy=multi-user.target
```

## Security considerations

- **Passphrase never written to disk.** `run.sh` reads into a shell variable; the binary reads from env and clears the env entry as soon as it's derived. Documented.
- **Admin token printed once.** Regeneration requires the passphrase. Loss of both is unrecoverable by design — `docs/OPS.md` says so plainly.
- **`init` is one-shot.** No re-init over an existing `data/` to prevent accidental CA replacement and the resulting trust break across all clients.
- **`status` reports only non-sensitive metadata.** Reviewed in Phase 09's pentest checklist.
- **Systemd unit hardening flags** (`NoNewPrivileges`, `ProtectSystem=strict`, `PrivateTmp`, `CapabilityBoundingSet=`) are in the example.
- **EnvironmentFile in the example is mode 0600** and owned by the `meet` user; documented.
- **Backup security:** the `data/` directory contains the encrypted CA blob, the leaf private key (chmod 0600), the SQLite DB. Operators are warned to back up to encrypted storage; the data is sensitive even though core secrets are sealed with the passphrase.
- Cross-references: prompt §4.4, §4.5, §4.8, §4.13.

## Test plan

- **Manual (must pass end-to-end before declaring v1 done):**
  1. On a fresh Linux box: `tar xzf meet-platform-1.0.0-x86_64-linux-musl.tar.gz && cd meet-platform-1.0.0/`.
  2. `./run.sh`, enter a passphrase, observe the three first-boot lines.
  3. From a second machine on the LAN: download `/ca.crt`, trust it in the OS store, open `/setup`, confirm no browser warning.
  4. Use the admin token to create a room via curl; receive the join URL + password.
  5. Open the join URL in two browsers on different machines; complete a 5-minute call with audio + video + chat.
  6. Stop the server (Ctrl-C). Restart via `./run.sh` with the same passphrase; existing room still joinable.
  7. `./meet-server admin status` reports the right room count and leaf cert expiry.
  8. `./meet-server admin token regenerate` issues a new token; the old one is rejected by `/admin/*`.
- **CI:**
  - A smoke test in `release.yml` extracts the tarball into a clean Ubuntu container and runs `./meet-server --version`.
  - A docker-compose-driven integration test runs `init`, `serve`, and a single API call. (Not blocking on flakes; informational.)
- **Documentation:**
  - `docs/INSTALL.md`, `docs/OPS.md`, `docs/CA-TRUST.md` reviewed by a second pair of eyes (the user); pass-through pen runs from prompt §4 confirmed.

## Acceptance criteria

- [ ] `run.sh` follows the documented workflow exactly.
- [ ] First-boot stdout matches the documented contract; admin token shown once and only once.
- [ ] `meet-server init` refuses to run against an existing `data/`.
- [ ] `meet-server serve` starts and serves all routes from earlier phases.
- [ ] `meet-server admin token regenerate` rotates the admin secret and prints the new token.
- [ ] `meet-server admin status` prints non-sensitive status.
- [ ] systemd unit example tested on a Linux host.
- [ ] `docs/INSTALL.md`, `docs/OPS.md`, `docs/CA-TRUST.md` are present and accurate.
- [ ] End-to-end LAN test (steps 1–8 above) all pass.
- [ ] `just check` is green; release tarball builds via `.github/workflows/release.yml`.
- [ ] CHANGELOG `[Unreleased]` becomes `[1.0.0] — <date>` in the same PR that closes Phase 11.

## Open questions

- Whether to print the leaf cert fingerprint on first-boot stdout. Recommendation: yes — it's the simplest in-band verification a security-minded operator can perform.
- Whether to ship a tiny `meet-server doctor` subcommand that pre-flights file permissions and ports. Recommendation: yes; small, useful, low risk.
- Whether the CA cert should be auto-served at port 80 (plain HTTP) so users on locked-down devices can grab it without trusting first. Recommendation: yes — the HTTP redirect listener already exists; an exception for `GET /ca.crt` is small and well-bounded. Document the trade-off.
