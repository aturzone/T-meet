#!/usr/bin/env bash
# Refuses to package a meet-server binary that isn't statically linked.
# Used by `just package` and by the release workflow in CI.

set -euo pipefail

if [ $# -ne 1 ]; then
    echo "usage: $0 <path-to-meet-server-binary>" >&2
    exit 2
fi

BIN="$1"
if [ ! -x "$BIN" ]; then
    echo "verify_static: $BIN is not executable" >&2
    exit 1
fi

step() {
    printf "\n\033[1;34m==> %s\033[0m\n" "$*"
}

step "file $BIN"
FILE_OUT=$(file "$BIN")
echo "$FILE_OUT"

# Static-PIE binaries report "pie executable" + "static-pie linked"; classic
# static is "statically linked". Accept either.
if ! grep -qE 'static(ally)?-?(pie)? *linked|statically linked' <<<"$FILE_OUT"; then
    echo "verify_static: file output does not mention static linkage" >&2
    exit 1
fi

step "ldd $BIN"
LDD_OUT=$(ldd "$BIN" 2>&1 || true)
echo "$LDD_OUT"

# `ldd` on a static-PIE binary prints "statically linked", on classic-static
# it prints "not a dynamic executable". Reject anything that looks like a
# real shared-object dependency list.
if grep -qE '\.so[0-9.]*' <<<"$LDD_OUT"; then
    echo "verify_static: ldd shows shared-object dependencies" >&2
    exit 1
fi

if ! grep -qE 'statically linked|not a dynamic executable' <<<"$LDD_OUT"; then
    echo "verify_static: ldd output is unexpected" >&2
    exit 1
fi

# Sanity: every release binary must run --version cleanly with no glibc /
# loader errors at startup.
step "$BIN --version"
"$BIN" --version

echo
echo "verify_static: OK"
