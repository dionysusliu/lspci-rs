# More Standard Capability Decoders Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add decoders for the four remaining standard PCI capabilities — VPD (0x03), Slot Identification (0x04), PCI-X (0x07), Hot-Plug (0x0c) — reusing the existing pure-function decoder framework.

**Architecture:** Four new decoder files under `crates/pci/src/decoders/`, each a pure function over `ConfigSpaceSnapshot`. Four new variants are added to `PciCapabilityContent`, dispatch gains four ID branches, renderers gain four content formats. Session prefetch is unchanged (64-byte prefetch already covers all four protocols).

**Tech Stack:** Rust 2024 workspace, libpci FFI (unchanged), clap, serde. Build in ECS container `95c90e05ab1a` on host `myece` (`/workspace` = host `/home/leo/dev/lspci-rs`); real-device validation on dev48 (device `0000:00:1f.0` carries Slot Identification at 0x48 and Hot-Plug at 0x40).

## Global Constraints

- No unit tests (user decision); per-task verification is `cargo fmt --check` + `cargo check`; final validation is dev48 comparison against `sudo lspci -vv`.
- Decoder modules contain zero FFI; decoder failure yields `content = None` and never fails `inspect()`.
- Only the standard chain is touched; `list` behavior unchanged; no new Rust dependencies.
- Branch `sdd/more-standard-decoders` from `main`; finish via finishing-a-development-branch.
- Verification commands run inside the container: `ssh myece 'docker exec 95c90e05ab1a bash -lc "cd /workspace && <cmd>"'`.
- Binary transfer chain (myece cannot reach dev48): `podman cp 95c90e05ab1a:/workspace/target/x86_64-unknown-linux-gnu/debug/lspci-rs /tmp/lspci-rs` on myece, then `sftp myece <<< "get /tmp/lspci-rs <local>"`, then `sftp dev48 <<< "put <local> /tmp/lspci-rs"`. Do NOT use scp — it gets killed in this environment.

---

### Task 0: Create the feature branch

**Files:** none (git only)

- [ ] **Step 1: Create and switch branch**

```bash
cd /workspace && git checkout main && git checkout -b sdd/more-standard-decoders
```

Expected: `Switched to a new branch 'sdd/more-standard-decoders'`.

---

### Task 1: Four decoders, enum variants, dispatch

**Files:**
- Create: `crates/pci/src/decoders/slot_id.rs`
- Create: `crates/pci/src/decoders/hot_plug.rs`
- Create: `crates/pci/src/decoders/vpd.rs`
- Create: `crates/pci/src/decoders/pci_x.rs`
- Modify: `crates/pci/src/decoders/mod.rs`
- Modify: `crates/pci/src/lib.rs`

**Interfaces:**
- Consumes: `ConfigSpaceSnapshot::read`, `decoders::read_word` (existing helpers in `mod.rs`).
- Produces: `decode_slot_id`, `decode_hot_plug`, `decode_vpd`, `decode_pci_x` — all `fn(snapshot: &ConfigSpaceSnapshot, offset: u16) -> Option<T>`; structs `SlotIdCapability`, `HotPlugCapability`, `VpdCapability`, `PciXCapability`; dispatch for IDs 0x03/0x04/0x07/0x0c in `decode_content`.

- [ ] **Step 1: Create `slot_id.rs`**

Registers: slot byte at `offset+2`, chassis byte at `offset+3`. Slot byte layout (matches lspci `Slot ID: N slots, First+/-, chassis NN` output): bits 0–6 = slot count, bit 7 = first-in-chassis flag. Verify against dev48 in Task 3; if the raw byte disagrees with lspci's rendered form, adjust the bit split there.

```rust
use crate::ConfigSpaceSnapshot;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SlotIdCapability {
    pub slots: u8,
    pub first: bool,
    pub chassis: u8,
}

pub fn decode_slot_id(snapshot: &ConfigSpaceSnapshot, offset: u16) -> Option<SlotIdCapability> {
    let base = u32::from(offset);
    let slot = snapshot.read(base + 2, 1).ok()?[0];
    let chassis = snapshot.read(base + 3, 1).ok()?[0];

    Some(SlotIdCapability {
        slots: slot & 0x7f,
        first: slot & 0x80 != 0,
        chassis,
    })
}
```

- [ ] **Step 2: Create `hot_plug.rs`**

