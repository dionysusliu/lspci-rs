# Remaining Extended Capability Decoders Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add the 11 remaining extended capability decoders present on the validation machine — VC, Vendor-Ext, ATS, PRI, TPH, LTR, Secondary PCIe, PASID, DPC, PTM, DVSEC — with lspci -vvv-parity output calibrated on real hardware.

**Architecture:** Eleven new pure-function decoders over `ConfigSpaceSnapshot` join the existing decoder module; `PciCapabilityContent` gains eleven variants and dispatch gains eleven extended-ID arms. Extended prefetch grows from 0x40 to 0x60 bytes per node. Register layouts in this plan are best-guess starting points; Task 7 calibrates them against sg-232e-224 raw bytes + lspci output.

**Tech Stack:** Rust 2024 workspace, libpci FFI (unchanged), clap, serde. Build in container `95c90e05ab1a` on host `myece` (`/workspace`); validate on physical machine `sg-232e-224` (sudo, lspci 3.8.0).

## Global Constraints

- No unit tests (user decision); per-task verification is `cargo fmt --check` + `cargo check`; final verification is sg-232e-224 comparison against `sudo lspci -vvv`.
- Decoder modules contain zero FFI; decoder failure yields `content = None` and never fails `inspect()`.
- `list` behavior unchanged; no new Rust dependencies.
- Register layouts are calibrated in Task 7 against real hardware; mismatches are fixed there, not guessed further.
- VC and Secondary PCIe may be degraded to "raw registers + key fields" if calibration cost is excessive; record any degradation in the progress doc.
- Dump-type decoders never read past their declared structure length.
- Verification commands run inside the container: `ssh myece 'docker exec 95c90e05ab1a bash -lc "cd /workspace && <cmd>"'`.
- Binary transfer chain (sftp only; scp is killed): build in container → on myece `podman cp 95c90e05ab1a:/workspace/target/x86_64-unknown-linux-gnu/debug/lspci-rs /tmp/lspci-rs` → locally `sftp myece <<< "get /tmp/lspci-rs <local>"` → `sftp sg-232e-224 <<< "put <local> /tmp/lspci-rs"` → on sg machine `sudo chmod +x /tmp/lspci-rs`.
- Branch `sdd/remaining-extended-decoders` from `main`; finish via finishing-a-development-branch.
- Between Tasks 2–5 the workspace check FAILS on output.rs (non-exhaustive match) — expected, fixed in Task 6. Verify only `-p pci` in those tasks.

---

### Task 0: Create the feature branch

- [ ] **Step 1: Create and switch branch**

```bash
cd /workspace && git checkout main && git checkout -b sdd/remaining-extended-decoders
```

---

### Task 1: Bump extended prefetch to 0x60

**Files:**
- Modify: `crates/pci/src/session.rs`

**Interfaces:**
- Consumes: existing extended prefetch loop in `inspect()`.
- Produces: 0x60-byte prefetch for Valid extended nodes.

- [ ] **Step 1: Change the prefetch bound**

In `inspect()`, find the extended prefetch loop:

```rust
                    for capability in report.extended.iter() {
                        if matches!(capability.state, PciCapabilityState::Valid) {
                            let start = u32::from(capability.offset);
                            let end = (start + 0x40).min(0x1000);
                            let _ = reader.fetch(start, end - start);
                        }
                    }
```

Replace `(start + 0x40)` with `(start + 0x60)`:

```rust
                            let end = (start + 0x60).min(0x1000);
```

- [ ] **Step 2: Verify and commit**

```bash
cargo fmt --all && cargo fmt --all -- --check
cargo check --workspace --target x86_64-unknown-linux-gnu
cargo run -q -p lspci-rs --target x86_64-unknown-linux-gnu -- show 0000:00:05.0 --format text
```

Expected (myece): unchanged output (`extended: chain=unavailable: ReadError`).

```bash
git add crates/pci/src/session.rs
git commit -m "pci: extend extended-capability prefetch to 96 bytes"
```

---

### Task 2: LTR, ATS, PRI, PASID, PTM decoders

**Files:**
- Create: `crates/pci/src/decoders/ltr.rs`
- Create: `crates/pci/src/decoders/ats.rs`
- Create: `crates/pci/src/decoders/pri.rs`
- Create: `crates/pci/src/decoders/pasid.rs`
- Create: `crates/pci/src/decoders/ptm.rs`
- Modify: `crates/pci/src/decoders/mod.rs`
- Modify: `crates/pci/src/lib.rs`

**Interfaces:**
- Consumes: `super::read_word`, `super::read_dword`, extended dispatch arms.
- Produces: `decode_ltr`, `decode_ats`, `decode_pri`, `decode_pasid`, `decode_ptm` (`fn(snapshot: &ConfigSpaceSnapshot, offset: u16) -> Option<T>`); structs `LtrCapability`, `AtsCapability`, `PriCapability`, `PasidCapability`, `PtmCapability`; variants `Ltr`, `Ats`, `Pri`, `Pasid`, `Ptm`.

- [ ] **Step 1: Create `ltr.rs`**

Single dword at cap+4: Max Snoop Latency (value bits 0–9, scale bits 10–12), Max No-Snoop Latency (value bits 16–25, scale bits 26–28).

```rust
use super::read_dword;
use crate::ConfigSpaceSnapshot;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LtrCapability {
    pub snoop_value: u16,
    pub snoop_scale: u8,
    pub no_snoop_value: u16,
    pub no_snoop_scale: u8,
}

pub fn decode_ltr(snapshot: &ConfigSpaceSnapshot, offset: u16) -> Option<LtrCapability> {
    let base = u32::from(offset);
    let latency = read_dword(snapshot, base + 4).ok()?;

    Some(LtrCapability {
        snoop_value: (latency & 0x0000_03ff) as u16,
        snoop_scale: ((latency >> 10) & 0x0000_0007) as u8,
        no_snoop_value: ((latency >> 16) & 0x0000_03ff) as u16,
        no_snoop_scale: ((latency >> 26) & 0x0000_0007) as u8,
    })
}
```

