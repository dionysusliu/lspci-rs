# PCIe Capability Full Decoding Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the simplified PCIe capability decoder with a full lspci -vv-parity implementation covering DevCap/DevCtl/DevSta, LnkCap/LnkCtl/LnkSta, Slot/Root registers, and the v2 DevCap2/DevCtl2/LnkCtl2/LnkSta2 group.

**Architecture:** `crates/pci/src/decoders/pcie.rs` is rewritten around a nested struct model (one struct per register group, `Option` for type/version-conditional groups); the decoder reads conditionally by device/port type and cap version. Renderers (text arm + `JsonPcie`) are rewritten to consume the new model. No session/lib changes — dispatch and prefetch already cover this capability.

**Tech Stack:** Rust 2024 workspace, pure decoding over `ConfigSpaceSnapshot`, serde. Build in container `95c90e05ab1a` on host `myece` (`/workspace`); validate on sg-232e-224 (X710 endpoint v2 + root port), dev48 (auxiliary), myece (regression).

## Global Constraints

- No unit tests (user decision); verification is `cargo fmt --check` + `cargo check` + real-hardware comparison.
- Decoder contains zero FFI; decode failure yields `content = None` and never fails `inspect()`.
- Bit layouts below are PCI-express-spec starting points; Task 3 calibrates them against sg-232e-224 `lspci -vv` output and fixes mismatches.
- `list` behavior unchanged; no new dependencies.
- Verification commands run inside the container: `ssh myece 'docker exec 95c90e05ab1a bash -lc "cd /workspace && <cmd>"'`.
- Binary transfer chain (sftp only; scp is killed): build in container → on myece `podman cp 95c90e05ab1a:/workspace/target/x86_64-unknown-linux-gnu/debug/lspci-rs /tmp/lspci-rs` → locally `sftp myece <<< "get /tmp/lspci-rs <local>"` → `sftp sg-232e-224 <<< "put <local> /tmp/lspci-rs"` (and same for dev48) → on target `sudo chmod +x /tmp/lspci-rs`.
- Branch `sdd/pcie-full-decoding` from `main`; finish via finishing-a-development-branch.
- During Task 1 the workspace check FAILS on lspci-rs (renderer still uses the old PcieCapability fields) — expected; verify only `-p pci` until Task 2 lands.

---

### Task 0: Create the feature branch

- [ ] **Step 1: Create and switch branch**

```bash
cd /workspace && git checkout main && git checkout -b sdd/pcie-full-decoding
```

---

### Task 1: Rewrite the PCIe decoder

**Files:**
- Replace: `crates/pci/src/decoders/pcie.rs`

**Interfaces:**
- Consumes: `super::read_word`, `super::read_dword`, `ConfigSpaceSnapshot`.
- Produces: new `PcieCapability` nested model; the renderer rewrite in Task 2 consumes these exact struct/field names.

- [ ] **Step 1: Replace `crates/pci/src/decoders/pcie.rs` with the full implementation**

