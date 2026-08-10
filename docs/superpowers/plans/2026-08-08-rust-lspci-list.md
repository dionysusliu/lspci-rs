# Rust lspci List Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a Linux-only Rust CLI that uses the system `libpci` through `bindgen` to enumerate real PCI devices and render text or stable JSON output.

**Architecture:** Use a Cargo workspace with `pci-sys` for generated unsafe bindings, `pci` for the owned Rust session/snapshot model, and `lspci-rs` for CLI parsing and rendering. Build on an EL8-compatible Linux builder, upload an `x86_64-unknown-linux-gnu` binary to Alibaba Linux 3, and validate it against the real ECS environment.

**Tech Stack:** Rust stable, Cargo workspace, `bindgen`, `pkg-config`, system `libpci`, `clap`, `serde`, `serde_json`, EL8-compatible container, GitHub Actions, SSH.

## Global Constraints

- Support Linux only in this plan.
- Target `x86_64-unknown-linux-gnu`.
- Use the system `libpci`; do not vendor `pciutils` or `libpci`.
- Generate raw C bindings with `bindgen`; keep all direct `unsafe` C access in `pci-sys`.
- `pci` returns owned Rust values; no raw C pointer or borrowed C string crosses into the CLI.
- The first release is read-only and non-privileged; it must not write PCI configuration or require `sudo`.
- The first CLI surface is `lspci-rs list`, with `--format text` and `--format json`.
- Use `<unknown>`, `<permission denied>`, and `<not available>` as string status markers; do not use `null` for readable-name fields.
- Do not add TUI, uevent/netlink listeners, configuration-space detail, driver operations, legacy `lspci` flags, or non-Linux backends.
- Do not create PCI fixtures; validation is performed against the real ECS system.
- The deployment target is Alibaba Linux 3 with EL8 userspace, glibc 2.32, and `libpci.so.3` from pciutils 3.8.0.
- Keep `.superpowers/` out of version control; it contains local brainstorming screens.
- Commit at each task boundary once the user initializes the Git repository; the current directory is not yet a Git repository.

## File Map

The implementation should create the following focused files:

```text
Cargo.toml                         workspace members and shared profile settings
Cargo.lock                         resolved dependency versions
rust-toolchain.toml                stable toolchain and Linux target
.gitignore                         build and local-artifact exclusions
README.md                          local build, ECS deployment, and CLI usage

crates/pci-sys/Cargo.toml          raw binding crate metadata
crates/pci-sys/build.rs            libpci discovery and bindgen invocation
crates/pci-sys/wrapper.h           allowlisted libpci header entry point
crates/pci-sys/src/lib.rs          generated bindings module and re-exports

crates/pci/Cargo.toml              safe library metadata
crates/pci/src/lib.rs              public library surface
crates/pci/src/device.rs            owned address, device, and snapshot types
crates/pci/src/error.rs             Rust-side error types
crates/pci/src/session.rs           libpci context ownership and scanning

crates/lspci-rs/Cargo.toml         binary metadata and CLI dependencies
crates/lspci-rs/src/main.rs        process entry and exit-code mapping
crates/lspci-rs/src/cli.rs         clap command model
crates/lspci-rs/src/output.rs      text and JSON renderers

containers/el8-builder/Containerfile  reproducible Linux build environment
scripts/build-linux.sh              local macOS-to-Linux builder wrapper
scripts/live-smoke.sh               real-host binary validation
.github/workflows/build-and-smoke.yml CI build, upload, and ECS smoke check
```

## Task 1: Create the workspace and reproducible toolchain contract

**Files:**
- Create: `Cargo.toml`
- Create: `rust-toolchain.toml`
- Create: `.gitignore`
- Create: `crates/pci-sys/Cargo.toml`
- Create: `crates/pci/Cargo.toml`
- Create: `crates/lspci-rs/Cargo.toml`
- Create: `crates/pci-sys/src/lib.rs`
- Create: `crates/pci/src/lib.rs`
- Create: `crates/lspci-rs/src/main.rs`

**Interfaces:**
- Produces workspace packages named `pci-sys`, `pci`, and `lspci-rs`.
- `pci` depends on `pci-sys`; `lspci-rs` depends on `pci`.
- The binary package exposes a `lspci-rs` executable.

