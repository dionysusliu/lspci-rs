# Header Field Semantic Decoding Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Decode the PCI standard header Command register, Status register, and BAR types into typed fields with lspci -v-style text/JSON output.

**Architecture:** A new pure module `crates/pci/src/header.rs` decodes Command/Status/BAR bytes from the `ConfigSpaceSnapshot` that `inspect()` already prefetches (0x000..0x040) during capability discovery — zero new reads. `PciDeviceDetails` gains `command`/`status` fields and `PciResource` gains `bar_type`; renderers add `control:`/`status:` lines and per-BAR type annotations.

**Tech Stack:** Rust 2024 workspace, libpci FFI (unchanged), clap, serde. Build in container `95c90e05ab1a` on host `myece` (`/workspace`); validate on myece (header readable), dev48 (`sudo lspci -v`), sg-232e-224.

## Global Constraints

- No unit tests (user decision); per-task verification is `cargo fmt --check` + `cargo check`; final verification is real-hardware comparison.
- Decoder module contains zero FFI; decode failure yields `Unavailable`/`None` and never fails `inspect()`.
- `list` behavior unchanged; no new Rust dependencies.
- Bit tables follow the PCI specification; still cross-check against lspci output in Task 4.
- Verification commands run inside the container: `ssh myece 'docker exec 95c90e05ab1a bash -lc "cd /workspace && <cmd>"'`.
- Binary transfer chain (sftp only; scp is killed): build in container → on myece `podman cp 95c90e05ab1a:/workspace/target/x86_64-unknown-linux-gnu/debug/lspci-rs /tmp/lspci-rs` → locally `sftp myece <<< "get /tmp/lspci-rs <local>"` → `sftp dev48 <<< "put <local> /tmp/lspci-rs"` → on dev48 `sudo chmod +x /tmp/lspci-rs`.
- Branch `sdd/header-field-decoding` from `main`; finish via finishing-a-development-branch.
- Between Tasks 1–2 the workspace check FAILS on lspci-rs (struct literal changes) — expected; verify only `-p pci` until Task 3.

---

### Task 0: Create the feature branch

- [ ] **Step 1: Create and switch branch**

```bash
cd /workspace && git checkout main && git checkout -b sdd/header-field-decoding
```

---

### Task 1: header.rs decoders and types

**Files:**
- Create: `crates/pci/src/header.rs`
- Modify: `crates/pci/src/lib.rs`

**Interfaces:**
- Consumes: `ConfigSpaceSnapshot::read`, `PciField`, `PciFieldUnavailableReason`.
- Produces: `CommandRegister`, `StatusRegister`, `PciBarKind`, `PciBarType`; `pub(crate)` snapshot-based helpers `command_field`, `status_field`, `bar_type_field` consumed by Task 2.

- [ ] **Step 1: Create `crates/pci/src/header.rs`**

