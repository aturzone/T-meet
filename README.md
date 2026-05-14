# T-meet

A small, secure, self-hostable video meeting platform.

- **Single statically-linked binary** (Rust + musl) with the React frontend embedded inside.
- **No internet, no package manager, no dependencies on the target server.** Extract the tarball, run `./run.sh`, done.
- **No domain name, no public CA.** A local CA is generated on first boot; users download and trust it once per device, then access rooms at `https://<server-ip>:<port>/r/<room-id>`.
- **End-to-end encrypted chat.** WebRTC DTLS-SRTP for media. PASETO v4 tokens.
- **Designed for 10–20 participants per room** with several concurrent rooms on a modest VPS.

> Status: **planning stage**. This repository currently contains the full design document set in [docs/plan/](docs/plan/README.md). No code has been written yet. See the [bootstrap brief](prompt.txt) for the project's original mission statement.

## Quick links

- [Plan index — start here](docs/plan/README.md)
- [Security policy](SECURITY.md)
- [Contributing guide](CONTRIBUTING.md)
- [Changelog](CHANGELOG.md)
- [License (AGPL-3.0)](LICENSE)

## Why another video meeting tool?

- **Air-gapped friendly.** Many teams cannot install software on production servers and cannot reach the public internet from them. The tarball deploy model fits that constraint without compromise.
- **No SaaS, no telemetry.** The server makes zero outbound network calls. Verifiable with `ss -tnp`.
- **Self-contained TLS.** No Let's Encrypt, no Cloudflare, no domain registration. A local CA you trust once per device.
- **Single binary, single tarball.** No Docker, no Kubernetes, no `apt install`.

## Building from source (once Stage B starts)

Prerequisites on the **developer** machine (not the production server):

- `rustup` with the `x86_64-unknown-linux-musl` target installed.
- `node` 20+ and `pnpm` (via Corepack).
- `just`.
- `musl-tools` (or `cross`).

Then:

```bash
just build       # build everything
just check       # fmt + clippy + tests + frontend lint/typecheck/tests
just package     # produces dist/meet-platform-<version>-<arch>.tar.gz
```

The production server only needs the tarball and a working POSIX shell.

## Running on a server

Once `just package` produces a tarball:

```bash
scp dist/meet-platform-*.tar.gz user@server:~
ssh user@server
tar -xzf meet-platform-*.tar.gz
cd meet-platform-*
export MEET_ADMIN_PASSPHRASE='something-long-and-random'
./run.sh
```

The first boot prints the admin token (shown once) and the CA download URL. Distribute the CA to participants via `https://<server-ip>:<port>/setup` and they install it once per device. After that: no browser warnings.

## License

[GNU AGPL-3.0](LICENSE). If you run a modified version of T-meet as a network service, you must offer the modified source to your users.

## Security

Found a vulnerability? Please **do not** open a public issue. See [SECURITY.md](SECURITY.md) for the private disclosure channel.
