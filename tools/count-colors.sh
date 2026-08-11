#!/bin/sh
# count SGR color sequences in a typescript.
# ratatui emits indexed colors like 38;5;6;49m (fg+bg combined)
awk 'BEGIN{esc=sprintf("%c",27)} { n += gsub(esc "[[][0-9;]*38;5;[0-9]+[0-9;]*m", "&") } END { print n+0 }' "$1"