```rust
use crate::{ConfigSpaceSnapshot, PciField, PciFieldUnavailableReason};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommandRegister {
    pub io_space: bool,
    pub memory_space: bool,
    pub bus_master: bool,
    pub special_cycle: bool,
    pub mem_write_invalidate: bool,
    pub vga_palette_snoop: bool,
    pub parity_error_response: bool,
    pub stepping: bool,
    pub serr_enable: bool,
    pub fast_back_to_back: bool,
    pub interrupt_disable: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StatusRegister {
    pub interrupt_status: bool,
    pub capabilities_list: bool,
    pub capable_66mhz: bool,
    pub udf: bool,
    pub capable_fast_back_to_back: bool,
    pub master_parity_error: bool,
    /// 0 = fast, 1 = medium, 2 = slow
    pub devsel_timing: u8,
    pub signaled_target_abort: bool,
    pub received_target_abort: bool,
    pub received_master_abort: bool,
    pub signaled_system_error: bool,
    pub detected_parity_error: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PciBarKind {
    Io,
    Memory,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PciBarType {
    pub kind: PciBarKind,
    pub is_64_bit: bool,
    pub prefetchable: bool,
}

pub fn decode_command(word: u16) -> CommandRegister {
    CommandRegister {
        io_space: word & 0x0001 != 0,
        memory_space: word & 0x0002 != 0,
        bus_master: word & 0x0004 != 0,
        special_cycle: word & 0x0008 != 0,
        mem_write_invalidate: word & 0x0010 != 0,
        vga_palette_snoop: word & 0x0020 != 0,
        parity_error_response: word & 0x0040 != 0,
        stepping: word & 0x0080 != 0,
        serr_enable: word & 0x0100 != 0,
        fast_back_to_back: word & 0x0200 != 0,
        interrupt_disable: word & 0x0400 != 0,
    }
}

pub fn decode_status(word: u16) -> StatusRegister {
    StatusRegister {
        interrupt_status: word & 0x0008 != 0,
        capabilities_list: word & 0x0010 != 0,
        capable_66mhz: word & 0x0020 != 0,
        udf: word & 0x0040 != 0,
        capable_fast_back_to_back: word & 0x0080 != 0,
        master_parity_error: word & 0x0100 != 0,
        devsel_timing: ((word >> 9) & 0x0003) as u8,
        signaled_target_abort: word & 0x0800 != 0,
        received_target_abort: word & 0x1000 != 0,
        received_master_abort: word & 0x2000 != 0,
        signaled_system_error: word & 0x4000 != 0,
        detected_parity_error: word & 0x8000 != 0,
    }
}

pub fn decode_bar_type(bar: u32) -> PciBarType {
    if bar & 0x1 != 0 {
        PciBarType {
            kind: PciBarKind::Io,
            is_64_bit: false,
            prefetchable: false,
        }
    } else {
        PciBarType {
            kind: PciBarKind::Memory,
            is_64_bit: (bar >> 1) & 0x3 == 0x2,
            prefetchable: bar & 0x8 != 0,
        }
    }
}

fn read_word_field(
    snapshot: &ConfigSpaceSnapshot,
    offset: u32,
) -> Result<u16, PciFieldUnavailableReason> {
    let bytes = snapshot
        .read(offset, 2)
        .map_err(|_| PciFieldUnavailableReason::ReadError)?;
    Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
}

pub(crate) fn command_field(snapshot: &ConfigSpaceSnapshot) -> PciField<CommandRegister> {
    match read_word_field(snapshot, 0x04) {
        Ok(word) => PciField::Available(decode_command(word)),
        Err(reason) => PciField::Unavailable { reason },
    }
}

pub(crate) fn status_field(snapshot: &ConfigSpaceSnapshot) -> PciField<StatusRegister> {
    match read_word_field(snapshot, 0x06) {
        Ok(word) => PciField::Available(decode_status(word)),
        Err(reason) => PciField::Unavailable { reason },
    }
}

pub(crate) fn bar_type_field(snapshot: &ConfigSpaceSnapshot, index: u8) -> Option<PciBarType> {
    let bytes = snapshot.read(0x10 + u32::from(index) * 4, 4).ok()?;
    let bar = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
    Some(decode_bar_type(bar))
}
```

- [ ] **Step 2: Register and export in `crates/pci/src/lib.rs`**

Add the module declaration with the existing private modules:

```rust
mod header;
```

Add re-exports next to the existing ones:

```rust
pub use header::{CommandRegister, PciBarKind, PciBarType, StatusRegister};
```

- [ ] **Step 3: Verify and commit**

```bash
cargo fmt --all && cargo fmt --all -- --check
cargo check -p pci --target x86_64-unknown-linux-gnu
git add crates/pci/src/header.rs crates/pci/src/lib.rs
git commit -m "pci: add header register decoders"
```

---

### Task 2: Domain types and session wiring

**Files:**
- Modify: `crates/pci/src/details.rs`
- Modify: `crates/pci/src/session.rs`

**Interfaces:**
- Consumes: `header::command_field`, `header::status_field`, `header::bar_type_field` (Task 1).
- Produces: `PciDeviceDetails.command`, `PciDeviceDetails.status`, `PciResource.bar_type` populated by `inspect()`.

- [ ] **Step 1: Extend the domain types in `crates/pci/src/details.rs`**

