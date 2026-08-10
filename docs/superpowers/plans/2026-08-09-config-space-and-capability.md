# PCI Configuration Space and Capability Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a read-only, segment-based PCI configuration-space reader with generic standard/extended capability discovery and explicit CLI output.

**Architecture:** `pci-sys` continues to expose the generated libpci ABI. The `pci` crate owns a short-lived `ConfigSpaceReader` that calls `pci_read_block`, stores successful ranges as segments, records failed ranges, and feeds a generic capability walker. Normal `PciInspection` keeps decoded capability metadata only; `read_config` returns raw configuration bytes only when explicitly requested by the CLI.

**Tech Stack:** Rust 2024 workspace, libpci/pciutils, bindgen-generated FFI, clap, serde, Alibaba Cloud Linux 3 x86_64 ECS container.

## Global Constraints

- Use the real ECS PCI environment for validation; do not create hardware fixtures.
- Keep `list` summary-only; it must not read configuration space.
- Preserve partial reads and failure ranges; never zero-fill unread bytes.
- Report `PermissionDenied` only when the backend/system provides evidence for denied access.
- Keep raw libpci pointers and all `unsafe` FFI access inside `pci`.
- Do not add configuration-space writes in this slice.
- Do not add protocol-specific decoders for MSI, PCIe, AER, SR-IOV, or similar capabilities.
- Do not add new Rust dependencies.
- User performs the code editing; each task includes the exact interfaces and ECS verification commands.

---

### Task 1: Define configuration-space and capability result types

**Files:**
- Create: `crates/pci/src/config.rs`
- Modify: `crates/pci/src/field.rs`
- Modify: `crates/pci/src/details.rs`
- Modify: `crates/pci/src/lib.rs`

**Interfaces:**
- Consumes: existing `PciFieldUnavailableReason`, `PciField`, and `PciInspection` types.
- Produces: public raw-config types, `ConfigReadLevel`, and the grouped capability result consumed by the reader, session, and renderers.

- [ ] **Step 1: Add the public config types**

Create `config.rs` with these owned types and accessors:

```rust
use std::ops::Range;

use crate::PciFieldUnavailableReason;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConfigReadLevel {
    Header,
    Standard,
    Extended,
}

impl ConfigReadLevel {
    pub fn range(self) -> Range<u32> {
        match self {
            Self::Header => 0x000..0x040,
            Self::Standard => 0x000..0x100,
            Self::Extended => 0x000..0x1000,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConfigSegment {
    pub offset: u32,
    pub bytes: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConfigReadFailure {
    pub offset: u32,
    pub length: u32,
    pub reason: PciFieldUnavailableReason,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConfigSpaceSnapshot {
    pub requested: Range<u32>,
    pub segments: Vec<ConfigSegment>,
    pub failures: Vec<ConfigReadFailure>,
}

impl ConfigSpaceSnapshot {
    pub(crate) fn new(requested: Range<u32>) -> Self {
        Self {
            requested,
            segments: Vec::new(),
            failures: Vec::new(),
        }
    }
}
```

Add public read-only accessors if the renderers should not access fields directly.

- [ ] **Step 2: Add capability chain state types**