- [ ] **Step 2: Create `ats.rs`**

Capability word at cap+4 (bits 0–4 Invalidate Queue Depth), control word at cap+6 (bit 15 Enable, bit 12 Page Aligned Request, bits 0–4 Smallest Translation Unit).

```rust
use super::read_word;
use crate::ConfigSpaceSnapshot;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AtsCapability {
    pub invalidate_queue_depth: u8,
    pub enable: bool,
    pub page_aligned: bool,
    pub smallest_translation_unit: u8,
}

pub fn decode_ats(snapshot: &ConfigSpaceSnapshot, offset: u16) -> Option<AtsCapability> {
    let base = u32::from(offset);
    let capability = read_word(snapshot, base + 4).ok()?;
    let control = read_word(snapshot, base + 6).ok()?;

    Some(AtsCapability {
        invalidate_queue_depth: (capability & 0x001f) as u8,
        enable: control & 0x8000 != 0,
        page_aligned: control & 0x1000 != 0,
        smallest_translation_unit: (control & 0x001f) as u8,
    })
}
```

- [ ] **Step 3: Create `pri.rs`**

Control word at cap+4 (bit 0 Enable, bit 1 Reset), status word at cap+6 (bit 0 Response Failure, bit 1 Unexpected Page Request Group Index, bit 2 Stopped), dwords at cap+8 (Outstanding Page Request Capacity) and cap+12 (Outstanding Page Request Allocation).

```rust
use super::{read_dword, read_word};
use crate::ConfigSpaceSnapshot;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PriCapability {
    pub enable: bool,
    pub reset: bool,
    pub response_failure: bool,
    pub unexpected_group_index: bool,
    pub stopped: bool,
    pub outstanding_capacity: u32,
    pub outstanding_allocation: u32,
}

pub fn decode_pri(snapshot: &ConfigSpaceSnapshot, offset: u16) -> Option<PriCapability> {
    let base = u32::from(offset);
    let control = read_word(snapshot, base + 4).ok()?;
    let status = read_word(snapshot, base + 6).ok()?;
    let outstanding_capacity = read_dword(snapshot, base + 8).ok()?;
    let outstanding_allocation = read_dword(snapshot, base + 12).ok()?;

    Some(PriCapability {
        enable: control & 0x0001 != 0,
        reset: control & 0x0002 != 0,
        response_failure: status & 0x0001 != 0,
        unexpected_group_index: status & 0x0002 != 0,
        stopped: status & 0x0004 != 0,
        outstanding_capacity,
        outstanding_allocation,
    })
}
```

- [ ] **Step 4: Create `pasid.rs`**

Capability word at cap+4 (bit 1 Execute Permission Supported, bit 2 Privileged Mode Supported, bits 8–12 Max PASID Width), control word at cap+6 (bit 0 Enable, bit 1 Execute Enable, bit 2 Privileged Enable).

```rust
use super::read_word;
use crate::ConfigSpaceSnapshot;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PasidCapability {
    pub execute_supported: bool,
    pub privileged_supported: bool,
    pub max_pasid_width: u8,
    pub enable: bool,
    pub execute_enable: bool,
    pub privileged_enable: bool,
}

pub fn decode_pasid(snapshot: &ConfigSpaceSnapshot, offset: u16) -> Option<PasidCapability> {
    let base = u32::from(offset);
    let capability = read_word(snapshot, base + 4).ok()?;
    let control = read_word(snapshot, base + 6).ok()?;

    Some(PasidCapability {
        execute_supported: capability & 0x0002 != 0,
        privileged_supported: capability & 0x0004 != 0,
        max_pasid_width: ((capability >> 8) & 0x001f) as u8,
        enable: control & 0x0001 != 0,
        execute_enable: control & 0x0002 != 0,
        privileged_enable: control & 0x0004 != 0,
    })
}
```

- [ ] **Step 5: Create `ptm.rs`**

Capability dword at cap+4 (bit 0 PTM Root Capable, bit 1 PTM Clock Capable), control dword at cap+8 (bit 0 PTM Enable, bit 1 Root Select, bits 31–24 PTM Granularity).

```rust
use super::read_dword;
use crate::ConfigSpaceSnapshot;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PtmCapability {
    pub root_capable: bool,
    pub clock_capable: bool,
    pub enable: bool,
    pub root_select: bool,
    pub granularity: u8,
}

pub fn decode_ptm(snapshot: &ConfigSpaceSnapshot, offset: u16) -> Option<PtmCapability> {
    let base = u32::from(offset);
    let capability = read_dword(snapshot, base + 4).ok()?;
    let control = read_dword(snapshot, base + 8).ok()?;

    Some(PtmCapability {
        root_capable: capability & 0x0000_0001 != 0,
        clock_capable: capability & 0x0000_0002 != 0,
        enable: control & 0x0000_0001 != 0,
        root_select: control & 0x0000_0002 != 0,
        granularity: ((control >> 24) & 0x0000_00ff) as u8,
    })
}
```

- [ ] **Step 6: Register modules, variants, dispatch, exports**

Add module declarations in `decoders/mod.rs` keeping alphabetical order (insert `ats`, `ltr`, `pasid`, `pri`, `ptm`), re-exports (`pub use ats::AtsCapability;` etc.), five variants after `Aer(AerCapability)`:

```rust
    Ltr(LtrCapability),
    Ats(AtsCapability),
    Pri(PriCapability),
    Pasid(PasidCapability),
    Ptm(PtmCapability),
```

and dispatch arms (before `_ => None`):

