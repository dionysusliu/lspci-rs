#!/bin/sh
# usage: tui-smoke.sh <keys-printf-format> <typescript-out>
KEYS="$1"
OUT="$2"
BIN=/workspace/target/x86_64-unknown-linux-gnu/debug/lspci-rs
printf '%b' "$KEYS" | script -qec "stty rows 30 cols 100; exec $BIN tui" "$OUT" >/dev/null
echo "exit=$?"
