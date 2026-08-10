# Capability Protocol Decoders Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Decode the five standard PCI capabilities (PM, MSI, MSI-X, PCIe, Vendor Specific) into typed `content` fields on `PciCapability`, with text/JSON output aligned to `lspci -vv`.

**Architecture:** Decoders are pure functions over `ConfigSpaceSnapshot` (no FFI). `PciCapability` gains `content: Option<PciCapabilityContent>`; decoders run inside `inspect()` right after chain discovery using the already-cached reader snapshot. Renderers print/serialize the typed content.

**Tech Stack:** Rust 2024 workspace, libpci/pciutils FFI (unchanged), clap, serde. Real-device validation on dev48 (Alibaba Cloud Linux 3 x86_64); build environment is ECS container `95c90e05ab1a` on host `myece`, workspace `/workspace` (= host `/home/leo/dev/lspci-rs`).

## Global Constraints

- No unit tests and no fixture files (user decision); verification is `cargo fmt --check` + `cargo check` per task and dev48 real-device comparison at the end.
- Decoders must not touch FFI; all `unsafe` stays in the existing pci-crate boundary.
- Decoder failure (unreadable payload bytes) must never fail `inspect()`; it yields `content = None`.
- Only the standard capability chain is decoded; extended nodes keep `content = None`.
- `list` behavior must not change; no new Rust dependencies.
- Work on branch `sdd/capability-decoders` created from `main`; merge via finishing-a-development-branch at the end.
- Intermediate compile errors in `lspci-rs` renderers are acceptable between Task 1 and Task 5 (same convention as the config-space slice).
- All verification commands run inside the container: `ssh myece 'docker exec 95c90e05ab1a bash -lc "cd /workspace && <cmd>"'`.

---

### Task 0: Create the feature branch

**Files:** none (git only)

- [ ] **Step 1: Create and switch branch**

```bash
cd /workspace && git checkout main && git checkout -b sdd/capability-decoders
```

Expected: `Switched to a new branch 'sdd/capability-decoders'`.

---

### Task 1: Domain types and pure snapshot read

**Files:**
- Create: `crates/pci/src/decoders/mod.rs`
- Modify: `crates/pci/src/field.rs`
- Modify: `crates/pci/src/config.rs`
- Modify: `crates/pci/src/lib.rs`

**Interfaces:**
- Consumes: existing `PciCapability`, `ConfigSpaceSnapshot`, `ConfigReadFailure`.
- Produces: `PciCapabilityContent` enum, `PciCapability.content` field, `ConfigSpaceSnapshot::read(&self, offset, length) -> Result<Vec<u8>, ConfigReadFailure>` used by every decoder task.

- [ ] **Step 1: Add the content enum and PM struct**

Create `crates/pci/src/decoders/mod.rs`. In this task it only carries the enum scaffold (Task 2 adds the other submodules and variants):

```rust
pub mod pm;

pub use pm::PmCapability;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PciCapabilityContent {
    Pm(PmCapability),
}
```

Create `crates/pci/src/decoders/pm.rs` with the final struct (Task 2 fills in the decode function):

```rust
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PmCapability {
    pub version: u8,
    pub pme_clock: bool,
    pub dsi: bool,
    pub aux_current: u8,
    pub d1_support: bool,
    pub d2_support: bool,
    /// bitmask: bit 0 = D0 ... bit 4 = D3cold
    pub pme_support: u8,
    /// 0 = D0, 1 = D1, 2 = D2, 3 = D3hot
    pub power_state: u8,
    pub no_soft_reset: bool,
    pub pme_enable: bool,
    pub data_select: u8,
    pub data_scale: u8,
    pub pme_status: bool,
}
```

- [ ] **Step 2: Add the `content` field**

In `crates/pci/src/field.rs`, extend `PciCapability`:

```rust
pub struct PciCapability {
    pub id: u16,
    pub kind: PciCapabilityKind,
    pub offset: u16,
    pub next: Option<u16>,
    pub state: PciCapabilityState,
    pub content: Option<crate::decoders::PciCapabilityContent>,
}
```

Update every `PciCapability { ... }` constructor in `crates/pci/src/capability.rs` to include `content: None`.

- [ ] **Step 3: Add pure `ConfigSpaceSnapshot::read`**

In `crates/pci/src/config.rs`, move the segment-copy loop out of `ConfigSpaceReader::read` into the snapshot:

```rust
impl ConfigSpaceSnapshot {
    pub fn read(&self, offset: u32, length: u32) -> Result<Vec<u8>, ConfigReadFailure> {
        let end = match offset.checked_add(length) {
            Some(end) if length != 0 => end,
            _ => return Err(self.missing_failure(offset, length)),
        };

        let mut bytes = Vec::with_capacity(length as usize);
        let mut cursor = offset;

        while cursor < end {
            let segment = match self.segment_covering(cursor) {
                Some(segment) => segment,
                None => return Err(self.missing_failure(offset, length)),
            };

            let segment_end = segment_end(segment);
            let take_end = segment_end.min(end);
            let start = (cursor - segment.offset) as usize;
            let len = (take_end - cursor) as usize;
            bytes.extend_from_slice(&segment.bytes[start..start + len]);
            cursor = take_end;
        }

        Ok(bytes)
    }

    fn missing_failure(&self, offset: u32, length: u32) -> ConfigReadFailure {
        ConfigReadFailure {
            offset,
            length,
            reason: PciFieldUnavailableReason::ReadError,
        }
    }

    fn segment_covering(&self, offset: u32) -> Option<&ConfigSegment> {
        self.segments
            .iter()
            .find(|segment| segment.offset <= offset && offset < segment_end(segment))
    }
}
```

Move `segment_covering`, `covering_segment_end`, and `next_segment_start` from `ConfigSpaceReader` to `ConfigSpaceSnapshot` (the reader methods become delegations), then simplify `ConfigSpaceReader::read` to:

```rust
pub(crate) fn read(&mut self, offset: u32, length: u32) -> Result<Vec<u8>, ConfigReadFailure> {
    self.fetch(offset, length)?;
    self.snapshot.read(offset, length)
}
```

- [ ] **Step 4: Export the module**

In `crates/pci/src/lib.rs`:

```rust
pub(crate) mod capability;
mod config;
pub(crate) mod decoders;
...
pub use decoders::PciCapabilityContent;
```

- [ ] **Step 5: Verify**

```bash
cargo fmt --all && cargo fmt --all -- --check
cargo check -p pci --target x86_64-unknown-linux-gnu
```

Expected: `pci` compiles. `lspci-rs` compile errors are acceptable until Task 5 only if constructors changed; the renderer does not construct `PciCapability`, so the workspace should still check — if it fails on `lspci-rs`, stop and investigate.

- [ ] **Step 6: Commit**

```bash
git add crates/pci/src/decoders/mod.rs crates/pci/src/decoders/pm.rs \
        crates/pci/src/field.rs crates/pci/src/config.rs \
        crates/pci/src/capability.rs crates/pci/src/lib.rs
git commit -m "pci: add capability content types and pure snapshot read"
```

---

### Task 2: PM, MSI, MSI-X, Vendor Specific decoders

**Files:**
- Create: `crates/pci/src/decoders/msi.rs`
- Create: `crates/pci/src/decoders/msix.rs`
- Create: `crates/pci/src/decoders/vendor.rs`
- Modify: `crates/pci/src/decoders/pm.rs`
- Modify: `crates/pci/src/decoders/mod.rs`

**Interfaces:**
- Consumes: `ConfigSpaceSnapshot::read`, `PmCapability` struct (Task 1).
- Produces: `decode_pm`, `decode_msi`, `decode_msix`, `decode_vendor_specific` — all `fn(snapshot: &ConfigSpaceSnapshot, offset: u16) -> Option<T>`; structs `MsiCapability`, `MsiXCapability`, `VendorSpecificCapability`.

- [ ] **Step 1: Add shared read helpers to `decoders/mod.rs`**

```rust
use crate::{ConfigReadFailure, ConfigSpaceSnapshot};

pub(crate) fn read_word(
    snapshot: &ConfigSpaceSnapshot,
    offset: u32,
) -> Result<u16, ConfigReadFailure> {
    let bytes = snapshot.read(offset, 2)?;
    Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
}

pub(crate) fn read_dword(
    snapshot: &ConfigSpaceSnapshot,
    offset: u32,
) -> Result<u32, ConfigReadFailure> {
    let bytes = snapshot.read(offset, 4)?;
    Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}
```

Also extend the enum (final form except PCIe, which lands in Task 3):

