# Operating T-meet

Day-2 guidance: status, backups, rotation, systemd, troubleshooting.

## Status

```
./meet-server admin status
```

Prints non-sensitive operational state — no passphrase needed:

```
data_dir         /opt/meet-platform/data
leaf valid from  2026-05-14 19:58:30 +00:00:00
leaf valid until 2026-08-12 20:03:30 +00:00:00
leaf days left   89
rooms            3
audit entries    42
db size (bytes)  98304
```

Use this in a periodic monitoring check; if `leaf days left` drops below
~7 the server should be restarted soon so auto-rotation kicks in (the
rotation threshold is 60 days remaining).

## Pre-flight: `doctor`

```
./meet-server doctor
```

Read-only sanity check: required files exist, permissions are `0600`,
the configured ports are bindable. Returns exit code 1 on any FAIL row.
Run before any upgrade.

## Backups

### Cold backup (recommended)

```
systemctl stop meet-platform
tar czf meet-backup-$(date +%Y%m%d).tar.gz -C /opt/meet-platform data/
systemctl start meet-platform
```

Cold backup gives you a guaranteed-consistent snapshot of the SQLite WAL,
the encrypted CA bundle, and the admin secret. The archive is sensitive
— treat it like a copy of the production secrets, because it is.

### Hot backup

If downtime isn't acceptable, use SQLite's online backup API:

```
sqlite3 /opt/meet-platform/data/meet.db ".backup '/tmp/meet.db.bak'"

# Then archive everything else cold-ish (no in-flight writes touch these):
tar czf meet-backup-$(date +%Y%m%d).tar.gz \
    -C /opt/meet-platform \
    --exclude='data/meet.db*' \
    data/

# Add the consistent DB copy:
tar -rf meet-backup-$(date +%Y%m%d).tar /tmp/meet.db.bak
```

### Restore

1. `systemctl stop meet-platform`.
2. `rm -rf /opt/meet-platform/data && mkdir -p /opt/meet-platform/data`.
3. `tar xzf meet-backup-<date>.tar.gz -C /opt/meet-platform`.
4. `chown -R meet:meet /opt/meet-platform/data`.
5. `systemctl start meet-platform`.

The same passphrase decrypts the restored CA blob.

## Rotating the admin token

```
./meet-server admin token regenerate
```

Asks for the passphrase, rotates the admin secret, prints a new admin
token ONCE. Every outstanding admin token is immediately invalid.

Common triggers:

- Token leaked / suspected leaked.
- Operator handover.
- Scheduled rotation policy.

## Updating the binary

1. Download + verify the new tarball ([INSTALL.md](INSTALL.md)).
2. `systemctl stop meet-platform`.
3. Replace `meet-server` in `/opt/meet-platform/`.
4. `./meet-server doctor` (catches permission regressions).
5. `systemctl start meet-platform`.

The same passphrase decrypts the existing `data/` blobs — no
re-trust of the CA needed.

## Logs

T-meet logs to stdout. Two delivery models:

- **systemd journald** (recommended) — comes for free with the example
  unit. Query with `journalctl -u meet-platform`.
- **File** — wrap `./run.sh` in a script that redirects stdout to
  `/var/log/meet/meet-server.log`, then install
  `examples/logrotate-meet`.

The access log at info level contains method, path, status,
latency_ms, request_id. No IPs, no tokens, no passwords, no chat
content. Set `log.level = "debug"` in `config.toml` to surface
per-request IP addresses for triage; rotate back to `info` afterwards.

## What's safe to delete

- `frontend/dist/` — rebuilt automatically by the binary's embedded copy.
- `target/` — only relevant on build hosts.
- Audit log entries older than a retention policy you define — the
  table is append-only by design but you can `DELETE FROM audit_log
  WHERE at < strftime('%s','now','-180 days')` if storage is tight.

## What's NOT safe to delete

- `data/ca.bin` — losing this means every joined device has to re-trust
  a new CA.
- `data/admin.bin` — losing this means no admin auth until a fresh
  `init` (which orphans existing rooms).
- `data/leaf.key` — losing this means an immediate cert mismatch; new
  serves regenerate but in-flight TLS sessions die.
- `data/meet.db` — losing this drops every room.

## Passphrase loss

There is no recovery. The CA + admin secret + per-room secrets are all
sealed with the passphrase-derived key. If the passphrase is lost:

1. Stop the server.
2. Remove `data/` and start over with `./run.sh`.
3. Every device has to re-trust the new CA.
4. Every room has to be re-created.

Treat the passphrase the way you'd treat a SSH master key: write it
down, secure it, share it with at most one other operator.

## systemd unit

See [`examples/meet-platform.service`](../examples/meet-platform.service)
for a fully hardened drop-in. Highlights:

- Runs as a dedicated `meet` user.
- `EnvironmentFile=/etc/meet/passphrase.env` (mode 0600) supplies the
  passphrase.
- `ProtectSystem=strict` + `ReadWritePaths=/opt/meet-platform/data`
  pins write access to one directory.
- Restart on failure with a 5-second cool-down.

## Common issues

| Symptom | Cause | Fix |
|---|---|---|
| `admin token regenerate failed: aead encrypt / decrypt failed` | Wrong passphrase | Try again with the correct one |
| `init refuses to run` | `data/ca.bin` already exists | Use `serve` instead, or remove `data/` to start fresh (irreversible) |
| `429 Too Many Requests` on join | Rate limit | Wait the `Retry-After` window |
| Browser cert warning | Device doesn't trust the CA | Send user to `/setup` |
| `bind: address already in use` | Port conflict | Pick different ports in `config.toml` |