Extend `field.rs` with the exact result types:

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PciCapabilityState {
    Valid,
    Truncated,
    Unavailable(PciFieldUnavailableReason),
    Malformed(PciCapabilityMalformedReason),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PciCapabilityMalformedReason {
    MisalignedOffset,
    OffsetOutOfRange,
    CycleDetected,
    InvalidNextPointer,
    MissingHeader,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PciCapabilityChainStatus {
    NotPresent,
    Complete,
    Truncated,
    Unavailable(PciFieldUnavailableReason),
    Malformed(PciCapabilityMalformedReason),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PciCapabilityReport {
    pub standard: Vec<PciCapability>,
    pub extended: Vec<PciCapability>,
    pub standard_status: PciCapabilityChainStatus,
    pub extended_status: PciCapabilityChainStatus,
}
```

Add `next: Option<u16>` and `state: PciCapabilityState` to `PciCapability`. Keep `PciCapabilityKind::Unknown(u16)` so unknown IDs remain data.

- [ ] **Step 3: Change the inspection domain type**

In `details.rs`, change:

```rust
pub capabilities: PciField<Vec<PciCapability>>,
```

to:

```rust
pub capabilities: PciField<PciCapabilityReport>,
```

Do not add `config_space` to `PciDeviceDetails`. Re-export `ConfigReadLevel`, `ConfigSegment`, `ConfigReadFailure`, `ConfigSpaceSnapshot`, `PciCapabilityReport`, and the new capability state types from `crates/pci/src/lib.rs`.

- [ ] **Step 4: Verify the type-only change**

Run in the ECS container:

```bash
cargo fmt --all
cargo check -p pci --target x86_64-unknown-linux-gnu
```

Expected: the new types compile; downstream renderer/session type errors are acceptable at this intermediate step and must be fixed in later tasks.

### Task 2: Implement the segment reader and FFI boundary

**Files:**
- Modify: `crates/pci/src/config.rs`
- Modify: `crates/pci/src/lib.rs` only if a `pub(crate)` module declaration is needed

**Interfaces:**
- Consumes: `ConfigSpaceSnapshot`, `ConfigSegment`, `ConfigReadFailure`, and a live `*mut pci_dev` from `PciSession`.
- Produces: internal `ConfigSpaceReader::fetch` and `ConfigSpaceReader::read` used by capability parsing and `PciSession::read_config`.

- [ ] **Step 1: Add the internal reader**

Define an internal reader in `config.rs`:

```rust
pub(crate) struct ConfigSpaceReader {
    raw: *mut pci_sys::bindings::pci_dev,
    snapshot: ConfigSpaceSnapshot,
}

impl ConfigSpaceReader {
    pub(crate) unsafe fn new(
        raw: *mut pci_sys::bindings::pci_dev,
        requested: Range<u32>,
    ) -> Self;

    pub(crate) fn snapshot(&self) -> &ConfigSpaceSnapshot;
    pub(crate) fn fetch(&mut self, offset: u32, length: u32) -> Result<(), ConfigReadFailure>;
    pub(crate) fn read(&mut self, offset: u32, length: u32) -> Result<Vec<u8>, ConfigReadFailure>;
}
```

The constructor is the only place that stores the raw pointer. Keep the type `pub(crate)` so it cannot escape the `pci` crate.

- [ ] **Step 2: Implement coverage lookup and segment insertion**

Implement `fetch` with this behavior:

```text
1. Reject offset/length arithmetic overflow and zero-length requests.
2. Find each uncovered subrange of [offset, offset + length) by comparing it
   with existing segment ranges.
3. Call pci_read_block only for uncovered subranges.
4. On a successful exact read, insert the bytes at the requested offset.
5. Merge overlapping or adjacent successful segments.
6. On a zero/short/failed read, append ConfigReadFailure and return Err.
7. Never insert bytes for a failed range and never remove prior successes.
```

Use `u32` for offsets and convert the FFI length only after checking it fits `c_int`. Keep the single `unsafe { pci_read_block(...) }` call in this module.

- [ ] **Step 3: Implement cached reads**

Implement `read(offset, length)` by calling `fetch`, then copying bytes from the now-covered range across one or more segments. If the range is not fully available, return the recorded `ConfigReadFailure`; do not return a zero-filled vector.

- [ ] **Step 4: Verify the reader compiles**

Run:

```bash
cargo fmt --all -- --check
cargo check -p pci --target x86_64-unknown-linux-gnu
```

### Task 3: Implement generic standard and extended capability walking

**Files:**
- Create: `crates/pci/src/capability.rs`
- Modify: `crates/pci/src/lib.rs`

**Interfaces:**
- Consumes: `&mut ConfigSpaceReader` and the capability result types from Task 1.
- Produces: `pub(crate) fn discover(reader: &mut ConfigSpaceReader) -> PciCapabilityReport`.

- [ ] **Step 1: Implement standard capability discovery**

Read the 64-byte header through the reader. Check Status register offset `0x06`, bit 4. If the bit is clear, return `NotPresent`. If it is set, read the pointer at `0x34` and walk 2-byte headers:

```text
offset + 0: capability ID
offset + 1: next capability offset
```

Require a non-zero pointer to be within `0x40..0x100` and 4-byte aligned. Stop with `Complete` at `next == 0`. Track visited offsets and stop with `Malformed(CycleDetected)` on repetition. Stop with `Truncated` or `Unavailable(reason)` when a later header cannot be read, preserving earlier entries. Limit traversal to 48 nodes.

- [ ] **Step 2: Implement extended capability discovery**

Start at `0x100` and read 4-byte headers:

```text
id      = header & 0xffff
version = (header >> 16) & 0xf
next    = (header >> 20) & 0xfff
```

Treat an all-zero header at a valid starting location as `NotPresent`. Require next offsets in `0x100..0x1000` and 4-byte alignment. Detect cycles, invalid offsets, unavailable headers, and more than 256 nodes. Store `id`, `PciCapabilityKind::Extended`, `offset`, `next`, and `PciCapabilityState::Valid` for valid nodes. The version is not yet exposed because protocol-specific decoding is out of scope; preserve it only if the chosen result type explicitly needs it.

- [ ] **Step 3: Keep FFI out of the walker**

The capability module must import no `pci_sys::bindings::pci_read_block` symbol. It may use only `ConfigSpaceReader::read` and the domain types. This keeps malformed-chain handling independent from libpci.

- [ ] **Step 4: Verify capability module compilation**

Run:

```bash
cargo fmt --all -- --check
cargo check -p pci --target x86_64-unknown-linux-gnu
```

### Task 4: Integrate the reader into `PciSession`

**Files:**
- Modify: `crates/pci/src/session.rs`
- Modify: `crates/pci/src/lib.rs`

**Interfaces:**
- Consumes: `ConfigReadLevel`, `ConfigSpaceReader`, and `capability::discover`.
- Produces: `PciSession::inspect` with semantic capability results and `PciSession::read_config` for explicit raw dumps.

- [ ] **Step 1: Remove the old libpci capability-chain path**

Delete `capabilities_from_raw` and stop using `PCI_CAP_NORMAL`, `PCI_CAP_EXTENDED`, `first_cap`, and `PCI_FILL_CAPS | PCI_FILL_EXT_CAPS` for the Rust capability result. Keep the other `pci_fill_info` fields needed by `PciDeviceDetails`.

- [ ] **Step 2: Add a raw-device lookup helper**

Factor the repeated scan traversal into an internal helper with this behavior:

```rust
unsafe fn find_raw_device(
    &mut self,
    address: PciAddress,
) -> Result<*mut pci_sys::bindings::pci_dev, PciError>;
```

It calls `pci_scan_bus`, compares `PciAddress`, and returns `DeviceNotFound` when no device matches. The returned pointer is consumed before the current session method returns; do not store it in a public value.

- [ ] **Step 3: Parse capabilities during inspection**

After the required summary/detail fields have been filled, create a temporary reader for the matched raw device and call:

```rust
let mut reader = unsafe { ConfigSpaceReader::new(raw, 0x000..0x1000) };
let capability_report = capability::discover(&mut reader);
```

Pass `PciField<PciCapabilityReport>` into `details_from_raw`. The reader and snapshot then drop before returning `PciInspection`. If the header cannot be read, return `PciField::Unavailable`; if no capability list exists, return `PciField::NotApplicable`; preserve partial chain statuses in an available report.

- [ ] **Step 4: Add explicit raw config reading**

Implement:

```rust
pub fn read_config(
    &mut self,
    address: PciAddress,
    level: ConfigReadLevel,
) -> Result<ConfigSpaceSnapshot, PciError>;
```

Find the raw device, create a reader with `level.range()`, call `fetch` for that range, ignore the range-level error because it is recorded in the snapshot, and return the owned snapshot. Return `PciError` only for context/scan/device lookup failures.

- [ ] **Step 5: Verify the session integration**

Run:

```bash
cargo fmt --all -- --check
cargo check --workspace --target x86_64-unknown-linux-gnu
cargo run -p lspci-rs --target x86_64-unknown-linux-gnu -- show 0000:00:05.0 --format text
```

Expected: the command still prints the device details and capability section, and no FFI capability-chain imports remain unused.

### Task 5: Add CLI config levels and render raw segments

**Files:**
- Modify: `crates/lspci-rs/src/cli.rs`
- Modify: `crates/lspci-rs/src/main.rs`
- Modify: `crates/lspci-rs/src/output.rs`

**Interfaces:**
- Consumes: `PciSession::read_config`, `ConfigReadLevel`, `ConfigSpaceSnapshot`, and `PciCapabilityReport`.
- Produces: `show --config header|standard|extended` with text and JSON output.

- [ ] **Step 1: Add a CLI-only value enum and conversion**

Keep clap out of the `pci` library. Add:

```rust
#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum ConfigLevel {
    Header,
    Standard,
    Extended,
}

impl From<ConfigLevel> for pci::ConfigReadLevel {
    fn from(level: ConfigLevel) -> Self {
        match level {
            ConfigLevel::Header => Self::Header,
            ConfigLevel::Standard => Self::Standard,
            ConfigLevel::Extended => Self::Extended,
        }
    }
}
```

Add `config: Option<ConfigLevel>` to `Command::Show` and pass it through `main.rs` to `run_show`.

- [ ] **Step 2: Render text config output**

Change the inspection renderer to accept `Option<&ConfigSpaceSnapshot>`. When present, render each successful segment as 16-byte rows with the segment-relative bytes and absolute four-digit offset; render every failure as `<unavailable: reason>` with its offset and length. Do not print synthetic rows for holes.

Use this shape:

```text
config-space:
  requested: 0x000..0x100
  0000: 1f 00 ...
  unavailable: 0x040..0x100 <read-error>
```

Render capabilities as separate metadata and update the current single-vector loop to print `standard` and `extended` groups plus chain status.

- [ ] **Step 3: Render JSON config output**

Add serializable internal types:

```rust
struct JsonConfigSpace {
    requested: JsonRange,
    segments: Vec<JsonConfigSegment>,
    failures: Vec<JsonConfigFailure>,
}

struct JsonConfigSegment {
    offset: String,
    bytes: String,
}

struct JsonConfigFailure {
    offset: String,
    length: String,
    reason: String,
}
```

Encode bytes as a lowercase hexadecimal string, preserve failures as records, and make the config field optional when `--config` was not provided. Update `json_capabilities` to encode the standard/extended groups and chain statuses.

- [ ] **Step 4: Wire `run_show` without creating a second session**

Create one `PciSession`, call `inspect`, then call `read_config` on that same session only when `config` is `Some`. Pass the optional snapshot to the selected text/JSON renderer. A missing `--config` must not call `read_config`.

- [ ] **Step 5: Verify all CLI forms**

Run:

```bash
cargo fmt --all -- --check
cargo check --workspace --target x86_64-unknown-linux-gnu
cargo run -p lspci-rs --target x86_64-unknown-linux-gnu -- list --format text
cargo run -p lspci-rs --target x86_64-unknown-linux-gnu -- show 0000:00:05.0 --format text
cargo run -p lspci-rs --target x86_64-unknown-linux-gnu -- show 0000:00:05.0 --config standard --format text
cargo run -p lspci-rs --target x86_64-unknown-linux-gnu -- show 0000:00:05.0 --config extended --format json
```

### Task 6: Real-system comparison and cleanup

**Files:**
- Modify only files with compiler warnings or output defects discovered in Tasks 1–5.
- Do not modify generated `target/**/out/bindings.rs` files.

**Interfaces:**
- Consumes: the completed library and CLI.
- Produces: verified real-device behavior and a clean, reviewable worktree diff.

- [ ] **Step 1: Compare device counts and summary identity**

Run inside the ECS container:

```bash
cargo run -p lspci-rs --target x86_64-unknown-linux-gnu -- list --format text > /tmp/lspci-rs-list.txt
lspci > /tmp/lspci-list.txt
wc -l /tmp/lspci-rs-list.txt /tmp/lspci-list.txt
```

The counts should match on the unchanged real system. Differences in formatting are expected; vendor/device/class IDs and addresses must correspond.

- [ ] **Step 2: Compare one device's capabilities and raw config**

Use a device that exists in both outputs, for example `0000:00:05.0` when present:

```bash
lspci -s 00:05.0 -vv
cargo run -p lspci-rs --target x86_64-unknown-linux-gnu -- show 0000:00:05.0 --config standard --format text
cargo run -p lspci-rs --target x86_64-unknown-linux-gnu -- show 0000:00:05.0 --config standard --format json
```

When libpci reports access failure, the Rust output must show an explicit failure reason/range rather than fabricated zero bytes.

- [ ] **Step 3: Run final static checks**

Run:

```bash
cargo fmt --all -- --check
cargo check --workspace --target x86_64-unknown-linux-gnu
git diff --check
git status --short
```

Review that only intended source files and the existing research/design documents are present; generated bindings remain ignored.

- [ ] **Step 4: Record the implementation handoff**

Summarize the changed files, ECS commands that passed, the real device used for smoke verification, and any capability ranges that remained inaccessible. Do not claim access-denied causes beyond the evidence returned by libpci or the system.