```rust
        (PciCapabilityKind::Extended, 0x0f) => {
            ats::decode_ats(snapshot, offset).map(PciCapabilityContent::Ats)
        }
        (PciCapabilityKind::Extended, 0x13) => {
            pri::decode_pri(snapshot, offset).map(PciCapabilityContent::Pri)
        }
        (PciCapabilityKind::Extended, 0x18) => {
            ltr::decode_ltr(snapshot, offset).map(PciCapabilityContent::Ltr)
        }
        (PciCapabilityKind::Extended, 0x1b) => {
            pasid::decode_pasid(snapshot, offset).map(PciCapabilityContent::Pasid)
        }
        (PciCapabilityKind::Extended, 0x1f) => {
            ptm::decode_ptm(snapshot, offset).map(PciCapabilityContent::Ptm)
        }
```

Add `AtsCapability, LtrCapability, PasidCapability, PriCapability, PtmCapability` to the decoders re-export in `crates/pci/src/lib.rs` (cargo fmt re-sorts).

- [ ] **Step 7: Verify and commit**

```bash
cargo fmt --all && cargo fmt --all -- --check
cargo check -p pci --target x86_64-unknown-linux-gnu
git add crates/pci/src/decoders/ crates/pci/src/lib.rs
git commit -m "pci: add LTR, ATS, PRI, PASID and PTM decoders"
```

---

### Task 3: DPC and TPH decoders

**Files:**
- Create: `crates/pci/src/decoders/dpc.rs`
- Create: `crates/pci/src/decoders/tph.rs`
- Modify: `crates/pci/src/decoders/mod.rs`
- Modify: `crates/pci/src/lib.rs`

**Interfaces:**
- Consumes: `super::read_word`, `super::read_dword`, extended dispatch.
- Produces: `decode_dpc`, `decode_tph`; structs `DpcCapability`, `TphCapability`; variants `Dpc`, `Tph`.

- [ ] **Step 1: Create `dpc.rs`**

Capability word at cap+4 (bits 0–2 DPC Interrupt Message Number, bit 4 RP PIO Extensions, bits 8–12 RP PIO Log Size), control word at cap+6 (bits 0–1 Trigger Enable, bit 2 Completion Control, bit 3 Interrupt Enable, bit 4 ERR_COR, bit 6 Software Trigger), status word at cap+8 (bit 0 Trigger Status, bits 1–2 Trigger Reason, bit 3 Interrupt Status, bit 4 Reason Extension), error source word at cap+10. When RP PIO Extensions is set, read RP PIO First Error Pointer dword at cap+12 and RP PIO Status dword at cap+16.

```rust
use super::{read_dword, read_word};
use crate::ConfigSpaceSnapshot;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DpcCapability {
    pub interrupt_message_number: u8,
    pub rp_pio_extensions: bool,
    pub rp_pio_log_size: u8,
    pub trigger_enable: u8,
    pub completion_control: bool,
    pub interrupt_enable: bool,
    pub err_cor_enable: bool,
    pub software_trigger: bool,
    pub trigger_status: bool,
    pub trigger_reason: u8,
    pub interrupt_status: bool,
    pub reason_extension: bool,
    pub error_source_id: u16,
    pub rp_pio_first_error_pointer: Option<u8>,
    pub rp_pio_status: Option<u32>,
}

pub fn decode_dpc(snapshot: &ConfigSpaceSnapshot, offset: u16) -> Option<DpcCapability> {
    let base = u32::from(offset);
    let capability = read_word(snapshot, base + 4).ok()?;
    let control = read_word(snapshot, base + 6).ok()?;
    let status = read_word(snapshot, base + 8).ok()?;
    let error_source_id = read_word(snapshot, base + 10).ok()?;

    let rp_pio_extensions = capability & 0x0010 != 0;
    let (rp_pio_first_error_pointer, rp_pio_status) = if rp_pio_extensions {
        let first = read_dword(snapshot, base + 12).ok()?;
        let status = read_dword(snapshot, base + 16).ok()?;
        (Some((first & 0x0000_003f) as u8), Some(status))
    } else {
        (None, None)
    };

    Some(DpcCapability {
        interrupt_message_number: (capability & 0x0007) as u8,
        rp_pio_extensions,
        rp_pio_log_size: ((capability >> 8) & 0x001f) as u8,
        trigger_enable: (control & 0x0003) as u8,
        completion_control: control & 0x0004 != 0,
        interrupt_enable: control & 0x0008 != 0,
        err_cor_enable: control & 0x0010 != 0,
        software_trigger: control & 0x0040 != 0,
        trigger_status: status & 0x0001 != 0,
        trigger_reason: ((status >> 1) & 0x0003) as u8,
        interrupt_status: status & 0x0008 != 0,
        reason_extension: status & 0x0010 != 0,
        error_source_id,
        rp_pio_first_error_pointer,
        rp_pio_status,
    })
}
```

- [ ] **Step 2: Create `tph.rs`**

Capability dword at cap+4 (bit 0 No ST Mode, bit 1 Device Specific Mode, bit 2 Interrupt Vector Mode, bits 8–9 ST Table Location, bits 16–26 ST Table Size), control dword at cap+8 (bits 0–2 ST Mode Select). When ST Table Location == 2 (in TPH cap), read `st_table_size` two-byte entries starting at cap+12.

```rust
use super::read_dword;
use crate::ConfigSpaceSnapshot;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TphCapability {
    pub no_st_mode: bool,
    pub device_specific_mode: bool,
    pub interrupt_vector_mode: bool,
    pub st_table_location: u8,
    pub st_table_size: u16,
    pub st_mode_select: u8,
    pub st_table: Vec<u16>,
}

pub fn decode_tph(snapshot: &ConfigSpaceSnapshot, offset: u16) -> Option<TphCapability> {
    let base = u32::from(offset);
    let capability = read_dword(snapshot, base + 4).ok()?;
    let control = read_dword(snapshot, base + 8).ok()?;

    let st_table_location = ((capability >> 8) & 0x0000_0003) as u8;
    let st_table_size = ((capability >> 16) & 0x0000_07ff) as u16;

    let mut st_table = Vec::new();
    if st_table_location == 2 {
        for index in 0..st_table_size {
            let entry_offset = base + 12 + u32::from(index) * 2;
            let bytes = snapshot.read(entry_offset, 2).ok()?;
            st_table.push(u16::from_le_bytes([bytes[0], bytes[1]]));
        }
    }

    Some(TphCapability {
        no_st_mode: capability & 0x0000_0001 != 0,
        device_specific_mode: capability & 0x0000_0002 != 0,
        interrupt_vector_mode: capability & 0x0000_0004 != 0,
        st_table_location,
        st_table_size,
        st_mode_select: (control & 0x0000_0007) as u8,
        st_table,
    })
}
```

