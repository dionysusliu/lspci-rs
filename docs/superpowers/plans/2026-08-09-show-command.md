# `show` Command Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a read-only `lspci-rs show <PCI_ADDRESS>` command that inspects one PCI function through libpci and renders text or structured JSON output.

**Architecture:** Keep libpci as the only primary PCI data interface. `PciSession::inspect()` requests detailed fields from libpci, which normally selects the Linux sysfs backend internally, then copies all values into owned Rust data. Field-level availability is represented by `PciField<T>`; fatal libpci callback recovery remains out of scope.

**Tech Stack:** Rust 2024, Cargo workspace, bindgen-generated libpci FFI, libpci/pciutils 3.8.0 on Alibaba Cloud Linux 3, clap, serde, serde_json.

## Global Constraints

- Use the real Alibaba Cloud Linux 3 ECS environment for libpci integration and smoke validation.
- Do not create PCI fixtures, fake devices, or mock libpci responses.
- Keep raw C pointers inside `PciSession`; never expose them through the public `pci` API.
- Use libpci’s `linux-sysfs` backend through the normal automatic backend selection; do not implement a parallel sysfs reader for normal fields.
- Do not implement PCI writes, reset, remove, rescan, bind, or unbind operations in this feature.
- Do not implement libpci fatal callback recovery in this feature.
- Keep the existing text and JSON `list` output contracts unchanged.
- Do not add dependencies unless the current build requires one for an already-approved FFI boundary.
- The user writes the code; the assistant provides task guidance and reviews command output.

---

### Task 1: Add field-state and detail domain types

**Files:**
- Create: `crates/pci/src/field.rs`
- Create: `crates/pci/src/details.rs`
- Modify: `crates/pci/src/lib.rs`
- Test: `crates/pci/src/field.rs` unit tests

**Interfaces:**
- Consumes: existing `PciAddress` and `PciDevice`.
- Produces: `PciField<T>`, `PciFieldUnavailableReason`, `PciResource`, `PciDeviceDetails`, and `PciInspection` for `session.rs` and the CLI.

- [ ] **Step 1: Add the field-state enum and reason enum**

Create `field.rs` with these public types:

```rust
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PciField<T> {
    Available(T),
    Unavailable {
        reason: PciFieldUnavailableReason,
    },
    NotApplicable,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PciFieldUnavailableReason {
    PermissionDenied,
    UnsupportedByBackend,
    UnsupportedByLibrary,
    DeviceUnavailable,
    NotBound,
    ReadError,
    Unknown,
}
```

Do not add `serde` derives to these `pci` types. Serialization remains a CLI concern.

- [ ] **Step 2: Add resource and inspection types**

Create `details.rs`:

```rust
use crate::{field::PciField, PciAddress, PciDevice};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PciResource {
    pub index: u8,
    pub start: u64,
    pub size: u64,
    pub flags: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PciDeviceDetails {
    pub revision: PciField<u8>,
    pub programming_interface: PciField<u8>,
    pub subsystem_vendor_id: PciField<u16>,
    pub subsystem_device_id: PciField<u16>,
    pub parent: PciField<PciAddress>,
    pub irq: PciField<u32>,
    pub driver: PciField<String>,
    pub resources: PciField<Vec<PciResource>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PciInspection {
    pub device: PciDevice,
    pub details: PciDeviceDetails,
}
```

- [ ] **Step 3: Re-export the public types**

In `crates/pci/src/lib.rs`, add the modules and re-exports:

```rust
mod details;
mod field;

pub use details::{PciDeviceDetails, PciInspection, PciResource};
pub use field::{PciField, PciFieldUnavailableReason};
```

- [ ] **Step 4: Add focused pure tests**

Test that the field states preserve their values and reasons:

```rust
#[test]
fn unavailable_field_keeps_reason() {
    let field: PciField<u32> = PciField::Unavailable {
        reason: PciFieldUnavailableReason::PermissionDenied,
    };

    assert_eq!(
        field,
        PciField::Unavailable {
            reason: PciFieldUnavailableReason::PermissionDenied,
        }
    );
}
```

Run:

```bash
cargo test -p pci field
```

Expected: PASS without accessing PCI hardware.

- [ ] **Step 5: Commit the domain model**

