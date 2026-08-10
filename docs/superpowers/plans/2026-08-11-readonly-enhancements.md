# Read-Only Decoder Enhancements Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add four read-only decoder enhancements: LnkCap2 decode, latency/timeout ns/µs text, AER root register group, SR-IOV VF BAR decode.

**Architecture:** Extend the existing `decoders/pcie.rs`, `decoders/aer.rs`, `decoders/sriov.rs` in place; new typed fields flow through the existing content enum variants; renderers gain the new lines. No new capability types, no FFI changes.

**Tech Stack:** Rust 2024 workspace, pure decoding over `ConfigSpaceSnapshot`, serde. Build in container `95c90e05ab1a` on host `myece` (`/workspace`); validate on sg-232e-224 (X710 v2 endpoint + root port), dev48 auxiliary, myece regression.

## Global Constraints

- No unit tests (user decision); verification is `cargo fmt --check` + `cargo check` + real-hardware comparison.
- Decode code contains zero FFI; decode failure yields `None`/`Unavailable` and never fails `inspect()`.
- Config-space writes are out of scope; `list` behavior unchanged; no new dependencies.
- Bit layouts are PCI-express-spec starting points; Task 5 calibrates them against sg-232e-224 `lspci -vvv` output and fixes mismatches.
- Verification commands run inside the container: `ssh myece 'docker exec 95c90e05ab1a bash -lc "cd /workspace && <cmd>"'`.
- Binary transfer chain (sftp only; scp is killed): build in container → on myece `podman cp 95c90e05ab1a:/workspace/target/x86_64-unknown-linux-gnu/debug/lspci-rs /tmp/lspci-rs` → locally `sftp myece <<< "get /tmp/lspci-rs <local>"` → `sftp sg-232e-224 <<< "put <local> /tmp/lspci-rs"` (same for dev48) → on target `sudo chmod +x /tmp/lspci-rs`.
- Branch `sdd/readonly-enhancements` from `main`; finish via finishing-a-development-branch.

---

### Task 0: Create the feature branch

- [ ] **Step 1: Create and switch branch**

```bash
cd /workspace && git checkout main && git checkout -b sdd/readonly-enhancements
```

---

### Task 1: LnkCap2 decode and rendering

**Files:**
- Modify: `crates/pci/src/decoders/pcie.rs`
- Modify: `crates/lspci-rs/src/output.rs`

**Interfaces:**
- Consumes: existing `PcieCapability`, `decode_pcie` v2 block (reads at cap base +0x24/0x28/0x30/0x32).
- Produces: `PcieLinkCap2` struct and `PcieCapability.lnk_cap2: Option<PcieLinkCap2>`; renderer line `LnkCap2: ...`.

- [ ] **Step 1: Add the struct in `pcie.rs`**

Add after the `PcieLinkSta2` struct:

```rust
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PcieLinkCap2 {
    /// Supported Link Speeds Vector, bits 7-1 of the register
    pub supported_speeds: u8,
    pub crosslink: bool,
    pub retimer_supported: bool,
    pub two_retimers_supported: bool,
    pub drs_supported: bool,
}
```

Add the field to `PcieCapability` (after `pub lnk_sta2: Option<PcieLinkSta2>,`):

```rust
    pub lnk_cap2: Option<PcieLinkCap2>,
```

- [ ] **Step 2: Decode LnkCap2 in the v2 block**

In `decode_pcie`, the v2 block currently reads four registers into `(dev_cap2, dev_ctl2, lnk_ctl2, lnk_sta2)`. Extend it to also read LnkCap2 at `base + 0x2c` and return it in the tuple:

```rust
        let lnk_cap2_raw = read_dword(snapshot, base + 0x2c).ok()?;
        let lnk_cap2 = PcieLinkCap2 {
            supported_speeds: ((lnk_cap2_raw >> 1) & 0x0000_007f) as u8,
            crosslink: lnk_cap2_raw & 0x0000_0001 != 0,
            retimer_supported: lnk_cap2_raw & 0x0000_0100 != 0,
            two_retimers_supported: lnk_cap2_raw & 0x0000_0200 != 0,
            drs_supported: lnk_cap2_raw & 0x0000_0400 != 0,
        };
```

Change the v2 block's final tuple to `(Some(dev_cap2), Some(dev_ctl2), Some(lnk_ctl2), Some(lnk_sta2), Some(lnk_cap2))` and the else branch to `(None, None, None, None, None)`; destructure into `let (dev_cap2, dev_ctl2, lnk_ctl2, lnk_sta2, lnk_cap2) = ...` and pass `lnk_cap2` into the `PcieCapability` construction.

