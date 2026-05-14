# Penetration test checklist

Run through this against a deployed v1 before declaring the install
production-ready. Every item below is something an attacker can verify from
the outside; the operator should verify in the same way.

> All curl commands assume the CA is trusted on the test machine. Replace
> `<host>:<tls-port>` with the configured values from `config.toml`.

## TLS / transport

- [ ] `curl --cacert <data>/ca.crt -I https://<host>:<tls-port>/healthz` →
      `200 ok`, every Phase 09 security header present.
- [ ] `curl -I http://<host>:<http-port>/healthz` → `301` to the HTTPS host.
- [ ] `curl -I http://<host>:<http-port>/ca.crt` → `200` (escape hatch).
- [ ] `openssl s_client -connect <host>:<tls-port> -tls1` fails (TLS 1.0 off).
- [ ] `openssl s_client -connect <host>:<tls-port> -tls1_1` fails (TLS 1.1 off).
- [ ] `openssl s_client -connect <host>:<tls-port> -tls1_3` succeeds and the
      cipher is AEAD (`TLS_AES_128_GCM_SHA256` or `TLS_CHACHA20_POLY1305_SHA256`).

## Headers (run against `/` and `/api/setup-info`)

- [ ] `content-security-policy` starts with `default-src 'self'; connect-src 'self' wss:`.
- [ ] `strict-transport-security: max-age=31536000; includeSubDomains`.
- [ ] `x-content-type-options: nosniff`.
- [ ] `referrer-policy: no-referrer`.
- [ ] `permissions-policy` includes `camera=(self), microphone=(self), geolocation=(), interest-cohort=()`.
- [ ] `cross-origin-opener-policy: same-origin`.
- [ ] `cross-origin-resource-policy: same-origin`.
- [ ] `x-request-id` is set on every response.

## Auth & abuse

- [ ] `curl -X POST https://<host>:<tls-port>/admin/rooms` without a Bearer →
      `401`.
- [ ] Six rapid `POST /r/<id>/join` with wrong password from the same IP →
      6th returns `429` with `Retry-After` ≥ 1.
- [ ] `POST /r/<unknown-id>/join` returns `401` (NOT `404`) — room existence
      isn't disclosed.
- [ ] After room creation, the response includes a password ONCE; subsequent
      `GET /admin/rooms/:id` does not return the password.
- [ ] Browser DevTools network log shows the admin token never appears in any
      URL or query string.

## WebSocket

- [ ] WSS upgrade requires a valid PASETO room token as the first frame; no
      `Join` within 5s → close code `4401`.
- [ ] WSS upgrade from an `Origin` header not matching the bind IP / external
      host → `403`.
- [ ] Binary frame or oversize (>64 KiB) frame → close `4413` / `4400`.
- [ ] A second WS connection with the same `pid` evicts the first with close
      `4409`.

## Chat E2E

- [ ] Two browsers join the room; sender A's plaintext is visible to A and
      to B. Open `data/meet.db` while messages are flying: `chat.*` rows
      appear in `audit_log` but contain no ciphertext bodies.
- [ ] Tamper a chat ciphertext at the WS layer (e.g. via DevTools) → recipient
      silently drops it; no plaintext leaks.
- [ ] Page reload clears scrollback (in-memory only).

## DB hygiene

- [ ] `stat data/meet.db` shows `0600`. Same for `data/leaf.key` and
      `data/ca.bin` and `data/admin.bin`.
- [ ] `sqlite3 data/meet.db "select details_json from audit_log"` shows no
      passwords / tokens / IPs.

## Static analysis

- [ ] `cargo audit` is clean (advisories file refreshed within the last 24h).
- [ ] `cargo deny check` is clean.
- [ ] `pnpm -C frontend audit --prod --audit-level moderate` is clean.

## Fuzz

- [ ] `cd crates/meet-core/fuzz && cargo +nightly fuzz run signaling_parse`
      runs for ≥ 60s without finding a panic. (CI doesn't run this — local /
      scheduled job.)

## Outbound network

- [ ] `ss -tnp | grep meet-server` shows only the bound TLS + HTTP ports.
      No outbound TCP connections from the server process.

If every box is checked, the deploy passes the v1 bar.