```bash
git add crates/pci/src/field.rs crates/pci/src/details.rs crates/pci/src/lib.rs crates/pci/src/device.rs
git commit -m "feat: add pci detail field model"
```

### Task 2: Implement `PciSession::inspect()` with libpci fields

**Files:**
- Modify: `crates/pci/src/session.rs`
- Modify: `crates/pci/src/error.rs`
- Modify: `crates/pci-sys/src/build.rs` only if the generated 3.8.0 bindings omit a header symbol
- Test: ECS real-system smoke commands

**Interfaces:**
- Consumes: `PciField`, `PciDeviceDetails`, `PciInspection`, existing `PciSession::scan()` helpers.
- Produces: `PciSession::inspect(&mut self, address: PciAddress) -> Result<PciInspection, PciError>`.

- [ ] **Step 1: Verify the generated libpci API before editing Rust**

Inside the ECS Dev Container, run:

```bash
rg -n "PCI_FILL_(IRQ|BASES|SIZES|CLASS_EXT|SUBSYS|DRIVER|PARENT)|pci_get_string_property" \
  target/x86_64-unknown-linux-gnu/debug/build/pci-sys-*/out/bindings.rs
```

Expected: the output contains the requested constants and the `pci_get_string_property` declaration. The container’s installed `pciutils-devel` package is the authority for the actual ABI.

- [ ] **Step 2: Add a target-not-found error**

In `PciError`, add:

```rust
DeviceNotFound { address: PciAddress },
```

Format it as:

```text
PCI device 0000:00:05.0 was not found
```

Do not use this error for an individual field that is unavailable.

- [ ] **Step 3: Define the detail request mask**

In `session.rs`, request the fields supported by pciutils 3.8.0:

```rust
let requested_fields = PCI_FILL_IDENT
    | PCI_FILL_CLASS
    | PCI_FILL_IRQ
    | PCI_FILL_BASES
    | PCI_FILL_SIZES
    | PCI_FILL_CLASS_EXT
    | PCI_FILL_SUBSYS
    | PCI_FILL_PARENT
    | PCI_FILL_DRIVER
    | PCI_FILL_IO_FLAGS;
```

Pass the mask as the `c_int` expected by `pci_fill_info`.

- [ ] **Step 4: Find the target raw device and copy the summary**

Use the existing bus scan and linked-list traversal. Compare all four address components:

```rust
raw_domain == address.domain
    && raw_bus == address.bus
    && raw_slot == address.slot
    && raw_function == address.function
```

When the target is found, reuse the existing identity/name conversion logic to construct `PciDevice`. If traversal ends without a match, return `PciError::DeviceNotFound { address }`.

- [ ] **Step 5: Map `known_fields` to field states**

Use `PciField::Unavailable { reason: Unknown }` whenever a requested flag is absent. Do not infer `PermissionDenied` from a missing bit.

Map known scalar values:

```text
PCI_FILL_CLASS_EXT → rev_id and prog_if
PCI_FILL_SUBSYS    → subsys_vendor_id and subsys_id
PCI_FILL_IRQ       → irq when non-negative
```

If a known field is semantically absent, use `PciField::NotApplicable`. For example, a null parent pointer is `NotApplicable`.

- [ ] **Step 6: Read the driver property through libpci**

After requesting `PCI_FILL_DRIVER`, call:

```rust
pci_get_string_property(raw, PCI_FILL_DRIVER)
```

Interpret the result as follows:

```text
known flag absent → Unavailable(Unknown)
known flag present + null pointer → Unavailable(NotBound)
known flag present + valid C string → Available(String)
```

Copy the string before `PciSession` is dropped.

- [ ] **Step 7: Copy BAR/resource information**

Read the six `base_addr`, `size`, and `flags` entries from `struct pci_dev`. Create a `PciResource` for each entry that has a non-zero address, size, or flags. Cast the libpci address type to `u64` only after copying it out of the C structure.

If the required resource flags are unavailable, return `PciField::Unavailable { reason: Unknown }`. If all six entries are empty and the resource fields are known, return `PciField::NotApplicable`.

- [ ] **Step 8: Copy the parent address safely**

When `PCI_FILL_PARENT` is known and `raw.parent` is non-null, copy the parent’s domain, bus, device, and function into a new `PciAddress`. Never return or store the parent raw pointer.

- [ ] **Step 9: Run real ECS validation**