Add `bar_type` to `PciResource`:

```rust
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PciResource {
    pub index: u8,
    pub start: u64,
    pub size: u64,
    pub flags: u64,
    pub bar_type: Option<crate::PciBarType>,
}
```

Add two fields after `capabilities` in `PciDeviceDetails`:

```rust
    pub command: PciField<crate::CommandRegister>,
    pub status: PciField<crate::StatusRegister>,
```

Update every construction site in this crate: each `PciResource { ... }` literal gains `bar_type: None`, and the `PciDeviceDetails` literal in `session.rs` `details_from_raw` gains:

```rust
                command: PciField::Unavailable {
                    reason: PciFieldUnavailableReason::ReadError,
                },
                status: PciField::Unavailable {
                    reason: PciFieldUnavailableReason::ReadError,
                },
```

(These defaults are overwritten by `inspect()` when the header is readable.)

- [ ] **Step 2: Rewire `inspect()` in `crates/pci/src/session.rs`**

Replace the block from `let device = Self::device_from_raw(self.access, raw);` through `Ok(PciInspection { device, details })` with:

```rust
            let device = Self::device_from_raw(self.access, raw);
            let mut reader = ConfigSpaceReader::new(raw, 0x000..0x1000);
            let header_readable = reader.read(0x000, 0x040).is_ok();
            let mut report = capability::discover(&mut reader);

            if header_readable {
                for capability in report.standard.iter() {
                    if matches!(capability.state, PciCapabilityState::Valid) {
                        let start = u32::from(capability.offset);
                        let end = (start + 0x40).min(0x100);
                        let _ = reader.fetch(start, end - start);
                    }
                }

                for capability in report.extended.iter() {
                    if matches!(capability.state, PciCapabilityState::Valid) {
                        let start = u32::from(capability.offset);
                        let end = (start + 0x60).min(0x1000);
                        let _ = reader.fetch(start, end - start);
                    }
                }

                // vendor-specific payloads can exceed the 64-byte prefetch
                for capability in report.standard.iter() {
                    if capability.id == 0x09
                        && matches!(capability.state, PciCapabilityState::Valid)
                    {
                        let length_offset = u32::from(capability.offset) + 2;
                        if let Ok(bytes) = reader.snapshot().read(length_offset, 1) {
                            let start = u32::from(capability.offset) + 3;
                            let end = (start + u32::from(bytes[0])).min(0x100);
                            if end > start {
                                let _ = reader.fetch(start, end - start);
                            }
                        }
                    }
                }

                let snapshot = reader.snapshot();
                for capability in report.standard.iter_mut() {
                    decoders::decode_content(snapshot, capability);
                }
                for capability in report.extended.iter_mut() {
                    decoders::decode_content(snapshot, capability);
                }
            }

            let capabilities = Self::capabilities_from_report(report, header_readable);
            let mut details = Self::details_from_raw(raw, known_fields, capabilities);

            if header_readable {
                let snapshot = reader.snapshot();
                details.command = header::command_field(snapshot);
                details.status = header::status_field(snapshot);
                if let PciField::Available(resources) = &mut details.resources {
                    for resource in resources {
                        resource.bar_type = header::bar_type_field(snapshot, resource.index);
                    }
                }
            }

            Ok(PciInspection { device, details })
```

Add `header` to the `use crate::{...}` import list in `session.rs` (cargo fmt re-sorts). Note: the existing capabilities block previously created its own reader and ended with `Self::capabilities_from_report(report, header_readable)` inside the block — that whole block is replaced by the flat structure above.

- [ ] **Step 3: Verify and commit**

```bash
cargo fmt --all && cargo fmt --all -- --check
cargo check -p pci --target x86_64-unknown-linux-gnu
git add crates/pci/src/details.rs crates/pci/src/session.rs
git commit -m "pci: decode command, status and BAR types during inspection"
```

---

### Task 3: Text and JSON rendering

**Files:**
- Modify: `crates/lspci-rs/src/output.rs`