```rust
pub mod msi;
pub mod msix;
pub mod pm;
pub mod vendor;

pub use msi::MsiCapability;
pub use msix::MsiXCapability;
pub use pm::PmCapability;
pub use vendor::VendorSpecificCapability;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PciCapabilityContent {
    Pm(PmCapability),
    Msi(MsiCapability),
    MsiX(MsiXCapability),
    VendorSpecific(VendorSpecificCapability),
}
```

- [ ] **Step 2: Implement `decode_pm` in `pm.rs`**

Registers: PMC word at `offset+2`, PMCSR word at `offset+4`.

```rust
use super::read_word;
use crate::ConfigSpaceSnapshot;

pub fn decode_pm(snapshot: &ConfigSpaceSnapshot, offset: u16) -> Option<PmCapability> {
    let base = u32::from(offset);
    let pmc = read_word(snapshot, base + 2).ok()?;
    let pmcsr = read_word(snapshot, base + 4).ok()?;

    Some(PmCapability {
        version: (pmc & 0x0007) as u8,
        pme_clock: pmc & 0x0008 != 0,
        dsi: pmc & 0x0010 != 0,
        aux_current: ((pmc >> 6) & 0x0003) as u8,
        d1_support: pmc & 0x0200 != 0,
        d2_support: pmc & 0x0400 != 0,
        pme_support: ((pmc >> 11) & 0x001f) as u8,
        power_state: (pmcsr & 0x0003) as u8,
        no_soft_reset: pmcsr & 0x0008 != 0,
        pme_enable: pmcsr & 0x0100 != 0,
        data_select: ((pmcsr >> 9) & 0x000f) as u8,
        data_scale: ((pmcsr >> 13) & 0x0003) as u8,
        pme_status: pmcsr & 0x8000 != 0,
    })
}
```

- [ ] **Step 3: Implement `decode_msi` in `msi.rs`**

Struct:

```rust
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MsiCapability {
    pub enable: bool,
    /// log2 count; vectors = 1 << n
    pub multiple_message_capable: u8,
    pub multiple_message_enable: u8,
    pub is_64_bit: bool,
    pub per_vector_masking: bool,
    pub address: u64,
    pub data: u16,
}
```

Message Control word at `offset+2`: enable bit 0, capable bits 1–3, enable bits 4–6, 64-bit bit 7, per-vector masking bit 8. Address dword at `offset+4`; when 64-bit, upper dword at `offset+8` and data word at `offset+12`; otherwise data word at `offset+8`.

```rust
pub fn decode_msi(snapshot: &ConfigSpaceSnapshot, offset: u16) -> Option<MsiCapability> {
    let base = u32::from(offset);
    let control = read_word(snapshot, base + 2).ok()?;

    let is_64_bit = control & 0x0080 != 0;
    let (address, data) = if is_64_bit {
        let low = read_dword(snapshot, base + 4).ok()?;
        let high = read_dword(snapshot, base + 8).ok()?;
        let data = read_word(snapshot, base + 12).ok()?;
        (u64::from(low) | (u64::from(high) << 32), data)
    } else {
        let low = read_dword(snapshot, base + 4).ok()?;
        let data = read_word(snapshot, base + 8).ok()?;
        (u64::from(low), data)
    };

    Some(MsiCapability {
        enable: control & 0x0001 != 0,
        multiple_message_capable: ((control >> 1) & 0x0007) as u8,
        multiple_message_enable: ((control >> 4) & 0x0007) as u8,
        is_64_bit,
        per_vector_masking: control & 0x0100 != 0,
        address,
        data,
    })
}
```

- [ ] **Step 4: Implement `decode_msix` in `msix.rs`**

Struct:

```rust
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MsiXCapability {
    pub enable: bool,
    pub count: u16,
    pub masked: bool,
    pub table_bar: u8,
    pub table_offset: u32,
    pub pba_bar: u8,
    pub pba_offset: u32,
}
```

Message Control word at `offset+2`: table size bits 10:0 (count = value + 1), function mask bit 14, enable bit 15. Table dword at `offset+4`, PBA dword at `offset+8`: BIR bits 2:0, offset = value & !0x7.

