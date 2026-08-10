# Extended Capability Decoders Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Decode the extended-configuration-space capabilities AER, DSN, ACS, ARI, and SR-IOV into typed `content` fields, with output aligned to `lspci -vvv` including AER per-bit flag names.

**Architecture:** Session wiring is extended so the extended capability chain gets the same "prefetch + decode" treatment as the standard chain. Five new pure-function decoders over `ConfigSpaceSnapshot` feed five new `PciCapabilityContent` variants; dispatch selects by `(kind, id)` because standard and extended ID namespaces overlap. Renderers add text and JSON formats; AER flag registers render as lspci-style `Name+`/`Name-` lists.

**Tech Stack:** Rust 2024 workspace, libpci FFI (unchanged), clap, serde. Build in ECS container `95c90e05ab1a` on host `myece` (`/workspace`). Real-device validation on physical machine `sg-232e-224` (NIC `0000:3d:00.0`, passwordless sudo, lspci 3.8.0, glibc 2.32 — container binary runs directly).

## Global Constraints

- No unit tests (user decision); per-task verification is `cargo fmt --check` + `cargo check`; final verification is sg-232e-224 comparison against `sudo lspci -vvv`.
- Decoder modules contain zero FFI; decoder failure yields `content = None` and never fails `inspect()`.
- `list` behavior unchanged; no new Rust dependencies.
- Bit-number tables (especially AER UE/CE) must be calibrated against real `lspci -vvv` output on sg-232e-224 during Task 5; fix mismatches there before finishing.
- Verification commands run inside the container: `ssh myece 'docker exec 95c90e05ab1a bash -lc "cd /workspace && <cmd>"'`.
- Binary transfer chain (do NOT use scp — it gets killed): build in container, then on myece `podman cp 95c90e05ab1a:/workspace/target/x86_64-unknown-linux-gnu/debug/lspci-rs /tmp/lspci-rs`, then locally `sftp myece <<< "get /tmp/lspci-rs <local>"` and `sftp sg-232e-224 <<< "put <local> /tmp/lspci-rs"`.
- Branch `sdd/extended-decoders` from `main`; finish via finishing-a-development-branch.
- Regression invariant for dev48/myece: both have unreadable extended config, so their output must keep `extended: chain=unavailable: ReadError` exactly.

---

### Task 0: Create the feature branch

**Files:** none (git only)

- [ ] **Step 1: Create and switch branch**

```bash
cd /workspace && git checkout main && git checkout -b sdd/extended-decoders
```

Expected: `Switched to a new branch 'sdd/extended-decoders'`.

---

### Task 1: Extend session wiring to the extended chain

**Files:**
- Modify: `crates/pci/src/session.rs`
- Modify: `crates/pci/src/decoders/mod.rs`

**Interfaces:**
- Consumes: existing `decode_content`, prefetch loop, `ConfigSpaceReader::fetch`.
- Produces: extended-chain prefetch and decode in `inspect()`; `(kind, id)` dispatch in `decode_content`. No new content variants yet — extended dispatch arms land in Tasks 2–3.

- [ ] **Step 1: Rewrite `decode_content` dispatch to match on kind**

Replace the whole `decode_content` function body in `crates/pci/src/decoders/mod.rs` with:

```rust
pub(crate) fn decode_content(snapshot: &ConfigSpaceSnapshot, capability: &mut PciCapability) {
    if !matches!(capability.state, PciCapabilityState::Valid) {
        return;
    }

    let offset = capability.offset;
    capability.content = match (&capability.kind, capability.id) {
        (PciCapabilityKind::Standard, 0x01) => {
            pm::decode_pm(snapshot, offset).map(PciCapabilityContent::Pm)
        }
        (PciCapabilityKind::Standard, 0x03) => {
            vpd::decode_vpd(snapshot, offset).map(PciCapabilityContent::Vpd)
        }
        (PciCapabilityKind::Standard, 0x04) => {
            slot_id::decode_slot_id(snapshot, offset).map(PciCapabilityContent::SlotId)
        }
        (PciCapabilityKind::Standard, 0x05) => {
            msi::decode_msi(snapshot, offset).map(PciCapabilityContent::Msi)
        }
        (PciCapabilityKind::Standard, 0x07) => {
            pci_x::decode_pci_x(snapshot, offset).map(PciCapabilityContent::PciX)
        }
        (PciCapabilityKind::Standard, 0x09) => vendor::decode_vendor_specific(snapshot, offset)
            .map(PciCapabilityContent::VendorSpecific),
        (PciCapabilityKind::Standard, 0x0c) => {
            hot_plug::decode_hot_plug(snapshot, offset).map(PciCapabilityContent::HotPlug)
        }
        (PciCapabilityKind::Standard, 0x10) => {
            pcie::decode_pcie(snapshot, offset).map(PciCapabilityContent::Pcie)
        }
        (PciCapabilityKind::Standard, 0x11) => {
            msix::decode_msix(snapshot, offset).map(PciCapabilityContent::MsiX)
        }
        _ => None,
    };
}
```