```rust
use super::{read_dword, read_word};
use crate::ConfigSpaceSnapshot;

const ROOT_PORT: u8 = 4;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PcieDeviceCap {
    pub max_payload: u8,
    pub phantom_functions: u8,
    pub extended_tag: bool,
    pub l0s_latency: u8,
    pub l1_latency: u8,
    pub role_based_error: bool,
    pub attention_button: bool,
    pub attention_indicator: bool,
    pub power_indicator: bool,
    pub flreset: bool,
    pub slot_power_limit: u8,
    pub slot_power_limit_scale: u8,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PcieDeviceCtl {
    pub corr_err: bool,
    pub non_fatal_err: bool,
    pub fatal_err: bool,
    pub unsup_req: bool,
    pub relaxed_ordering: bool,
    pub max_payload: u8,
    pub extended_tag: bool,
    pub phantom_functions: bool,
    pub aux_power: bool,
    pub no_snoop: bool,
    pub max_read_req: u8,
    pub bridge_config_retry: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PcieDeviceSta {
    pub corr_err: bool,
    pub non_fatal_err: bool,
    pub fatal_err: bool,
    pub unsup_req: bool,
    pub aux_power: bool,
    pub trans_pending: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PcieLinkCap {
    pub max_speed: u8,
    pub max_width: u8,
    pub aspm: u8,
    pub l0s_exit_latency: u8,
    pub l1_exit_latency: u8,
    pub clock_pm: bool,
    pub surprise_down: bool,
    pub dll_active: bool,
    pub link_bw_notif: bool,
    pub aspm_opt_compliance: bool,
    pub port_number: u8,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PcieLinkCtl {
    pub aspm: u8,
    pub rcb: bool,
    pub link_disable: bool,
    pub retrain: bool,
    pub common_clock: bool,
    pub extended_synch: bool,
    pub clock_pm: bool,
    pub autonomous_width_disable: bool,
    pub bw_interrupt: bool,
    pub autonomous_bw_interrupt: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PcieLinkSta {
    pub speed: u8,
    pub width: u8,
    pub tr_err: bool,
    pub training: bool,
    pub slot_clock: bool,
    pub dll_active: bool,
    pub bw_management: bool,
    pub autonomous_bw: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PcieSlotCap {
    pub attention_button: bool,
    pub power_controller: bool,
    pub mrl: bool,
    pub attention_indicator: bool,
    pub power_indicator: bool,
    pub hotplug_surprise: bool,
    pub hotplug_capable: bool,
    pub slot_power_limit: u8,
    pub slot_power_limit_scale: u8,
    pub electromechanical: bool,
    pub no_command_completed: bool,
    pub physical_slot_number: u16,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PcieSlotCtl {
    pub attention_button_enable: bool,
    pub power_fault_detect: bool,
    pub mrl_sensor: bool,
    pub presence_detect: bool,
    pub command_completed: bool,
    pub hotplug_interrupt: bool,
    pub attention_indicator: u8,
    pub power_indicator: u8,
    pub power_controller_control: bool,
    pub power_interlock: bool,
    pub dll_state_changed: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PcieSlotSta {
    pub attention_button: bool,
    pub power_fault: bool,
    pub mrl_sensor: bool,
    pub presence_detect: bool,
    pub command_completed: bool,
    pub mrl_state: bool,
    pub presence_state: bool,
    pub power_interlock: bool,
    pub dll_state: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PcieRootCtl {
    pub serr_corr: bool,
    pub serr_non_fatal: bool,
    pub serr_fatal: bool,
    pub pme_interrupt: bool,
    pub crs_visible: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PcieRootSta {
    pub pme_requester_id: u16,
    pub pme_status: bool,
    pub pme_pending: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PcieDeviceCap2 {
    pub completion_timeout_ranges: u8,
    pub completion_timeout_disable: bool,
    pub ari: bool,
    pub atomic_op_routing: bool,
    pub atomic_32: bool,
    pub atomic_64: bool,
    pub atomic_128_cas: bool,
    pub no_ro_pr_pr_passing: bool,
    pub ltr: bool,
    pub tph_completer: u8,
    pub ten_bit_tag_completer: bool,
    pub ten_bit_tag_requester: bool,
    pub obff: u8,
    pub ext_fmt: bool,
    pub ee_tlp_prefix: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PcieDeviceCtl2 {
    pub completion_timeout: u8,
    pub completion_timeout_disable: bool,
    pub ari: bool,
    pub atomic_op_requester: bool,
    pub atomic_op_egress_blocking: bool,
    pub ido_request: bool,
    pub ido_completion: bool,
    pub ltr: bool,
    pub ten_bit_tag_requester: bool,
    pub obff: u8,
    pub ee_tlp_prefix_blocking: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PcieLinkCtl2 {
    pub target_speed: u8,
    pub compliance_de_emphasis: bool,
    pub transmit_margin: u8,
    pub enter_modified_compliance: bool,
    pub compliance_sos: bool,
    pub compliance_preset: u8,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PcieLinkSta2 {
    pub current_de_emphasis: bool,
    pub equalization_complete: bool,
    pub equalization_phase1: bool,
    pub equalization_phase2: bool,
    pub equalization_phase3: bool,
    pub equalization_request: bool,
    pub retimer_presence: u8,
    pub crosslink_resolution: u8,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PcieCapability {
    pub version: u8,
    pub device_type: u8,
    pub slot_implemented: bool,
    pub interrupt_message_number: u8,
    pub dev_cap: PcieDeviceCap,
    pub dev_ctl: PcieDeviceCtl,
    pub dev_sta: PcieDeviceSta,
    pub lnk_cap: PcieLinkCap,
    pub lnk_ctl: PcieLinkCtl,
    pub lnk_sta: PcieLinkSta,
    pub slot_cap: Option<PcieSlotCap>,
    pub slot_ctl: Option<PcieSlotCtl>,
    pub slot_sta: Option<PcieSlotSta>,
    pub root_ctl: Option<PcieRootCtl>,
    pub root_sta: Option<PcieRootSta>,
    pub dev_cap2: Option<PcieDeviceCap2>,
    pub dev_ctl2: Option<PcieDeviceCtl2>,
    pub lnk_ctl2: Option<PcieLinkCtl2>,
    pub lnk_sta2: Option<PcieLinkSta2>,
}

pub fn decode_pcie(snapshot: &ConfigSpaceSnapshot, offset: u16) -> Option<PcieCapability> {
    let base = u32::from(offset);

    let flags = read_word(snapshot, base + 2).ok()?;
    let version = (flags & 0x000f) as u8;
    let device_type = ((flags >> 4) & 0x000f) as u8;
    let slot_implemented = flags & 0x0100 != 0;

    let dev_cap_raw = read_dword(snapshot, base + 4).ok()?;
    let dev_ctl_raw = read_word(snapshot, base + 8).ok()?;
    let dev_sta_raw = read_word(snapshot, base + 0x0a).ok()?;
    let lnk_cap_raw = read_dword(snapshot, base + 0x0c).ok()?;
    let lnk_ctl_raw = read_word(snapshot, base + 0x10).ok()?;
    let lnk_sta_raw = read_word(snapshot, base + 0x12).ok()?;

    let (slot_cap, slot_ctl, slot_sta) = if slot_implemented {
        let cap = read_dword(snapshot, base + 0x14).ok()?;
        let ctl = read_word(snapshot, base + 0x18).ok()?;
        let sta = read_word(snapshot, base + 0x1a).ok()?;
        (
            Some(PcieSlotCap {
                attention_button: cap & 0x0000_0001 != 0,
                power_controller: cap & 0x0000_0002 != 0,
                mrl: cap & 0x0000_0004 != 0,
                attention_indicator: cap & 0x0000_0008 != 0,
                power_indicator: cap & 0x0000_0010 != 0,
                hotplug_surprise: cap & 0x0000_0020 != 0,
                hotplug_capable: cap & 0x0000_0040 != 0,
                slot_power_limit: ((cap >> 7) & 0x0000_00ff) as u8,
                slot_power_limit_scale: ((cap >> 15) & 0x0000_0003) as u8,
                electromechanical: cap & 0x0002_0000 != 0,
                no_command_completed: cap & 0x0004_0000 != 0,
                physical_slot_number: ((cap >> 19) & 0x0000_1fff) as u16,
            }),
            Some(PcieSlotCtl {
                attention_button_enable: ctl & 0x0001 != 0,
                power_fault_detect: ctl & 0x0002 != 0,
                mrl_sensor: ctl & 0x0004 != 0,
                presence_detect: ctl & 0x0008 != 0,
                command_completed: ctl & 0x0010 != 0,
                hotplug_interrupt: ctl & 0x0020 != 0,
                attention_indicator: ((ctl >> 6) & 0x0003) as u8,
                power_indicator: ((ctl >> 8) & 0x0003) as u8,
                power_controller_control: ctl & 0x0400 != 0,
                power_interlock: ctl & 0x0800 != 0,
                dll_state_changed: ctl & 0x1000 != 0,
            }),
            Some(PcieSlotSta {
                attention_button: sta & 0x0001 != 0,
                power_fault: sta & 0x0002 != 0,
                mrl_sensor: sta & 0x0004 != 0,
                presence_detect: sta & 0x0008 != 0,
                command_completed: sta & 0x0010 != 0,
                mrl_state: sta & 0x0020 != 0,
                presence_state: sta & 0x0040 != 0,
                power_interlock: sta & 0x0080 != 0,
                dll_state: sta & 0x0100 != 0,
            }),
        )
    } else {
        (None, None, None)
    };

    let (root_ctl, root_sta) = if device_type == ROOT_PORT {
        let ctl = read_word(snapshot, base + 0x1c).ok()?;
        let sta = read_dword(snapshot, base + 0x20).ok()?;
        (
            Some(PcieRootCtl {
                serr_corr: ctl & 0x0001 != 0,
                serr_non_fatal: ctl & 0x0002 != 0,
                serr_fatal: ctl & 0x0004 != 0,
                pme_interrupt: ctl & 0x0008 != 0,
                crs_visible: ctl & 0x0010 != 0,
            }),
            Some(PcieRootSta {
                pme_requester_id: (sta & 0x0000_ffff) as u16,
                pme_status: sta & 0x0001_0000 != 0,
                pme_pending: sta & 0x0002_0000 != 0,
            }),
        )
    } else {
        (None, None)
    };

    let (dev_cap2, dev_ctl2, lnk_ctl2, lnk_sta2) = if version >= 2 {
        let cap2 = read_dword(snapshot, base + 0x24).ok()?;
        let ctl2 = read_word(snapshot, base + 0x28).ok()?;
        let lnk_ctl2_raw = read_word(snapshot, base + 0x30).ok()?;
        let lnk_sta2_raw = read_word(snapshot, base + 0x32).ok()?;
        (
            Some(PcieDeviceCap2 {
                completion_timeout_ranges: (cap2 & 0x0000_000f) as u8,
                completion_timeout_disable: cap2 & 0x0000_0010 != 0,
                ari: cap2 & 0x0000_0020 != 0,
                atomic_op_routing: cap2 & 0x0000_0040 != 0,
                atomic_32: cap2 & 0x0000_0080 != 0,
                atomic_64: cap2 & 0x0000_0100 != 0,
                atomic_128_cas: cap2 & 0x0000_0200 != 0,
                no_ro_pr_pr_passing: cap2 & 0x0000_0400 != 0,
                ltr: cap2 & 0x0000_0800 != 0,
                tph_completer: ((cap2 >> 12) & 0x0000_0003) as u8,
                ten_bit_tag_completer: cap2 & 0x0001_0000 != 0,
                ten_bit_tag_requester: cap2 & 0x0002_0000 != 0,
                obff: ((cap2 >> 18) & 0x0000_0003) as u8,
                ext_fmt: cap2 & 0x0010_0000 != 0,
                ee_tlp_prefix: cap2 & 0x0020_0000 != 0,
            }),
            Some(PcieDeviceCtl2 {
                completion_timeout: (ctl2 & 0x000f) as u8,
                completion_timeout_disable: ctl2 & 0x0010 != 0,
                ari: ctl2 & 0x0020 != 0,
                atomic_op_requester: ctl2 & 0x0040 != 0,
                atomic_op_egress_blocking: ctl2 & 0x0080 != 0,
                ido_request: ctl2 & 0x0100 != 0,
                ido_completion: ctl2 & 0x0200 != 0,
                ltr: ctl2 & 0x0400 != 0,
                ten_bit_tag_requester: ctl2 & 0x1000 != 0,
                obff: ((ctl2 >> 13) & 0x0003) as u8,
                ee_tlp_prefix_blocking: ctl2 & 0x8000 != 0,
            }),
            Some(PcieLinkCtl2 {
                target_speed: (lnk_ctl2_raw & 0x000f) as u8,
                compliance_de_emphasis: lnk_ctl2_raw & 0x0010 != 0,
                transmit_margin: ((lnk_ctl2_raw >> 7) & 0x0003) as u8,
                enter_modified_compliance: lnk_ctl2_raw & 0x0080 != 0,
                compliance_sos: lnk_ctl2_raw & 0x0100 != 0,
                compliance_preset: ((lnk_ctl2_raw >> 9) & 0x000f) as u8,
            }),
            Some(PcieLinkSta2 {
                current_de_emphasis: lnk_sta2_raw & 0x0001 != 0,
                equalization_complete: lnk_sta2_raw & 0x0002 != 0,
                equalization_phase1: lnk_sta2_raw & 0x0004 != 0,
                equalization_phase2: lnk_sta2_raw & 0x0008 != 0,
                equalization_phase3: lnk_sta2_raw & 0x0010 != 0,
                equalization_request: lnk_sta2_raw & 0x0020 != 0,
                retimer_presence: ((lnk_sta2_raw >> 6) & 0x0003) as u8,
                crosslink_resolution: ((lnk_sta2_raw >> 9) & 0x0003) as u8,
            }),
        )
    } else {
        (None, None, None, None)
    };

    Some(PcieCapability {
        version,
        device_type,
        slot_implemented,
        interrupt_message_number: ((flags >> 9) & 0x001f) as u8,
        dev_cap: PcieDeviceCap {
            max_payload: (dev_cap_raw & 0x0000_0007) as u8,
            phantom_functions: ((dev_cap_raw >> 3) & 0x0000_0003) as u8,
            extended_tag: dev_cap_raw & 0x0000_0020 != 0,
            l0s_latency: ((dev_cap_raw >> 6) & 0x0000_0007) as u8,
            l1_latency: ((dev_cap_raw >> 9) & 0x0000_0007) as u8,
            role_based_error: dev_cap_raw & 0x0000_8000 != 0,
            attention_button: dev_cap_raw & 0x0001_0000 != 0,
            attention_indicator: dev_cap_raw & 0x0002_0000 != 0,
            power_indicator: dev_cap_raw & 0x0004_0000 != 0,
            flreset: dev_cap_raw & 0x1000_0000 != 0,
            slot_power_limit: ((dev_cap_raw >> 18) & 0x0000_00ff) as u8,
            slot_power_limit_scale: ((dev_cap_raw >> 26) & 0x0000_0003) as u8,
        },
        dev_ctl: PcieDeviceCtl {
            corr_err: dev_ctl_raw & 0x0001 != 0,
            non_fatal_err: dev_ctl_raw & 0x0002 != 0,
            fatal_err: dev_ctl_raw & 0x0004 != 0,
            unsup_req: dev_ctl_raw & 0x0008 != 0,
            relaxed_ordering: dev_ctl_raw & 0x0010 != 0,
            max_payload: ((dev_ctl_raw >> 5) & 0x0007) as u8,
            extended_tag: dev_ctl_raw & 0x0100 != 0,
            phantom_functions: dev_ctl_raw & 0x0200 != 0,
            aux_power: dev_ctl_raw & 0x0400 != 0,
            no_snoop: dev_ctl_raw & 0x0800 != 0,
            max_read_req: ((dev_ctl_raw >> 12) & 0x0007) as u8,
            bridge_config_retry: dev_ctl_raw & 0x8000 != 0,
        },
        dev_sta: PcieDeviceSta {
            corr_err: dev_sta_raw & 0x0001 != 0,
            non_fatal_err: dev_sta_raw & 0x0002 != 0,
            fatal_err: dev_sta_raw & 0x0004 != 0,
            unsup_req: dev_sta_raw & 0x0008 != 0,
            aux_power: dev_sta_raw & 0x0010 != 0,
            trans_pending: dev_sta_raw & 0x0020 != 0,
        },
        lnk_cap: PcieLinkCap {
            max_speed: (lnk_cap_raw & 0x0000_000f) as u8,
            max_width: ((lnk_cap_raw >> 4) & 0x0000_003f) as u8,
            aspm: ((lnk_cap_raw >> 10) & 0x0000_0003) as u8,
            l0s_exit_latency: ((lnk_cap_raw >> 12) & 0x0000_0007) as u8,
            l1_exit_latency: ((lnk_cap_raw >> 15) & 0x0000_0007) as u8,
            clock_pm: lnk_cap_raw & 0x0004_0000 != 0,
            surprise_down: lnk_cap_raw & 0x0008_0000 != 0,
            dll_active: lnk_cap_raw & 0x0010_0000 != 0,
            link_bw_notif: lnk_cap_raw & 0x0020_0000 != 0,
            aspm_opt_compliance: lnk_cap_raw & 0x0040_0000 != 0,
            port_number: ((lnk_cap_raw >> 24) & 0x0000_00ff) as u8,
        },
        lnk_ctl: PcieLinkCtl {
            aspm: (lnk_ctl_raw & 0x0003) as u8,
            rcb: lnk_ctl_raw & 0x0008 != 0,
            link_disable: lnk_ctl_raw & 0x0010 != 0,
            retrain: lnk_ctl_raw & 0x0020 != 0,
            common_clock: lnk_ctl_raw & 0x0040 != 0,
            extended_synch: lnk_ctl_raw & 0x0080 != 0,
            clock_pm: lnk_ctl_raw & 0x0100 != 0,
            autonomous_width_disable: lnk_ctl_raw & 0x0200 != 0,
            bw_interrupt: lnk_ctl_raw & 0x0400 != 0,
            autonomous_bw_interrupt: lnk_ctl_raw & 0x0800 != 0,
        },
        lnk_sta: PcieLinkSta {
            speed: (lnk_sta_raw & 0x000f) as u8,
            width: ((lnk_sta_raw >> 4) & 0x003f) as u8,
            tr_err: lnk_sta_raw & 0x0400 != 0,
            training: lnk_sta_raw & 0x0800 != 0,
            slot_clock: lnk_sta_raw & 0x1000 != 0,
            dll_active: lnk_sta_raw & 0x2000 != 0,
            bw_management: lnk_sta_raw & 0x4000 != 0,
            autonomous_bw: lnk_sta_raw & 0x8000 != 0,
        },
        slot_cap,
        slot_ctl,
        slot_sta,
        root_ctl,
        root_sta,
        dev_cap2,
        dev_ctl2,
        lnk_ctl2,
        lnk_sta2,
    })
}
```