Inside the ECS Dev Container:

```bash
cargo fmt --all -- --check
cargo check -p pci --target x86_64-unknown-linux-gnu
cargo test -p pci
```

Then run the real binary against an existing BDF:

```bash
target/x86_64-unknown-linux-gnu/debug/lspci-rs show 0000:00:05.0
```

Expected: the command returns a device inspection or a precise `DeviceNotFound` error; no generated fixture is used.

- [ ] **Step 10: Commit the session implementation**

```bash
git add crates/pci/src/session.rs crates/pci/src/error.rs
git commit -m "feat: inspect one pci device through libpci"
```

### Task 3: Parse BDF addresses and add the `show` subcommand

**Files:**
- Modify: `crates/pci/src/device.rs`
- Modify: `crates/pci/src/lib.rs`
- Modify: `crates/lspci-rs/src/cli.rs`
- Modify: `crates/lspci-rs/src/main.rs`
- Test: `crates/pci/src/device.rs` unit tests and clap help output

**Interfaces:**
- Consumes: `PciAddress`, `PciSession::inspect`, `OutputFormat`.
- Produces: `Command::Show { address: PciAddress, format: OutputFormat }`.

- [ ] **Step 1: Add a dedicated parse error**

Define a public `PciAddressParseError` implementing `Debug`, `Display`, and `std::error::Error`. Its display text must state:

```text
invalid PCI address; expected dddd:bb:ss.f
```

Re-export `PciAddressParseError` from `crates/pci/src/lib.rs` next to `PciAddress` so the CLI can use the public parser error type.

- [ ] **Step 2: Implement `FromStr` for `PciAddress`**

Accept exactly four hexadecimal domain digits, two bus digits, two slot digits, and one function digit separated as:

```text
dddd:bb:ss.f
```

Reject malformed separators, non-hex characters, slot values above `0x1f`, and function values above `0x7`.

- [ ] **Step 3: Add parser unit tests**

Cover:

```text
0000:00:05.0 → success
ffff:ff:1f.7 → success
0000:00:05   → error
0000:00:20.0 → error
0000:00:05.8 → error
```

Run:

```bash
cargo test -p pci address
```

- [ ] **Step 4: Add the clap command**

Add:

```rust
Show {
    address: PciAddress,
    #[arg(long, value_enum, default_value_t = OutputFormat::Text)]
    format: OutputFormat,
},
```

Because `PciAddress` implements `FromStr`, clap can use it as the typed argument.

- [ ] **Step 5: Connect `show` in `main.rs`**

The command path should:

1. create `PciSession`;
2. call `inspect(address)`;
3. render the inspection according to `format`;
4. write normal output to stdout;
5. write `PciError` text to stderr and return a non-zero exit status.

Do not print debug formatting or a hard-coded success message.

- [ ] **Step 6: Verify CLI parsing**

Run:

```bash
cargo run -p lspci-rs -- show --help
cargo run -p lspci-rs -- show invalid-address
```

Expected: help lists the new command, and the invalid address fails before attempting PCI access.

- [ ] **Step 7: Commit the command path**

```bash
git add crates/pci/src/device.rs crates/pci/src/lib.rs crates/lspci-rs/src/cli.rs crates/lspci-rs/src/main.rs
git commit -m "feat: add pci show command"
```

### Task 4: Render inspection results as text and JSON

**Files:**
- Modify: `crates/lspci-rs/src/output.rs`
- Modify: `crates/lspci-rs/src/main.rs` or the existing output module declaration
- Test: renderer unit tests using manually constructed domain values, not PCI fixtures

**Interfaces:**
- Consumes: `PciInspection`, `PciField<T>`, `OutputFormat`.
- Produces: `render_inspection_text(&PciInspection) -> String` and `render_inspection_json(&PciInspection) -> Result<String, serde_json::Error>`.

- [ ] **Step 1: Add text rendering for field states**

Render the three states distinctly:

```text
Available(value) → field value
Unavailable(reason) → <unavailable: reason>
NotApplicable → <not applicable>
```

Use stable labels:

```text
Identity:
  Address:
  Vendor:
  Device:
  Class:
  Revision:
  Programming interface:
  Subsystem vendor:
  Subsystem device:

Topology:
  Parent:

Kernel:
  Driver:
  IRQ:

Resources:
  BAR0:
```