**Interfaces:**
- Consumes: `CommandRegister`, `StatusRegister`, `PciResource.bar_type` (Task 2).
- Produces: `control:`/`status:` text lines before `resources:`; `type=...` on BAR lines; JSON `control`/`status` objects and per-resource `bar_type` string.

- [ ] **Step 1: Add text renderers**

Add `CommandRegister, StatusRegister, PciBarKind` (via the `pci::` import list) and these functions:

```rust
fn render_command_text(command: &CommandRegister) -> String {
    let flag = |enabled: bool| if enabled { "+" } else { "-" };
    format!(
        "I/O{} Mem{} BusMaster{} SpecCycle{} MemWINV{} VGASnoop{} ParErr{} Stepping{} SERR{} FastB2B{} DisINTx{}",
        flag(command.io_space),
        flag(command.memory_space),
        flag(command.bus_master),
        flag(command.special_cycle),
        flag(command.mem_write_invalidate),
        flag(command.vga_palette_snoop),
        flag(command.parity_error_response),
        flag(command.stepping),
        flag(command.serr_enable),
        flag(command.fast_back_to_back),
        flag(command.interrupt_disable),
    )
}

fn render_status_text(status: &StatusRegister) -> String {
    let flag = |enabled: bool| if enabled { "+" } else { "-" };
    let devsel = match status.devsel_timing {
        0 => "fast",
        1 => "medium",
        2 => "slow",
        _ => "unknown",
    };
    format!(
        "Cap{} 66MHz{} UDF{} FastB2B{} ParErr{} DEVSEL={} >TAbort{} <TAbort{} <MAbort{} >SERR{} <PERR{} INTx{}",
        flag(status.capabilities_list),
        flag(status.capable_66mhz),
        flag(status.udf),
        flag(status.capable_fast_back_to_back),
        flag(status.master_parity_error),
        devsel,
        flag(status.signaled_target_abort),
        flag(status.received_target_abort),
        flag(status.received_master_abort),
        flag(status.signaled_system_error),
        flag(status.detected_parity_error),
        flag(status.interrupt_status),
    )
}

fn render_bar_type(bar_type: &PciBarType) -> String {
    match bar_type.kind {
        PciBarKind::Io => "io".to_owned(),
        PciBarKind::Memory => {
            let width = if bar_type.is_64_bit { "64" } else { "32" };
            if bar_type.prefetchable {
                format!("memory-{width}-prefetch")
            } else {
                format!("memory-{width}")
            }
        }
    }
}
```

- [ ] **Step 2: Wire the text lines into `render_inspection_text`**

Insert before the existing `match &details.resources {` block:

```rust
    match &details.command {
        PciField::Available(command) => {
            writeln!(output, "  control: {}", render_command_text(command)).unwrap();
        }
        PciField::Unavailable { reason } => {
            writeln!(output, "  control: <unavailable: {reason:?}>").unwrap();
        }
        PciField::NotApplicable => {
            writeln!(output, "  control: <not-applicable>").unwrap();
        }
    }

    match &details.status {
        PciField::Available(status) => {
            writeln!(output, "  status: {}", render_status_text(status)).unwrap();
        }
        PciField::Unavailable { reason } => {
            writeln!(output, "  status: <unavailable: {reason:?}>").unwrap();
        }
        PciField::NotApplicable => {
            writeln!(output, "  status: <not-applicable>").unwrap();
        }
    }
```

In the resources loop, replace the BAR `writeln!` with:

```rust
            for resource in resources {
                let bar_type = resource
                    .bar_type
                    .as_ref()
                    .map(render_bar_type)
                    .unwrap_or_else(|| "unknown".to_owned());
                writeln!(
                    output,
                    "    BAR{} start=0x{:x} size=0x{:x} type={} flags=0x{:x}",
                    resource.index, resource.start, resource.size, bar_type, resource.flags
                )
                .unwrap();
            }
```

- [ ] **Step 3: Add JSON structs**