- [ ] **Step 2: Verify the pci crate compiles**

```bash
cargo fmt --all && cargo fmt --all -- --check
cargo check -p pci --target x86_64-unknown-linux-gnu
```

Expected: `pci` compiles. `cargo check --workspace` FAILS on `output.rs` (old renderer references removed fields) — expected until Task 2.

- [ ] **Step 3: Commit**

```bash
git add crates/pci/src/decoders/pcie.rs
git commit -m "pci: decode full PCIe capability register groups"
```

---

### Task 2: Rewrite text and JSON rendering

**Files:**
- Modify: `crates/lspci-rs/src/output.rs`

**Interfaces:**
- Consumes: the Task 1 struct model (all names exactly as defined there).
- Produces: multi-line pcie text block and nested `JsonPcie` object.

- [ ] **Step 1: Replace the pcie text arm and add helpers**

Import the new types by extending the `use pci::{...}` list with:
`PcieCapability, PcieDeviceCap, PcieDeviceCtl, PcieDeviceSta, PcieLinkCap, PcieLinkCtl, PcieLinkSta, PcieSlotCap, PcieSlotCtl, PcieSlotSta, PcieRootCtl, PcieRootSta, PcieDeviceCap2, PcieDeviceCtl2, PcieLinkCtl2, PcieLinkSta2` (cargo fmt re-sorts).

