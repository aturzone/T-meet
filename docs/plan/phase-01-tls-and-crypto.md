# Phase 01 — TLS & Crypto Foundation

## Goal

Stand up the cryptographic spine of the server: generate a local CA on first boot, derive a key from the admin passphrase, persist the CA private key encrypted at rest, issue leaf certs covering the configured IPs/hostnames, build a `rustls` server config that uses them, and serve a public `/ca.crt` endpoint so end-users can trust the CA once. After this phase, `meet-server init` and `meet-server serve` both work even without any HTTP routes mounted.

## Deliverables

- `crates/meet-core/src/crypto/passphrase.rs` — `derive_key(passphrase: &SecretString, salt: &[u8]) -> [u8; 32]` using `argon2id` (m=64MiB, t=3, p=1).
- `crates/meet-core/src/crypto/seal.rs` — `seal(key, plaintext) -> Vec<u8>` / `open(key, ciphertext) -> Result<Vec<u8>>` using `chacha20poly1305` with random 24-byte nonce prepended.
- `crates/meet-core/src/crypto/ca.rs` — `generate_ca()` returning `(cert_pem, key_pem)` via `rcgen`; long-lived (10 years), `BasicConstraints::Ca(Constrained(0))`.
- `crates/meet-core/src/crypto/leaf.rs` — `issue_leaf(ca, sans: &[SanEntry]) -> (cert_pem, key_pem)`; 90-day validity; `SanEntry::Ip(IpAddr)` or `SanEntry::Dns(String)`.
- `crates/meet-core/src/crypto/rotation.rs` — pure logic that, given a leaf cert and a clock, returns `Action::Reissue` once age >= 60 days. Uses a trait `Clock` so tests can freeze time.
- `crates/meet-server/src/tls.rs` — `build_rustls_config(ca, leaf) -> ServerConfig`; TLS 1.3 only; AEAD ciphers only.
- `crates/meet-server/src/init.rs` — `cmd_init(config)`: prompt or read passphrase, generate CA, issue first leaf, write encrypted blob to `data/ca.bin`, write leaf to `data/leaf.pem` + `data/leaf.key` (key chmod 0600), write CA cert (PEM) to `data/ca.crt` for serving.
- `crates/meet-server/src/passphrase.rs` — read `MEET_ADMIN_PASSPHRASE` env var or prompt on TTY (`rpassword`); zeroize on drop (`secrecy::SecretString`).
- `crates/meet-server/src/routes/ca.rs` — `GET /ca.crt` returns `data/ca.crt` with `Content-Type: application/x-pem-file`.
- Migrations directory `migrations/` empty for now (created in Phase 02).
- Tests with a frozen clock for rotation logic.

## Design decisions

- **`rcgen` for CA + leaf.** Pure Rust, no OpenSSL, supports IP SAN entries (required by prompt §4.2).
- **`argon2id` parameters m=64MiB, t=3, p=1.** Prompt §4.5 floor. Tuned higher only if benchmarks show >1s on a Raspberry-Pi-class machine.
- **`chacha20poly1305` not `aes-gcm`.** ChaCha is constant-time without AES-NI and avoids nonce-misuse cliffs in the wider API surface. Both are AEAD; either would satisfy the prompt.
- **Random 24-byte nonce per `seal` call.** Encoding the nonce in front of the ciphertext keeps the format self-describing; we sacrifice 24 bytes per blob for no nonce-tracking state.
- **`SecretString` from `secrecy` crate.** Forces `expose_secret()` at the use-site and zeroizes on drop. Cheap insurance against accidental `Debug` leaks.
- **`rpassword` for interactive prompts.** Avoids echoing the passphrase to the terminal.
- **CA validity 10 years, leaf 90 days, reissue at 60 days.** Matches modern public-CA practice; users only have to trust the CA once.
- **`/ca.crt` is unauthenticated.** It's a public cert; secrecy doesn't apply. Rate limiting added in Phase 09.
- **Plaintext PEM of leaf cert and key on disk, key file chmod 0600.** The leaf is short-lived and re-derivable. Encrypting it would add a startup cycle for marginal gain. The CA key is the long-lived secret and is encrypted.

## Public interfaces