- [ ] **Step 3: Register modules, variants, dispatch, exports**

Module declarations keeping alphabetical order: `pub mod dpc;` immediately before `pub mod dsn;`, and `pub mod tph;` between `pub mod sriov;` and `pub mod vendor;`. Re-exports, variants:

```rust
    Dpc(DpcCapability),
    Tph(TphCapability),
```

dispatch arms:

```rust
        (PciCapabilityKind::Extended, 0x17) => {
            tph::decode_tph(snapshot, offset).map(PciCapabilityContent::Tph)
        }
        (PciCapabilityKind::Extended, 0x1d) => {
            dpc::decode_dpc(snapshot, offset).map(PciCapabilityContent::Dpc)
        }
```

add `DpcCapability, TphCapability` to the `lib.rs` re-export.

- [ ] **Step 4: Verify and commit**

```bash
cargo fmt --all && cargo fmt --all -- --check
cargo check -p pci --target x86_64-unknown-linux-gnu
git add crates/pci/src/decoders/ crates/pci/src/lib.rs
git commit -m "pci: add DPC and TPH decoders"
```

---

### Task 4: Vendor-Ext and DVSEC dump decoders

**Files:**
- Create: `crates/pci/src/decoders/vendor_ext.rs`
- Create: `crates/pci/src/decoders/dvsec.rs`
- Modify: `crates/pci/src/decoders/mod.rs`
- Modify: `crates/pci/src/lib.rs`

**Interfaces:**
- Consumes: `super::read_dword`, `ConfigSpaceSnapshot::read`.
- Produces: `decode_vendor_ext`, `decode_dvsec`; structs `VendorExtCapability`, `DvsecCapability`; variants `VendorExt`, `Dvsec`.

- [ ] **Step 1: Create `vendor_ext.rs`**

Dword at cap+4: Vendor ID bits 0–15, Revision bits 16–19, Length bits 20–31 (total structure length in bytes, including the 8-byte header). Dump `length - 8` payload bytes starting at cap+8 (skip if length < 8).

```rust
use super::read_dword;
use crate::ConfigSpaceSnapshot;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VendorExtCapability {
    pub vendor_id: u16,
    pub revision: u8,
    pub length: u16,
    pub data: Vec<u8>,
}

pub fn decode_vendor_ext(snapshot: &ConfigSpaceSnapshot, offset: u16) -> Option<VendorExtCapability> {
    let base = u32::from(offset);
    let header = read_dword(snapshot, base + 4).ok()?;

    let vendor_id = (header & 0x0000_ffff) as u16;
    let revision = ((header >> 16) & 0x0000_000f) as u8;
    let length = ((header >> 20) & 0x0000_0fff) as u16;

    let payload = length.saturating_sub(8);
    let data = if payload == 0 {
        Vec::new()
    } else {
        snapshot.read(base + 8, u32::from(payload)).ok()?
    };

    Some(VendorExtCapability {
        vendor_id,
        revision,
        length,
        data,
    })
}
```

- [ ] **Step 2: Create `dvsec.rs`**

Dword 1 at cap+4: Vendor ID bits 0–15, Revision bits 16–19, Length bits 20–31. Dword 2 at cap+8: DVSEC ID bits 0–15. Dump `length - 8` payload bytes starting at cap+8 (payload includes the DVSEC ID dword; dump it as part of the raw bytes).

```rust
use super::read_dword;
use crate::ConfigSpaceSnapshot;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DvsecCapability {
    pub vendor_id: u16,
    pub revision: u8,
    pub dvsec_id: u16,
    pub length: u16,
    pub data: Vec<u8>,
}

pub fn decode_dvsec(snapshot: &ConfigSpaceSnapshot, offset: u16) -> Option<DvsecCapability> {
    let base = u32::from(offset);
    let header = read_dword(snapshot, base + 4).ok()?;
    let dvsec_id = read_dword(snapshot, base + 8).ok()? as u16;

    let vendor_id = (header & 0x0000_ffff) as u16;
    let revision = ((header >> 16) & 0x0000_000f) as u8;
    let length = ((header >> 20) & 0x0000_0fff) as u16;

    let payload = length.saturating_sub(8);
    let data = if payload == 0 {
        Vec::new()
    } else {
        snapshot.read(base + 8, u32::from(payload)).ok()?
    };

    Some(DvsecCapability {
        vendor_id,
        revision,
        dvsec_id,
        length,
        data,
    })
}
```

- [ ] **Step 3: Register modules, variants, dispatch, exports**

Module declarations (`dvsec` after `dsn`; `vendor_ext` after `vendor`), re-exports, variants:

```rust
    VendorExt(VendorExtCapability),
    Dvsec(DvsecCapability),
```

dispatch arms:

```rust
        (PciCapabilityKind::Extended, 0x0b) => vendor_ext::decode_vendor_ext(snapshot, offset)
            .map(PciCapabilityContent::VendorExt),
        (PciCapabilityKind::Extended, 0x23) => {
            dvsec::decode_dvsec(snapshot, offset).map(PciCapabilityContent::Dvsec)
        }
```

add `DvsecCapability, VendorExtCapability` to the `lib.rs` re-export.