Extend the existing `use crate::{...}` import in `mod.rs` to include `PciCapabilityKind`:

```rust
use crate::{
    ConfigReadFailure, ConfigSpaceSnapshot, PciCapability, PciCapabilityKind, PciCapabilityState,
};
```

- [ ] **Step 2: Extend the session capabilities block**

In `crates/pci/src/session.rs` `inspect()`, replace the entire `if header_readable { ... }` block with:

```rust
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
                            let end = (start + 0x40).min(0x1000);
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
```

- [ ] **Step 3: Verify**

```bash
cargo fmt --all && cargo fmt --all -- --check
cargo check --workspace --target x86_64-unknown-linux-gnu
cargo run -q -p lspci-rs --target x86_64-unknown-linux-gnu -- show 0000:00:05.0 --format text
```

Expected (myece container): unchanged output — `extended: chain=unavailable: ReadError`, standard chain unchanged, no panic.

- [ ] **Step 4: Commit**

```bash
git add crates/pci/src/decoders/mod.rs crates/pci/src/session.rs
git commit -m "pci: extend capability decoding to the extended chain"
```

---

### Task 2: DSN, ARI, and ACS decoders

**Files:**
- Create: `crates/pci/src/decoders/dsn.rs`
- Create: `crates/pci/src/decoders/ari.rs`
- Create: `crates/pci/src/decoders/acs.rs`
- Modify: `crates/pci/src/decoders/mod.rs`
- Modify: `crates/pci/src/lib.rs`

**Interfaces:**
- Consumes: `ConfigSpaceSnapshot::read`, `super::read_word`, `(Extended, id)` dispatch shape (Task 1).
- Produces: `decode_dsn`, `decode_ari`, `decode_acs` (`fn(snapshot: &ConfigSpaceSnapshot, offset: u16) -> Option<T>`); structs `DsnCapability`, `AriCapability`, `AcsCapability`; enum variants `Dsn`, `Ari`, `Acs`.

- [ ] **Step 1: Create `dsn.rs`**

Device Serial Number: 8 bytes starting at cap+4, rendered in byte order (matches lspci `xx-xx-...`).

```rust
use crate::ConfigSpaceSnapshot;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DsnCapability {
    pub serial: [u8; 8],
}

pub fn decode_dsn(snapshot: &ConfigSpaceSnapshot, offset: u16) -> Option<DsnCapability> {
    let base = u32::from(offset);
    let bytes = snapshot.read(base + 4, 8).ok()?;
    let mut serial = [0u8; 8];
    serial.copy_from_slice(&bytes);
    Some(DsnCapability { serial })
}
```

- [ ] **Step 2: Create `ari.rs`**

ARI: capability word at cap+4 (bits 0–7 capability flags, bits 8–15 next function number), control word at cap+6. Raw registers are stored; flag-name rendering is calibrated against lspci in Task 5.

```rust
use super::read_word;
use crate::ConfigSpaceSnapshot;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AriCapability {
    pub capability: u16,
    pub control: u16,
}

pub fn decode_ari(snapshot: &ConfigSpaceSnapshot, offset: u16) -> Option<AriCapability> {
    let base = u32::from(offset);
    let capability = read_word(snapshot, base + 4).ok()?;
    let control = read_word(snapshot, base + 6).ok()?;

    Some(AriCapability { capability, control })
}
```

- [ ] **Step 3: Create `acs.rs`**

ACS: capability word at cap+4, control word at cap+6. When the P2P Egress Control bit (bit 5) is set, an egress control vector follows at cap+8; its length in bits is capability bits 15:8.