Register: flag byte at `offset+2`, bit 0 = hot-plug capable.

```rust
use crate::ConfigSpaceSnapshot;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HotPlugCapability {
    pub hot_plug_capable: bool,
}

pub fn decode_hot_plug(snapshot: &ConfigSpaceSnapshot, offset: u16) -> Option<HotPlugCapability> {
    let base = u32::from(offset);
    let flags = snapshot.read(base + 2, 1).ok()?[0];

    Some(HotPlugCapability {
        hot_plug_capable: flags & 0x01 != 0,
    })
}
```

- [ ] **Step 3: Create `vpd.rs`**

Registers: address word at `offset+2` (bit 15 = F flag, bits 0–14 = address), data word at `offset+4`. Uses the existing `super::read_word` helper.

```rust
use super::read_word;
use crate::ConfigSpaceSnapshot;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VpdCapability {
    pub address_flag: bool,
    pub address: u16,
    pub data: u16,
}

pub fn decode_vpd(snapshot: &ConfigSpaceSnapshot, offset: u16) -> Option<VpdCapability> {
    let base = u32::from(offset);
    let address_register = read_word(snapshot, base + 2).ok()?;
    let data = read_word(snapshot, base + 4).ok()?;

    Some(VpdCapability {
        address_flag: address_register & 0x8000 != 0,
        address: address_register & 0x7fff,
        data,
    })
}
```

- [ ] **Step 4: Create `pci_x.rs`**

Registers: command word at `offset+2`, status dword at `offset+4`. Command bits: 0 = data parity error recovery, 1 = relaxed ordering, 2–3 = maximum memory byte count, 4–6 = maximum outstanding split transactions. Status bits: 0–2 function, 3–7 device, 8–15 bus.

```rust
use super::{read_dword, read_word};
use crate::ConfigSpaceSnapshot;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PciXCapability {
    pub parity_error_recovery: bool,
    pub relaxed_ordering: bool,
    pub max_memory_block: u8,
    pub max_split: u8,
    pub bus: u8,
    pub device: u8,
    pub function: u8,
    pub status_raw: u32,
}

pub fn decode_pci_x(snapshot: &ConfigSpaceSnapshot, offset: u16) -> Option<PciXCapability> {
    let base = u32::from(offset);
    let command = read_word(snapshot, base + 2).ok()?;
    let status = read_dword(snapshot, base + 4).ok()?;

    Some(PciXCapability {
        parity_error_recovery: command & 0x0001 != 0,
        relaxed_ordering: command & 0x0002 != 0,
        max_memory_block: ((command >> 2) & 0x0003) as u8,
        max_split: ((command >> 4) & 0x0007) as u8,
        bus: ((status >> 8) & 0xff) as u8,
        device: ((status >> 3) & 0x001f) as u8,
        function: (status & 0x0007) as u8,
        status_raw: status,
    })
}
```

- [ ] **Step 5: Register modules, variants, and dispatch in `decoders/mod.rs`**

Add to the module list (alphabetical position shown):

```rust
pub mod hot_plug;
...
pub mod pci_x;
...
pub mod slot_id;
...
pub mod vpd;
```

Add re-exports next to the existing ones:

```rust
pub use hot_plug::HotPlugCapability;
pub use pci_x::PciXCapability;
pub use slot_id::SlotIdCapability;
pub use vpd::VpdCapability;
```

Add four variants to `PciCapabilityContent` (order: after `MsiX`, before `Pcie` is fine — keep it grouped sensibly):

```rust
pub enum PciCapabilityContent {
    Pm(PmCapability),
    Msi(MsiCapability),
    MsiX(MsiXCapability),
    Pcie(PcieCapability),
    VendorSpecific(VendorSpecificCapability),
    SlotId(SlotIdCapability),
    HotPlug(HotPlugCapability),
    Vpd(VpdCapability),
    PciX(PciXCapability),
}
```

Add four branches to the `decode_content` match (keep numeric order):

```rust
        0x03 => vpd::decode_vpd(snapshot, offset).map(PciCapabilityContent::Vpd),
        0x04 => slot_id::decode_slot_id(snapshot, offset).map(PciCapabilityContent::SlotId),
        0x07 => pci_x::decode_pci_x(snapshot, offset).map(PciCapabilityContent::PciX),
        0x0c => hot_plug::decode_hot_plug(snapshot, offset).map(PciCapabilityContent::HotPlug),
```