- [ ] **Step 3: Render the LnkCap2 text line in `output.rs`**

Add this helper next to `render_pcie_speed`:

```rust
fn render_supported_speeds(vector: u8) -> String {
    let speeds: [(u8, &str); 6] = [
        (0x01, "2.5"),
        (0x02, "5"),
        (0x04, "8"),
        (0x08, "16"),
        (0x10, "32"),
        (0x20, "64"),
    ];
    let names: Vec<&str> = speeds
        .iter()
        .filter(|(bit, _)| vector & bit != 0)
        .map(|(_, name)| *name)
        .collect();
    if names.is_empty() {
        "unknown".to_owned()
    } else {
        format!("{}GT/s", names.join("-"))
    }
}
```

In `render_pcie_text`, after the LnkSta2 block (the `if let ... lnk_sta2` block), append:

```rust
    if let Some(lnk_cap2) = &pcie.lnk_cap2 {
        output.push_str(&format!(
            "\n          LnkCap2: Supported Link Speeds: {}, Crosslink{} Retimer{} 2Retimers{} DRS{}",
            render_supported_speeds(lnk_cap2.supported_speeds),
            pcie_flag(lnk_cap2.crosslink),
            pcie_flag(lnk_cap2.retimer_supported),
            pcie_flag(lnk_cap2.two_retimers_supported),
            pcie_flag(lnk_cap2.drs_supported)
        ));
    }
```

- [ ] **Step 4: Add JSON support**

Add to `JsonPcie` (after `lnk_sta2`):

```rust
    #[serde(skip_serializing_if = "Option::is_none")]
    lnk_cap2: Option<JsonPcieLnkCap2>,
```

Add the struct next to `JsonPcieLnkSta2`:

```rust
#[derive(Debug, Serialize)]
struct JsonPcieLnkCap2 {
    supported_speeds: String,
    crosslink: bool,
    retimer_supported: bool,
    two_retimers_supported: bool,
    drs_supported: bool,
}
```

In the `JsonPcie` construction (the `PciCapabilityContent::Pcie` mapping arm), after `lnk_sta2: ...`, add:

```rust
                lnk_cap2: pcie.lnk_cap2.as_ref().map(|lnk_cap2| JsonPcieLnkCap2 {
                    supported_speeds: render_supported_speeds(lnk_cap2.supported_speeds),
                    crosslink: lnk_cap2.crosslink,
                    retimer_supported: lnk_cap2.retimer_supported,
                    two_retimers_supported: lnk_cap2.two_retimers_supported,
                    drs_supported: lnk_cap2.drs_supported,
                }),
```

- [ ] **Step 5: Verify and commit**

```bash
cargo fmt --all && cargo fmt --all -- --check
cargo check --workspace --target x86_64-unknown-linux-gnu
git add crates/pci/src/decoders/pcie.rs crates/lspci-rs/src/output.rs
git commit -m "pci: decode LnkCap2 supported speeds and feature bits"
```

---

### Task 2: Latency encoding text

**Files:**
- Modify: `crates/lspci-rs/src/output.rs`

**Interfaces:**
- Consumes: existing `dev_cap.l0s_latency`, `dev_cap.l1_latency`, `lnk_cap.l0s_exit_latency`, `lnk_cap.l1_exit_latency` (raw 3-bit codes).
- Produces: ns/µs text in the DevCap and LnkCap lines. JSON keeps the raw codes.

- [ ] **Step 1: Add encoding helpers**

Add next to `render_supported_speeds`:

```rust
fn pcie_l0s_latency(code: u8) -> &'static str {
    match code {
        0 => "<64ns",
        1 => "<128ns",
        2 => "<256ns",
        3 => "<512ns",
        4 => "<1us",
        5 => "<2us",
        6 => "<4us",
        _ => ">4us",
    }
}

fn pcie_l1_latency(code: u8) -> &'static str {
    match code {
        0 => "<1us",
        1 => "<2us",
        2 => "<4us",
        3 => "<8us",
        4 => "<16us",
        5 => "<32us",
        6 => "<64us",
        _ => ">64us",
    }
}
```

- [ ] **Step 2: Use them in the DevCap and LnkCap lines**

In `render_pcie_text`, the DevCap line currently formats `dev_cap.l0s_latency` and `dev_cap.l1_latency` directly; replace those two arguments with `pcie_l0s_latency(dev_cap.l0s_latency)` and `pcie_l1_latency(dev_cap.l1_latency)`.

The LnkCap line currently formats `lnk_cap.l0s_exit_latency` and `lnk_cap.l1_exit_latency` directly; replace those two arguments with `pcie_l0s_latency(lnk_cap.l0s_exit_latency)` and `pcie_l1_latency(lnk_cap.l1_exit_latency)`.