- [ ] **Step 2: Add JSON DTOs in the CLI layer**

Keep `serde` out of the `pci` domain types. The JSON root should contain:

```json
{
  "device": { ...existing summary fields... },
  "details": {
    "revision": { "status": "available", "value": "0x00" },
    "driver": { "status": "not_bound" },
    "irq": { "status": "unavailable", "reason": "unknown" }
  }
}
```

Use lowercase machine-readable reason strings and fixed-width lowercase hexadecimal strings for IDs, matching the existing JSON list conventions.

- [ ] **Step 3: Add renderer tests**

Construct `PciInspection` directly with values such as `Available(0)`, `Unavailable(Unknown)`, and `NotApplicable`. These tests do not pretend to be PCI hardware; they only verify formatting of already-defined domain states.

Cover:

- available revision appears in text;
- unavailable field includes its reason;
- not-applicable field is not confused with an error;
- JSON contains `status` and the correct `value` or `reason`.

- [ ] **Step 4: Run formatting and tests**

```bash
cargo fmt --all -- --check
cargo test -p lspci-rs
cargo test --workspace
```

- [ ] **Step 5: Commit the renderers**

```bash
git add crates/lspci-rs/src/output.rs crates/lspci-rs/src/main.rs
git commit -m "feat: render pci inspection details"
```

### Task 5: Perform real ECS comparison and release smoke validation

**Files:**
- Modify: `scripts/live-smoke.sh` if the existing script needs the new command
- Modify: `README.md` if the project has a README in the ECS checkout
- Test: real ECS commands only

**Interfaces:**
- Consumes: completed `show` command and existing release build process.
- Produces: verified real-system output and documented usage.

- [ ] **Step 1: Select a real BDF from the host**

Inside the ECS container, run:

```bash
lspci -D | sed -n '1p'
```

Use the BDF from that output as the target for the following commands.

- [ ] **Step 2: Compare against pciutils**

Run both tools for the same address:

```bash
lspci -s <BDF> -v
target/x86_64-unknown-linux-gnu/debug/lspci-rs show <BDF>
```

Compare identity, revision, subsystem, IRQ, driver, parent, and resource values. Differences in wording are acceptable; the underlying values must be explainable.

- [ ] **Step 3: Compare normal user and root behavior**

Run once as the normal ECS development user and once as root. Record fields that become available under root. Do not classify a field as permission denied unless the underlying diagnostic explicitly says so.

- [ ] **Step 4: Verify JSON output**

```bash
target/x86_64-unknown-linux-gnu/debug/lspci-rs show <BDF> --format json > /tmp/lspci-rs-show.json
wc -c /tmp/lspci-rs-show.json
grep -E '"device"|"details"|"status"' /tmp/lspci-rs-show.json
```

The command itself must return success after serialization; no Python or JSON fixture is required.

- [ ] **Step 5: Verify invalid target behavior**

```bash
target/x86_64-unknown-linux-gnu/debug/lspci-rs show ffff:ff:1f.7
```

Expected: non-zero exit status and a `DeviceNotFound` message on stderr.

- [ ] **Step 6: Build the release binary**

```bash
cargo fmt --all -- --check
cargo check --workspace --target x86_64-unknown-linux-gnu
cargo build --release -p lspci-rs --target x86_64-unknown-linux-gnu
ldd target/x86_64-unknown-linux-gnu/release/lspci-rs
target/x86_64-unknown-linux-gnu/release/lspci-rs show <BDF>
```

Expected: the binary links against the ECS runtime libraries and can inspect the same real BDF.

- [ ] **Step 7: Commit documentation and smoke updates**

```bash
git add scripts/live-smoke.sh README.md
git commit -m "docs: document pci show usage"
```

## Plan self-review

- Domain model: covered by Task 1.
- libpci field flags and owned-pointer conversion: covered by Task 2.
- BDF parsing and CLI contract: covered by Task 3.
- Text/JSON field status representation: covered by Task 4.
- Real ECS-only verification: covered by Task 5.
- PCI writes and callback recovery are explicitly excluded and have no implementation task.
- No fake PCI devices or fixtures are introduced.
- `PciDeviceDetails::parent` matches the requested `PCI_FILL_PARENT` flag.
- `PciSession::inspect()` signatures and renderer interfaces are consistent across tasks.