- [ ] **Step 4: Verify and commit**

```bash
cargo fmt --all && cargo fmt --all -- --check
cargo check -p pci --target x86_64-unknown-linux-gnu
git add crates/pci/src/decoders/ crates/pci/src/lib.rs
git commit -m "pci: add vendor-extended and DVSEC dump decoders"
```

---

### Task 5: VC and Secondary PCIe decoders

**Files:**
- Create: `crates/pci/src/decoders/vc.rs`
- Create: `crates/pci/src/decoders/secondary_pcie.rs`
- Modify: `crates/pci/src/decoders/mod.rs`
- Modify: `crates/pci/src/lib.rs`

**Interfaces:**
- Consumes: `super::read_word`, `super::read_dword`, extended dispatch.
- Produces: `decode_vc`, `decode_secondary_pcie`; structs `VcCapability`, `SecondaryPcieCapability`; variants `Vc`, `SecondaryPcie`.

- [ ] **Step 1: Create `vc.rs`**

Layout (calibrate in Task 7): extended VC capability dword at cap+4 (Extended VC Count bits 4–6, Port VC Capability bits 8–9), port VC capability 1 dword at cap+8 (Reference Clock bits 0–7, Port Arbitration Table Entry Count bits 8–15), port VC capability 2 dword at cap+12 (VC Arbitration Table Offset bits 0–3, VC Arbitration Table Entry Count bits 8–15), port VC control word at cap+16 (VC Arbitration Select bits 4–6, bit 15 table status), port VC status word at cap+18. Per-VC resources start at cap+20, 12 bytes each (resource capability dword, control dword, status dword), count = extended_vc_count + 1.

```rust
use super::{read_dword, read_word};
use crate::ConfigSpaceSnapshot;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VcResource {
    pub capability: u32,
    pub control: u32,
    pub status: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VcCapability {
    pub extended_vc_count: u8,
    pub port_vc_capability: u8,
    pub reference_clock: u8,
    pub port_arbitration_table_entry_count: u8,
    pub vc_arbitration_table_offset: u8,
    pub vc_arbitration_table_entry_count: u8,
    pub port_control: u16,
    pub port_status: u16,
    pub resources: Vec<VcResource>,
}

pub fn decode_vc(snapshot: &ConfigSpaceSnapshot, offset: u16) -> Option<VcCapability> {
    let base = u32::from(offset);

    let extended_cap = read_dword(snapshot, base + 4).ok()?;
    let port_cap_1 = read_dword(snapshot, base + 8).ok()?;
    let port_cap_2 = read_dword(snapshot, base + 12).ok()?;
    let port_control = read_word(snapshot, base + 16).ok()?;
    let port_status = read_word(snapshot, base + 18).ok()?;

    let extended_vc_count = ((extended_cap >> 4) & 0x0000_0007) as u8;

    let mut resources = Vec::new();
    for index in 0..=u32::from(extended_vc_count) {
        let entry = base + 20 + index * 12;
        let capability = read_dword(snapshot, entry).ok()?;
        let control = read_dword(snapshot, entry + 4).ok()?;
        let status = read_dword(snapshot, entry + 8).ok()?;
        resources.push(VcResource {
            capability,
            control,
            status,
        });
    }

    Some(VcCapability {
        extended_vc_count,
        port_vc_capability: ((extended_cap >> 8) & 0x0000_0003) as u8,
        reference_clock: (port_cap_1 & 0x0000_00ff) as u8,
        port_arbitration_table_entry_count: ((port_cap_1 >> 8) & 0x0000_00ff) as u8,
        vc_arbitration_table_offset: (port_cap_2 & 0x0000_000f) as u8,
        vc_arbitration_table_entry_count: ((port_cap_2 >> 8) & 0x0000_00ff) as u8,
        port_control,
        port_status,
        resources,
    })
}
```

- [ ] **Step 2: Create `secondary_pcie.rs`**

Layout (calibrate in Task 7): Link Control 3 dword at cap+4 (bit 0 Perform Equalization, bit 1 Link Equalization Request Interrupt Enable), Lane Equalization Control dword at cap+8 (downstream/upstream port presets for the first lane pair).

```rust
use super::read_dword;
use crate::ConfigSpaceSnapshot;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SecondaryPcieCapability {
    pub perform_equalization: bool,
    pub equalization_request_interrupt: bool,
    pub lane_equalization_control: u32,
}

pub fn decode_secondary_pcie(
    snapshot: &ConfigSpaceSnapshot,
    offset: u16,
) -> Option<SecondaryPcieCapability> {
    let base = u32::from(offset);
    let link_control_3 = read_dword(snapshot, base + 4).ok()?;
    let lane_equalization_control = read_dword(snapshot, base + 8).ok()?;

    Some(SecondaryPcieCapability {
        perform_equalization: link_control_3 & 0x0000_0001 != 0,
        equalization_request_interrupt: link_control_3 & 0x0000_0002 != 0,
        lane_equalization_control,
    })
}
```

- [ ] **Step 3: Register modules, variants, dispatch, exports**

Module declarations (`secondary_pcie` after `sriov`… alphabetically `secondary_pcie` before `slot_id`; `vc` after `vendor_ext`), re-exports, variants:

```rust
    Vc(VcCapability),
    SecondaryPcie(SecondaryPcieCapability),
```

dispatch arms:

```rust
        (PciCapabilityKind::Extended, 0x02) => {
            vc::decode_vc(snapshot, offset).map(PciCapabilityContent::Vc)
        }
        (PciCapabilityKind::Extended, 0x19) => secondary_pcie::decode_secondary_pcie(snapshot, offset)
            .map(PciCapabilityContent::SecondaryPcie),
```

add `SecondaryPcieCapability, VcCapability` to the `lib.rs` re-export.

- [ ] **Step 4: Verify and commit**