Replace the existing `PciCapabilityContent::Pcie(secondary)` text arm (currently a single-line `format!`) with:

```rust
        PciCapabilityContent::Pcie(pcie) => render_pcie_text(pcie),
```

Add these functions (next to `render_aer_text`):

```rust
fn pcie_max_payload(code: u8) -> &'static str {
    match code {
        0 => "128 bytes",
        1 => "256 bytes",
        2 => "512 bytes",
        3 => "1024 bytes",
        4 => "2048 bytes",
        5 => "4096 bytes",
        _ => "reserved",
    }
}

fn pcie_max_read_req(code: u8) -> &'static str {
    match code {
        0 => "128 bytes",
        1 => "256 bytes",
        2 => "512 bytes",
        3 => "1024 bytes",
        4 => "2048 bytes",
        5 => "4096 bytes",
        _ => "reserved",
    }
}

fn pcie_aspm_support(aspm: u8) -> &'static str {
    match aspm {
        0 => "not supported",
        1 => "L0s",
        2 => "L1",
        3 => "L0s L1",
        _ => "reserved",
    }
}

fn pcie_aspm_control(aspm: u8) -> &'static str {
    match aspm {
        0 => "Disabled",
        1 => "L0s Only",
        2 => "L1 Only",
        3 => "L0s L1",
        _ => "reserved",
    }
}

fn pcie_flag(enabled: bool) -> &'static str {
    if enabled { "+" } else { "-" }
}

fn render_pcie_text(pcie: &PcieCapability) -> String {
    let dev_cap = &pcie.dev_cap;
    let dev_ctl = &pcie.dev_ctl;
    let dev_sta = &pcie.dev_sta;
    let lnk_cap = &pcie.lnk_cap;
    let lnk_ctl = &pcie.lnk_ctl;
    let lnk_sta = &pcie.lnk_sta;

    let mut output = format!(
        "version={} type={} slot={} msi={}",
        pcie.version,
        render_pcie_device_type(pcie.device_type),
        pcie.slot_implemented,
        pcie.interrupt_message_number
    );

    output.push_str(&format!(
        "\n          DevCap: MaxPayload {} PhantFunc {} Latency L0s {} L1 {}",
        pcie_max_payload(dev_cap.max_payload),
        dev_cap.phantom_functions,
        dev_cap.l0s_latency,
        dev_cap.l1_latency
    ));
    output.push_str(&format!(
        "\n                  ExtTag{} AttnBtn{} AttnInd{} PwrInd{} RBE{} FLReset{}",
        pcie_flag(dev_cap.extended_tag),
        pcie_flag(dev_cap.attention_button),
        pcie_flag(dev_cap.attention_indicator),
        pcie_flag(dev_cap.power_indicator),
        pcie_flag(dev_cap.role_based_error),
        pcie_flag(dev_cap.flreset)
    ));
    output.push_str(&format!(
        "\n          DevCtl: CorrErr{} NonFatalErr{} FatalErr{} UnsupReq{}",
        pcie_flag(dev_ctl.corr_err),
        pcie_flag(dev_ctl.non_fatal_err),
        pcie_flag(dev_ctl.fatal_err),
        pcie_flag(dev_ctl.unsup_req)
    ));
    output.push_str(&format!(
        "\n                  RlxdOrd{} ExtTag{} PhantFunc{} AuxPwr{} NoSnoop{}",
        pcie_flag(dev_ctl.relaxed_ordering),
        pcie_flag(dev_ctl.extended_tag),
        pcie_flag(dev_ctl.phantom_functions),
        pcie_flag(dev_ctl.aux_power),
        pcie_flag(dev_ctl.no_snoop)
    ));
    output.push_str(&format!(
        "\n                  MaxPayload {}, MaxReadReq {}",
        pcie_max_payload(dev_ctl.max_payload),
        pcie_max_read_req(dev_ctl.max_read_req)
    ));
    output.push_str(&format!(
        "\n          DevSta: CorrErr{} NonFatalErr{} FatalErr{} UnsupReq{} AuxPwr{} TransPend{}",
        pcie_flag(dev_sta.corr_err),
        pcie_flag(dev_sta.non_fatal_err),
        pcie_flag(dev_sta.fatal_err),
        pcie_flag(dev_sta.unsup_req),
        pcie_flag(dev_sta.aux_power),
        pcie_flag(dev_sta.trans_pending)
    ));

    let speed_downgraded = lnk_sta.speed < lnk_cap.max_speed;
    let width_downgraded = lnk_sta.width < lnk_cap.max_width;
    let speed_note = if speed_downgraded { " (downgraded)" } else { " (ok)" };
    let width_note = if width_downgraded { " (downgraded)" } else { "" };

    output.push_str(&format!(
        "\n          LnkCap: Port #{}, Speed {}GT/s, Width x{}, ASPM {}, Exit Latency L0s {} L1 {}",
        lnk_cap.port_number,
        render_pcie_speed(lnk_cap.max_speed),
        lnk_cap.max_width,
        pcie_aspm_support(lnk_cap.aspm),
        lnk_cap.l0s_exit_latency,
        lnk_cap.l1_exit_latency
    ));
    output.push_str(&format!(
        "\n                  ClockPM{} Surprise{} LLActRep{} BwNot{} ASPMOptComp{}",
        pcie_flag(lnk_cap.clock_pm),
        pcie_flag(lnk_cap.surprise_down),
        pcie_flag(lnk_cap.dll_active),
        pcie_flag(lnk_cap.link_bw_notif),
        pcie_flag(lnk_cap.aspm_opt_compliance)
    ));
    output.push_str(&format!(
        "\n          LnkCtl: ASPM {}; RCB {} bytes, Disabled{} CommClk{}",
        pcie_aspm_control(lnk_ctl.aspm),
        if lnk_ctl.rcb { "128" } else { "64" },
        pcie_flag(lnk_ctl.link_disable),
        pcie_flag(lnk_ctl.common_clock)
    ));
    output.push_str(&format!(
        "\n                  ExtSynch{} ClockPM{} AutWidDis{} BWInt{} AutBWInt{}",
        pcie_flag(lnk_ctl.extended_synch),
        pcie_flag(lnk_ctl.clock_pm),
        pcie_flag(lnk_ctl.autonomous_width_disable),
        pcie_flag(lnk_ctl.bw_interrupt),
        pcie_flag(lnk_ctl.autonomous_bw_interrupt)
    ));
    output.push_str(&format!(
        "\n          LnkSta: Speed {}GT/s{}, Width x{}{}, TrErr{} Train{} SlotClk{} DLActive{} BWMgmt{} ABWMgmt{}",
        render_pcie_speed(lnk_sta.speed),
        speed_note,
        lnk_sta.width,
        width_note,
        pcie_flag(lnk_sta.tr_err),
        pcie_flag(lnk_sta.training),
        pcie_flag(lnk_sta.slot_clock),
        pcie_flag(lnk_sta.dll_active),
        pcie_flag(lnk_sta.bw_management),
        pcie_flag(lnk_sta.autonomous_bw)
    ));

    if let (Some(slot_cap), Some(slot_ctl), Some(slot_sta)) =
        (&pcie.slot_cap, &pcie.slot_ctl, &pcie.slot_sta)
    {
        output.push_str(&format!(
            "\n          SlotCap: AttnBtn{} PwrCtrl{} MRL{} AttnInd{} PwrInd{} HotPlugSurprise{} HotPlug{} PhysSlot={}",
            pcie_flag(slot_cap.attention_button),
            pcie_flag(slot_cap.power_controller),
            pcie_flag(slot_cap.mrl),
            pcie_flag(slot_cap.attention_indicator),
            pcie_flag(slot_cap.power_indicator),
            pcie_flag(slot_cap.hotplug_surprise),
            pcie_flag(slot_cap.hotplug_capable),
            slot_cap.physical_slot_number
        ));
        output.push_str(&format!(
            "\n          SlotCtl: AttnBtn{} PwrFlt{} MRL{} Pres{} CmdCplt{} HPIrq{} PwrCtrl{}",
            pcie_flag(slot_ctl.attention_button_enable),
            pcie_flag(slot_ctl.power_fault_detect),
            pcie_flag(slot_ctl.mrl_sensor),
            pcie_flag(slot_ctl.presence_detect),
            pcie_flag(slot_ctl.command_completed),
            pcie_flag(slot_ctl.hotplug_interrupt),
            pcie_flag(slot_ctl.power_controller_control)
        ));
        output.push_str(&format!(
            "\n          SlotSta: AttnBtn{} PwrFlt{} MRL{} Pres{} CmdCplt{} MRLSta{} PresSta{} DLLSta{}",
            pcie_flag(slot_sta.attention_button),
            pcie_flag(slot_sta.power_fault),
            pcie_flag(slot_sta.mrl_sensor),
            pcie_flag(slot_sta.presence_detect),
            pcie_flag(slot_sta.command_completed),
            pcie_flag(slot_sta.mrl_state),
            pcie_flag(slot_sta.presence_state),
            pcie_flag(slot_sta.dll_state)
        ));
    }

    if let (Some(root_ctl), Some(root_sta)) = (&pcie.root_ctl, &pcie.root_sta) {
        output.push_str(&format!(
            "\n          RootCtl: ErrCorrectable{} ErrNon-Fatal{} ErrFatal{} PMEInterrupt{} CRSVisible{}",
            pcie_flag(root_ctl.serr_corr),
            pcie_flag(root_ctl.serr_non_fatal),
            pcie_flag(root_ctl.serr_fatal),
            pcie_flag(root_ctl.pme_interrupt),
            pcie_flag(root_ctl.crs_visible)
        ));
        output.push_str(&format!(
            "\n          RootSta: PME ReqID 0x{:04x}, PME Status {}, PME Pending {}",
            root_sta.pme_requester_id,
            pcie_flag(root_sta.pme_status),
            pcie_flag(root_sta.pme_pending)
        ));
    }

    if let (Some(dev_cap2), Some(dev_ctl2), Some(lnk_ctl2), Some(lnk_sta2)) = (
        &pcie.dev_cap2,
        &pcie.dev_ctl2,
        &pcie.lnk_ctl2,
        &pcie.lnk_sta2,
    ) {
        output.push_str(&format!(
            "\n          DevCap2: Completion Timeout: {:02x}, TimeoutDis{} ARI{} AtomicOpsRouting{} LTR{} 10BitTagComp{} 10BitTagReq{} OBFF {} ExtFmt{} EETLPPrefix{}",
            dev_cap2.completion_timeout_ranges,
            pcie_flag(dev_cap2.completion_timeout_disable),
            pcie_flag(dev_cap2.ari),
            pcie_flag(dev_cap2.atomic_op_routing),
            pcie_flag(dev_cap2.ltr),
            pcie_flag(dev_cap2.ten_bit_tag_completer),
            pcie_flag(dev_cap2.ten_bit_tag_requester),
            dev_cap2.obff,
            pcie_flag(dev_cap2.ext_fmt),
            pcie_flag(dev_cap2.ee_tlp_prefix)
        ));
        output.push_str(&format!(
            "\n                  AtomicOpsCap: 32bit{} 64bit{} 128bitCAS{}",
            pcie_flag(dev_cap2.atomic_32),
            pcie_flag(dev_cap2.atomic_64),
            pcie_flag(dev_cap2.atomic_128_cas)
        ));
        output.push_str(&format!(
            "\n          DevCtl2: Completion Timeout: {:02x}, TimeoutDis{} LTR{} 10BitTagReq{} OBFF {}",
            dev_ctl2.completion_timeout,
            pcie_flag(dev_ctl2.completion_timeout_disable),
            pcie_flag(dev_ctl2.ltr),
            pcie_flag(dev_ctl2.ten_bit_tag_requester),
            dev_ctl2.obff
        ));
        output.push_str(&format!(
            "\n                  AtomicOpsCtl: ReqEn{} EgressBlk{}",
            pcie_flag(dev_ctl2.atomic_op_requester),
            pcie_flag(dev_ctl2.atomic_op_egress_blocking)
        ));
        output.push_str(&format!(
            "\n          LnkCtl2: Target Speed: {}GT/s, ComplianceDeemphasis{} ComplianceSOS{}",
            render_pcie_speed(lnk_ctl2.target_speed),
            pcie_flag(lnk_ctl2.compliance_de_emphasis),
            pcie_flag(lnk_ctl2.compliance_sos)
        ));
        output.push_str(&format!(
            "\n          LnkSta2: Current De-emphasis: {}, EqualizationComplete{} EqualizationPhase1{} EqualizationPhase2{} EqualizationPhase3{} LinkEqualizationRequest{} Retimer{}",
            if lnk_sta2.current_de_emphasis { "-6dB" } else { "-3.5dB" },
            pcie_flag(lnk_sta2.equalization_complete),
            pcie_flag(lnk_sta2.equalization_phase1),
            pcie_flag(lnk_sta2.equalization_phase2),
            pcie_flag(lnk_sta2.equalization_phase3),
            pcie_flag(lnk_sta2.equalization_request),
            lnk_sta2.retimer_presence
        ));
    }

    output
}
```