```rust
#[derive(Debug, Serialize)]
struct JsonCommand {
    io_space: bool,
    memory_space: bool,
    bus_master: bool,
    special_cycle: bool,
    mem_write_invalidate: bool,
    vga_palette_snoop: bool,
    parity_error_response: bool,
    stepping: bool,
    serr_enable: bool,
    fast_back_to_back: bool,
    interrupt_disable: bool,
}

#[derive(Debug, Serialize)]
struct JsonStatus {
    interrupt_status: bool,
    capabilities_list: bool,
    capable_66mhz: bool,
    udf: bool,
    capable_fast_back_to_back: bool,
    master_parity_error: bool,
    devsel: String,
    signaled_target_abort: bool,
    received_target_abort: bool,
    received_master_abort: bool,
    signaled_system_error: bool,
    detected_parity_error: bool,
}
```

Add to `JsonDetails` (after `capabilities`):

```rust
    command: JsonField<JsonCommand>,
    status: JsonField<JsonStatus>,
```

Add `bar_type` to `JsonResource`:

```rust
#[derive(Debug, Serialize)]
struct JsonResource {
    index: u8,
    start: String,
    size: String,
    flags: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    bar_type: Option<String>,
}
```

- [ ] **Step 4: Add JSON mappings**

Add to the `JsonDetails` construction in `render_inspection_json`:

```rust
            command: json_command(&details.command),
            status: json_status(&details.status),
```

Add these functions:

```rust
fn json_command(field: &PciField<CommandRegister>) -> JsonField<JsonCommand> {
    match field {
        PciField::Available(command) => JsonField {
            state: "available",
            value: Some(JsonCommand {
                io_space: command.io_space,
                memory_space: command.memory_space,
                bus_master: command.bus_master,
                special_cycle: command.special_cycle,
                mem_write_invalidate: command.mem_write_invalidate,
                vga_palette_snoop: command.vga_palette_snoop,
                parity_error_response: command.parity_error_response,
                stepping: command.stepping,
                serr_enable: command.serr_enable,
                fast_back_to_back: command.fast_back_to_back,
                interrupt_disable: command.interrupt_disable,
            }),
            reason: None,
        },
        PciField::Unavailable { reason } => JsonField {
            state: "unavailable",
            value: None,
            reason: Some(format!("{reason:?}")),
        },
        PciField::NotApplicable => JsonField {
            state: "not_applicable",
            value: None,
            reason: None,
        },
    }
}

fn json_status(field: &PciField<StatusRegister>) -> JsonField<JsonStatus> {
    match field {
        PciField::Available(status) => JsonField {
            state: "available",
            value: Some(JsonStatus {
                interrupt_status: status.interrupt_status,
                capabilities_list: status.capabilities_list,
                capable_66mhz: status.capable_66mhz,
                udf: status.udf,
                capable_fast_back_to_back: status.capable_fast_back_to_back,
                master_parity_error: status.master_parity_error,
                devsel: match status.devsel_timing {
                    0 => "fast".to_owned(),
                    1 => "medium".to_owned(),
                    2 => "slow".to_owned(),
                    _ => "unknown".to_owned(),
                },
                signaled_target_abort: status.signaled_target_abort,
                received_target_abort: status.received_target_abort,
                received_master_abort: status.received_master_abort,
                signaled_system_error: status.signaled_system_error,
                detected_parity_error: status.detected_parity_error,
            }),
            reason: None,
        },
        PciField::Unavailable { reason } => JsonField {
            state: "unavailable",
            value: None,
            reason: Some(format!("{reason:?}")),
        },
        PciField::NotApplicable => JsonField {
            state: "not_applicable",
            value: None,
            reason: None,
        },
    }
}
```

In `json_resources`, extend the per-resource mapping:

```rust
                    .map(|resource| JsonResource {
                        index: resource.index,
                        start: format!("0x{:x}", resource.start),
                        size: format!("0x{:x}", resource.size),
                        flags: format!("0x{:x}", resource.flags),
                        bar_type: resource.bar_type.as_ref().map(render_bar_type),
                    })
```

- [ ] **Step 5: Verify and commit**

