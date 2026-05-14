#!/usr/bin/env bash
# Produce dist/meet-platform-<version>-<arch>-linux-musl.tar.gz.
#
# Called by `just package`; also used by .github/workflows/release.yml.
#
# Required tools on the build host:
#  - cargo (stable)
#  - For x86_64 native musl: musl-gcc (`apt install musl-tools`)
#  - For aarch64 cross-build: `cross` (`cargo install --locked cross`)
#
# Honors SOURCE_DATE_EPOCH for reproducible-ish tarball timestamps.

set -euo pipefail

TARGET="${1:-x86_64-unknown-linux-musl}"

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

case "$TARGET" in
    x86_64-unknown-linux-musl) ARCH="x86_64" ;;
    aarch64-unknown-linux-musl) ARCH="aarch64" ;;
    *)
        echo "package.sh: unsupported target $TARGET" >&2
        exit 2
        ;;
esac

VERSION=$(grep -m1 '^version' Cargo.toml | sed -E 's/.*"([^"]+)".*/\1/')
if [ -z "${VERSION}" ]; then
    echo "package.sh: could not read workspace version from Cargo.toml" >&2
    exit 1
fi

PKG="meet-platform-${VERSION}-${ARCH}-linux-musl"
STAGE="dist/${PKG}"

# Reproducible-ish tarballs: pin mtimes to the latest commit timestamp.
if [ -z "${SOURCE_DATE_EPOCH:-}" ]; then
    if command -v git >/dev/null 2>&1 && [ -d .git ]; then
        SOURCE_DATE_EPOCH=$(git log -1 --pretty=%ct 2>/dev/null || echo 0)
    fi
    export SOURCE_DATE_EPOCH="${SOURCE_DATE_EPOCH:-0}"
fi

step() { printf "\n\033[1;34m==> %s\033[0m\n" "$*"; }

step "frontend build"
if command -v pnpm >/dev/null 2>&1; then
    pnpm -C frontend install --frozen-lockfile
    pnpm -C frontend build
elif [ -d frontend/dist ]; then
    echo "skipping pnpm — using existing frontend/dist"
else
    echo "package.sh: pnpm not available and frontend/dist missing" >&2
    exit 1
fi

step "cargo build --release --target ${TARGET}"
if [ "$TARGET" = "aarch64-unknown-linux-musl" ]; then
    if ! command -v cross >/dev/null 2>&1; then
        echo "package.sh: aarch64 needs the 'cross' tool (cargo install cross)" >&2
        exit 1
    fi
    cross build --release --target "$TARGET" -p meet-server
else
    cargo build --release --target "$TARGET" -p meet-server
fi

BIN="target/${TARGET}/release/meet-server"
[ -x "$BIN" ] || { echo "package.sh: built binary missing: $BIN" >&2; exit 1; }

step "verify static linkage"
bash scripts/verify_static.sh "$BIN"

step "assemble tarball"
rm -rf "$STAGE"
mkdir -p "$STAGE/docs"
cp "$BIN" "$STAGE/"
cp config.example.toml "$STAGE/"
cp run.sh "$STAGE/"
cp LICENSE "$STAGE/"
cp docs/INSTALL.md "$STAGE/docs/"
cp docs/CA-TRUST.md "$STAGE/docs/"
chmod +x "$STAGE/meet-server" "$STAGE/run.sh"

# Strip mtime variance + sort entries for byte-stable tarballs.
tar --sort=name \
    --owner=0 --group=0 --numeric-owner \
    --mtime="@${SOURCE_DATE_EPOCH}" \
    -czf "dist/${PKG}.tar.gz" -C dist "${PKG}"

# Checksum next to the tarball.
(cd dist && sha256sum "${PKG}.tar.gz" > "${PKG}.tar.gz.sha256")

step "done"
ls -la "dist/${PKG}.tar.gz" "dist/${PKG}.tar.gz.sha256"