- [ ] **Step 2: Replace the JSON structs**

Replace the existing `JsonPcie` struct and its mapping arm with this nested model. Structs:

```rust
#[derive(Debug, Serialize)]
struct JsonPcie {
    version: u8,
    device_type: String,
    slot_implemented: bool,
    interrupt_message_number: u8,
    dev_cap: JsonPcieDevCap,
    dev_ctl: JsonPcieDevCtl,
    dev_sta: JsonPcieDevSta,
    lnk_cap: JsonPcieLnkCap,
    lnk_ctl: JsonPcieLnkCtl,
    lnk_sta: JsonPcieLnkSta,
    #[serde(skip_serializing_if = "Option::is_none")]
    slot_cap: Option<JsonPcieSlotCap>,
    #[serde(skip_serializing_if = "Option::is_none")]
    slot_ctl: Option<JsonPcieSlotCtl>,
    #[serde(skip_serializing_if = "Option::is_none")]
    slot_sta: Option<JsonPcieSlotSta>,
    #[serde(skip_serializing_if = "Option::is_none")]
    root_ctl: Option<JsonPcieRootCtl>,
    #[serde(skip_serializing_if = "Option::is_none")]
    root_sta: Option<JsonPcieRootSta>,
    #[serde(skip_serializing_if = "Option::is_none")]
    dev_cap2: Option<JsonPcieDevCap2>,
    #[serde(skip_serializing_if = "Option::is_none")]
    dev_ctl2: Option<JsonPcieDevCtl2>,
    #[serde(skip_serializing_if = "Option::is_none")]
    lnk_ctl2: Option<JsonPcieLnkCtl2>,
    #[serde(skip_serializing_if = "Option::is_none")]
    lnk_sta2: Option<JsonPcieLnkSta2>,
}
```