- [ ] **Step 6: Export the new types in `crates/pci/src/lib.rs`**

Extend the existing decoders re-export to:

```rust
pub use decoders::{
    HotPlugCapability, MsiCapability, MsiXCapability, PciCapabilityContent, PciXCapability,
    PcieCapability, PmCapability, SlotIdCapability, VendorSpecificCapability, VpdCapability,
};
```

- [ ] **Step 7: Verify**

```bash
cargo fmt --all && cargo fmt --all -- --check
cargo check -p pci --target x86_64-unknown-linux-gnu
```

Expected: `pci` compiles cleanly. `cargo check --workspace` will FAIL at this point because `output.rs` matches on `PciCapabilityContent` and the new variants are not handled yet — that is expected and fixed in Task 2. Do not attempt to fix it in this task.

- [ ] **Step 8: Commit**

```bash
git add crates/pci/src/decoders/ crates/pci/src/lib.rs
git commit -m "pci: add VPD, slot-ID, PCI-X and hot-plug decoders"
```

---

### Task 2: Text and JSON rendering

**Files:**
- Modify: `crates/lspci-rs/src/output.rs`

**Interfaces:**
- Consumes: the four new structs and `PciCapabilityContent` variants (Task 1).
- Produces: text content lines and JSON `content` objects with `type` values `slot_id`, `hot_plug`, `vpd`, `pci_x`.

- [ ] **Step 1: Add text arms to `render_capability_content`**

Append four match arms to `render_capability_content` in `output.rs`:

```rust
        PciCapabilityContent::SlotId(slot_id) => format!(
            "slots={} first={} chassis=0x{:02x}",
            slot_id.slots, slot_id.first, slot_id.chassis
        ),
        PciCapabilityContent::HotPlug(hot_plug) => {
            format!("hot_plug_capable={}", hot_plug.hot_plug_capable)
        }
        PciCapabilityContent::Vpd(vpd) => format!(
            "flag={} address=0x{:04x} data=0x{:04x}",
            vpd.address_flag, vpd.address, vpd.data
        ),
        PciCapabilityContent::PciX(pci_x) => format!(
            "parity_recovery={} relaxed_ordering={} max_mem_block={} max_split={} bus=0x{:02x} device={} function={} status=0x{:08x}",
            pci_x.parity_error_recovery,
            pci_x.relaxed_ordering,
            pci_x.max_memory_block,
            pci_x.max_split,
            pci_x.bus,
            pci_x.device,
            pci_x.function,
            pci_x.status_raw
        ),
```

- [ ] **Step 2: Add JSON structs and enum variants**

Add to the `JsonCapabilityContent` enum:

```rust
    #[serde(rename = "slot_id")]
    SlotId(JsonSlotId),
    #[serde(rename = "hot_plug")]
    HotPlug(JsonHotPlug),
    #[serde(rename = "vpd")]
    Vpd(JsonVpd),
    #[serde(rename = "pci_x")]
    PciX(JsonPciX),
```

Add the four JSON structs next to the existing ones:

```rust
#[derive(Debug, Serialize)]
struct JsonSlotId {
    slots: u8,
    first: bool,
    chassis: String,
}

#[derive(Debug, Serialize)]
struct JsonHotPlug {
    hot_plug_capable: bool,
}

#[derive(Debug, Serialize)]
struct JsonVpd {
    address_flag: bool,
    address: String,
    data: String,
}

#[derive(Debug, Serialize)]
struct JsonPciX {
    parity_error_recovery: bool,
    relaxed_ordering: bool,
    max_memory_block: u8,
    max_split: u8,
    bus: String,
    device: u8,
    function: u8,
    status_raw: String,
}
```

- [ ] **Step 3: Add JSON mapping arms to `json_capability_content`**

