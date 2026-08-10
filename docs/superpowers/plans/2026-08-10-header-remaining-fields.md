# Remaining Header Fields Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Decode the remaining PCI header fields — Cache Line Size, Latency Timer, Header Type, BIST, Expansion ROM, Interrupt Line/Pin, CardBus CIS, and the full Type 1 bridge block (bus numbers, IO/Memory/Prefetchable windows, Secondary Status, Bridge Control) — with lspci -v-style output.

**Architecture:** `crates/pci/src/header.rs` gains pure decode functions consuming the already-prefetched header snapshot (0x00–0x40); `PciDeviceDetails` gains the new fields; session wiring fills them from the snapshot; renderers add the new lines/objects. Zero new config reads.

**Tech Stack:** Rust 2024 workspace, pure decoding over `ConfigSpaceSnapshot`, serde. Build in container `95c90e05ab1a` on host `myece` (`/workspace`); validate on dev48 (bridge 00:1f.0) and sg-232e-224 bridges vs `lspci -v`; myece endpoint regression.

## Global Constraints

- No unit tests (user decision); verification is `cargo fmt --check` + `cargo check` + real-hardware comparison.
- Decode functions contain zero FFI; decode failure yields `Unavailable`/`NotApplicable` and never fails `inspect()`.
- `list` behavior unchanged; no new dependencies.
- Verification commands run inside the container: `ssh myece 'docker exec 95c90e05ab1a bash -lc "cd /workspace && <cmd>"'`.
- Binary transfer chain (sftp only; scp is killed): build in container → on myece `podman cp 95c90e05ab1a:/workspace/target/x86_64-unknown-linux-gnu/debug/lspci-rs /tmp/lspci-rs` → locally `sftp myece <<< "get /tmp/lspci-rs <local>"` → `sftp dev48 <<< "put <local> /tmp/lspci-rs"` (same for sg-232e-224) → on target `sudo chmod +x /tmp/lspci-rs`.
- Branch `sdd/header-remaining-fields` from `main`; finish via finishing-a-development-branch.
- Tasks 1–2 keep the workspace compiling (renderers don't reference new fields until Task 3); verify the full workspace at the end of each task.

---

### Task 0: Create the feature branch

- [ ] **Step 1: Create and switch branch**

```bash
cd /workspace && git checkout main && git checkout -b sdd/header-remaining-fields
```

---

### Task 1: Header decode functions

**Files:**
- Modify: `crates/pci/src/header.rs`
- Modify: `crates/pci/src/lib.rs`

**Interfaces:**
- Consumes: `ConfigSpaceSnapshot::read`, `PciField`, `PciFieldUnavailableReason` (existing).
- Produces: types `PciHeaderKind`, `PciHeaderType`, `PciBist`, `PciExpansionRom`, `PciInterruptPin`, `PciBridgeHeader`; `pub(crate)` field decoders consumed by Task 2 wiring.

- [ ] **Step 1: Add types and decode functions to `crates/pci/src/header.rs`**

Append to the existing file:

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PciHeaderKind {
    Device,
    Bridge,
    CardBus,
    Unknown(u8),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PciHeaderType {
    pub kind: PciHeaderKind,
    pub multifunction: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PciBist {
    pub capable: bool,
    pub start: bool,
    pub completion_code: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PciExpansionRom {
    pub enable: bool,
    pub address: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PciInterruptPin {
    None,
    IntA,
    IntB,
    IntC,
    IntD,
    Unknown(u8),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PciBridgeHeader {
    pub primary_bus: u8,
    pub secondary_bus: u8,
    pub subordinate_bus: u8,
    pub secondary_latency_timer: u8,
    pub io_base: u32,
    pub io_limit: u32,
    pub io_enabled: bool,
    pub secondary_status: u16,
    pub memory_base: u32,
    pub memory_limit: u32,
    pub memory_enabled: bool,
    pub prefetchable_base: u64,
    pub prefetchable_limit: u64,
    pub prefetchable_64_bit: bool,
    pub prefetchable_enabled: bool,
    pub bridge_control: u16,
}

fn read_byte_field(
    snapshot: &ConfigSpaceSnapshot,
    offset: u32,
) -> Result<u8, PciFieldUnavailableReason> {
    let bytes = snapshot
        .read(offset, 1)
        .map_err(|_| PciFieldUnavailableReason::ReadError)?;
    Ok(bytes[0])
}

fn read_word_field_at(
    snapshot: &ConfigSpaceSnapshot,
    offset: u32,
) -> Result<u16, PciFieldUnavailableReason> {
    let bytes = snapshot
        .read(offset, 2)
        .map_err(|_| PciFieldUnavailableReason::ReadError)?;
    Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
}

fn read_dword_field(
    snapshot: &ConfigSpaceSnapshot,
    offset: u32,
) -> Result<u32, PciFieldUnavailableReason> {
    let bytes = snapshot
        .read(offset, 4)
        .map_err(|_| PciFieldUnavailableReason::ReadError)?;
    Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}

pub(crate) fn cache_line_size_field(snapshot: &ConfigSpaceSnapshot) -> PciField<u8> {
    match read_byte_field(snapshot, 0x0c) {
        Ok(value) => PciField::Available(value),
        Err(reason) => PciField::Unavailable { reason },
    }
}

pub(crate) fn latency_timer_field(snapshot: &ConfigSpaceSnapshot) -> PciField<u8> {
    match read_byte_field(snapshot, 0x0d) {
        Ok(value) => PciField::Available(value),
        Err(reason) => PciField::Unavailable { reason },
    }
}

pub(crate) fn header_type_field(snapshot: &ConfigSpaceSnapshot) -> PciField<PciHeaderType> {
    match read_byte_field(snapshot, 0x0e) {
        Ok(raw) => {
            let kind = match raw & 0x7f {
                0 => PciHeaderKind::Device,
                1 => PciHeaderKind::Bridge,
                2 => PciHeaderKind::CardBus,
                other => PciHeaderKind::Unknown(other),
            };
            PciField::Available(PciHeaderType {
                kind,
                multifunction: raw & 0x80 != 0,
            })
        }
        Err(reason) => PciField::Unavailable { reason },
    }
}

pub(crate) fn bist_field(snapshot: &ConfigSpaceSnapshot) -> PciField<PciBist> {
    match read_byte_field(snapshot, 0x0f) {
        Ok(raw) => PciField::Available(PciBist {
            capable: raw & 0x80 != 0,
            start: raw & 0x40 != 0,
            completion_code: raw & 0x0f,
        }),
        Err(reason) => PciField::Unavailable { reason },
    }
}

pub(crate) fn expansion_rom_field(
    snapshot: &ConfigSpaceSnapshot,
    is_bridge: bool,
) -> PciField<PciExpansionRom> {
    let offset = if is_bridge { 0x38 } else { 0x30 };
    match read_dword_field(snapshot, offset) {
        Ok(raw) => PciField::Available(PciExpansionRom {
            enable: raw & 0x1 != 0,
            address: raw & 0xfffff800,
        }),
        Err(reason) => PciField::Unavailable { reason },
    }
}

pub(crate) fn interrupt_line_field(snapshot: &ConfigSpaceSnapshot) -> PciField<u8> {
    match read_byte_field(snapshot, 0x3c) {
        Ok(value) => PciField::Available(value),
        Err(reason) => PciField::Unavailable { reason },
    }
}

pub(crate) fn interrupt_pin_field(snapshot: &ConfigSpaceSnapshot) -> PciField<PciInterruptPin> {
    match read_byte_field(snapshot, 0x3d) {
        Ok(raw) => {
            let pin = match raw {
                0 => PciInterruptPin::None,
                1 => PciInterruptPin::IntA,
                2 => PciInterruptPin::IntB,
                3 => PciInterruptPin::IntC,
                4 => PciInterruptPin::IntD,
                other => PciInterruptPin::Unknown(other),
            };
            PciField::Available(pin)
        }
        Err(reason) => PciField::Unavailable { reason },
    }
}

pub(crate) fn cardbus_cis_field(snapshot: &ConfigSpaceSnapshot) -> PciField<u32> {
    match read_dword_field(snapshot, 0x28) {
        Ok(value) => PciField::Available(value),
        Err(reason) => PciField::Unavailable { reason },
    }
}

pub(crate) fn bridge_header_field(snapshot: &ConfigSpaceSnapshot) -> PciField<PciBridgeHeader> {
    // 0x18..0x40 covers every Type 1 register decoded here.
    let bytes = match snapshot.read(0x18, 0x28) {
        Ok(bytes) => bytes,
        Err(_) => {
            return PciField::Unavailable {
                reason: PciFieldUnavailableReason::ReadError,
            }
        }
    };

    let word = |offset: usize| u16::from_le_bytes([bytes[offset], bytes[offset + 1]]);
    let dword = |offset: usize| {
        u32::from_le_bytes([
            bytes[offset],
            bytes[offset + 1],
            bytes[offset + 2],
            bytes[offset + 3],
        ])
    };

    let primary_bus = bytes[0x00];
    let secondary_bus = bytes[0x01];
    let subordinate_bus = bytes[0x02];
    let secondary_latency_timer = bytes[0x03];
    let io_base_raw = bytes[0x04]; // 0x1c
    let io_limit_raw = bytes[0x05]; // 0x1d
    let secondary_status = word(0x06); // 0x1e
    let memory_base_raw = word(0x08); // 0x20
    let memory_limit_raw = word(0x0a); // 0x22
    let pref_base_lo = word(0x0c); // 0x24
    let pref_limit_lo = word(0x0e); // 0x26
    let pref_base_hi = dword(0x10); // 0x28
    let pref_limit_hi = dword(0x14); // 0x2c
    let io_base_upper = word(0x18); // 0x30
    let io_limit_upper = word(0x1a); // 0x32
    let bridge_control = word(0x26); // 0x3e

    let io_base = u32::from(io_base_raw & 0xf0) << 8 | u32::from(io_base_upper) << 16;
    let io_limit =
        u32::from(io_limit_raw & 0xf0) << 8 | u32::from(io_limit_upper) << 16 | 0xfff;
    let memory_base = u32::from(memory_base_raw) << 16;
    let memory_limit = (u32::from(memory_limit_raw) << 16) | 0x000f_ffff;
    let prefetchable_64_bit = pref_base_lo & 0x1 != 0;
    let prefetchable_base =
        (u64::from(pref_base_hi) << 32) | (u64::from(pref_base_lo & 0xfff0) << 16);
    let prefetchable_limit =
        (u64::from(pref_limit_hi) << 32) | ((u64::from(pref_limit_lo) << 16) | 0x000f_ffff);

    PciField::Available(PciBridgeHeader {
        primary_bus,
        secondary_bus,
        subordinate_bus,
        secondary_latency_timer,
        io_base,
        io_limit,
        io_enabled: io_base_raw != 0 || io_limit_raw != 0,
        secondary_status,
        memory_base,
        memory_limit,
        memory_enabled: memory_base_raw != 0 || memory_limit_raw != 0,
        prefetchable_base,
        prefetchable_limit,
        prefetchable_64_bit,
        prefetchable_enabled: pref_base_lo != 0 || pref_limit_lo != 0,
        bridge_control,
    })
}
```

Note: the existing file already has a private `read_word_field` helper (offset 0x04 used by command_field); the new helpers above use different names (`read_byte_field`, `read_word_field_at`, `read_dword_field`) to avoid clashing with it.

- [ ] **Step 2: Export the new types in `crates/pci/src/lib.rs`**

Replace the existing `pub use header::{...};` line with:

```rust
pub use header::{
    CommandRegister, PciBarKind, PciBarType, PciBist, PciBridgeHeader, PciExpansionRom,
    PciHeaderKind, PciHeaderType, PciInterruptPin, StatusRegister,
};
```

(Keeps the four existing exports and adds the six new types; cargo fmt re-sorts.)

- [ ] **Step 3: Verify and commit**

```bash
cargo fmt --all && cargo fmt --all -- --check
cargo check --workspace --target x86_64-unknown-linux-gnu
git add crates/pci/src/header.rs crates/pci/src/lib.rs
git commit -m "pci: add remaining header field decoders"
```

Expected: workspace compiles (the new functions are unused by session yet — dead-code warnings are acceptable at this step and disappear in Task 2).

---

### Task 2: Domain fields and session wiring

**Files:**
- Modify: `crates/pci/src/details.rs`
- Modify: `crates/pci/src/session.rs`

**Interfaces:**
- Consumes: Task 1 decode functions and types.
- Produces: new `PciDeviceDetails` fields filled by `inspect()`; renderers consume these in Task 3.

- [ ] **Step 1: Add fields to `PciDeviceDetails` in `crates/pci/src/details.rs`**

After the existing `pub status: PciField<crate::StatusRegister>,` line, add:

```rust
    pub cache_line_size: PciField<u8>,
    pub latency_timer: PciField<u8>,
    pub header_type: PciField<crate::PciHeaderType>,
    pub bist: PciField<crate::PciBist>,
    pub expansion_rom: PciField<crate::PciExpansionRom>,
    pub interrupt_line: PciField<u8>,
    pub interrupt_pin: PciField<crate::PciInterruptPin>,
    pub cardbus_cis_pointer: PciField<u32>,
    pub bridge: PciField<crate::PciBridgeHeader>,
```

- [ ] **Step 2: Initialize defaults in `details_from_raw` (session.rs)**

In the `PciDeviceDetails` struct literal inside `details_from_raw`, after the `status: PciField::Unavailable { ... },` initializer, add:

```rust
                cache_line_size: PciField::Unavailable {
                    reason: PciFieldUnavailableReason::ReadError,
                },
                latency_timer: PciField::Unavailable {
                    reason: PciFieldUnavailableReason::ReadError,
                },
                header_type: PciField::Unavailable {
                    reason: PciFieldUnavailableReason::ReadError,
                },
                bist: PciField::Unavailable {
                    reason: PciFieldUnavailableReason::ReadError,
                },
                expansion_rom: PciField::Unavailable {
                    reason: PciFieldUnavailableReason::ReadError,
                },
                interrupt_line: PciField::Unavailable {
                    reason: PciFieldUnavailableReason::ReadError,
                },
                interrupt_pin: PciField::Unavailable {
                    reason: PciFieldUnavailableReason::ReadError,
                },
                cardbus_cis_pointer: PciField::NotApplicable,
                bridge: PciField::NotApplicable,
```

- [ ] **Step 3: Fill from the snapshot in `inspect()`**

Extend the existing `if header_readable { let snapshot = reader.snapshot(); ... }` block in `inspect()`: after the `details.status = header::status_field(snapshot);` line, add:

```rust
                details.cache_line_size = header::cache_line_size_field(snapshot);
                details.latency_timer = header::latency_timer_field(snapshot);
                details.header_type = header::header_type_field(snapshot);
                details.bist = header::bist_field(snapshot);
                details.interrupt_line = header::interrupt_line_field(snapshot);
                details.interrupt_pin = header::interrupt_pin_field(snapshot);

                let header_kind = match &details.header_type {
                    PciField::Available(header_type) => Some(&header_type.kind),
                    _ => None,
                };
                let is_bridge = matches!(header_kind, Some(PciHeaderKind::Bridge));
                let is_cardbus = matches!(header_kind, Some(PciHeaderKind::CardBus));

                details.expansion_rom = header::expansion_rom_field(snapshot, is_bridge);
                if is_bridge {
                    details.bridge = header::bridge_header_field(snapshot);
                }
                if is_cardbus {
                    details.cardbus_cis_pointer = header::cardbus_cis_field(snapshot);
                }
```

Add `PciHeaderKind` to the `use crate::{...}` import list in session.rs (cargo fmt re-sorts).

- [ ] **Step 4: Verify and commit**

```bash
cargo fmt --all && cargo fmt --all -- --check
cargo check --workspace --target x86_64-unknown-linux-gnu
cargo run -q -p lspci-rs --target x86_64-unknown-linux-gnu -- show 0000:00:05.0 --format text
```

Expected: compiles; output unchanged (new fields are not rendered yet).

```bash
git add crates/pci/src/details.rs crates/pci/src/session.rs
git commit -m "pci: wire remaining header fields into inspection"
```

---

### Task 3: Text and JSON rendering

**Files:**
- Modify: `crates/lspci-rs/src/output.rs`

**Interfaces:**
- Consumes: Task 2 `PciDeviceDetails` fields.
- Produces: new text lines and JSON fields.

- [ ] **Step 1: Add text helpers**

Extend the `use pci::{...}` import list with `PciBist, PciBridgeHeader, PciExpansionRom, PciHeaderKind, PciHeaderType, PciInterruptPin` (cargo fmt re-sorts).

Add these functions next to the other render helpers:

```rust
fn render_header_kind(kind: &PciHeaderKind) -> String {
    match kind {
        PciHeaderKind::Device => "device".to_owned(),
        PciHeaderKind::Bridge => "bridge".to_owned(),
        PciHeaderKind::CardBus => "cardbus".to_owned(),
        PciHeaderKind::Unknown(value) => format!("unknown({value})"),
    }
}

fn render_interrupt_pin(pin: &PciInterruptPin) -> String {
    match pin {
        PciInterruptPin::None => "none".to_owned(),
        PciInterruptPin::IntA => "INTA".to_owned(),
        PciInterruptPin::IntB => "INTB".to_owned(),
        PciInterruptPin::IntC => "INTC".to_owned(),
        PciInterruptPin::IntD => "INTD".to_owned(),
        PciInterruptPin::Unknown(value) => format!("unknown({value})"),
    }
}

fn render_window_size(size: u64) -> String {
    if size >= 0x10_0000 && size % 0x10_0000 == 0 {
        format!("{}M", size / 0x10_0000)
    } else if size >= 0x400 && size % 0x400 == 0 {
        format!("{}K", size / 0x400)
    } else {
        format!("{size}")
    }
}

fn render_bridge_text(bridge: &PciBridgeHeader) -> String {
    let mut output = format!(
        "bus: primary={:02x} secondary={:02x} subordinate={:02x} sec_latency={}",
        bridge.primary_bus,
        bridge.secondary_bus,
        bridge.subordinate_bus,
        bridge.secondary_latency_timer
    );

    if bridge.io_enabled {
        output.push_str(&format!(
            "\nio behind bridge: 0x{:x}-0x{:x} [size={}]",
            bridge.io_base,
            bridge.io_limit,
            render_window_size(u64::from(bridge.io_limit - bridge.io_base + 1))
        ));
    } else {
        output.push_str("\nio behind bridge: disabled");
    }

    if bridge.memory_enabled {
        output.push_str(&format!(
            "\nmemory behind bridge: 0x{:x}-0x{:x} [size={}]",
            bridge.memory_base,
            bridge.memory_limit,
            render_window_size(u64::from(bridge.memory_limit - bridge.memory_base + 1))
        ));
    } else {
        output.push_str("\nmemory behind bridge: disabled");
    }

    if bridge.prefetchable_enabled {
        output.push_str(&format!(
            "\nprefetchable memory behind bridge: 0x{:x}-0x{:x} [size={}{}]",
            bridge.prefetchable_base,
            bridge.prefetchable_limit,
            render_window_size(bridge.prefetchable_limit - bridge.prefetchable_base + 1),
            if bridge.prefetchable_64_bit {
                ", 64-bit"
            } else {
                ""
            }
        ));
    } else {
        output.push_str("\nprefetchable memory behind bridge: disabled");
    }

    let flag = |word: u16, bit: u16| if word & bit != 0 { "+" } else { "-" };
    let status = bridge.secondary_status;
    let devsel = match (status >> 9) & 0x3 {
        0 => "fast",
        1 => "medium",
        2 => "slow",
        _ => "unknown",
    };
    output.push_str(&format!(
        "\nsecondary status: 66MHz{} FastB2B{} ParErr{} DEVSEL={} >TAbort{} <TAbort{} <MAbort{} >SERR{} <PERR{}",
        flag(status, 0x0020),
        flag(status, 0x0080),
        flag(status, 0x0100),
        devsel,
        flag(status, 0x0800),
        flag(status, 0x1000),
        flag(status, 0x2000),
        flag(status, 0x4000),
        flag(status, 0x8000),
    ));

    let control = bridge.bridge_control;
    output.push_str(&format!(
        "\nbridge control: ParErr{} SERR{} ISA{} VGA{} VGA16{} MasterAbort{} SecBusReset{} FastB2B{} PrimDiscard{} SecDiscard{} DiscardTimeout{} DiscardSERR{} SplitResp{}",
        flag(control, 0x0001),
        flag(control, 0x0002),
        flag(control, 0x0004),
        flag(control, 0x0008),
        flag(control, 0x0010),
        flag(control, 0x0020),
        flag(control, 0x0040),
        flag(control, 0x0080),
        flag(control, 0x0100),
        flag(control, 0x0200),
        flag(control, 0x0400),
        flag(control, 0x0800),
        flag(control, 0x1000),
    ));

    output
}
```

- [ ] **Step 2: Wire the new text lines into `render_inspection_text`**

Insert these blocks after the existing `match &details.status { ... }` block and before `match &details.resources {`:

```rust
    match &details.cache_line_size {
        PciField::Available(value) => {
            writeln!(output, "  cache line size: {} bytes", u32::from(*value) * 4).unwrap();
        }
        PciField::Unavailable { reason } => {
            writeln!(output, "  cache line size: <unavailable: {reason:?}>").unwrap();
        }
        PciField::NotApplicable => {
            writeln!(output, "  cache line size: <not-applicable>").unwrap();
        }
    }

    match &details.latency_timer {
        PciField::Available(value) => {
            writeln!(output, "  latency: {value}").unwrap();
        }
        PciField::Unavailable { reason } => {
            writeln!(output, "  latency: <unavailable: {reason:?}>").unwrap();
        }
        PciField::NotApplicable => {
            writeln!(output, "  latency: <not-applicable>").unwrap();
        }
    }

    match &details.header_type {
        PciField::Available(header_type) => {
            writeln!(
                output,
                "  header type: {} multifunction={}",
                render_header_kind(&header_type.kind),
                header_type.multifunction
            )
            .unwrap();
        }
        PciField::Unavailable { reason } => {
            writeln!(output, "  header type: <unavailable: {reason:?}>").unwrap();
        }
        PciField::NotApplicable => {
            writeln!(output, "  header type: <not-applicable>").unwrap();
        }
    }

    match &details.bist {
        PciField::Available(bist) => {
            writeln!(
                output,
                "  bist: capable={} start={} completion={}",
                bist.capable, bist.start, bist.completion_code
            )
            .unwrap();
        }
        PciField::Unavailable { reason } => {
            writeln!(output, "  bist: <unavailable: {reason:?}>").unwrap();
        }
        PciField::NotApplicable => {
            writeln!(output, "  bist: <not-applicable>").unwrap();
        }
    }

    match &details.expansion_rom {
        PciField::Available(rom) => {
            if rom.enable {
                writeln!(output, "  expansion rom: 0x{:08x} enabled", rom.address).unwrap();
            } else {
                writeln!(output, "  expansion rom: disabled").unwrap();
            }
        }
        PciField::Unavailable { reason } => {
            writeln!(output, "  expansion rom: <unavailable: {reason:?}>").unwrap();
        }
        PciField::NotApplicable => {
            writeln!(output, "  expansion rom: <not-applicable>").unwrap();
        }
    }

    match (&details.interrupt_pin, &details.interrupt_line) {
        (PciField::Available(pin), PciField::Available(line)) => {
            writeln!(output, "  interrupt: pin={} line={}", render_interrupt_pin(pin), line)
                .unwrap();
        }
        _ => {
            writeln!(output, "  interrupt: <unavailable>").unwrap();
        }
    }

    match &details.cardbus_cis_pointer {
        PciField::Available(value) => {
            writeln!(output, "  cardbus cis pointer: 0x{value:08x}").unwrap();
        }
        PciField::Unavailable { reason } => {
            writeln!(output, "  cardbus cis pointer: <unavailable: {reason:?}>").unwrap();
        }
        PciField::NotApplicable => {}
    }

    match &details.bridge {
        PciField::Available(bridge) => {
            writeln!(output, "  {}", render_bridge_text(bridge).replace('\n', "\n  "))
                .unwrap();
        }
        PciField::Unavailable { reason } => {
            writeln!(output, "  bridge: <unavailable: {reason:?}>").unwrap();
        }
        PciField::NotApplicable => {}
    }
```

- [ ] **Step 3: Add JSON structs and fields**

Add these structs next to the existing JSON structs:

```rust
#[derive(Debug, Serialize)]
struct JsonHeaderType {
    kind: String,
    multifunction: bool,
}

#[derive(Debug, Serialize)]
struct JsonBist {
    capable: bool,
    start: bool,
    completion_code: u8,
}

#[derive(Debug, Serialize)]
struct JsonExpansionRom {
    enable: bool,
    address: String,
}

#[derive(Debug, Serialize)]
struct JsonBridgeWindow {
    base: String,
    limit: String,
    size: String,
}

#[derive(Debug, Serialize)]
struct JsonBridge {
    primary_bus: String,
    secondary_bus: String,
    subordinate_bus: String,
    secondary_latency_timer: u8,
    io: Option<JsonBridgeWindow>,
    memory: Option<JsonBridgeWindow>,
    prefetchable: Option<JsonBridgeWindow>,
    prefetchable_64_bit: bool,
    secondary_status: String,
    bridge_control: String,
}
```

Add these fields to `JsonDetails` (after `status`):

```rust
    cache_line_size: JsonField<u8>,
    latency_timer: JsonField<u8>,
    header_type: JsonField<JsonHeaderType>,
    bist: JsonField<JsonBist>,
    expansion_rom: JsonField<JsonExpansionRom>,
    interrupt_line: JsonField<u8>,
    interrupt_pin: JsonField<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cardbus_cis_pointer: Option<JsonField<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    bridge: Option<JsonField<JsonBridge>>,
```

- [ ] **Step 4: Add JSON mapping helpers and wire them**

Add these helpers next to the other `json_*` functions:

```rust
fn json_interrupt_pin(field: &PciField<PciInterruptPin>) -> JsonField<String> {
    match field {
        PciField::Available(pin) => JsonField {
            state: "available",
            value: Some(render_interrupt_pin(pin)),
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

fn json_header_type(field: &PciField<PciHeaderType>) -> JsonField<JsonHeaderType> {
    match field {
        PciField::Available(header_type) => JsonField {
            state: "available",
            value: Some(JsonHeaderType {
                kind: render_header_kind(&header_type.kind),
                multifunction: header_type.multifunction,
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

fn json_bist(field: &PciField<PciBist>) -> JsonField<JsonBist> {
    match field {
        PciField::Available(bist) => JsonField {
            state: "available",
            value: Some(JsonBist {
                capable: bist.capable,
                start: bist.start,
                completion_code: bist.completion_code,
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

fn json_expansion_rom(field: &PciField<PciExpansionRom>) -> JsonField<JsonExpansionRom> {
    match field {
        PciField::Available(rom) => JsonField {
            state: "available",
            value: Some(JsonExpansionRom {
                enable: rom.enable,
                address: format!("0x{:08x}", rom.address),
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

fn json_bridge_window(base: u64, limit: u64) -> JsonBridgeWindow {
    JsonBridgeWindow {
        base: format!("0x{base:x}"),
        limit: format!("0x{limit:x}"),
        size: render_window_size(limit - base + 1),
    }
}

fn json_bridge(field: &PciField<PciBridgeHeader>) -> Option<JsonField<JsonBridge>> {
    match field {
        PciField::Available(bridge) => Some(JsonField {
            state: "available",
            value: Some(JsonBridge {
                primary_bus: format!("{:02x}", bridge.primary_bus),
                secondary_bus: format!("{:02x}", bridge.secondary_bus),
                subordinate_bus: format!("{:02x}", bridge.subordinate_bus),
                secondary_latency_timer: bridge.secondary_latency_timer,
                io: bridge
                    .io_enabled
                    .then(|| json_bridge_window(u64::from(bridge.io_base), u64::from(bridge.io_limit))),
                memory: bridge.memory_enabled.then(|| {
                    json_bridge_window(
                        u64::from(bridge.memory_base),
                        u64::from(bridge.memory_limit),
                    )
                }),
                prefetchable: bridge.prefetchable_enabled.then(|| {
                    json_bridge_window(bridge.prefetchable_base, bridge.prefetchable_limit)
                }),
                prefetchable_64_bit: bridge.prefetchable_64_bit,
                secondary_status: format!("0x{:04x}", bridge.secondary_status),
                bridge_control: format!("0x{:04x}", bridge.bridge_control),
            }),
            reason: None,
        }),
        PciField::Unavailable { reason } => Some(JsonField {
            state: "unavailable",
            value: None,
            reason: Some(format!("{reason:?}")),
        }),
        PciField::NotApplicable => None,
    }
}
```

In the `JsonDetails` construction inside `render_inspection_json`, after the `status: json_status(&details.status),` line, add:

```rust
            cache_line_size: json_field(&details.cache_line_size),
            latency_timer: json_field(&details.latency_timer),
            header_type: json_header_type(&details.header_type),
            bist: json_bist(&details.bist),
            expansion_rom: json_expansion_rom(&details.expansion_rom),
            interrupt_line: json_field(&details.interrupt_line),
            interrupt_pin: json_interrupt_pin(&details.interrupt_pin),
            cardbus_cis_pointer: match &details.cardbus_cis_pointer {
                PciField::NotApplicable => None,
                other => Some(json_hex_field(other)),
            },
            bridge: json_bridge(&details.bridge),
```

- [ ] **Step 5: Verify and commit**

```bash
cargo fmt --all && cargo fmt --all -- --check
cargo check --workspace --target x86_64-unknown-linux-gnu
cargo run -q -p lspci-rs --target x86_64-unknown-linux-gnu -- show 0000:00:05.0 --format text
cargo run -q -p lspci-rs --target x86_64-unknown-linux-gnu -- show 0000:00:05.0 --format json
```

Expected (myece): new lines appear (cache line size, latency, header type device, bist, expansion rom disabled, interrupt, no bridge block).

```bash
git add crates/lspci-rs/src/output.rs
git commit -m "cli: render remaining header fields"
```

---

### Task 4: Real-hardware validation and finish

**Files:** none (verification only), plus progress doc.

**Interfaces:**
- Consumes: completed branch binary; dev48 and sg-232e-224 access.
- Produces: comparison evidence, handoff doc.

- [ ] **Step 1: Build and transfer to dev48**

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

- [ ] **Step 2: Validate the bridge on dev48**

```bash
ssh dev48 'sudo /tmp/lspci-rs show 0000:00:1f.0 --format text'
ssh dev48 'sudo lspci -s 00:1f.0 -v'
```

Compare: bus numbers, I/O and memory windows (ranges + sizes), bridge control flags, interrupt pin/line, expansion ROM. Fix any mismatch (adjust bit extraction or window computation in header.rs), rebuild, re-transfer, re-compare.

- [ ] **Step 3: Validate endpoint fields and a bridge on sg-232e-224**

Transfer the same binary to sg-232e-224, then:

```bash
ssh sg-232e-224 'sudo /tmp/lspci-rs show 0000:3d:00.0 --format text'
ssh sg-232e-224 'sudo lspci -s 3d:00.0 -v'
ssh sg-232e-224 'sudo lspci -vvv | awk "/^[0-9a-f]+:/{d=\$1} /PCI bridge/{print d; exit}"'
```

Then compare that bridge the same way as Step 2. Note: sg lspci is vendor-patched; where it disagrees with the PCI spec, keep the spec interpretation and record the difference.

- [ ] **Step 4: Regression on myece**

```bash
cd /workspace
cargo fmt --all -- --check
cargo check --workspace --target x86_64-unknown-linux-gnu
cargo run -q -p lspci-rs --target x86_64-unknown-linux-gnu -- list --format text | wc -l
cargo run -q -p lspci-rs --target x86_64-unknown-linux-gnu -- show 0000:00:05.0 --config standard --format text
git diff --check
```

Expected: 9 devices; config dump and capability output unchanged; new header lines present.

- [ ] **Step 5: Record the handoff**

Create `docs/superpowers/progress/2026-08-10-header-remaining-fields-progress.md` recording: commit list, validation devices, comparison results, any fixes made. Commit:

```bash
git add docs/superpowers/progress/2026-08-10-header-remaining-fields-progress.md
git commit -m "docs: record header remaining fields validation results"
```

- [ ] **Step 6: Finish the branch**

Use superpowers:finishing-a-development-branch to merge `sdd/header-remaining-fields` into `main` (or follow the user's chosen option).