```bash
cargo fmt --all && cargo fmt --all -- --check
cargo check -p pci --target x86_64-unknown-linux-gnu
git add crates/pci/src/decoders/ crates/pci/src/lib.rs
git commit -m "pci: add VC and Secondary PCIe decoders"
```

---

### Task 6: Text and JSON rendering for all 11 variants

**Files:**
- Modify: `crates/lspci-rs/src/output.rs`

**Interfaces:**
- Consumes: all eleven new structs/variants (Tasks 2–5).
- Produces: text content arms and JSON `content` objects with `type` values `ltr`, `ats`, `pri`, `pasid`, `ptm`, `dpc`, `tph`, `vendor_ext`, `dvsec`, `vc`, `secondary_pcie`.

- [ ] **Step 1: Add text arms to `render_capability_content`**

Append these arms (before the closing `}`):

```rust
        PciCapabilityContent::Ltr(ltr) => format!(
            "snoop={}:{} no_snoop={}:{}",
            ltr.snoop_value, ltr.snoop_scale, ltr.no_snoop_value, ltr.no_snoop_scale
        ),
        PciCapabilityContent::Ats(ats) => format!(
            "queue_depth={} enable={} page_aligned={} stu={}",
            ats.invalidate_queue_depth, ats.enable, ats.page_aligned, ats.smallest_translation_unit
        ),
        PciCapabilityContent::Pri(pri) => format!(
            "enable={} stopped={} capacity={} allocation={}",
            pri.enable, pri.stopped, pri.outstanding_capacity, pri.outstanding_allocation
        ),
        PciCapabilityContent::Pasid(pasid) => format!(
            "width={} exec_supported={} priv_supported={} enable={}",
            pasid.max_pasid_width,
            pasid.execute_supported,
            pasid.privileged_supported,
            pasid.enable
        ),
        PciCapabilityContent::Ptm(ptm) => format!(
            "root_capable={} clock_capable={} enable={} root_select={} granularity={}",
            ptm.root_capable, ptm.clock_capable, ptm.enable, ptm.root_select, ptm.granularity
        ),
        PciCapabilityContent::Dpc(dpc) => format!(
            "trigger_enable={} trigger_status={} reason={} interrupt_enable={} source=0x{:04x}",
            dpc.trigger_enable,
            dpc.trigger_status,
            dpc.trigger_reason,
            dpc.interrupt_enable,
            dpc.error_source_id
        ),
        PciCapabilityContent::Tph(tph) => format!(
            "location={} size={} mode_select={} no_st={} device_specific={}",
            tph.st_table_location,
            tph.st_table_size,
            tph.st_mode_select,
            tph.no_st_mode,
            tph.device_specific_mode
        ),
        PciCapabilityContent::VendorExt(vendor_ext) => format!(
            "vendor=0x{:04x} rev={} len={} data={}",
            vendor_ext.vendor_id,
            vendor_ext.revision,
            vendor_ext.length,
            vendor_ext
                .data
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<Vec<_>>()
                .join("")
        ),
        PciCapabilityContent::Dvsec(dvsec) => format!(
            "vendor=0x{:04x} rev={} id=0x{:04x} len={} data={}",
            dvsec.vendor_id,
            dvsec.revision,
            dvsec.dvsec_id,
            dvsec.length,
            dvsec
                .data
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<Vec<_>>()
                .join("")
        ),
        PciCapabilityContent::Vc(vc) => format!(
            "vc_count={} ref_clock={} port_control=0x{:04x} port_status=0x{:04x} resources={}",
            vc.extended_vc_count, vc.reference_clock, vc.port_control, vc.port_status,
            vc.resources.len()
        ),
        PciCapabilityContent::SecondaryPcie(secondary) => format!(
            "perform_eq={} eq_interrupt={} lane_eq=0x{:08x}",
            secondary.perform_equalization,
            secondary.equalization_request_interrupt,
            secondary.lane_equalization_control
        ),
```

- [ ] **Step 2: Add JSON enum variants and structs**

Add to `JsonCapabilityContent`:

```rust
    #[serde(rename = "ltr")]
    Ltr(JsonLtr),
    #[serde(rename = "ats")]
    Ats(JsonAts),
    #[serde(rename = "pri")]
    Pri(JsonPri),
    #[serde(rename = "pasid")]
    Pasid(JsonPasid),
    #[serde(rename = "ptm")]
    Ptm(JsonPtm),
    #[serde(rename = "dpc")]
    Dpc(JsonDpc),
    #[serde(rename = "tph")]
    Tph(JsonTph),
    #[serde(rename = "vendor_ext")]
    VendorExt(JsonVendorExt),
    #[serde(rename = "dvsec")]
    Dvsec(JsonDvsec),
    #[serde(rename = "vc")]
    Vc(JsonVc),
    #[serde(rename = "secondary_pcie")]
    SecondaryPcie(JsonSecondaryPcie),
```

Add these structs next to the existing JSON structs:

```rust
#[derive(Debug, Serialize)]
struct JsonLtr {
    snoop_value: u16,
    snoop_scale: u8,
    no_snoop_value: u16,
    no_snoop_scale: u8,
}

#[derive(Debug, Serialize)]
struct JsonAts {
    invalidate_queue_depth: u8,
    enable: bool,
    page_aligned: bool,
    smallest_translation_unit: u8,
}

#[derive(Debug, Serialize)]
struct JsonPri {
    enable: bool,
    reset: bool,
    response_failure: bool,
    unexpected_group_index: bool,
    stopped: bool,
    outstanding_capacity: u32,
    outstanding_allocation: u32,
}

#[derive(Debug, Serialize)]
struct JsonPasid {
    execute_supported: bool,
    privileged_supported: bool,
    max_pasid_width: u8,
    enable: bool,
    execute_enable: bool,
    privileged_enable: bool,
}

#[derive(Debug, Serialize)]
struct JsonPtm {
    root_capable: bool,
    clock_capable: bool,
    enable: bool,
    root_select: bool,
    granularity: u8,
}

#[derive(Debug, Serialize)]
struct JsonDpc {
    interrupt_message_number: u8,
    rp_pio_extensions: bool,
    rp_pio_log_size: u8,
    trigger_enable: u8,
    completion_control: bool,
    interrupt_enable: bool,
    err_cor_enable: bool,
    software_trigger: bool,
    trigger_status: bool,
    trigger_reason: u8,
    interrupt_status: bool,
    reason_extension: bool,
    error_source_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    rp_pio_first_error_pointer: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    rp_pio_status: Option<String>,
}

#[derive(Debug, Serialize)]
struct JsonTph {
    no_st_mode: bool,
    device_specific_mode: bool,
    interrupt_vector_mode: bool,
    st_table_location: u8,
    st_table_size: u16,
    st_mode_select: u8,
    st_table: Vec<String>,
}

#[derive(Debug, Serialize)]
struct JsonVendorExt {
    vendor_id: String,
    revision: u8,
    length: u16,
    data: String,
}

#[derive(Debug, Serialize)]
struct JsonDvsec {
    vendor_id: String,
    revision: u8,
    dvsec_id: String,
    length: u16,
    data: String,
}

#[derive(Debug, Serialize)]
struct JsonVcResource {
    capability: String,
    control: String,
    status: String,
}

#[derive(Debug, Serialize)]
struct JsonVc {
    extended_vc_count: u8,
    port_vc_capability: u8,
    reference_clock: u8,
    port_arbitration_table_entry_count: u8,
    vc_arbitration_table_offset: u8,
    vc_arbitration_table_entry_count: u8,
    port_control: String,
    port_status: String,
    resources: Vec<JsonVcResource>,
}

#[derive(Debug, Serialize)]
struct JsonSecondaryPcie {
    perform_equalization: bool,
    equalization_request_interrupt: bool,
    lane_equalization_control: String,
}
```

- [ ] **Step 3: Add JSON mapping arms to `json_capability_content`**

Append these arms (before the closing `}`):

```rust
        PciCapabilityContent::Ltr(ltr) => JsonCapabilityContent::Ltr(JsonLtr {
            snoop_value: ltr.snoop_value,
            snoop_scale: ltr.snoop_scale,
            no_snoop_value: ltr.no_snoop_value,
            no_snoop_scale: ltr.no_snoop_scale,
        }),
        PciCapabilityContent::Ats(ats) => JsonCapabilityContent::Ats(JsonAts {
            invalidate_queue_depth: ats.invalidate_queue_depth,
            enable: ats.enable,
            page_aligned: ats.page_aligned,
            smallest_translation_unit: ats.smallest_translation_unit,
        }),
        PciCapabilityContent::Pri(pri) => JsonCapabilityContent::Pri(JsonPri {
            enable: pri.enable,
            reset: pri.reset,
            response_failure: pri.response_failure,
            unexpected_group_index: pri.unexpected_group_index,
            stopped: pri.stopped,
            outstanding_capacity: pri.outstanding_capacity,
            outstanding_allocation: pri.outstanding_allocation,
        }),
        PciCapabilityContent::Pasid(pasid) => JsonCapabilityContent::Pasid(JsonPasid {
            execute_supported: pasid.execute_supported,
            privileged_supported: pasid.privileged_supported,
            max_pasid_width: pasid.max_pasid_width,
            enable: pasid.enable,
            execute_enable: pasid.execute_enable,
            privileged_enable: pasid.privileged_enable,
        }),
        PciCapabilityContent::Ptm(ptm) => JsonCapabilityContent::Ptm(JsonPtm {
            root_capable: ptm.root_capable,
            clock_capable: ptm.clock_capable,
            enable: ptm.enable,
            root_select: ptm.root_select,
            granularity: ptm.granularity,
        }),
        PciCapabilityContent::Dpc(dpc) => JsonCapabilityContent::Dpc(JsonDpc {
            interrupt_message_number: dpc.interrupt_message_number,
            rp_pio_extensions: dpc.rp_pio_extensions,
            rp_pio_log_size: dpc.rp_pio_log_size,
            trigger_enable: dpc.trigger_enable,
            completion_control: dpc.completion_control,
            interrupt_enable: dpc.interrupt_enable,
            err_cor_enable: dpc.err_cor_enable,
            software_trigger: dpc.software_trigger,
            trigger_status: dpc.trigger_status,
            trigger_reason: dpc.trigger_reason,
            interrupt_status: dpc.interrupt_status,
            reason_extension: dpc.reason_extension,
            error_source_id: format!("0x{:04x}", dpc.error_source_id),
            rp_pio_first_error_pointer: dpc
                .rp_pio_first_error_pointer
                .map(|value| format!("0x{value:02x}")),
            rp_pio_status: dpc.rp_pio_status.map(|value| format!("0x{value:08x}")),
        }),
        PciCapabilityContent::Tph(tph) => JsonCapabilityContent::Tph(JsonTph {
            no_st_mode: tph.no_st_mode,
            device_specific_mode: tph.device_specific_mode,
            interrupt_vector_mode: tph.interrupt_vector_mode,
            st_table_location: tph.st_table_location,
            st_table_size: tph.st_table_size,
            st_mode_select: tph.st_mode_select,
            st_table: tph
                .st_table
                .iter()
                .map(|entry| format!("0x{entry:04x}"))
                .collect(),
        }),
        PciCapabilityContent::VendorExt(vendor_ext) => {
            JsonCapabilityContent::VendorExt(JsonVendorExt {
                vendor_id: format!("0x{:04x}", vendor_ext.vendor_id),
                revision: vendor_ext.revision,
                length: vendor_ext.length,
                data: vendor_ext
                    .data
                    .iter()
                    .map(|byte| format!("{byte:02x}"))
                    .collect::<Vec<_>>()
                    .join(""),
            })
        }
        PciCapabilityContent::Dvsec(dvsec) => JsonCapabilityContent::Dvsec(JsonDvsec {
            vendor_id: format!("0x{:04x}", dvsec.vendor_id),
            revision: dvsec.revision,
            dvsec_id: format!("0x{:04x}", dvsec.dvsec_id),
            length: dvsec.length,
            data: dvsec
                .data
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<Vec<_>>()
                .join(""),
        }),
        PciCapabilityContent::Vc(vc) => JsonCapabilityContent::Vc(JsonVc {
            extended_vc_count: vc.extended_vc_count,
            port_vc_capability: vc.port_vc_capability,
            reference_clock: vc.reference_clock,
            port_arbitration_table_entry_count: vc.port_arbitration_table_entry_count,
            vc_arbitration_table_offset: vc.vc_arbitration_table_offset,
            vc_arbitration_table_entry_count: vc.vc_arbitration_table_entry_count,
            port_control: format!("0x{:04x}", vc.port_control),
            port_status: format!("0x{:04x}", vc.port_status),
            resources: vc
                .resources
                .iter()
                .map(|resource| JsonVcResource {
                    capability: format!("0x{:08x}", resource.capability),
                    control: format!("0x{:08x}", resource.control),
                    status: format!("0x{:08x}", resource.status),
                })
                .collect(),
        }),
        PciCapabilityContent::SecondaryPcie(secondary) => {
            JsonCapabilityContent::SecondaryPcie(JsonSecondaryPcie {
                perform_equalization: secondary.perform_equalization,
                equalization_request_interrupt: secondary.equalization_request_interrupt,
                lane_equalization_control: format!(
                    "0x{:08x}",
                    secondary.lane_equalization_control
                ),
            })
        }
```