```rust
pub fn decode_msix(snapshot: &ConfigSpaceSnapshot, offset: u16) -> Option<MsiXCapability> {
    let base = u32::from(offset);
    let control = read_word(snapshot, base + 2).ok()?;
    let table = read_dword(snapshot, base + 4).ok()?;
    let pba = read_dword(snapshot, base + 8).ok()?;

    Some(MsiXCapability {
        enable: control & 0x8000 != 0,
        count: (control & 0x07ff) + 1,
        masked: control & 0x4000 != 0,
        table_bar: (table & 0x7) as u8,
        table_offset: table & !0x7,
        pba_bar: (pba & 0x7) as u8,
        pba_offset: pba & !0x7,
    })
}
```

- [ ] **Step 5: Implement `decode_vendor_specific` in `vendor.rs`**

Struct:

```rust
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VendorSpecificCapability {
    pub length: u8,
    pub data: Vec<u8>,
}
```

Length byte at `offset+2`; data is the following `length` bytes starting at `offset+3`.

```rust
pub fn decode_vendor_specific(
    snapshot: &ConfigSpaceSnapshot,
    offset: u16,
) -> Option<VendorSpecificCapability> {
    let base = u32::from(offset);
    let length = snapshot.read(base + 2, 1).ok()?[0];
    let data = if length == 0 {
        Vec::new()
    } else {
        snapshot.read(base + 3, u32::from(length)).ok()?
    };

    Some(VendorSpecificCapability { length, data })
}
```

- [ ] **Step 6: Verify**

```bash
cargo fmt --all && cargo fmt --all -- --check
cargo check -p pci --target x86_64-unknown-linux-gnu
```

- [ ] **Step 7: Commit**

```bash
git add crates/pci/src/decoders/
git commit -m "pci: add PM, MSI, MSI-X and vendor-specific decoders"
```

---

### Task 3: PCIe decoder

**Files:**
- Create: `crates/pci/src/decoders/pcie.rs`
- Modify: `crates/pci/src/decoders/mod.rs`

**Interfaces:**
- Consumes: `read_word`, `read_dword` from `decoders/mod.rs`.
- Produces: `decode_pcie`, `PcieCapability`.

- [ ] **Step 1: Implement `decode_pcie` in `pcie.rs`**

Struct:

```rust
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PcieCapability {
    pub version: u8,
    /// device/port type code: 0 endpoint, 4 root port, 5/6 switch ports,
    /// 7/8 bridges, 9 root-complex integrated endpoint, 0xa event collector
    pub device_type: u8,
    pub slot_implemented: bool,
    pub interrupt_message_number: u8,
    pub dev_ctl: u16,
    pub dev_sta: u16,
    /// gen code: 1 = 2.5GT/s, 2 = 5, 3 = 8, 4 = 16, 5 = 32, 6 = 64
    pub link_max_speed: u8,
    pub link_max_width: u8,
    pub link_target_speed: u8,
    pub link_current_speed: u8,
    pub link_current_width: u8,
    pub link_training: bool,
    pub slot_ctl: Option<u16>,
    pub slot_sta: Option<u16>,
    pub root_ctl: Option<u16>,
    pub root_sta: Option<u32>,
}
```

Register map relative to cap base: Flags word +2 (version bits 3:0, type bits 7:4, slot bit 8, interrupt message bits 13:9); DevCap dword +4 (read and discard — needed only to stay consistent with the register map, do not store); DevCtl word +8; DevSta word +0xA; LnkCap dword +0xC (max speed bits 3:0, max width bits 9:4); LnkCtl word +0x10 (target speed bits 3:0); LnkSta word +0x12 (current speed bits 3:0, width bits 9:4, training bit 11); SlotCap dword +0x14, SlotCtl word +0x18, SlotSta word +0x1A (only when `slot_implemented`); RootCtl word +0x1C, RootSta dword +0x20 (only when `device_type == 4`).

