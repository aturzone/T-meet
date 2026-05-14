#!/usr/bin/env bash
# T-meet operator entry point. First boot: prompt for the admin passphrase,
# generate CA + leaf + admin token, then serve. Subsequent boots: prompt only,
# then serve.
#
# Honor MEET_ADMIN_PASSPHRASE in the environment so the systemd unit (and
# CI smoke runs) can skip the prompt.

set -euo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
cd "$HERE"

if [ -z "${MEET_ADMIN_PASSPHRASE:-}" ]; then
    if [ -t 0 ]; then
        read -srp "Admin passphrase: " MEET_ADMIN_PASSPHRASE
        echo
        export MEET_ADMIN_PASSPHRASE
    else
        echo "run.sh: no MEET_ADMIN_PASSPHRASE and no TTY" >&2
        exit 2
    fi
fi

# First boot: data/ doesn't exist (or is empty of the CA bundle).
if [ ! -f data/ca.bin ]; then
    ./meet-server init
fi

exec ./meet-server serve