```rust
use super::read_word;
use crate::ConfigSpaceSnapshot;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AcsCapability {
    pub capability: u16,
    pub control: u16,
    pub egress_vector: Vec<u8>,
}

pub fn decode_acs(snapshot: &ConfigSpaceSnapshot, offset: u16) -> Option<AcsCapability> {
    let base = u32::from(offset);
    let capability = read_word(snapshot, base + 4).ok()?;
    let control = read_word(snapshot, base + 6).ok()?;

    let mut egress_vector = Vec::new();
    if capability & 0x0020 != 0 {
        let bits = ((capability >> 8) & 0x00ff) as usize;
        let bytes = bits.div_ceil(8);
        for index in 0..bytes {
            egress_vector.push(snapshot.read(base + 8 + index as u32, 1).ok()?[0]);
        }
    }

    Some(AcsCapability {
        capability,
        control,
        egress_vector,
    })
}
```

- [ ] **Step 4: Register modules, variants, and dispatch in `decoders/mod.rs`**

Add module declarations keeping the existing alphabetical order; after this task the decoder module list becomes:

```rust
pub mod acs;
pub mod ari;
pub mod dsn;
pub mod hot_plug;
pub mod msi;
pub mod msix;
pub mod pci_x;
pub mod pcie;
pub mod pm;
pub mod slot_id;
pub mod vendor;
pub mod vpd;
```

Add re-exports:

```rust
pub use acs::AcsCapability;
pub use ari::AriCapability;
pub use dsn::DsnCapability;
```

Add enum variants (after `VendorSpecific`):

```rust
    Dsn(DsnCapability),
    Ari(AriCapability),
    Acs(AcsCapability),
```

