#!/bin/sh
# strip ANSI escape sequences from a typescript, replacing each with a space
# so rendered TUI text keeps word separation and can be grepped
awk 'BEGIN { esc = sprintf("%c", 27) } { gsub(esc "\\[[0-9;?]*[A-Za-z]", " "); print }' "$1"