```rust
use super::{read_dword, read_word};
use crate::ConfigSpaceSnapshot;

const ROOT_PORT: u8 = 4;

pub fn decode_pcie(snapshot: &ConfigSpaceSnapshot, offset: u16) -> Option<PcieCapability> {
    let base = u32::from(offset);
    let flags = read_word(snapshot, base + 2).ok()?;
    let device_type = ((flags >> 4) & 0x000f) as u8;
    let slot_implemented = flags & 0x0100 != 0;

    let dev_ctl = read_word(snapshot, base + 8).ok()?;
    let dev_sta = read_word(snapshot, base + 0x0a).ok()?;

    let link_cap = read_dword(snapshot, base + 0x0c).ok()?;
    let link_ctl = read_word(snapshot, base + 0x10).ok()?;
    let link_sta = read_word(snapshot, base + 0x12).ok()?;

    let (slot_ctl, slot_sta) = if slot_implemented {
        (
            Some(read_word(snapshot, base + 0x18).ok()?),
            Some(read_word(snapshot, base + 0x1a).ok()?),
        )
    } else {
        (None, None)
    };

    let (root_ctl, root_sta) = if device_type == ROOT_PORT {
        (
            Some(read_word(snapshot, base + 0x1c).ok()?),
            Some(read_dword(snapshot, base + 0x20).ok()?),
        )
    } else {
        (None, None)
    };

    Some(PcieCapability {
        version: (flags & 0x000f) as u8,
        device_type,
        slot_implemented,
        interrupt_message_number: ((flags >> 9) & 0x001f) as u8,
        dev_ctl,
        dev_sta,
        link_max_speed: (link_cap & 0x0000_000f) as u8,
        link_max_width: ((link_cap >> 4) & 0x003f) as u8,
        link_target_speed: (link_ctl & 0x000f) as u8,
        link_current_speed: (link_sta & 0x000f) as u8,
        link_current_width: ((link_sta >> 4) & 0x003f) as u8,
        link_training: link_sta & 0x0800 != 0,
        slot_ctl,
        slot_sta,
        root_ctl,
        root_sta,
    })
}
```

- [ ] **Step 2: Register the variant in `mod.rs`**

Add `pub mod pcie;`, `pub use pcie::PcieCapability;`, and the `Pcie(PcieCapability)` variant to `PciCapabilityContent`.

- [ ] **Step 3: Verify**

```bash
cargo fmt --all && cargo fmt --all -- --check
cargo check -p pci --target x86_64-unknown-linux-gnu
```

- [ ] **Step 4: Commit**

```bash
git add crates/pci/src/decoders/
git commit -m "pci: add PCI Express capability decoder"
```

---

### Task 4: Dispatch and session wiring

**Files:**
- Modify: `crates/pci/src/decoders/mod.rs`
- Modify: `crates/pci/src/session.rs`

**Interfaces:**
- Consumes: all five `decode_*` functions, `PciCapabilityState::Valid`.
- Produces: `pub(crate) fn decode_content(snapshot: &ConfigSpaceSnapshot, capability: &mut PciCapability)`; `inspect()` fills `content` for standard-chain nodes.

- [ ] **Step 1: Add dispatch in `decoders/mod.rs`**

```rust
use crate::{ConfigSpaceSnapshot, PciCapability, PciCapabilityState};

pub(crate) fn decode_content(snapshot: &ConfigSpaceSnapshot, capability: &mut PciCapability) {
    if !matches!(capability.state, PciCapabilityState::Valid) {
        return;
    }

    let offset = capability.offset;
    capability.content = match capability.id {
        0x01 => pm::decode_pm(snapshot, offset).map(PciCapabilityContent::Pm),
        0x05 => msi::decode_msi(snapshot, offset).map(PciCapabilityContent::Msi),
        0x09 => vendor::decode_vendor_specific(snapshot, offset)
            .map(PciCapabilityContent::VendorSpecific),
        0x10 => pcie::decode_pcie(snapshot, offset).map(PciCapabilityContent::Pcie),
        0x11 => msix::decode_msix(snapshot, offset).map(PciCapabilityContent::MsiX),
        _ => None,
    };
}
```

- [ ] **Step 2: Wire into `inspect()` in `session.rs`**

Replace the current capabilities block with:

```rust
let capabilities = {
    let mut reader = ConfigSpaceReader::new(raw, 0x000..0x1000);
    let header_readable = reader.read(0x000, 0x040).is_ok();
    let mut report = capability::discover(&mut reader);

    if header_readable {
        let snapshot = reader.snapshot();
        for capability in report.standard.iter_mut() {
            decoders::decode_content(snapshot, capability);
        }
    }

    Self::capabilities_from_report(report, header_readable)
};
```

Add `decoders` to the `use crate::{...}` import list in `session.rs`.

- [ ] **Step 3: Verify**