with these sub-structs (field names mirror the Task 1 decoder structs exactly, serializing bools as bools and speeds/widths as their raw numbers):

```rust
#[derive(Debug, Serialize)]
struct JsonPcieDevCap {
    max_payload: u8,
    phantom_functions: u8,
    extended_tag: bool,
    l0s_latency: u8,
    l1_latency: u8,
    role_based_error: bool,
    attention_button: bool,
    attention_indicator: bool,
    power_indicator: bool,
    flreset: bool,
    slot_power_limit: u8,
    slot_power_limit_scale: u8,
}

#[derive(Debug, Serialize)]
struct JsonPcieDevCtl {
    corr_err: bool,
    non_fatal_err: bool,
    fatal_err: bool,
    unsup_req: bool,
    relaxed_ordering: bool,
    max_payload: u8,
    extended_tag: bool,
    phantom_functions: bool,
    aux_power: bool,
    no_snoop: bool,
    max_read_req: u8,
    bridge_config_retry: bool,
}

#[derive(Debug, Serialize)]
struct JsonPcieDevSta {
    corr_err: bool,
    non_fatal_err: bool,
    fatal_err: bool,
    unsup_req: bool,
    aux_power: bool,
    trans_pending: bool,
}

#[derive(Debug, Serialize)]
struct JsonPcieLnkCap {
    max_speed: u8,
    max_width: u8,
    aspm: u8,
    l0s_exit_latency: u8,
    l1_exit_latency: u8,
    clock_pm: bool,
    surprise_down: bool,
    dll_active: bool,
    link_bw_notif: bool,
    aspm_opt_compliance: bool,
    port_number: u8,
}

#[derive(Debug, Serialize)]
struct JsonPcieLnkCtl {
    aspm: u8,
    rcb: bool,
    link_disable: bool,
    retrain: bool,
    common_clock: bool,
    extended_synch: bool,
    clock_pm: bool,
    autonomous_width_disable: bool,
    bw_interrupt: bool,
    autonomous_bw_interrupt: bool,
}

#[derive(Debug, Serialize)]
struct JsonPcieLnkSta {
    speed: u8,
    width: u8,
    downgraded: bool,
    tr_err: bool,
    training: bool,
    slot_clock: bool,
    dll_active: bool,
    bw_management: bool,
    autonomous_bw: bool,
}

#[derive(Debug, Serialize)]
struct JsonPcieSlotCap {
    attention_button: bool,
    power_controller: bool,
    mrl: bool,
    attention_indicator: bool,
    power_indicator: bool,
    hotplug_surprise: bool,
    hotplug_capable: bool,
    slot_power_limit: u8,
    slot_power_limit_scale: u8,
    electromechanical: bool,
    no_command_completed: bool,
    physical_slot_number: u16,
}

#[derive(Debug, Serialize)]
struct JsonPcieSlotCtl {
    attention_button_enable: bool,
    power_fault_detect: bool,
    mrl_sensor: bool,
    presence_detect: bool,
    command_completed: bool,
    hotplug_interrupt: bool,
    attention_indicator: u8,
    power_indicator: u8,
    power_controller_control: bool,
    power_interlock: bool,
    dll_state_changed: bool,
}

#[derive(Debug, Serialize)]
struct JsonPcieSlotSta {
    attention_button: bool,
    power_fault: bool,
    mrl_sensor: bool,
    presence_detect: bool,
    command_completed: bool,
    mrl_state: bool,
    presence_state: bool,
    power_interlock: bool,
    dll_state: bool,
}

#[derive(Debug, Serialize)]
struct JsonPcieRootCtl {
    serr_corr: bool,
    serr_non_fatal: bool,
    serr_fatal: bool,
    pme_interrupt: bool,
    crs_visible: bool,
}

#[derive(Debug, Serialize)]
struct JsonPcieRootSta {
    pme_requester_id: String,
    pme_status: bool,
    pme_pending: bool,
}

#[derive(Debug, Serialize)]
struct JsonPcieDevCap2 {
    completion_timeout_ranges: u8,
    completion_timeout_disable: bool,
    ari: bool,
    atomic_op_routing: bool,
    atomic_32: bool,
    atomic_64: bool,
    atomic_128_cas: bool,
    no_ro_pr_pr_passing: bool,
    ltr: bool,
    tph_completer: u8,
    ten_bit_tag_completer: bool,
    ten_bit_tag_requester: bool,
    obff: u8,
    ext_fmt: bool,
    ee_tlp_prefix: bool,
}

#[derive(Debug, Serialize)]
struct JsonPcieDevCtl2 {
    completion_timeout: u8,
    completion_timeout_disable: bool,
    ari: bool,
    atomic_op_requester: bool,
    atomic_op_egress_blocking: bool,
    ido_request: bool,
    ido_completion: bool,
    ltr: bool,
    ten_bit_tag_requester: bool,
    obff: u8,
    ee_tlp_prefix_blocking: bool,
}

#[derive(Debug, Serialize)]
struct JsonPcieLnkCtl2 {
    target_speed: u8,
    compliance_de_emphasis: bool,
    transmit_margin: u8,
    enter_modified_compliance: bool,
    compliance_sos: bool,
    compliance_preset: u8,
}

#[derive(Debug, Serialize)]
struct JsonPcieLnkSta2 {
    current_de_emphasis: bool,
    equalization_complete: bool,
    equalization_phase1: bool,
    equalization_phase2: bool,
    equalization_phase3: bool,
    equalization_request: bool,
    retimer_presence: u8,
    crosslink_resolution: u8,
}
```

- [ ] **Step 3: Replace the JSON mapping arm**

Replace the old `PciCapabilityContent::Pcie` mapping arm with a field-for-field conversion from the Task 1 structs into the Step 2 JSON structs. The conversion is mechanical (copy each field by name); compute the two derived fields:

```rust
        PciCapabilityContent::Pcie(pcie) => {
            let downgraded = pcie.lnk_sta.speed < pcie.lnk_cap.max_speed
                || pcie.lnk_sta.width < pcie.lnk_cap.max_width;
            JsonCapabilityContent::Pcie(JsonPcie {
                version: pcie.version,
                device_type: render_pcie_device_type(pcie.device_type).to_owned(),
                slot_implemented: pcie.slot_implemented,
                interrupt_message_number: pcie.interrupt_message_number,
                dev_cap: JsonPcieDevCap {
                    max_payload: pcie.dev_cap.max_payload,
                    phantom_functions: pcie.dev_cap.phantom_functions,
                    extended_tag: pcie.dev_cap.extended_tag,
                    l0s_latency: pcie.dev_cap.l0s_latency,
                    l1_latency: pcie.dev_cap.l1_latency,
                    role_based_error: pcie.dev_cap.role_based_error,
                    attention_button: pcie.dev_cap.attention_button,
                    attention_indicator: pcie.dev_cap.attention_indicator,
                    power_indicator: pcie.dev_cap.power_indicator,
                    flreset: pcie.dev_cap.flreset,
                    slot_power_limit: pcie.dev_cap.slot_power_limit,
                    slot_power_limit_scale: pcie.dev_cap.slot_power_limit_scale,
                },
                dev_ctl: JsonPcieDevCtl {
                    corr_err: pcie.dev_ctl.corr_err,
                    non_fatal_err: pcie.dev_ctl.non_fatal_err,
                    fatal_err: pcie.dev_ctl.fatal_err,
                    unsup_req: pcie.dev_ctl.unsup_req,
                    relaxed_ordering: pcie.dev_ctl.relaxed_ordering,
                    max_payload: pcie.dev_ctl.max_payload,
                    extended_tag: pcie.dev_ctl.extended_tag,
                    phantom_functions: pcie.dev_ctl.phantom_functions,
                    aux_power: pcie.dev_ctl.aux_power,
                    no_snoop: pcie.dev_ctl.no_snoop,
                    max_read_req: pcie.dev_ctl.max_read_req,
                    bridge_config_retry: pcie.dev_ctl.bridge_config_retry,
                },
                dev_sta: JsonPcieDevSta {
                    corr_err: pcie.dev_sta.corr_err,
                    non_fatal_err: pcie.dev_sta.non_fatal_err,
                    fatal_err: pcie.dev_sta.fatal_err,
                    unsup_req: pcie.dev_sta.unsup_req,
                    aux_power: pcie.dev_sta.aux_power,
                    trans_pending: pcie.dev_sta.trans_pending,
                },
                lnk_cap: JsonPcieLnkCap {
                    max_speed: pcie.lnk_cap.max_speed,
                    max_width: pcie.lnk_cap.max_width,
                    aspm: pcie.lnk_cap.aspm,
                    l0s_exit_latency: pcie.lnk_cap.l0s_exit_latency,
                    l1_exit_latency: pcie.lnk_cap.l1_exit_latency,
                    clock_pm: pcie.lnk_cap.clock_pm,
                    surprise_down: pcie.lnk_cap.surprise_down,
                    dll_active: pcie.lnk_cap.dll_active,
                    link_bw_notif: pcie.lnk_cap.link_bw_notif,
                    aspm_opt_compliance: pcie.lnk_cap.aspm_opt_compliance,
                    port_number: pcie.lnk_cap.port_number,
                },
                lnk_ctl: JsonPcieLnkCtl {
                    aspm: pcie.lnk_ctl.aspm,
                    rcb: pcie.lnk_ctl.rcb,
                    link_disable: pcie.lnk_ctl.link_disable,
                    retrain: pcie.lnk_ctl.retrain,
                    common_clock: pcie.lnk_ctl.common_clock,
                    extended_synch: pcie.lnk_ctl.extended_synch,
                    clock_pm: pcie.lnk_ctl.clock_pm,
                    autonomous_width_disable: pcie.lnk_ctl.autonomous_width_disable,
                    bw_interrupt: pcie.lnk_ctl.bw_interrupt,
                    autonomous_bw_interrupt: pcie.lnk_ctl.autonomous_bw_interrupt,
                },
                lnk_sta: JsonPcieLnkSta {
                    speed: pcie.lnk_sta.speed,
                    width: pcie.lnk_sta.width,
                    downgraded,
                    tr_err: pcie.lnk_sta.tr_err,
                    training: pcie.lnk_sta.training,
                    slot_clock: pcie.lnk_sta.slot_clock,
                    dll_active: pcie.lnk_sta.dll_active,
                    bw_management: pcie.lnk_sta.bw_management,
                    autonomous_bw: pcie.lnk_sta.autonomous_bw,
                },
                slot_cap: pcie.slot_cap.as_ref().map(|slot_cap| JsonPcieSlotCap {
                    attention_button: slot_cap.attention_button,
                    power_controller: slot_cap.power_controller,
                    mrl: slot_cap.mrl,
                    attention_indicator: slot_cap.attention_indicator,
                    power_indicator: slot_cap.power_indicator,
                    hotplug_surprise: slot_cap.hotplug_surprise,
                    hotplug_capable: slot_cap.hotplug_capable,
                    slot_power_limit: slot_cap.slot_power_limit,
                    slot_power_limit_scale: slot_cap.slot_power_limit_scale,
                    electromechanical: slot_cap.electromechanical,
                    no_command_completed: slot_cap.no_command_completed,
                    physical_slot_number: slot_cap.physical_slot_number,
                }),
                slot_ctl: pcie.slot_ctl.as_ref().map(|slot_ctl| JsonPcieSlotCtl {
                    attention_button_enable: slot_ctl.attention_button_enable,
                    power_fault_detect: slot_ctl.power_fault_detect,
                    mrl_sensor: slot_ctl.mrl_sensor,
                    presence_detect: slot_ctl.presence_detect,
                    command_completed: slot_ctl.command_completed,
                    hotplug_interrupt: slot_ctl.hotplug_interrupt,
                    attention_indicator: slot_ctl.attention_indicator,
                    power_indicator: slot_ctl.power_indicator,
                    power_controller_control: slot_ctl.power_controller_control,
                    power_interlock: slot_ctl.power_interlock,
                    dll_state_changed: slot_ctl.dll_state_changed,
                }),
                slot_sta: pcie.slot_sta.as_ref().map(|slot_sta| JsonPcieSlotSta {
                    attention_button: slot_sta.attention_button,
                    power_fault: slot_sta.power_fault,
                    mrl_sensor: slot_sta.mrl_sensor,
                    presence_detect: slot_sta.presence_detect,
                    command_completed: slot_sta.command_completed,
                    mrl_state: slot_sta.mrl_state,
                    presence_state: slot_sta.presence_state,
                    power_interlock: slot_sta.power_interlock,
                    dll_state: slot_sta.dll_state,
                }),
                root_ctl: pcie.root_ctl.as_ref().map(|root_ctl| JsonPcieRootCtl {
                    serr_corr: root_ctl.serr_corr,
                    serr_non_fatal: root_ctl.serr_non_fatal,
                    serr_fatal: root_ctl.serr_fatal,
                    pme_interrupt: root_ctl.pme_interrupt,
                    crs_visible: root_ctl.crs_visible,
                }),
                root_sta: pcie.root_sta.as_ref().map(|root_sta| JsonPcieRootSta {
                    pme_requester_id: format!("0x{:04x}", root_sta.pme_requester_id),
                    pme_status: root_sta.pme_status,
                    pme_pending: root_sta.pme_pending,
                }),
                dev_cap2: pcie.dev_cap2.as_ref().map(|dev_cap2| JsonPcieDevCap2 {
                    completion_timeout_ranges: dev_cap2.completion_timeout_ranges,
                    completion_timeout_disable: dev_cap2.completion_timeout_disable,
                    ari: dev_cap2.ari,
                    atomic_op_routing: dev_cap2.atomic_op_routing,
                    atomic_32: dev_cap2.atomic_32,
                    atomic_64: dev_cap2.atomic_64,
                    atomic_128_cas: dev_cap2.atomic_128_cas,
                    no_ro_pr_pr_passing: dev_cap2.no_ro_pr_pr_passing,
                    ltr: dev_cap2.ltr,
                    tph_completer: dev_cap2.tph_completer,
                    ten_bit_tag_completer: dev_cap2.ten_bit_tag_completer,
                    ten_bit_tag_requester: dev_cap2.ten_bit_tag_requester,
                    obff: dev_cap2.obff,
                    ext_fmt: dev_cap2.ext_fmt,
                    ee_tlp_prefix: dev_cap2.ee_tlp_prefix,
                }),
                dev_ctl2: pcie.dev_ctl2.as_ref().map(|dev_ctl2| JsonPcieDevCtl2 {
                    completion_timeout: dev_ctl2.completion_timeout,
                    completion_timeout_disable: dev_ctl2.completion_timeout_disable,
                    ari: dev_ctl2.ari,
                    atomic_op_requester: dev_ctl2.atomic_op_requester,
                    atomic_op_egress_blocking: dev_ctl2.atomic_op_egress_blocking,
                    ido_request: dev_ctl2.ido_request,
                    ido_completion: dev_ctl2.ido_completion,
                    ltr: dev_ctl2.ltr,
                    ten_bit_tag_requester: dev_ctl2.ten_bit_tag_requester,
                    obff: dev_ctl2.obff,
                    ee_tlp_prefix_blocking: dev_ctl2.ee_tlp_prefix_blocking,
                }),
                lnk_ctl2: pcie.lnk_ctl2.as_ref().map(|lnk_ctl2| JsonPcieLnkCtl2 {
                    target_speed: lnk_ctl2.target_speed,
                    compliance_de_emphasis: lnk_ctl2.compliance_de_emphasis,
                    transmit_margin: lnk_ctl2.transmit_margin,
                    enter_modified_compliance: lnk_ctl2.enter_modified_compliance,
                    compliance_sos: lnk_ctl2.compliance_sos,
                    compliance_preset: lnk_ctl2.compliance_preset,
                }),
                lnk_sta2: pcie.lnk_sta2.as_ref().map(|lnk_sta2| JsonPcieLnkSta2 {
                    current_de_emphasis: lnk_sta2.current_de_emphasis,
                    equalization_complete: lnk_sta2.equalization_complete,
                    equalization_phase1: lnk_sta2.equalization_phase1,
                    equalization_phase2: lnk_sta2.equalization_phase2,
                    equalization_phase3: lnk_sta2.equalization_phase3,
                    equalization_request: lnk_sta2.equalization_request,
                    retimer_presence: lnk_sta2.retimer_presence,
                    crosslink_resolution: lnk_sta2.crosslink_resolution,
                }),
            })
        }
```