- [ ] **Step 3: Verify and commit**

```bash
cargo fmt --all && cargo fmt --all -- --check
cargo check --workspace --target x86_64-unknown-linux-gnu
git add crates/lspci-rs/src/output.rs
git commit -m "cli: render PCIe latencies as encoded ns/us text"
```

---

### Task 3: AER root register group

**Files:**
- Modify: `crates/pci/src/decoders/aer.rs`
- Modify: `crates/lspci-rs/src/output.rs`

**Interfaces:**
- Consumes: existing `AerCapability` with `root_command/root_status/error_source_id: Option<u32>`.
- Produces: `AerRootGroup` struct replacing those three fields; expanded text/JSON rendering.

- [ ] **Step 1: Restructure the root fields in `aer.rs`**

Replace the three fields `pub root_command: Option<u32>, pub root_status: Option<u32>, pub error_source_id: Option<u32>,` in `AerCapability` with:

```rust
    pub root: Option<AerRootGroup>,
```

Add the struct above `AerCapability`:

```rust
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AerRootGroup {
    pub command: u32,
    pub status: u32,
    pub error_source_id: u32,
}
```

In `decode_aer`, replace the bridge-conditional block that fills the three Options with:

```rust
    let root = if is_bridge {
        let command = read_dword(snapshot, base + 0x2c).ok()?;
        let status = read_dword(snapshot, base + 0x30).ok()?;
        let error_source_id = read_dword(snapshot, base + 0x34).ok()?;
        Some(AerRootGroup {
            command,
            status,
            error_source_id,
        })
    } else {
        None
    };
```

and pass `root` into the `AerCapability` construction.

- [ ] **Step 2: Render the expanded root group in `output.rs`**

Replace the existing single-line RootCmd block inside `render_aer_text`:

```rust
    if let (Some(command), Some(status), Some(source)) =
        (&aer.root_command, &aer.root_status, &aer.error_source_id)
    {
        output.push_str(&format!(
            "\n          RootCmd: 0x{command:08x} RootSta: 0x{status:08x} ErrSrc: 0x{source:08x}"
        ));
    }
```

with:

```rust
    if let Some(root) = &aer.root {
        let flag = |word: u32, bit: u32| if word & bit != 0 { "+" } else { "-" };
        output.push_str(&format!(
            "\n          RootErrCmd: CorrErr{} NonFatalErr{} FatalErr{}",
            flag(root.command, 0x0000_0001),
            flag(root.command, 0x0000_0002),
            flag(root.command, 0x0000_0004),
        ));
        let status = root.status;
        output.push_str(&format!(
            "\n          RootErrSta: ErrCor{} MultErrCor{} NonFatalErr{} MultNonFatal{} FirstUEFatal{} FatalErr{} MultFatal{} AdvErrInt=0x{:02x}",
            flag(status, 0x0000_0001),
            flag(status, 0x0000_0002),
            flag(status, 0x0000_0004),
            flag(status, 0x0000_0008),
            flag(status, 0x0000_0010),
            flag(status, 0x0000_0020),
            flag(status, 0x0000_0040),
            (status >> 27) & 0x1f,
        ));
        output.push_str(&format!(
            "\n          ErrSrc: CE=0x{:04x} NFFatal=0x{:04x}",
            root.error_source_id & 0x0000_ffff,
            (root.error_source_id >> 16) & 0x0000_ffff,
        ));
    }
```

- [ ] **Step 3: Update the JSON mapping**

In `output.rs`, find the JSON mapping for the AER capability (the `PciCapabilityContent::Aer(aer)` mapping arm) and replace the three fields `root_command`, `root_status`, `error_source_id` with:

```rust
            root: aer.root.as_ref().map(|root| JsonAerRoot {
                command: format!("0x{:08x}", root.command),
                status: format!("0x{:08x}", root.status),
                ce_source_id: format!("0x{:04x}", root.error_source_id & 0x0000_ffff),
                nf_fatal_source_id: format!("0x{:04x}", (root.error_source_id >> 16) & 0x0000_ffff),
            }),
```

Replace the corresponding fields in `JsonAer` with:

```rust
    #[serde(skip_serializing_if = "Option::is_none")]
    root: Option<JsonAerRoot>,
```

and add the struct:

```rust
#[derive(Debug, Serialize)]
struct JsonAerRoot {
    command: String,
    status: String,
    ce_source_id: String,
    nf_fatal_source_id: String,
}
```

- [ ] **Step 4: Verify and commit**