```bash
cargo fmt --all && cargo fmt --all -- --check
cargo check --workspace --target x86_64-unknown-linux-gnu
cargo run -q -p lspci-rs --target x86_64-unknown-linux-gnu -- show 0000:00:05.0 --format text
```

Expected (myece container): output identical to before — `capabilities:` shows `chain=unavailable: ReadError` for both groups, no panic, no `content` lines.

- [ ] **Step 4: Commit**

```bash
git add crates/pci/src/decoders/mod.rs crates/pci/src/session.rs
git commit -m "pci: decode standard capability content during inspection"
```

---

### Task 5: Text and JSON rendering

**Files:**
- Modify: `crates/lspci-rs/src/output.rs`

**Interfaces:**
- Consumes: `PciCapabilityContent` and the five structs, existing `render_capability_group_text` / `json_capability_list`.
- Produces: content line in text output; `content` object with `type` discriminator in JSON.

- [ ] **Step 1: Text content line**

In `render_capability_group_text`, after the existing capability line, add:

```rust
if let Some(content) = &capability.content {
    writeln!(output, "        content: {}", render_capability_content(content)).unwrap();
}
```

Add the renderer (import `pci::PciCapabilityContent` and the five structs):

```rust
fn render_capability_content(content: &PciCapabilityContent) -> String {
    match content {
        PciCapabilityContent::Pm(pm) => format!(
            "version={} pme_support=0x{:02x} power_state=D{} pme_enable={} pme_status={} no_soft_reset={}",
            pm.version, pm.pme_support, pm.power_state, pm.pme_enable, pm.pme_status, pm.no_soft_reset
        ),
        PciCapabilityContent::Msi(msi) => format!(
            "enable={} count={}/{} 64bit={} maskable={} address=0x{:x} data=0x{:x}",
            msi.enable,
            1u32 << msi.multiple_message_enable,
            1u32 << msi.multiple_message_capable,
            msi.is_64_bit,
            msi.per_vector_masking,
            msi.address,
            msi.data
        ),
        PciCapabilityContent::MsiX(msix) => format!(
            "enable={} count={} masked={} table=BAR{}+0x{:x} pba=BAR{}+0x{:x}",
            msix.enable, msix.count, msix.masked, msix.table_bar, msix.table_offset, msix.pba_bar, msix.pba_offset
        ),
        PciCapabilityContent::Pcie(pcie) => format!(
            "version={} type={} slot={} link_max={}GT/s x{} link={}GT/s x{} training={}",
            pcie.version,
            render_pcie_device_type(pcie.device_type),
            pcie.slot_implemented,
            render_pcie_speed(pcie.link_max_speed),
            pcie.link_max_width,
            render_pcie_speed(pcie.link_current_speed),
            pcie.link_current_width,
            pcie.link_training
        ),
        PciCapabilityContent::VendorSpecific(vendor) => {
            let data: Vec<String> = vendor.data.iter().map(|byte| format!("{byte:02x}")).collect();
            format!("len={} data={}", vendor.length, data.join(" "))
        }
    }
}

fn render_pcie_device_type(device_type: u8) -> &'static str {
    match device_type {
        0x0 => "endpoint",
        0x1 => "legacy-endpoint",
        0x4 => "root-port",
        0x5 => "upstream-switch-port",
        0x6 => "downstream-switch-port",
        0x7 => "pcie-to-pci-bridge",
        0x8 => "pci-to-pcie-bridge",
        0x9 => "rc-integrated-endpoint",
        0xa => "rc-event-collector",
        _ => "unknown",
    }
}

fn render_pcie_speed(speed: u8) -> &'static str {
    match speed {
        1 => "2.5",
        2 => "5.0",
        3 => "8.0",
        4 => "16.0",
        5 => "32.0",
        6 => "64.0",
        _ => "?",
    }
}
```

- [ ] **Step 2: JSON content**

Add a `JsonCapabilityContent` representation using serde's adjacently tagged form. Change `JsonCapability`:

```rust
#[derive(Debug, Serialize)]
struct JsonCapability {
    id: String,
    kind: String,
    offset: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    next: Option<String>,

    state: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    content: Option<JsonCapabilityContent>,
}

#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum JsonCapabilityContent {
    Pm(JsonPm),
    Msi(JsonMsi),
    MsiX(JsonMsiX),
    Pcie(JsonPcie),
    VendorSpecific(JsonVendorSpecific),
}
```