- [ ] **Step 4: Verify and commit**

```bash
cargo fmt --all && cargo fmt --all -- --check
cargo check --workspace --target x86_64-unknown-linux-gnu
cargo run -q -p lspci-rs --target x86_64-unknown-linux-gnu -- show 0000:00:05.0 --format text
cargo run -q -p lspci-rs --target x86_64-unknown-linux-gnu -- show 0000:00:05.0 --format json
```

Expected (myece): extended chain remains `unavailable: ReadError`; no panic; standard output unchanged.

```bash
git add crates/lspci-rs/src/output.rs
git commit -m "cli: render full PCIe capability register groups"
```

---

### Task 3: Real-hardware calibration and finish

**Files:** none (verification only), plus progress doc.

**Interfaces:**
- Consumes: completed branch binary; sg-232e-224 and dev48 access.
- Produces: calibrated bit layouts, comparison evidence, handoff doc.

- [ ] **Step 1: Build and transfer to sg-232e-224**

```bash
# in container
cd /workspace && cargo build -p lspci-rs --target x86_64-unknown-linux-gnu
# on myece host
podman cp 95c90e05ab1a:/workspace/target/x86_64-unknown-linux-gnu/debug/lspci-rs /tmp/lspci-rs
# locally
sftp myece <<< "get /tmp/lspci-rs <local-staging-path>"
sftp sg-232e-224 <<< "put <local-staging-path> /tmp/lspci-rs"
ssh sg-232e-224 'sudo chmod +x /tmp/lspci-rs'
```

- [ ] **Step 2: Calibrate on the X710 endpoint (v2)**

```bash
ssh sg-232e-224 'sudo /tmp/lspci-rs show 0000:3d:00.0 --format text'
ssh sg-232e-224 'sudo lspci -s 3d:00.0 -vv'
```

Compare every line of the Express block: DevCap/DevCtl/DevSta, LnkCap/LnkCtl/LnkSta (speeds, widths, downgraded note), DevCap2/DevCtl2/LnkCtl2/LnkSta2. For any mismatch, dump the raw registers (`sudo dd if=/sys/bus/pci/devices/0000:3d:00.0/config bs=1 skip=$((<cap-offset>)) count=64 | od -An -tx1`), fix the bit extraction in `decoders/pcie.rs`, rebuild, re-transfer, re-compare. Repeat until all lines agree.

- [ ] **Step 3: Calibrate on a root port**

```bash
ssh sg-232e-224 'sudo lspci -vvv | grep -B6 "Root Port" | grep -E "^[0-9a-f]+:" | head -3'
```

Pick one root port, then:

```bash
ssh sg-232e-224 'sudo /tmp/lspci-rs show <addr> --format text'
ssh sg-232e-224 'sudo lspci -s <addr-short> -vv'
```

Compare SlotCap/SlotCtl/SlotSta and RootCtl/RootSta lines; fix and re-verify as in Step 2.

- [ ] **Step 4: Auxiliary check on dev48**

Transfer the final binary to dev48 via the same sftp chain, then compare one endpoint:

```bash
ssh dev48 'sudo /tmp/lspci-rs show 0000:00:05.0 --format text'
ssh dev48 'sudo lspci -s 00:05.0 -vv'
```

Note: dev48 lspci is vendor-patched (may omit some lines); use it as an auxiliary reference only.

- [ ] **Step 5: Regression on myece**

```bash
cd /workspace
cargo fmt --all -- --check
cargo check --workspace --target x86_64-unknown-linux-gnu
cargo run -q -p lspci-rs --target x86_64-unknown-linux-gnu -- list --format text | wc -l
cargo run -q -p lspci-rs --target x86_64-unknown-linux-gnu -- show 0000:00:05.0 --config standard --format text
git diff --check
```

Expected: 9 devices; config dump and capability outputs unchanged; extended chain still `unavailable: ReadError`.

- [ ] **Step 6: Record the handoff**

Create `docs/superpowers/progress/2026-08-10-pcie-full-decoding-progress.md` recording: commit list, calibration devices, every bit-layout fix made, and per-register comparison results. Commit:

```bash
git add docs/superpowers/progress/2026-08-10-pcie-full-decoding-progress.md
git commit -m "docs: record PCIe full decoding validation results"
```

- [ ] **Step 7: Finish the branch**

Use superpowers:finishing-a-development-branch to merge `sdd/pcie-full-decoding` into `main` (or follow the user's chosen option).