```bash
cargo fmt --all && cargo fmt --all -- --check
cargo check --workspace --target x86_64-unknown-linux-gnu
git add crates/pci/src/decoders/aer.rs crates/lspci-rs/src/output.rs
git commit -m "pci: expand AER root error register group"
```

---

### Task 4: SR-IOV VF BAR decode

**Files:**
- Modify: `crates/pci/src/decoders/sriov.rs`
- Modify: `crates/lspci-rs/src/output.rs`

**Interfaces:**
- Consumes: existing `SriovCapability` with `vf_bars: [u32; 6]` and `migration_state_array_offset: u32`.
- Produces: typed `SriovVfBar` entries, `migration_state_array_size`, expanded text/JSON rendering.

- [ ] **Step 1: Add types and rework the VF BAR decode in `sriov.rs`**

Replace `pub vf_bars: [u32; 6],` in `SriovCapability` with:

```rust
    pub vf_bars: [Option<SriovVfBar>; 6],
    pub migration_state_array_size: u32,
```

Add above `SriovCapability`:

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SriovVfBarKind {
    Io,
    Memory,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SriovVfBar {
    pub kind: SriovVfBarKind,
    pub is_64_bit: bool,
    pub prefetchable: bool,
    pub address: u64,
}
```

In `decode_sriov`, replace the existing `vf_bars` loop and the `migration_state_array_offset` line with:

```rust
    let mut raw_bars = [0u32; 6];
    for (index, bar) in raw_bars.iter_mut().enumerate() {
        *bar = read_dword(snapshot, base + 0x24 + (index as u32) * 4).ok()?;
    }

    let mut vf_bars: [Option<SriovVfBar>; 6] = Default::default();
    let mut index = 0;
    while index < 6 {
        let raw = raw_bars[index];
        if raw & 0x1 != 0 {
            vf_bars[index] = Some(SriovVfBar {
                kind: SriovVfBarKind::Io,
                is_64_bit: false,
                prefetchable: false,
                address: u64::from(raw & 0xffff_fffc),
            });
            index += 1;
        } else {
            let is_64_bit = (raw >> 1) & 0x3 == 0x2;
            let prefetchable = raw & 0x8 != 0;
            let mut address = u64::from(raw & 0xffff_fff0);
            if is_64_bit && index + 1 < 6 {
                address |= u64::from(raw_bars[index + 1]) << 32;
                vf_bars[index] = Some(SriovVfBar {
                    kind: SriovVfBarKind::Memory,
                    is_64_bit,
                    prefetchable,
                    address,
                });
                index += 2;
            } else {
                vf_bars[index] = Some(SriovVfBar {
                    kind: SriovVfBarKind::Memory,
                    is_64_bit,
                    prefetchable,
                    address,
                });
                index += 1;
            }
        }
    }

    let migration_state_array_offset = read_dword(snapshot, base + 0x40).ok()?;
    let migration_state_array_size = read_dword(snapshot, base + 0x44).ok()?;
```

Pass both new values into the `SriovCapability` construction.

- [ ] **Step 2: Render VF regions in `output.rs`**

Replace the existing single-line `PciCapabilityContent::Sriov(sriov) => format!(...)` arm with:

```rust
        PciCapabilityContent::Sriov(sriov) => {
            let mut text = format!(
                "initial_vfs={} total_vfs={} num_vfs={} vf_offset={} vf_stride={} vf_device_id=0x{:04x} control=0x{:04x}",
                sriov.initial_vfs,
                sriov.total_vfs,
                sriov.num_vfs,
                sriov.vf_offset,
                sriov.vf_stride,
                sriov.vf_device_id,
                sriov.control
            );
            for (index, bar) in sriov.vf_bars.iter().enumerate() {
                if let Some(bar) = bar {
                    let description = match bar.kind {
                        SriovVfBarKind::Io => format!("io at 0x{:x}", bar.address),
                        SriovVfBarKind::Memory => format!(
                            "memory-{}{} at 0x{:x}",
                            if bar.is_64_bit { "64" } else { "32" },
                            if bar.prefetchable { "-prefetch" } else { "" },
                            bar.address
                        ),
                    };
                    text.push_str(&format!("\n          VF Region {index}: {description}"));
                }
            }
            text.push_str(&format!(
                "\n          VF Migration: offset=0x{:08x} size=0x{:x}",
                sriov.migration_state_array_offset, sriov.migration_state_array_size
            ));
            text
        }
```

Add `SriovVfBarKind` to the `use pci::{...}` import list in output.rs.

- [ ] **Step 3: Update the JSON mapping**

In `JsonSriov`, replace `vf_bars: Vec<String>,` with:

```rust
    vf_bars: Vec<JsonSriovVfBar>,
    migration_state_array_size: String,
```

Add the struct:

```rust
#[derive(Debug, Serialize)]
struct JsonSriovVfBar {
    index: usize,
    kind: String,
    is_64_bit: bool,
    prefetchable: bool,
    address: String,
}
```

In the `JsonSriov` construction (inside the SR-IOV mapping arm), replace the `vf_bars` mapping with:

```rust
            vf_bars: sriov
                .vf_bars
                .iter()
                .enumerate()
                .filter_map(|(index, bar)| {
                    bar.map(|bar| JsonSriovVfBar {
                        index,
                        kind: match bar.kind {
                            SriovVfBarKind::Io => "io".to_owned(),
                            SriovVfBarKind::Memory => "memory".to_owned(),
                        },
                        is_64_bit: bar.is_64_bit,
                        prefetchable: bar.prefetchable,
                        address: format!("0x{:x}", bar.address),
                    })
                })
                .collect(),
            migration_state_array_size: format!("0x{:x}", sriov.migration_state_array_size),
```

- [ ] **Step 4: Verify and commit**

```bash
cargo fmt --all && cargo fmt --all -- --check
cargo check --workspace --target x86_64-unknown-linux-gnu
git add crates/pci/src/decoders/sriov.rs crates/lspci-rs/src/output.rs
git commit -m "pci: decode SR-IOV VF BAR types and migration size"
```

---

### Task 5: Real-hardware validation and finish

**Files:** none (verification only), plus progress doc.

**Interfaces:**
- Consumes: completed branch binary; sg-232e-224 and dev48 access.
- Produces: comparison evidence, handoff doc.

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

- [ ] **Step 2: Validate LnkCap2 and latency text on the X710 endpoint**

```bash
ssh sg-232e-224 'sudo /tmp/lspci-rs show 0000:3d:00.0 --format text'
ssh sg-232e-224 'sudo lspci -s 3d:00.0 -vv'
```

Compare: the `LnkCap2: Supported Link Speeds: 2.5-8GT/s, Crosslink- Retimer- 2Retimers- DRS-` line against lspci's LnkCap2 line; `Latency L0s <512ns, L1 <64us` and `Exit Latency L0s ..., L1 <16us` against lspci. Fix bit positions in pcie.rs/output.rs if mismatched (DRS bit is a known calibration point).

- [ ] **Step 3: Validate the AER root group on a root port**

```bash
ssh sg-232e-224 'sudo /tmp/lspci-rs show 0000:00:08.0 --format text'
ssh sg-232e-224 'sudo lspci -s 00:08.0 -vvv'
```

Compare RootErrCmd/RootErrSta/ErrSrc lines against lspci's AER output for the root port. Fix bit positions in the Task 3 rendering if mismatched.

- [ ] **Step 4: Validate SR-IOV VF regions**

```bash
ssh sg-232e-224 'sudo /tmp/lspci-rs show 0000:3d:00.0 --format text'
ssh sg-232e-224 'sudo lspci -s 3d:00.0 -vvv | grep -A9 "Single Root"'
```

Compare VF Region lines against lspci's `Region 0: Memory at 00000d3fff410000 (64-bit, prefetchable)` and `Region 3: Memory at 00000d3ffe000000`. VF BAR 0 and 3 should match; consumed upper slots (BAR1, BAR4) must not appear as separate regions.

- [ ] **Step 5: Auxiliary check on dev48 and regression on myece**

Transfer the same binary to dev48; compare one PCIe endpoint's latency lines. Then on myece:

```bash
cd /workspace
cargo fmt --all -- --check
cargo check --workspace --target x86_64-unknown-linux-gnu
cargo run -q -p lspci-rs --target x86_64-unknown-linux-gnu -- list --format text | wc -l
cargo run -q -p lspci-rs --target x86_64-unknown-linux-gnu -- show 0000:00:05.0 --format text
git diff --check
```

Expected: 9 devices; extended chain remains `unavailable: ReadError`; no regressions.

- [ ] **Step 6: Record the handoff**

Create `docs/superpowers/progress/2026-08-11-readonly-enhancements-progress.md` recording: commit list, per-enhancement comparison results, every calibration fix made. Commit:

```bash
git add docs/superpowers/progress/2026-08-11-readonly-enhancements-progress.md
git commit -m "docs: record read-only enhancements validation results"
```

- [ ] **Step 7: Finish the branch**

Use superpowers:finishing-a-development-branch to merge `sdd/readonly-enhancements` into `main` (or follow the user's chosen option).