Define the five JSON structs mirroring the domain fields: bools as bool, counters as numbers, addresses/offsets as `0x...` strings, BAR references as `{ "bar": <number>, "offset": "0x..." }`, MSI-X `table`/`pba` as those objects, PM `power_state` as `"D0"`–`"D3hot"` strings, PCIe `device_type` via `render_pcie_device_type` and speeds as `"8.0"` GT/s strings. Add `fn json_capability_content(content: &PciCapabilityContent) -> JsonCapabilityContent` and call it from `json_capability_list`:

```rust
content: capability.content.as_ref().map(json_capability_content),
```

- [ ] **Step 3: Verify**

```bash
cargo fmt --all && cargo fmt --all -- --check
cargo check --workspace --target x86_64-unknown-linux-gnu
cargo run -q -p lspci-rs --target x86_64-unknown-linux-gnu -- show 0000:00:05.0 --format text
cargo run -q -p lspci-rs --target x86_64-unknown-linux-gnu -- show 0000:00:05.0 --format json
```

Expected (myece): unchanged capability output (no content lines — bytes unreadable here), JSON identical shape to before.

- [ ] **Step 4: Commit**

```bash
git add crates/lspci-rs/src/output.rs
git commit -m "cli: render decoded capability content in text and JSON"
```

---

### Task 6: dev48 real-device validation and finish

**Files:** none (verification only), plus progress doc update.

**Interfaces:**
- Consumes: completed branch binary and dev48 sudo access (passwordless).
- Produces: field-by-field comparison evidence against `lspci -vv`.

- [ ] **Step 1: Build and transfer the binary**

Binary transfer path (myece cannot reach dev48; route through the local machine):

```bash
# in container: build
cd /workspace && cargo build -p lspci-rs --target x86_64-unknown-linux-gnu
# container -> myece host
docker cp 95c90e05ab1a:/workspace/target/x86_64-unknown-linux-gnu/debug/lspci-rs /tmp/lspci-rs   # run on myece host
# myece -> local -> dev48
scp myece:/tmp/lspci-rs /tmp/lspci-rs          # run locally
scp /tmp/lspci-rs dev48:/tmp/lspci-rs          # run locally
ssh dev48 'chmod +x /tmp/lspci-rs && /tmp/lspci-rs list'   # smoke test: fails only if libpci missing
```

Fallback if `/tmp/lspci-rs list` fails to run (glibc/libpci mismatch): install rustup on dev48, clone or scp the source, build there.

- [ ] **Step 2: Compare MSI-X on the virtio device**

```bash
ssh dev48 'sudo /tmp/lspci-rs show 0000:00:03.0 --format text'
ssh dev48 'sudo lspci -s 00:03.0 -vv'
```

Compare the MSI-X content line against lspci: Enable, Count, Masked, Vector table BAR+offset, PBA BAR+offset. All values must match.

- [ ] **Step 3: Compare remaining capability types**

Find devices for each type and compare the same way:

```bash
ssh dev48 'sudo lspci -vv | grep -B6 "Capabilities:.*\(MSI:\|Power Management\|Express\|Vendor Specific\)" | grep -E "^[0-9a-f]+:"'
```

For each capability type present, pick one device, run both tools, and compare field by field. Record any device lacking a type as "not verifiable in this environment" — do not fabricate coverage.

- [ ] **Step 4: myece no-regression check**

```bash
cd /workspace   # inside container
cargo fmt --all -- --check
cargo check --workspace --target x86_64-unknown-linux-gnu
cargo run -q -p lspci-rs --target x86_64-unknown-linux-gnu -- list --format text | wc -l
cargo run -q -p lspci-rs --target x86_64-unknown-linux-gnu -- show 0000:00:05.0 --config standard --format text
git diff --check
```

Expected: 9 devices listed; config dump and capability statuses unchanged from before this branch.

- [ ] **Step 5: Record the handoff**

Update `docs/superpowers/progress/` with a new dated file: commits, dev48 devices used, capability types verified, types that could not be verified in this environment, and any lspci discrepancies found. Commit it:

```bash
git add docs/superpowers/progress/
git commit -m "docs: record capability decoder validation results"
```

- [ ] **Step 6: Finish the branch**

Use superpowers:finishing-a-development-branch to merge `sdd/capability-decoders` into `main` (or follow the user's chosen option).
