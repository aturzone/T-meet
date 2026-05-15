# Installing T-meet

T-meet ships two ways:

1. **Docker Compose** — fastest. Unzip the release, set a passphrase,
   `docker compose up --build`. Skip to the Docker section below.
2. **Static binary** — single statically-linked `meet-server`. No
   containerization, no system services to install, no database to
   provision, no network calls to the outside world.

Both paths produce the same binary; pick whichever fits your operational
preferences.

## Docker Compose (recommended)

The release zip includes a multi-stage `Dockerfile` and a `docker-compose.yml`
that runs everything in one container, owned by an unprivileged user, with
a named volume for `data/`.

```bash
unzip t-meet-1.0.0.zip
cd t-meet-1.0.0
cp config.example.toml config.toml          # edit bind_ip / ports
echo "MEET_ADMIN_PASSPHRASE=correct horse battery staple" > .env
chmod 600 .env
docker compose up -d --build
```

First boot prints the admin token to the container log:

```bash
docker compose logs meet | head -40
```

Copy the admin token (shown ONCE), the CA URL, and the setup URL. The
container restarts on failure; subsequent boots re-use the same `data/`
volume and the same admin secret (no token reprint).

To stop:

```bash
docker compose down
```

To upgrade: pull the new release, `docker compose up -d --build`. The
`data/` volume carries forward — same passphrase decrypts the existing
CA bundle.

## Prerequisites

- A Linux host the meeting participants can reach (LAN, VPN, or 1:1-NAT'd
  public IP).
- Disk: ~25 MiB for the binary, plus space for `data/` (a SQLite file
  + the CA/leaf/admin blobs — typically a few MiB).
- An open TCP port for HTTPS (default 8443) and a second port for the
  HTTP → HTTPS redirect (default 8080).
- An admin passphrase you choose. Keep it somewhere safe — losing it
  means losing access to the CA and the admin secret.

## Install

1. Download the tarball from the GitHub release for your platform:
   `meet-platform-<version>-x86_64-linux-musl.tar.gz` (or aarch64).

2. Verify the checksum:

   ```
   sha256sum -c meet-platform-<version>-x86_64-linux-musl.tar.gz.sha256
   ```

3. Extract:

   ```
   tar xzf meet-platform-<version>-x86_64-linux-musl.tar.gz
   cd meet-platform-<version>-x86_64-linux-musl
   ```

4. Edit `config.example.toml` (the inline comments explain each field),
   save it as `config.toml`. The most important fields:

   - `server.bind_ip` — the IP you want clients to reach you on. Use
     `0.0.0.0` to bind everywhere, or the specific LAN/external IP.
   - `server.external_host` — set to your DNS name if you have one
     (e.g. `meet.example.lan`). Otherwise leave unset; the CA will be
     issued for the IP only.

## First boot

```
./run.sh
```

You'll be asked for an admin passphrase. The first boot:

- Generates the local CA + a 90-day leaf cert.
- Mints the admin token (printed **ONCE** — save it).
- Opens the HTTPS listener.

The banner ends with three things to copy:

- **Admin token** — needed for every `POST /admin/rooms` call.
- **CA download URL** — share with every device that will join a meeting.
- **Setup URL** — `https://<host>:<tls-port>/setup`, an in-browser page
  with per-OS trust instructions.

## Subsequent boots

```
./run.sh
```

Asks for the passphrase, no init step. The same admin token from first
boot keeps working until you rotate it with
`./meet-server admin token regenerate`.

For a non-interactive boot (e.g. systemd):

```
export MEET_ADMIN_PASSPHRASE='...'
./run.sh
```

The variable is read and immediately removed from the process
environment — child processes don't inherit it.

## Creating your first room

```
ADMIN=v4.local.<token-from-first-boot>

curl --cacert data/ca.crt \
  -H "Authorization: Bearer $ADMIN" \
  -H 'Content-Type: application/json' \
  -d '{"name":"My first room"}' \
  https://<host>:<tls-port>/admin/rooms
```

The response includes a randomly-generated password and a `join_url`.
Share the URL + password (separately) with attendees, after they've
trusted the CA per `docs/CA-TRUST.md`.

## What lives where

```
.
├── meet-server          # the static binary
├── run.sh               # entry point
├── config.toml          # your config (you create this)
├── config.example.toml  # documented defaults
├── LICENSE              # AGPL-3.0
├── docs/
│   ├── INSTALL.md
│   └── CA-TRUST.md
└── data/                # created on first boot (chmod 0700)
    ├── ca.bin           # encrypted CA bundle
    ├── ca.crt           # public CA cert (served at /ca.crt)
    ├── admin.bin        # encrypted admin secret
    ├── leaf.pem         # current leaf cert
    ├── leaf.key         # leaf private key (chmod 0600)
    └── meet.db          # SQLite room state (chmod 0600)
```

`data/` is the only stateful directory. Back it up cold (stop the
server, copy, restart) for a guaranteed-consistent snapshot. Hot
backups via `sqlite3 .backup` are documented in [OPS.md](OPS.md) once
that lands (Phase 11).

## Updating

1. Stop the running server (Ctrl-C, or `systemctl stop meet-platform`).
2. Extract the new tarball alongside the old install.
3. Copy your `config.toml` and `data/` directory across.
4. Start the new `./run.sh`. The same passphrase decrypts the existing
   blobs.

No schema migration steps for v1; the binary applies any new SQLite
migrations on startup.

## Troubleshooting

- **Browser shows a cert warning** — the CA isn't trusted on that
  device. Send the user to `https://<host>:<tls-port>/setup`.
- **First boot fails with `permission denied` on port 443/80** — those
  ports require root; pick higher numbers in `config.toml`
  (e.g. 8443 + 8080) and have your reverse proxy or firewall map them.
- **`run.sh: no MEET_ADMIN_PASSPHRASE and no TTY`** — running under a
  service manager without setting the env var. Set it in the unit
  file's `EnvironmentFile`.
- **`init` refuses to run** — `data/ca.bin` already exists. Use
  `./meet-server serve` directly, or wipe `data/` only if you really
  want a fresh CA (every device will need to re-trust).