```rust
// meet_core::crypto
pub fn derive_key(passphrase: &SecretString, salt: &[u8; 16]) -> [u8; 32];
pub fn seal(key: &[u8; 32], plaintext: &[u8]) -> Vec<u8>;          // returns nonce || ciphertext || tag
pub fn open(key: &[u8; 32], blob: &[u8]) -> Result<Vec<u8>, CryptoError>;

pub struct CaMaterial { pub cert_pem: String, pub key_pem: SecretString }
pub fn generate_ca() -> CaMaterial;

pub enum SanEntry { Ip(IpAddr), Dns(String) }
pub fn issue_leaf(ca: &CaMaterial, sans: &[SanEntry], now: SystemTime) -> LeafMaterial;

pub trait Clock { fn now(&self) -> SystemTime; }
pub enum Action { None, Reissue }
pub fn rotation_action(leaf: &LeafInfo, clock: &dyn Clock) -> Action;
```

```
GET /ca.crt -> 200, application/x-pem-file, body = PEM-encoded CA cert
```

## Security considerations

- Passphrase enters memory only via `SecretString`; never logged, never serialized. Phase 09 audit greps for any `expose_secret` outside `crypto::*` and the startup module.
- CA private key persisted only in encrypted form. The plaintext leaf private key is chmod 0600. Loss of the admin passphrase is unrecoverable by design — documented in `docs/plan/phase-11-first-boot-and-ops.md`.
- Rotation timing uses an explicit `Clock` trait so we can unit-test the boundary without `tokio::time::pause`.
- `rustls` configuration disables TLS 1.0/1.1/1.2 and accepts only AEAD ciphersuites — set with `with_safe_default_protocol_versions().with_safe_default_cipher_suites()`.
- The OS RNG is the sole entropy source (`OsRng`) — never `thread_rng` for any keying material.
- Cross-references: prompt §4.1, §4.2, §4.3, §4.5, §4.7, §4.11.

## Test plan

- **Unit (meet-core):**
  - `derive_key` returns 32 bytes; same input ⇒ same output; different salt ⇒ different output.
  - `seal`/`open` round-trip; tampered ciphertext ⇒ `CryptoError::Decrypt`.
  - `generate_ca` produces a parseable cert whose `BasicConstraints::ca` is true.
  - `issue_leaf` produces a cert whose SAN list matches the input.
  - `rotation_action` returns `None` at age 0 days; `Reissue` at age 60 days; `Reissue` at expiry.
- **Integration (meet-server):**
  - `cmd_init` against a temp dir writes the expected files and exits 0.
  - Re-running `cmd_init` against an existing dir refuses with a clear error.
  - `cmd_serve` loads the encrypted CA, issues a fresh leaf if the existing one is past 60 days, builds the rustls config, and a curl with `--cacert data/ca.crt` to `https://127.0.0.1:<port>/ca.crt` returns the CA PEM.
- **Manual:** start the server, paste the CA into the OS trust store, hit `https://127.0.0.1:<port>/ca.crt` in Brave — no browser warning.

## Acceptance criteria

- [ ] `cargo test -p meet-core --lib` covers all unit tests above.
- [ ] `cargo test -p meet-server` covers the integration tests above.
- [ ] `meet-server init` succeeds against an empty `data/` and writes `data/ca.bin`, `data/leaf.pem`, `data/leaf.key`, `data/ca.crt`.
- [ ] `meet-server serve` starts an HTTPS listener that serves `/ca.crt` correctly.
- [ ] Brave with the CA trusted opens `https://<server-ip>:<port>/ca.crt` with no warning.
- [ ] Leaf cert auto-rotates: a test with a frozen clock past day 60 reissues a new leaf and reloads `rustls`.
- [ ] No `unwrap`/`expect` outside tests and `main`.
- [ ] No `openssl-sys` in `cargo tree`.
- [ ] `tracing::info!` on init/serve does not include the passphrase, the CA private key, or the leaf private key.

## Open questions

- Whether to support automatic CA cert rotation. Recommendation: out of scope — 10-year CA is fine, and users have an escape hatch (re-init).
- Whether to expose the leaf cert fingerprint on `/setup` so users can verify the cert in-band. Recommendation: yes, decided in Phase 06.
- Whether the encrypted CA blob format should be versioned (a 4-byte magic + 1-byte version). Recommendation: yes — adds 5 bytes, saves a migration headache later.
