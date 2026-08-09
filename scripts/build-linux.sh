#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
image_name="lspci-rs-alinux3-builder"

docker build \
    --platform linux/amd64 \
    -f "$repo_root/containers/el8-builder/Containerfile" \
    -t "$image_name" \
    "$repo_root"

docker run --rm \
    --platform linux/amd64 \
    -v "$repo_root:/workspace" \
    -w /workspace \
    "$image_name" \
    bash -lc '
        set -euo pipefail 
        cat /etc/os-release 
        rpm -q pciutils-libs pciutils-devel
        pkg-config --modversion libpci
        rustc --version
        cargo check -p pci-sys --target x86_64-unknown-linux-gnu
    '