```bash
cargo fmt --all && cargo fmt --all -- --check
cargo check --workspace --target x86_64-unknown-linux-gnu
cargo run -q -p lspci-rs --target x86_64-unknown-linux-gnu -- show 0000:00:05.0 --format text
cargo run -q -p lspci-rs --target x86_64-unknown-linux-gnu -- show 0000:00:05.0 --format json
```

Expected (myece): new `control:`/`status:` lines with bit flags, BAR lines with `type=...`, capabilities section unchanged.

```bash
git add crates/lspci-rs/src/output.rs
git commit -m "cli: render header command, status and BAR types"
```

---

### Task 4: Real-hardware validation and finish

**Files:** none (verification only), plus progress doc.

**Interfaces:**
- Consumes: completed branch binary; myece, dev48, sg-232e-224 access.
- Produces: comparison evidence against `lspci -v`, regression evidence, handoff doc.

- [ ] **Step 1: Validate on myece first (header is readable there)**

```bash
cd /workspace
cargo run -q -p lspci-rs --target x86_64-unknown-linux-gnu -- show 0000:00:05.0 --format text
lspci -s 00:05.0 -v
```

Compare the `control:`/`status:` lines and BAR types against lspci's `Control:`/`Status:`/`Region` output inside the container (myece lspci is standard 3.8.0).

- [ ] **Step 2: Build and transfer to dev48**

```bash
# in container
cd /workspace && cargo build -p lspci-rs --target x86_64-unknown-linux-gnu
# on myece host
podman cp 95c90e05ab1a:/workspace/target/x86_64-unknown-linux-gnu/debug/lspci-rs /tmp/lspci-rs
# locally
sftp myece <<< "get /tmp/lspci-rs <local-staging-path>"
sftp dev48 <<< "put <local-staging-path> /tmp/lspci-rs"
ssh dev48 'sudo chmod +x /tmp/lspci-rs'
```

- [ ] **Step 3: Compare on dev48 (endpoint + bridge)**

```bash
ssh dev48 'sudo /tmp/lspci-rs show 0000:00:05.0 --format text'
ssh dev48 'sudo lspci -s 00:05.0 -v'
ssh dev48 'sudo /tmp/lspci-rs show 0000:00:1f.0 --format text'
ssh dev48 'sudo lspci -s 00:1f.0 -v'
```

Compare Control/Status flags and Region types field by field. Fix any mismatch in the container, rebuild, re-transfer, re-compare.

- [ ] **Step 4: Auxiliary cross-check on sg-232e-224**

Transfer the same binary to sg-232e-224 via the sftp chain, then compare one NIC:

```bash
ssh sg-232e-224 'sudo chmod +x /tmp/lspci-rs; sudo /tmp/lspci-rs show 0000:3d:00.0 --format text'
ssh sg-232e-224 'sudo lspci -s 3d:00.0 -v'
```

Note: this machine's lspci is vendor-patched; where it disagrees with the PCI spec, keep the spec interpretation and record the difference.

- [ ] **Step 5: Regression on myece and dev48**

```bash
# myece container
cd /workspace
cargo fmt --all -- --check
cargo check --workspace --target x86_64-unknown-linux-gnu
cargo run -q -p lspci-rs --target x86_64-unknown-linux-gnu -- list --format text | wc -l
cargo run -q -p lspci-rs --target x86_64-unknown-linux-gnu -- show 0000:00:05.0 --config standard --format text
git diff --check
# dev48
ssh dev48 'sudo /tmp/lspci-rs show 0000:00:1f.0 --format text | grep -c "slot-id\|hot-plug"'
```

Expected: myece 9 devices, config dump and capability output unchanged; dev48 capability decoding unchanged.

- [ ] **Step 6: Record the handoff**

Create `docs/superpowers/progress/2026-08-10-header-field-decoding-progress.md` recording: commit list, validation devices, comparison results per register, any mismatches fixed. Commit:

```bash
git add docs/superpowers/progress/2026-08-10-header-field-decoding-progress.md
git commit -m "docs: record header field decoding validation results"
```

- [ ] **Step 7: Finish the branch**

Use superpowers:finishing-a-development-branch to merge `sdd/header-field-decoding` into `main` (or follow the user's chosen option).
