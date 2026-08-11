#!/bin/sh
# usage: tui-smoke.sh <keys-printf-format> <typescript-out>
# NOTE: keys must be piped with a delay (e.g. `( sleep 1; printf ... ) |`)
# or they land in the pty canonical line buffer before raw mode is enabled
# and the run hangs. Use \r (not \n) for Enter.
KEYS="$1"
OUT="$2"
BIN="${BIN:-/workspace/target/x86_64-unknown-linux-gnu/debug/lspci-rs}"
printf '%b' "$KEYS" | script -qec "stty rows 30 cols 100; exec $BIN tui" "$OUT" >/dev/null
echo "exit=$?"