- [ ] **Step 1: Write the workspace manifests**

Use this root structure and keep package-specific dependencies in package manifests:

```toml
[workspace]
members = ["crates/pci-sys", "crates/pci", "crates/lspci-rs"]
resolver = "2"

[profile.release]
lto = "thin"
codegen-units = 1
strip = "symbols"
```

Declare `pci-sys` as a library, `pci` as a library, and `lspci-rs` as a binary. Add `pci` as a path dependency of `lspci-rs`, and `pci-sys` as a path dependency of `pci`.

- [ ] **Step 2: Pin the toolchain target**

Create `rust-toolchain.toml` with the stable channel and the deployment target:

```toml
[toolchain]
channel = "stable"
targets = ["x86_64-unknown-linux-gnu"]
profile = "minimal"
```

- [ ] **Step 3: Add repository exclusions**

Add `target/`, `.superpowers/`, local environment files, and generated release archives to `.gitignore`:

```gitignore
/target/
/.superpowers/
*.env
*.tar.gz
```

- [ ] **Step 4: Add compile-only crate entry points**

Use minimal entry points so the dependency graph can be checked before FFI code exists:

```rust
// crates/pci-sys/src/lib.rs
#![allow(non_camel_case_types, non_upper_case_globals, non_snake_case)]

pub mod bindings {}
```

```rust
// crates/pci/src/lib.rs
pub fn crate_is_ready() {}
```

```rust
// crates/lspci-rs/src/main.rs
fn main() {}
```

- [ ] **Step 5: Verify the empty workspace**

Run:

```bash
cargo metadata --no-deps --format-version 1
cargo check --workspace
```

Expected: all three packages are discovered and the workspace check exits successfully.

- [ ] **Step 6: Commit the scaffold**

```bash
git add Cargo.toml Cargo.lock rust-toolchain.toml .gitignore crates
git commit -m "chore: scaffold pci workspace"
```

If Git has not yet been initialized, preserve this exact boundary and commit it after initialization.

## Task 2: Add EL8 libpci discovery and bindgen generation

**Files:**
- Create: `crates/pci-sys/build.rs`
- Create: `crates/pci-sys/wrapper.h`
- Modify: `crates/pci-sys/Cargo.toml`
- Modify: `crates/pci-sys/src/lib.rs`

**Interfaces:**
- Produces `pci_sys::bindings` generated into Cargo `OUT_DIR`.
- Exposes only the allowlisted functions, structs, constants, and name-lookup flags needed by the first slice.
- Links the system `libpci` found through `pkg-config`.

- [ ] **Step 1: Declare build dependencies**

Add `bindgen` and `pkg-config` under `[build-dependencies]`. Keep runtime dependencies out of `pci-sys`; generated bindings must not pull a C wrapper library into the safe crate.

- [ ] **Step 2: Create the narrow C header entry point**

Create `wrapper.h`:

```c
#include <pci/pci.h>
```

Do not include unrelated system headers or allowlist the write APIs.

- [ ] **Step 3: Probe libpci and generate bindings**

In `build.rs`, call `pkg_config::Config::new().probe("libpci")`, pass every discovered include path to bindgen, and allowlist only:

```text
pci_access
pci_dev
pci_alloc
pci_init
pci_cleanup
pci_scan_bus
pci_fill_info
pci_lookup_name
PCI_FILL_IDENT
PCI_FILL_CLASS
PCI_LOOKUP_VENDOR
PCI_LOOKUP_DEVICE
PCI_LOOKUP_CLASS
```

Write the generated Rust file to `OUT_DIR/bindings.rs`, emit `rerun-if-changed=wrapper.h`, and emit a clear panic message that names `libpci` headers, `pkg-config`, and libclang when discovery fails.

- [ ] **Step 4: Re-export generated bindings**

Replace the placeholder module with:

```rust
#![allow(non_camel_case_types, non_upper_case_globals, non_snake_case)]

pub mod bindings {
    include!(concat!(env!("OUT_DIR"), "/bindings.rs"));
}
```

Keep all generated symbols under `pci_sys::bindings`; do not flatten them into the public root.