- [ ] **Step 4: Verify and commit**

```bash
cargo fmt --all && cargo fmt --all -- --check
cargo check --workspace --target x86_64-unknown-linux-gnu
cargo run -q -p lspci-rs --target x86_64-unknown-linux-gnu -- show 0000:00:05.0 --format text
```

Expected (myece): unchanged output (`extended: chain=unavailable: ReadError`).

```bash
git add crates/lspci-rs/src/output.rs
git commit -m "cli: render remaining extended capability content"
```

---

### Task 7: sg-232e-224 validation, calibration, and finish

**Files:** none (verification only), plus progress doc.

**Interfaces:**
- Consumes: completed branch binary; `ssh sg-232e-224` with passwordless sudo.
- Produces: per-type comparison evidence, calibrated layouts, regression evidence, handoff doc.

- [ ] **Step 1: Build and transfer**

```bash
# in container
cd /workspace && cargo build -p lspci-rs --target x86_64-unknown-linux-gnu
# on myece host
podman cp 95c90e05ab1a:/workspace/target/x86_64-unknown-linux-gnu/debug/lspci-rs /tmp/lspci-rs
# locally (sftp only)
sftp myece <<< "get /tmp/lspci-rs <local-staging-path>"
sftp sg-232e-224 <<< "put <local-staging-path> /tmp/lspci-rs"
ssh sg-232e-224 'sudo chmod +x /tmp/lspci-rs && /tmp/lspci-rs list | head -3'
```

- [ ] **Step 2: Locate a sample device for each type**

```bash
ssh sg-232e-224 'sudo lspci -vvv | grep -B8 "Downstream Port Containment\|Precision Time Measurement\|Transaction Processing Hints\|Latency Tolerance\|Virtual Channel\|Secondary PCI Express\|Process Address Space\|Page Request Interface\|Address Translation Service\|Vendor Specific Information\|Designated Vendor" | grep -E "^[0-9a-f]+:" | sort -u | head -30'
```

Pick one device per capability type for comparison.

- [ ] **Step 3: Compare and calibrate each type**

For each sample device run both tools and compare:

```bash
ssh sg-232e-224 'sudo /tmp/lspci-rs show <addr> --format text'
ssh sg-232e-224 'sudo lspci -s <addr-short> -vvv'
```

Order of priority: PTM, LTR, DPC, PASID, ATS, PRI, TPH (simple), then VC and Secondary PCIe (calibration-heavy), then the two dump types. For every mismatch, dump the raw extended bytes (`sudo dd if=/sys/bus/pci/devices/<bdf>/config bs=1 skip=$((<cap-offset>)) count=96 | od -An -tx1`), fix the decoder in the container, rebuild, re-transfer, and re-compare. If VC or Secondary PCIe calibration proves excessive, degrade to raw registers + key fields and note it. Record every adjustment.

- [ ] **Step 4: Regression on dev48 and myece**

```bash
# re-transfer the final binary to dev48 the same sftp way, then:
ssh dev48 'sudo /tmp/lspci-rs show 0000:00:1f.0 --format text | grep extended'
# myece container:
cd /workspace
cargo fmt --all -- --check
cargo check --workspace --target x86_64-unknown-linux-gnu
cargo run -q -p lspci-rs --target x86_64-unknown-linux-gnu -- list --format text | wc -l
cargo run -q -p lspci-rs --target x86_64-unknown-linux-gnu -- show 0000:00:05.0 --format text
git diff --check
```

Expected: dev48 and myece `extended: chain=unavailable: ReadError` unchanged; myece 9 devices.

- [ ] **Step 5: Record the handoff**

Create `docs/superpowers/progress/2026-08-10-remaining-extended-decoders-progress.md` recording: commit list, sample device per type, comparison results, every calibration adjustment, any VC/Secondary PCIe degradation. Commit:

```bash
git add docs/superpowers/progress/2026-08-10-remaining-extended-decoders-progress.md
git commit -m "docs: record remaining extended decoder validation results"
```

- [ ] **Step 6: Finish the branch**

Use superpowers:finishing-a-development-branch to merge `sdd/remaining-extended-decoders` into `main` (or follow the user's chosen option).