Add dispatch arms (extended namespace, numeric order — 0x01 reserved for Task 3's AER):

```rust
        (PciCapabilityKind::Extended, 0x03) => {
            dsn::decode_dsn(snapshot, offset).map(PciCapabilityContent::Dsn)
        }
        (PciCapabilityKind::Extended, 0x0a) => {
            acs::decode_acs(snapshot, offset).map(PciCapabilityContent::Acs)
        }
        (PciCapabilityKind::Extended, 0x0b) => {
            ari::decode_ari(snapshot, offset).map(PciCapabilityContent::Ari)
        }
```

- [ ] **Step 5: Export the new types in `crates/pci/src/lib.rs`**

Extend the decoders re-export list with `AcsCapability, AriCapability, DsnCapability` (keep alphabetical order, cargo fmt will re-sort).

- [ ] **Step 6: Verify**

```bash
cargo fmt --all && cargo fmt --all -- --check
cargo check -p pci --target x86_64-unknown-linux-gnu
```

Expected: `pci` compiles. `cargo check --workspace` will FAIL because `output.rs` matches `PciCapabilityContent` and the new variants are unhandled — expected until Task 4. Do not fix here.

- [ ] **Step 7: Commit**

```bash
git add crates/pci/src/decoders/ crates/pci/src/lib.rs
git commit -m "pci: add DSN, ARI and ACS extended capability decoders"
```

---

### Task 3: SR-IOV and AER decoders

**Files:**
- Create: `crates/pci/src/decoders/sriov.rs`
- Create: `crates/pci/src/decoders/aer.rs`
- Modify: `crates/pci/src/decoders/mod.rs`
- Modify: `crates/pci/src/lib.rs`

**Interfaces:**
- Consumes: `super::read_word`, `super::read_dword`, `ConfigSpaceSnapshot::read`, `(Extended, id)` dispatch.
- Produces: `decode_sriov`, `decode_aer`; structs `SriovCapability`, `AerCapability`; bit-name tables `AER_UE_BITS`, `AER_CE_BITS` (`&[(u8, &str)]`) consumed by Task 4 renderers; enum variants `Sriov`, `Aer`.

- [ ] **Step 1: Create `sriov.rs`**

Register map relative to cap base: +0x04 dword capabilities, +0x08 word control, +0x0A word status, +0x0C initial VFs, +0x0E total VFs, +0x10 num VFs, +0x12 function dependency link, +0x14 VF device ID, +0x18 dword supported page sizes, +0x1C dword system page size, +0x20..+0x38 VF BAR0–5 (six dwords), +0x38 dword VF migration state array offset, +0x3C dword VF migration state array size.

```rust
use super::{read_dword, read_word};
use crate::ConfigSpaceSnapshot;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SriovCapability {
    pub capabilities: u32,
    pub control: u16,
    pub status: u16,
    pub initial_vfs: u16,
    pub total_vfs: u16,
    pub num_vfs: u16,
    pub function_dependency_link: u16,
    pub vf_device_id: u16,
    pub supported_page_sizes: u32,
    pub system_page_size: u32,
    pub vf_bars: [u32; 6],
    pub migration_state_array_offset: u32,
    pub migration_state_array_size: u32,
}

pub fn decode_sriov(snapshot: &ConfigSpaceSnapshot, offset: u16) -> Option<SriovCapability> {
    let base = u32::from(offset);

    let capabilities = read_dword(snapshot, base + 0x04).ok()?;
    let control = read_word(snapshot, base + 0x08).ok()?;
    let status = read_word(snapshot, base + 0x0a).ok()?;
    let initial_vfs = read_word(snapshot, base + 0x0c).ok()?;
    let total_vfs = read_word(snapshot, base + 0x0e).ok()?;
    let num_vfs = read_word(snapshot, base + 0x10).ok()?;
    let function_dependency_link = read_word(snapshot, base + 0x12).ok()?;
    let vf_device_id = read_word(snapshot, base + 0x14).ok()?;
    let supported_page_sizes = read_dword(snapshot, base + 0x18).ok()?;
    let system_page_size = read_dword(snapshot, base + 0x1c).ok()?;

    let mut vf_bars = [0u32; 6];
    for (index, bar) in vf_bars.iter_mut().enumerate() {
        *bar = read_dword(snapshot, base + 0x20 + (index as u32) * 4).ok()?;
    }

    let migration_state_array_offset = read_dword(snapshot, base + 0x38).ok()?;
    let migration_state_array_size = read_dword(snapshot, base + 0x3c).ok()?;

    Some(SriovCapability {
        capabilities,
        control,
        status,
        initial_vfs,
        total_vfs,
        num_vfs,
        function_dependency_link,
        vf_device_id,
        supported_page_sizes,
        system_page_size,
        vf_bars,
        migration_state_array_offset,
        migration_state_array_size,
    })
}
```

- [ ] **Step 2: Create `aer.rs`**

Register map relative to cap base: +0x04 UE status, +0x08 UE mask, +0x0C UE severity, +0x10 CE status, +0x14 CE mask, +0x18 capabilities & control (version bits 3:0, first error pointer bits 12:8), +0x1C header log (four dwords), +0x2C/0x30/0x34 root error command/status/source ID (bridges only), +0x38 TLP prefix log (four dwords). Version comes from the extended cap header dword (bits 19:16). Bridge detection reads the header-type byte at config offset 0x0E from the same snapshot (`& 0x7f == 1`).

Bit-name tables match lspci's AER output; Task 5 calibrates them against real hardware.

```rust
use super::read_dword;
use crate::ConfigSpaceSnapshot;

pub const AER_UE_BITS: &[(u8, &str)] = &[
    (4, "DLP"),
    (5, "SDES"),
    (8, "TLP"),
    (9, "FCP"),
    (10, "CmpltTO"),
    (11, "CmpltAbrt"),
    (12, "UnxCmplt"),
    (13, "RxOF"),
    (14, "MalfTLP"),
    (15, "ECRC"),
    (16, "UnsupReq"),
    (17, "ACSViol"),
    (18, "UncorrIntErr"),
    (19, "BlockedTLP"),
    (20, "AtomicOpBlocked"),
    (21, "TLPPrefixBlocked"),
];

pub const AER_CE_BITS: &[(u8, &str)] = &[
    (0, "RxErr"),
    (6, "BadTLP"),
    (7, "BadDLLP"),
    (8, "Rollover"),
    (9, "Timeout"),
    (13, "AdvNonFatalErr"),
    (14, "CorrIntErr"),
    (15, "HeaderOF"),
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AerCapability {
    pub version: u8,
    pub ue_status: u32,
    pub ue_mask: u32,
    pub ue_severity: u32,
    pub ce_status: u32,
    pub ce_mask: u32,
    pub capabilities_control: u32,
    pub first_error_pointer: u8,
    pub header_log: [u32; 4],
    pub root_command: Option<u32>,
    pub root_status: Option<u32>,
    pub error_source_id: Option<u32>,
    pub tlp_prefix_log: [u32; 4],
}

pub fn decode_aer(snapshot: &ConfigSpaceSnapshot, offset: u16) -> Option<AerCapability> {
    let base = u32::from(offset);

    let header = read_dword(snapshot, base).ok()?;
    let version = ((header >> 16) & 0x000f) as u8;

    let ue_status = read_dword(snapshot, base + 0x04).ok()?;
    let ue_mask = read_dword(snapshot, base + 0x08).ok()?;
    let ue_severity = read_dword(snapshot, base + 0x0c).ok()?;
    let ce_status = read_dword(snapshot, base + 0x10).ok()?;
    let ce_mask = read_dword(snapshot, base + 0x14).ok()?;
    let capabilities_control = read_dword(snapshot, base + 0x18).ok()?;

    let mut header_log = [0u32; 4];
    for (index, entry) in header_log.iter_mut().enumerate() {
        *entry = read_dword(snapshot, base + 0x1c + (index as u32) * 4).ok()?;
    }

    let is_bridge = snapshot
        .read(0x0e, 1)
        .ok()
        .map(|bytes| bytes[0] & 0x7f == 1)
        .unwrap_or(false);

    let (root_command, root_status, error_source_id) = if is_bridge {
        (
            Some(read_dword(snapshot, base + 0x2c).ok()?),
            Some(read_dword(snapshot, base + 0x30).ok()?),
            Some(read_dword(snapshot, base + 0x34).ok()?),
        )
    } else {
        (None, None, None)
    };

    let mut tlp_prefix_log = [0u32; 4];
    for (index, entry) in tlp_prefix_log.iter_mut().enumerate() {
        *entry = read_dword(snapshot, base + 0x38 + (index as u32) * 4).ok()?;
    }

    Some(AerCapability {
        version,
        ue_status,
        ue_mask,
        ue_severity,
        ce_status,
        ce_mask,
        capabilities_control,
        first_error_pointer: ((capabilities_control >> 8) & 0x001f) as u8,
        header_log,
        root_command,
        root_status,
        error_source_id,
        tlp_prefix_log,
    })
}
```

- [ ] **Step 3: Register modules, variants, and dispatch in `decoders/mod.rs`**

Module declarations (alphabetical): `pub mod aer;` after `pub mod acs;`, `pub mod sriov;` after `pub mod slot_id;`. Re-exports:

```rust
pub use aer::AerCapability;
pub use sriov::SriovCapability;
```

Enum variants:

```rust
    Sriov(SriovCapability),
    Aer(AerCapability),
```

Dispatch arms:

```rust
        (PciCapabilityKind::Extended, 0x01) => {
            aer::decode_aer(snapshot, offset).map(PciCapabilityContent::Aer)
        }
        (PciCapabilityKind::Extended, 0x0d) => {
            sriov::decode_sriov(snapshot, offset).map(PciCapabilityContent::Sriov)
        }
```

- [ ] **Step 4: Export in `lib.rs`**

Add `AerCapability, SriovCapability` to the decoders re-export; additionally export the bit tables:

```rust
pub use decoders::aer::{AER_CE_BITS, AER_UE_BITS};
```

- [ ] **Step 5: Verify**

```bash
cargo fmt --all && cargo fmt --all -- --check
cargo check -p pci --target x86_64-unknown-linux-gnu
```

Expected: `pci` compiles (workspace check still fails on output.rs until Task 4).

- [ ] **Step 6: Commit**

```bash
git add crates/pci/src/decoders/ crates/pci/src/lib.rs
git commit -m "pci: add SR-IOV and AER extended capability decoders"
```

---

### Task 4: Text and JSON rendering

**Files:**
- Modify: `crates/lspci-rs/src/output.rs`

**Interfaces:**
- Consumes: the five new structs and variants (Tasks 2–3), `pci::{AER_CE_BITS, AER_UE_BITS}`.
- Produces: text content lines (AER multi-line with per-bit flags) and JSON `content` objects with `type` values `dsn`, `ari`, `acs`, `sr_iov`, `aer`.

- [ ] **Step 1: Add text arms to `render_capability_content`**

Add imports `AER_CE_BITS, AER_UE_BITS, AerCapability` to the `use pci::{...}` list, then append five arms:

```rust
        PciCapabilityContent::Dsn(dsn) => {
            let serial: Vec<String> = dsn
                .serial
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect();
            format!("serial={}", serial.join(":"))
        }
        PciCapabilityContent::Ari(ari) => format!(
            "capability=0x{:04x} control=0x{:04x} next_fn=0x{:02x}",
            ari.capability,
            ari.control,
            (ari.capability >> 8) & 0x00ff
        ),
        PciCapabilityContent::Acs(acs) => format!(
            "capability=0x{:04x} control=0x{:04x} egress_vector={}",
            acs.capability,
            acs.control,
            acs.egress_vector
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<Vec<_>>()
                .join("")
        ),
        PciCapabilityContent::Sriov(sriov) => format!(
            "initial_vfs={} total_vfs={} num_vfs={} vf_device_id=0x{:04x} control=0x{:04x}",
            sriov.initial_vfs, sriov.total_vfs, sriov.num_vfs, sriov.vf_device_id, sriov.control
        ),
        PciCapabilityContent::Aer(aer) => render_aer_text(aer),
```

- [ ] **Step 2: Add the AER text renderer**

```rust
fn aer_flag_text(value: u32, bits: &[(u8, &str)]) -> String {
    bits.iter()
        .map(|(bit, name)| {
            let flag = if value & (1u32 << bit) != 0 { "+" } else { "-" };
            format!("{name}{flag}")
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn render_aer_text(aer: &AerCapability) -> String {
    let header_log: Vec<String> = aer
        .header_log
        .iter()
        .map(|entry| format!("{entry:08x}"))
        .collect();
    let tlp_log: Vec<String> = aer
        .tlp_prefix_log
        .iter()
        .map(|entry| format!("{entry:08x}"))
        .collect();

    let mut output = format!(
        "version={} first_error=0x{:02x}",
        aer.version, aer.first_error_pointer
    );
    output.push_str(&format!(
        "\n          UESta: {}",
        aer_flag_text(aer.ue_status, AER_UE_BITS)
    ));
    output.push_str(&format!(
        "\n          UEMsk: {}",
        aer_flag_text(aer.ue_mask, AER_UE_BITS)
    ));
    output.push_str(&format!(
        "\n          UESvrt: {}",
        aer_flag_text(aer.ue_severity, AER_UE_BITS)
    ));
    output.push_str(&format!(
        "\n          CESta: {}",
        aer_flag_text(aer.ce_status, AER_CE_BITS)
    ));
    output.push_str(&format!(
        "\n          CEMsk: {}",
        aer_flag_text(aer.ce_mask, AER_CE_BITS)
    ));
    output.push_str(&format!("\n          HeaderLog: {}", header_log.join(" ")));
    if let (Some(command), Some(status), Some(source)) =
        (aer.root_command, aer.root_status, aer.error_source_id)
    {
        output.push_str(&format!(
            "\n          RootCmd: 0x{command:08x} RootSta: 0x{status:08x} ErrSrc: 0x{source:08x}"
        ));
    }
    output.push_str(&format!("\n          TLPLog: {}", tlp_log.join(" ")));
    output
}
```

- [ ] **Step 3: Add JSON enum variants and structs**

Add to `JsonCapabilityContent`:

```rust
    #[serde(rename = "dsn")]
    Dsn(JsonDsn),
    #[serde(rename = "ari")]
    Ari(JsonAri),
    #[serde(rename = "acs")]
    Acs(JsonAcs),
    #[serde(rename = "sr_iov")]
    Sriov(JsonSriov),
    #[serde(rename = "aer")]
    Aer(JsonAer),
```

Structs:

```rust
#[derive(Debug, Serialize)]
struct JsonDsn {
    serial: String,
}

#[derive(Debug, Serialize)]
struct JsonAri {
    capability: String,
    control: String,
    next_fn: String,
}

#[derive(Debug, Serialize)]
struct JsonAcs {
    capability: String,
    control: String,
    egress_vector: String,
}

#[derive(Debug, Serialize)]
struct JsonSriov {
    capabilities: String,
    control: String,
    status: String,
    initial_vfs: u16,
    total_vfs: u16,
    num_vfs: u16,
    function_dependency_link: u16,
    vf_device_id: String,
    supported_page_sizes: String,
    system_page_size: String,
    vf_bars: Vec<String>,
    migration_state_array_offset: String,
    migration_state_array_size: String,
}

#[derive(Debug, Serialize)]
struct JsonAer {
    version: u8,
    first_error_pointer: String,
    ue_status: String,
    ue_status_bits: Vec<String>,
    ue_mask: String,
    ue_mask_bits: Vec<String>,
    ue_severity: String,
    ue_severity_bits: Vec<String>,
    ce_status: String,
    ce_status_bits: Vec<String>,
    ce_mask: String,
    ce_mask_bits: Vec<String>,
    capabilities_control: String,
    header_log: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    root_command: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    root_status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error_source_id: Option<String>,
    tlp_prefix_log: Vec<String>,
}
```

- [ ] **Step 4: Add JSON mapping arms to `json_capability_content`**

```rust
        PciCapabilityContent::Dsn(dsn) => JsonCapabilityContent::Dsn(JsonDsn {
            serial: dsn
                .serial
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<Vec<_>>()
                .join(":"),
        }),
        PciCapabilityContent::Ari(ari) => JsonCapabilityContent::Ari(JsonAri {
            capability: format!("0x{:04x}", ari.capability),
            control: format!("0x{:04x}", ari.control),
            next_fn: format!("0x{:02x}", (ari.capability >> 8) & 0x00ff),
        }),
        PciCapabilityContent::Acs(acs) => JsonCapabilityContent::Acs(JsonAcs {
            capability: format!("0x{:04x}", acs.capability),
            control: format!("0x{:04x}", acs.control),
            egress_vector: acs
                .egress_vector
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<Vec<_>>()
                .join(""),
        }),
        PciCapabilityContent::Sriov(sriov) => JsonCapabilityContent::Sriov(JsonSriov {
            capabilities: format!("0x{:08x}", sriov.capabilities),
            control: format!("0x{:04x}", sriov.control),
            status: format!("0x{:04x}", sriov.status),
            initial_vfs: sriov.initial_vfs,
            total_vfs: sriov.total_vfs,
            num_vfs: sriov.num_vfs,
            function_dependency_link: sriov.function_dependency_link,
            vf_device_id: format!("0x{:04x}", sriov.vf_device_id),
            supported_page_sizes: format!("0x{:08x}", sriov.supported_page_sizes),
            system_page_size: format!("0x{:08x}", sriov.system_page_size),
            vf_bars: sriov
                .vf_bars
                .iter()
                .map(|bar| format!("0x{bar:08x}"))
                .collect(),
            migration_state_array_offset: format!("0x{:08x}", sriov.migration_state_array_offset),
            migration_state_array_size: format!("0x{:08x}", sriov.migration_state_array_size),
        }),
        PciCapabilityContent::Aer(aer) => JsonCapabilityContent::Aer(JsonAer {
            version: aer.version,
            first_error_pointer: format!("0x{:02x}", aer.first_error_pointer),
            ue_status: format!("0x{:08x}", aer.ue_status),
            ue_status_bits: aer_flag_bit_names(aer.ue_status, AER_UE_BITS),
            ue_mask: format!("0x{:08x}", aer.ue_mask),
            ue_mask_bits: aer_flag_bit_names(aer.ue_mask, AER_UE_BITS),
            ue_severity: format!("0x{:08x}", aer.ue_severity),
            ue_severity_bits: aer_flag_bit_names(aer.ue_severity, AER_UE_BITS),
            ce_status: format!("0x{:08x}", aer.ce_status),
            ce_status_bits: aer_flag_bit_names(aer.ce_status, AER_CE_BITS),
            ce_mask: format!("0x{:08x}", aer.ce_mask),
            ce_mask_bits: aer_flag_bit_names(aer.ce_mask, AER_CE_BITS),
            capabilities_control: format!("0x{:08x}", aer.capabilities_control),
            header_log: aer
                .header_log
                .iter()
                .map(|entry| format!("{entry:08x}"))
                .collect(),
            root_command: aer.root_command.map(|value| format!("0x{value:08x}")),
            root_status: aer.root_status.map(|value| format!("0x{value:08x}")),
            error_source_id: aer.error_source_id.map(|value| format!("0x{value:08x}")),
            tlp_prefix_log: aer
                .tlp_prefix_log
                .iter()
                .map(|entry| format!("{entry:08x}"))
                .collect(),
        }),
```

Add the helper next to `aer_flag_text`:

```rust
fn aer_flag_bit_names(value: u32, bits: &[(u8, &str)]) -> Vec<String> {
    bits.iter()
        .filter(|(bit, _)| value & (1u32 << bit) != 0)
        .map(|(_, name)| (*name).to_owned())
        .collect()
}
```

- [ ] **Step 5: Verify**

```bash
cargo fmt --all && cargo fmt --all -- --check
cargo check --workspace --target x86_64-unknown-linux-gnu
cargo run -q -p lspci-rs --target x86_64-unknown-linux-gnu -- show 0000:00:05.0 --format text
cargo run -q -p lspci-rs --target x86_64-unknown-linux-gnu -- show 0000:00:05.0 --format json
```

Expected (myece): unchanged output — extended chain `unavailable: ReadError`, standard caps unchanged, no panic.

- [ ] **Step 6: Commit**

```bash
git add crates/lspci-rs/src/output.rs
git commit -m "cli: render extended capability content in text and JSON"
```

---

### Task 5: sg-232e-224 validation and finish

**Files:** none (verification only), plus progress doc update.

**Interfaces:**
- Consumes: completed branch binary; sg-232e-224 access (`ssh sg-232e-224`, passwordless sudo).
- Produces: field-by-field comparison evidence against `sudo lspci -vvv`, calibrated bit tables, regression evidence.

- [ ] **Step 1: Build and transfer**

```bash
# in container
cd /workspace && cargo build -p lspci-rs --target x86_64-unknown-linux-gnu
# on myece host
podman cp 95c90e05ab1a:/workspace/target/x86_64-unknown-linux-gnu/debug/lspci-rs /tmp/lspci-rs
# from the local machine (sftp only; scp is killed)
sftp myece <<< "get /tmp/lspci-rs <local-staging-path>"
sftp sg-232e-224 <<< "put <local-staging-path> /tmp/lspci-rs"
ssh sg-232e-224 'chmod +x /tmp/lspci-rs && /tmp/lspci-rs list | head -3'
```

- [ ] **Step 2: Compare all five decoders on 3d:00.0**

```bash
ssh sg-232e-224 'sudo /tmp/lspci-rs show 0000:3d:00.0 --format text'
ssh sg-232e-224 'sudo lspci -s 3d:00.0 -vvv'
```

Compare per capability:
- AER: version, UESta/UEMsk/UESvrt/CESta/CEMsk flag sets against lspci lines, header log hex
- SR-IOV: Initial/Total/Num VFs, VF Device ID, VF BARs, page sizes
- DSN: serial bytes against lspci `Device Serial Number xx-xx-...`
- ARI: capability/control values
- ACS: capability/control values, egress vector

**Calibration requirement:** if any AER bit position or ACS/ARI flag interpretation disagrees with lspci's rendered output, fix the corresponding table/decoder in the container, rebuild, re-transfer, and re-compare until all five match. Record every adjustment. Also inspect the JSON form:

```bash
ssh sg-232e-224 'sudo /tmp/lspci-rs show 0000:3d:00.0 --format json' 
```

- [ ] **Step 3: Regression on dev48 and myece**

```bash
# dev48: rebuild/transfer not needed if /tmp/lspci-rs on dev48 is stale — re-transfer this branch's binary the same sftp way first
ssh dev48 'sudo /tmp/lspci-rs show 0000:00:1f.0 --format text | grep extended'
# myece container
cd /workspace
cargo fmt --all -- --check
cargo check --workspace --target x86_64-unknown-linux-gnu
cargo run -q -p lspci-rs --target x86_64-unknown-linux-gnu -- list --format text | wc -l
cargo run -q -p lspci-rs --target x86_64-unknown-linux-gnu -- show 0000:00:05.0 --format text
git diff --check
```

Expected: dev48 and myece both show `extended: chain=unavailable: ReadError` (unchanged); myece 9 devices listed.

- [ ] **Step 4: Record the handoff**

Create `docs/superpowers/progress/2026-08-10-extended-decoders-progress.md` recording: commit list, sg-232e-224 device used, per-capability verification results, every bit-table calibration adjustment made, and dev48/myece regression results. Commit:

```bash
git add docs/superpowers/progress/2026-08-10-extended-decoders-progress.md
git commit -m "docs: record extended decoder validation results"
```

- [ ] **Step 5: Finish the branch**

Use superpowers:finishing-a-development-branch to merge `sdd/extended-decoders` into `main` (or follow the user's chosen option).