- [ ] **Step 5: Verify binding generation in the EL8 builder**

Run inside the builder, where `pciutils-devel` is installed:

```bash
cargo check -p pci-sys -vv
```

Expected: the build log shows a `libpci` link search path and the generated binding file is created under Cargo `OUT_DIR`.

- [ ] **Step 6: Commit the FFI generation boundary**

```bash
git add crates/pci-sys
git commit -m "feat: generate narrow libpci bindings"
```

## Task 3: Define owned PCI data types

**Files:**
- Create: `crates/pci/src/device.rs`
- Create: `crates/pci/src/error.rs`
- Modify: `crates/pci/src/lib.rs`
- Modify: `crates/pci/Cargo.toml`

**Interfaces:**
- Produces `PciAddress`, `PciDevice`, and `PciSnapshot`.
- `PciAddress` exposes `domain`, `bus`, `slot`, `function`, and a canonical `display()`/`Display` representation.
- `PciDevice` owns all strings and numeric IDs.
- `PciSnapshot::devices(&self) -> &[PciDevice]` is the read-only consumer interface.
- `PciError` is the error type used by `PciSession` and the CLI.

- [ ] **Step 1: Define the public types**

Use this shape; keep names as `String` so every output field can carry a status marker:

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PciAddress {
    pub domain: u16,
    pub bus: u8,
    pub slot: u8,
    pub function: u8,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PciDevice {
    pub address: PciAddress,
    pub vendor_id: u16,
    pub device_id: u16,
    pub class_id: u16,
    pub vendor_name: String,
    pub device_name: String,
    pub class_name: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PciSnapshot {
    devices: Vec<PciDevice>,
}

impl PciSnapshot {
    pub(crate) fn from_devices(devices: Vec<PciDevice>) -> Self { Self { devices } }
    pub fn devices(&self) -> &[PciDevice] { &self.devices }
}
```

- [ ] **Step 2: Implement canonical address formatting**

Implement `Display` so the same address always renders as `dddd:bb:ss.f`, for example `0000:00:1f.3`. Reject no values at this layer; the fields are copied from `libpci` and the formatter is deterministic.

- [ ] **Step 3: Define stable error variants**

Define these variants and fields without exposing `pci_sys` types:

```rust
pub enum PciError {
    Allocation,
    DeviceInfo {
        address: PciAddress,
        known_fields: u32,
        requested_fields: u32,
    },
    Message(String),
}
```

Implement `Display` and `std::error::Error`. Invalid UTF-8 in a human-readable name is not fatal; it maps to `<unknown>` in the name-copy helper.

- [ ] **Step 4: Verify the pure data layer without fabricated PCI records**

Run:

```bash
cargo check -p pci
```

Use compiler checks for the type boundary in this task. Do not add synthetic PCI fixture records; real device validation is performed after the binary is deployed to ECS.

- [ ] **Step 5: Commit the data contract**

```bash
git add crates/pci/src crates/pci/Cargo.toml
git commit -m "feat: define owned pci device model"
```

## Task 4: Implement `PciSession` and the read-only libpci scan

**Files:**
- Create: `crates/pci/src/session.rs`
- Modify: `crates/pci/src/lib.rs`

**Interfaces:**
- Produces `PciSession::new() -> Result<PciSession, PciError>`.
- Produces `PciSession::scan(&mut self) -> Result<PciSnapshot, PciError>`.
- `PciSession` owns the `*mut pci_access` pointer and releases it exactly once in `Drop`.
- No write symbols from `pci_sys` are referenced.

- [ ] **Step 1: Add the session owner and null checks**

Store the raw pointer in a private field and make the type non-copyable. `PciSession::new` calls `pci_alloc`, rejects a null result, calls `pci_init`, and stores the initialized pointer. `Drop` calls `pci_cleanup` only when the pointer is non-null.

The public `pci_init` API returns `void` and upstream uses the access error callback for fatal initialization failures. Do not invent a Rust callback with a variadic C signature. Preserve the library’s process-fatal behavior for that path; map Rust-detectable allocation, conversion, and device-fill failures into `PciError` and non-zero CLI termination.

- [ ] **Step 2: Scan and request only required fields**

Implement `scan` with this sequence. The first-slice session is scanned once; do not call `pci_scan_bus` twice on the same session until a later refresh design explicitly adds `PCI_FILL_RESCAN` and snapshot replacement semantics:

```rust
    pci_scan_bus(access);
    let mut raw = (*access).devices;
    while !raw.is_null() {
    let fill_flags = PCI_FILL_IDENT | PCI_FILL_CLASS;
    let known_fields = pci_fill_info(raw, fill_flags);
    if (known_fields & fill_flags) != fill_flags {
        let address = PciAddress {
            domain: (*raw).domain,
            bus: (*raw).bus,
            slot: (*raw).dev,
            function: (*raw).func,
        };
        return Err(PciError::DeviceInfo {
            address,
            known_fields: known_fields as u32,
            requested_fields: fill_flags as u32,
        });
    }
    // Copy address, IDs, and names into a PciDevice before moving on.
    raw = (*raw).next;
}
```

Use the public device chain only while the session is alive. Do not return `pci_dev*`, `&CStr`, or a reference tied to `pci_access`.

- [ ] **Step 3: Copy and validate names**

For each device, allocate a fixed Rust byte buffer for `pci_lookup_name`. Use the vendor, device, and class lookup flags separately. Convert the returned C string with `CStr::from_ptr`; on a null pointer or invalid UTF-8, use `<unknown>` while preserving numeric IDs.

- [ ] **Step 4: Verify the FFI scan on the real ECS host**

After the CLI exists in Task 5, run the first real scan through the binary. Until then, verify compilation with:

```bash
cargo check -p pci
```

The first successful run must happen on the EL8 target with real `libpci.so.3`; do not replace it with a fixture or a mock backend.

- [ ] **Step 5: Commit the session boundary**

```bash
git add crates/pci/src
git commit -m "feat: add owned libpci scanning session"
```

## Task 5: Implement the `list` CLI and renderers

**Files:**
- Modify: `crates/lspci-rs/Cargo.toml`
- Create: `crates/lspci-rs/src/cli.rs`
- Create: `crates/lspci-rs/src/output.rs`
- Modify: `crates/lspci-rs/src/main.rs`

**Interfaces:**
- `Cli` parses `list`, `--format text`, `--format json`, `--help`, and `--version`.
- `OutputFormat` has `Text` and `Json` variants; text is the default.
- `render_text(&PciSnapshot) -> String` produces human-readable lines.
- `render_json(&PciSnapshot) -> Result<String, serde_json::Error>` produces stable JSON.
- `main` maps usage failures to exit code `2`, runtime failures to exit code `1`, and success to `0`.

- [ ] **Step 1: Add CLI and serialization dependencies**

Add `clap` with derive support to the binary crate, and add `serde`/`serde_json` for the explicit JSON boundary. Do not add these dependencies to `pci-sys`; the safe library remains independent of CLI argument parsing and output serialization.

- [ ] **Step 2: Define the command model**

Use a command model equivalent to:

```rust
#[derive(Parser)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    List {
        #[arg(long, value_enum, default_value_t = OutputFormat::Text)]
        format: OutputFormat,
    },
}
```

Reject unknown subcommands and formats through `clap`; do not implement legacy `lspci` flags in this task.

- [ ] **Step 3: Define the JSON wire shape**

Serialize an object containing a `devices` array. Use these exact keys:

```json
{
  "devices": [
    {
      "address": {
        "domain": "0000",
        "bus": "00",
        "slot": "1f",
        "function": "3",
        "display": "0000:00:1f.3"
      },
      "vendor_id": "0x8086",
      "device_id": "0x1234",
      "class_id": "0x0403",
      "vendor_name": "Intel Corporation",
      "device_name": "<unknown>",
      "class_name": "Audio device"
    }
  ]
}
```

IDs must be lowercase hexadecimal with fixed width and a `0x` prefix: four digits for vendor/device/class IDs. Names are strings, including尖括号 status markers.

- [ ] **Step 4: Implement the text renderer**

Render one device per line with the canonical address first, then fixed-width IDs and names. Keep text readable but do not claim compatibility with traditional `lspci` output. The renderer must consume only `PciSnapshot`.

- [ ] **Step 5: Wire session, render, and exit codes**

The command path must be:

```rust
let mut session = PciSession::new()?;
let snapshot = session.scan()?;
let output = match format { Text => render_text(&snapshot), Json => render_json(&snapshot)? };
print!("{output}");
```

Keep session creation inside the command invocation so it is dropped after output. Send runtime errors to stderr and avoid partial success output when initialization or scanning fails.

- [ ] **Step 6: Verify the real CLI on ECS**

Run on the deployed binary:

```bash
./lspci-rs list
./lspci-rs list --format text
./lspci-rs list --format json
./lspci-rs --help
./lspci-rs --version
```

Expected: text and JSON both succeed, JSON is parseable, and both outputs contain the same number of devices. Run as the normal ECS user without `sudo`.

- [ ] **Step 7: Commit the CLI slice**

```bash
git add crates/lspci-rs Cargo.lock
git commit -m "feat: add read-only list cli"
```

## Task 6: Create the EL8-compatible local builder

**Files:**
- Create: `containers/el8-builder/Containerfile`
- Create: `scripts/build-linux.sh`
- Modify: `README.md`

**Interfaces:**
- `scripts/build-linux.sh` builds `target/x86_64-unknown-linux-gnu/release/lspci-rs` inside the same builder used by CI.
- The builder installs `pciutils-devel`, `pkg-config`, Clang/libclang, and the Rust stable toolchain.
- The output is a dynamically linked Linux binary that expects `libpci.so.3` on the target host.

- [ ] **Step 1: Define the EL8 builder image**

Base the image on `rockylinux:8`, install the development packages with `dnf`, install the stable Rust toolchain with the minimal profile, and set `CARGO_HOME`, `RUSTUP_HOME`, and `PATH` explicitly. Include `gcc`, `make`, `pkgconf-pkg-config`, `pciutils-devel`, `clang`, `llvm`, `libclang`, `git`, and CA certificates.

- [ ] **Step 2: Add the local builder wrapper**

Make `scripts/build-linux.sh` fail on the first error, build the container image, mount the repository at `/workspace`, and run:

```bash
cargo build --release --target x86_64-unknown-linux-gnu
ldd target/x86_64-unknown-linux-gnu/release/lspci-rs
```

The script must not copy or link macOS libraries. It should print the resolved `libpci.so` dependency and the target triple.

- [ ] **Step 3: Verify local reproducibility**

On macOS with Docker available, run:

```bash
./scripts/build-linux.sh
file target/x86_64-unknown-linux-gnu/release/lspci-rs
```

Expected: an ELF x86-64 Linux executable, not a Mach-O binary.

- [ ] **Step 4: Document host/runtime prerequisites**

In `README.md`, record the confirmed ECS facts, the local builder command, the expected runtime dependency `libpci.so.3`, and the fact that ECS does not need Rust or `pciutils-devel`.

- [ ] **Step 5: Commit the builder**

```bash
git add containers scripts README.md
git commit -m "build: add el8 linux builder"
```

## Task 7: Add GitHub Actions artifact upload and real ECS smoke check

**Files:**
- Create: `.github/workflows/build-and-smoke.yml`
- Modify: `scripts/live-smoke.sh`
- Modify: `README.md`

**Interfaces:**
- The workflow builds the binary in the EL8-compatible builder, uploads the artifact, and invokes the smoke script on Ali ECS.
- `scripts/live-smoke.sh` accepts the deployed binary path as its only positional argument and exits non-zero on any failed check.
- CI uses repository secrets for ECS host, user, and SSH private key; no credential is committed.

- [ ] **Step 1: Define the real-host smoke script**

Implement these checks in order:

```bash
set -euo pipefail
binary="$1"
"$binary" list > /tmp/lspci-rs.text
"$binary" list --format json > /tmp/lspci-rs.json
python3 -m json.tool /tmp/lspci-rs.json >/dev/null
json_count=$(python3 -c 'import json,sys; print(len(json.load(open(sys.argv[1]))["devices"]))' /tmp/lspci-rs.json)
text_count=$(wc -l < /tmp/lspci-rs.text)
test "$text_count" -eq "$json_count"
command -v lspci >/dev/null
lspci >/tmp/system-lspci.text
```

Then print both outputs and the host metadata (`uname -m`, `/etc/os-release`, `ldd --version`, `ldconfig -p | grep libpci`). Do not mutate `/sys`, PCI configuration, drivers, or system packages.

- [ ] **Step 2: Create the workflow build job**

Use a Linux GitHub runner only as the orchestration host; run the actual Cargo build in the checked-in EL8 builder. Upload the release binary and a metadata file containing the commit SHA, target triple, Rust channel, and builder image identifier.

- [ ] **Step 3: Create the controlled deployment job**

Use an SSH key from GitHub secrets, copy both the binary and `scripts/live-smoke.sh` to a commit-scoped directory under the configured ECS user’s home or `/tmp`, mark only the binary and script executable, and invoke the smoke script remotely. Do not use broad deletion commands or overwrite an unrelated path.

- [ ] **Step 4: Verify the workflow contract without enabling destructive actions**

Run the builder locally first. Then use a manual workflow dispatch to upload one artifact and inspect the job log for:

```text
target=x86_64-unknown-linux-gnu
arch=x86_64
libpci.so.3 present
text output succeeded
json output parsed
system lspci comparison available
```

- [ ] **Step 5: Document the CI secrets and manual fallback**

Document the required secret names, the fact that the GitHub runner cannot use the local `aliecs` SSH alias, and the manual fallback:

```bash
scp target/x86_64-unknown-linux-gnu/release/lspci-rs aliecs:/tmp/lspci-rs
ssh aliecs /tmp/lspci-rs list
ssh aliecs '/tmp/lspci-rs list --format json'
```

- [ ] **Step 6: Commit the remote verification path**

```bash
git add .github scripts/live-smoke.sh README.md
git commit -m "ci: upload and smoke test linux binary on ecs"
```

## Task 8: Final implementation verification and handoff

**Files:**
- Modify: `README.md` only if the verified commands differ from the documented commands.

**Interfaces:**
- No new public API. This task confirms the approved first-slice contract end-to-end.

- [ ] **Step 1: Build from a clean checkout**

Run inside the EL8 builder:

```bash
cargo clean
cargo build --release --target x86_64-unknown-linux-gnu
```

Expected: a release binary at `target/x86_64-unknown-linux-gnu/release/lspci-rs`.

- [ ] **Step 2: Inspect dynamic dependencies**

Run:

```bash
ldd target/x86_64-unknown-linux-gnu/release/lspci-rs
```

Expected: `libpci.so.3` resolves through the target-compatible builder and no macOS or Ubuntu-only library path appears.

- [ ] **Step 3: Deploy and run the normal-user smoke check**

Upload the binary to ECS and run:

```bash
./scripts/live-smoke.sh /tmp/lspci-rs
```

Expected: both renderers succeed, JSON parses, the binary exits zero, and the system `lspci` command is available for manual comparison.

- [ ] **Step 4: Record evidence**

Save the verified command output in the development notes or the pull request description, including target triple, ECS architecture, kernel, glibc, and libpci versions. Do not add generated hardware output to the source tree.

- [ ] **Step 5: Commit only documentation changes**

```bash
git add README.md
git commit -m "docs: record real ecs verification"
```

## Plan Self-Review

- Spec coverage: workspace layering is covered by Tasks 1–2; owned session/snapshot is covered by Tasks 3–4; CLI and JSON contract by Task 5; EL8/macOS/GitHub build flow by Tasks 6–7; real ECS validation by Tasks 7–8; TUI and event behavior remain explicit non-goals.
- Placeholder scan: the plan contains no `TODO`, `TBD`, or unspecified implementation slots.
- Type consistency: `PciSession::scan` returns `PciSnapshot`; renderers consume `&PciSnapshot`; `PciDevice` owns `String` names and numeric IDs; the CLI never consumes `pci_sys` directly.
- Safety check: no task writes PCI configuration, changes drivers, requires `sudo`, or performs broad remote deletion.
- Environment check: all Linux compilation is delegated to an EL8-compatible builder; the remote host remains runtime-only.