```rust
        PciCapabilityContent::SlotId(slot_id) => {
            JsonCapabilityContent::SlotId(JsonSlotId {
                slots: slot_id.slots,
                first: slot_id.first,
                chassis: format!("0x{:02x}", slot_id.chassis),
            })
        }
        PciCapabilityContent::HotPlug(hot_plug) => {
            JsonCapabilityContent::HotPlug(JsonHotPlug {
                hot_plug_capable: hot_plug.hot_plug_capable,
            })
        }
        PciCapabilityContent::Vpd(vpd) => JsonCapabilityContent::Vpd(JsonVpd {
            address_flag: vpd.address_flag,
            address: format!("0x{:04x}", vpd.address),
            data: format!("0x{:04x}", vpd.data),
        }),
        PciCapabilityContent::PciX(pci_x) => JsonCapabilityContent::PciX(JsonPciX {
            parity_error_recovery: pci_x.parity_error_recovery,
            relaxed_ordering: pci_x.relaxed_ordering,
            max_memory_block: pci_x.max_memory_block,
            max_split: pci_x.max_split,
            bus: format!("0x{:02x}", pci_x.bus),
            device: pci_x.device,
            function: pci_x.function,
            status_raw: format!("0x{:08x}", pci_x.status_raw),
        }),
```

- [ ] **Step 4: Verify**

```bash
cargo fmt --all && cargo fmt --all -- --check
cargo check --workspace --target x86_64-unknown-linux-gnu
cargo run -q -p lspci-rs --target x86_64-unknown-linux-gnu -- show 0000:00:05.0 --format text
```

Expected (myece container): unchanged output — capabilities remain `chain=unavailable: ReadError`; no panic.

- [ ] **Step 5: Commit**

```bash
git add crates/lspci-rs/src/output.rs
git commit -m "cli: render VPD, slot-ID, PCI-X and hot-plug content"
```

---

### Task 3: dev48 validation and finish

**Files:** none (verification only), plus progress doc update.

**Interfaces:**
- Consumes: completed branch binary; dev48 sudo access (passwordless).
- Produces: comparison evidence for Slot Identification and Hot-Plug against `sudo lspci -vv`.

- [ ] **Step 1: Build and transfer**

```bash
# in container
cd /workspace && cargo build -p lspci-rs --target x86_64-unknown-linux-gnu
# on myece host
podman cp 95c90e05ab1a:/workspace/target/x86_64-unknown-linux-gnu/debug/lspci-rs /tmp/lspci-rs
# from the local machine
sftp myece <<< "get /tmp/lspci-rs <local-staging-path>"
sftp dev48 <<< "put <local-staging-path> /tmp/lspci-rs"
ssh dev48 'chmod +x /tmp/lspci-rs && /tmp/lspci-rs list | head -3'
```

- [ ] **Step 2: Compare Slot Identification and Hot-Plug**

```bash
ssh dev48 'sudo /tmp/lspci-rs show 0000:00:1f.0 --format text'
ssh dev48 'sudo lspci -s 00:1f.0 -vv | grep "Capabilities"'
```

Check on dev48 (from prior probes): lspci reports `Capabilities: [48] Slot ID: 0 slots, First+, chassis 01` and `Capabilities: [40] Hot-plug capable`. Our output must show for offset 0x048 `content: slots=0 first=true chassis=0x01` and for offset 0x040 `content: hot_plug_capable=true`. If the slot byte's bit split does not reproduce lspci's `slots`/`First` values, inspect the raw byte with `sudo lspci -s 00:1f.0 -xxx`, fix the bit split in `slot_id.rs`, rebuild, re-transfer, and re-verify before continuing.

Also verify the JSON form on the same device:

```bash
ssh dev48 'sudo /tmp/lspci-rs show 0000:00:1f.0 --format json | grep -A6 slot_id'
```

- [ ] **Step 3: myece no-regression check**

```bash
cd /workspace   # inside container
cargo fmt --all -- --check
cargo check --workspace --target x86_64-unknown-linux-gnu
cargo run -q -p lspci-rs --target x86_64-unknown-linux-gnu -- list --format text | wc -l
cargo run -q -p lspci-rs --target x86_64-unknown-linux-gnu -- show 0000:00:05.0 --config standard --format text
git diff --check
```

Expected: 9 devices listed; config dump and capability statuses unchanged.

- [ ] **Step 4: Record the handoff**

Create `docs/superpowers/progress/2026-08-10-more-standard-decoders-progress.md` recording: commit list, dev48 device used, Slot ID / Hot-Plug verification result, and explicit notes that VPD and PCI-X are "本环境不可验证"（no such devices on dev48）. Commit:

```bash
git add docs/superpowers/progress/2026-08-10-more-standard-decoders-progress.md
git commit -m "docs: record more-standard-decoders validation results"
```

- [ ] **Step 5: Finish the branch**

Use superpowers:finishing-a-development-branch to merge `sdd/more-standard-decoders` into `main` (or follow the user's chosen option).
