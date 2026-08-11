# CLI Enhancement Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add semantic color output (TTY auto-detect + `--color auto|always|never`) and a `tree` subcommand rendering the device topology in lspci -t style, with zero new dependencies.

**Architecture:** New `color.rs` module holds `ColorMode` + a `Palette` of semantic paint helpers; render functions in `output.rs` gain a `ColorMode` parameter. New `tree.rs` builds the topology from `scan()` plus per-bridge bus-window reads (0x19/0x1a) and renders an lspci -t style tree. JSON output stays uncolored.

**Tech Stack:** Rust 2024 workspace, clap, serde, `std::io::IsTerminal` (no new deps). Build in container `95c90e05ab1a` on host `myece` (`/workspace`); validate on myece (TTY/no-TTY/color flags) and dev48 (topology vs `sudo lspci -t`).

## Global Constraints

- No new dependencies; JSON output never colored; list/show text content semantics unchanged (colors only).
- Verification commands run inside the container: `ssh myece 'docker exec 95c90e05ab1a bash -lc "cd /workspace && <cmd>"'`.
- Binary transfer chain (sftp only; scp is killed): build in container → on myece `podman cp 95c90e05ab1a:/workspace/target/x86_64-unknown-linux-gnu/debug/lspci-rs /tmp/lspci-rs` → locally `sftp myece <<< "get /tmp/lspci-rs <local>"` → `sftp dev48 <<< "put <local> /tmp/lspci-rs"` → on target `sudo chmod +x /tmp/lspci-rs`.
- Branch `sdd/cli-enhancement` from `main`; finish via finishing-a-development-branch.
- No unit tests (user decision); verification is `cargo fmt --check` + `cargo check` + real runs.

---

### Task 0: Create the feature branch

- [ ] **Step 1: Create and switch branch**

```bash
cd /workspace && git checkout main && git checkout -b sdd/cli-enhancement
```

---

### Task 1: Color module and global --color flag

**Files:**
- Create: `crates/lspci-rs/src/color.rs`
- Modify: `crates/lspci-rs/src/main.rs`
- Modify: `crates/lspci-rs/src/cli.rs`

**Interfaces:**
- Produces: `ColorMode` (clap ValueEnum) and `Palette` consumed by Tasks 2–3.

- [ ] **Step 1: Create `crates/lspci-rs/src/color.rs`**

```rust
use clap::ValueEnum;
use std::io::IsTerminal;

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum ColorMode {
    Auto,
    Always,
    Never,
}

impl ColorMode {
    pub fn enabled(self) -> bool {
        match self {
            ColorMode::Auto => std::io::stdout().is_terminal(),
            ColorMode::Always => true,
            ColorMode::Never => false,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Palette {
    pub enabled: bool,
}

impl Palette {
    pub fn new(mode: ColorMode) -> Self {
        Self {
            enabled: mode.enabled(),
        }
    }

    fn paint(&self, code: &str, text: &str) -> String {
        if self.enabled {
            format!("\x1b[{code}m{text}\x1b[0m")
        } else {
            text.to_owned()
        }
    }

    /// device addresses — cyan bold
    pub fn address(&self, text: &str) -> String {
        self.paint("1;36", text)
    }

    /// field labels, IDs, offsets, disabled markers — dim
    pub fn dim(&self, text: &str) -> String {
        self.paint("2", text)
    }

    /// unavailable / failed values — red
    pub fn unavailable(&self, text: &str) -> String {
        self.paint("31", text)
    }

    /// capability names — green
    pub fn capability(&self, text: &str) -> String {
        self.paint("32", text)
    }
}
```

- [ ] **Step 2: Add the global flag in `cli.rs`**

Add `use crate::color::ColorMode;` and declare `mod color;` in `main.rs` (the `color` module lives in the binary crate). In the `Cli` struct add:

```rust
    #[arg(long, global = true, value_enum, default_value_t = ColorMode::Auto)]
    pub color: ColorMode,
```

- [ ] **Step 3: Plumb the flag through `main.rs`**

Capture `let color = cli.color;` before the `match cli.command`, and pass `color` into `run_list(format, color)` and `run_show(address, config, format, color)` (signatures extended; bodies still ignore it until Task 2).

- [ ] **Step 4: Verify and commit**

```bash
cargo fmt --all && cargo fmt --all -- --check
cargo check --workspace --target x86_64-unknown-linux-gnu
git add crates/lspci-rs/src/color.rs crates/lspci-rs/src/cli.rs crates/lspci-rs/src/main.rs
git commit -m "cli: add color module and global --color flag"
```

---

### Task 2: Colorize list and show text output

**Files:**
- Modify: `crates/lspci-rs/src/output.rs`
- Modify: `crates/lspci-rs/src/main.rs`

**Interfaces:**
- Consumes: `Palette` from Task 1.
- Produces: `render_text(snapshot, color)`, `render_inspection_text(inspection, config, color)` colorized; JSON untouched.

- [ ] **Step 1: Extend the render signatures**

Change `render_text(snapshot: &PciSnapshot)` to `render_text(snapshot: &PciSnapshot, color: ColorMode)` and `render_inspection_text(inspection: &PciInspection, config: Option<&ConfigSpaceSnapshot>)` to take a trailing `color: ColorMode` parameter. Start each function with `let palette = Palette::new(color);`. Update the call sites in `main.rs` to pass `color`.

- [ ] **Step 2: Colorize `render_text`**

In the device line, wrap the address with `palette.address(...)` and the three hex IDs with `palette.dim(...)`. The names stay uncolored.

- [ ] **Step 3: Colorize `render_inspection_text`**

Apply these rules:
- The `PCI device {address}` line: address via `palette.address`.
- Every field label (`vendor:`, `device:`, `class:`, `control:`, `status:`, ...) via `palette.dim`.
- Every `<unavailable: ...>` / `<not-applicable>` rendering via `palette.unavailable`.
- The capability group label lines (`standard: chain=...`) keep the chain status plain; the capability name token (the `render_capability_kind` output and the `id=` portion) — name via `palette.capability`, `id=`/`offset=` via `palette.dim`.
- The config-space dump offset column (`0000:` etc.) via `palette.dim`; `unavailable:` lines via `palette.unavailable`.
- `disabled` markers via `palette.dim`.

- [ ] **Step 4: Verify and commit**

```bash
cargo fmt --all && cargo fmt --all -- --check
cargo check --workspace --target x86_64-unknown-linux-gnu
cargo run -q -p lspci-rs --target x86_64-unknown-linux-gnu -- show 0000:00:05.0 --format text
cargo run -q -p lspci-rs --target x86_64-unknown-linux-gnu -- show 0000:00:05.0 --format json | head -3
cargo run -q -p lspci-rs --target x86_64-unknown-linux-gnu -- show 0000:00:05.0 --format text --color never | grep -c $'\x1b'
```

Expected: terminal output colored; JSON has no ANSI escapes; `--color never` output contains zero escape bytes.

```bash
git add crates/lspci-rs/src/output.rs crates/lspci-rs/src/main.rs
git commit -m "cli: colorize list and show text output"
```

---

### Task 3: tree subcommand

**Files:**
- Create: `crates/lspci-rs/src/tree.rs`
- Modify: `crates/lspci-rs/src/cli.rs`
- Modify: `crates/lspci-rs/src/main.rs`

**Interfaces:**
- Consumes: `PciSession::scan`, `PciSession::read_config`, `ConfigReadLevel::Header`, `Palette` from Task 1.
- Produces: `Command::Tree` subcommand rendering the topology.

- [ ] **Step 1: Create `crates/lspci-rs/src/tree.rs`**

The tree builder collects a bridge window per bridge device, then renders. A bridge window is the `(secondary, subordinate)` bus pair read from header bytes 0x19/0x1a; bridges whose window cannot be read render as leaf nodes.

```rust
use pci::{ConfigReadLevel, ConfigSpaceSnapshot, PciAddress, PciDevice, PciSession, PciSnapshot};

use crate::color::{ColorMode, Palette};

struct BridgeWindow {
    secondary: u8,
    subordinate: u8,
}

pub fn render_tree(
    session: &mut PciSession,
    snapshot: &PciSnapshot,
    color: ColorMode,
) -> Result<String, Box<dyn std::error::Error>> {
    let palette = Palette::new(color);

    let mut windows: Vec<(PciAddress, BridgeWindow)> = Vec::new();
    for device in snapshot.devices() {
        if device.class_id >> 8 == 0x06 {
            if let Ok(config) = session.read_config(device.address, ConfigReadLevel::Header) {
                if let Ok(bytes) = config.read(0x19, 2) {
                    windows.push((
                        device.address,
                        BridgeWindow {
                            secondary: bytes[0],
                            subordinate: bytes[1],
                        },
                    ));
                }
            }
        }
    }

    let mut output = String::new();
    let mut buses: Vec<u8> = snapshot
        .devices()
        .iter()
        .map(|device| device.address.bus)
        .collect();
    buses.sort_unstable();
    buses.dedup();
    // top-level buses: not contained in any bridge window
    for bus in buses {
        let covered = windows
            .iter()
            .any(|(_, window)| bus >= window.secondary && bus <= window.subordinate);
        if !covered {
            render_bus(
                &mut output,
                &palette,
                snapshot.devices(),
                &windows,
                bus,
                0,
                &String::new(),
            );
        }
    }
    Ok(output)
}

fn owner_bridge(
    windows: &[(PciAddress, BridgeWindow)],
    device: &PciDevice,
) -> Option<PciAddress> {
    // innermost bridge whose [secondary, subordinate] contains the device bus
    let mut best: Option<&(PciAddress, BridgeWindow)> = None;
    for (address, window) in windows {
        if *address == device.address {
            continue;
        }
        if device.address.bus >= window.secondary && device.address.bus <= window.subordinate {
            match best {
                Some((_, current))
                    if window.subordinate - window.secondary
                        >= current.subordinate - current.secondary => {}
                _ => best = Some((address, window)),
            }
        }
    }
    best.map(|(address, _)| *address)
}

#[allow(clippy::too_many_arguments)]
fn render_bus(
    output: &mut String,
    palette: &Palette,
    devices: &[PciDevice],
    windows: &[(PciAddress, BridgeWindow)],
    bus: u8,
    depth: usize,
    prefix: &str,
) {
    let mut sorted: Vec<&PciDevice> = devices
        .iter()
        .filter(|device| device.address.bus == bus)
        .filter(|device| owner_bridge(windows, device).map_or(depth == 0, |_| depth > 0))
        .collect();
    sorted.sort_by_key(|device| (device.address.slot, device.address.function));

    for device in sorted {
        let connector = if depth == 0 {
            format!("-[{:04x}:{:02x}]-+- ", device.address.domain, device.address.bus)
        } else {
            format!("{prefix}+- ")
        };
        let bridge_label = windows
            .iter()
            .find(|(address, _)| *address == device.address)
            .map(|(_, window)| format!("-[{:02x}-{:02x}]", window.secondary, window.subordinate));

        let address_text = format!("{:02x}:{:02x}.{}", device.address.bus, device.address.slot, device.address.function);
        match bridge_label {
            Some(label) => {
                output.push_str(&format!(
                    "{connector}{} {} {}\n",
                    palette.address(&address_text),
                    palette.dim(&label),
                    device.device_name
                ));
                let child_prefix = format!("{prefix}|  ");
                for child_bus in windows
                    .iter()
                    .find(|(address, _)| *address == device.address)
                    .map(|(_, window)| window.secondary..=window.subordinate)
                    .into_iter()
                    .flatten()
                {
                    render_bus(
                        output,
                        palette,
                        devices,
                        windows,
                        child_bus,
                        depth + 1,
                        &child_prefix,
                    );
                }
            }
            None => {
                output.push_str(&format!(
                    "{connector}{} {}\n",
                    palette.address(&address_text),
                    device.device_name
                ));
            }
        }
    }
}
```

Note: the `owner_bridge` filter keeps only devices owned by a bridge at the current depth; root-bus devices render at depth 0. Bridges whose window read failed have no entry in `windows` and render as leaves.

- [ ] **Step 2: Wire the subcommand**

In `cli.rs` add to `Command`:

```rust
    Tree {
        #[arg(long, value_enum, default_value_t = OutputFormat::Text)]
        format: OutputFormat,
    },
```

In `main.rs` add `mod tree;` and a `run_tree` handler:

```rust
        Command::Tree { format } => match run_tree(format, color) {
            Ok(output) => print!("{output}"),
            Err(error) => {
                eprintln!("{error}");
                std::process::exit(1);
            }
        },
```

with:

```rust
fn run_tree(
    format: OutputFormat,
    color: crate::color::ColorMode,
) -> Result<String, Box<dyn std::error::Error>> {
    let mut session = PciSession::new()?;
    let snapshot = session.scan()?;
    match format {
        OutputFormat::Text => Ok(tree::render_tree(&mut session, &snapshot, color)?),
        OutputFormat::Json => Ok(tree::render_tree(&mut session, &snapshot, crate::color::ColorMode::Never)?),
    }
}
```

(JSON format for tree reuses the plain-text tree with colors disabled; a structured JSON tree is out of scope.)

- [ ] **Step 3: Verify and commit**

```bash
cargo fmt --all && cargo fmt --all -- --check
cargo check --workspace --target x86_64-unknown-linux-gnu
cargo run -q -p lspci-rs --target x86_64-unknown-linux-gnu -- tree
git add crates/lspci-rs/src/tree.rs crates/lspci-rs/src/cli.rs crates/lspci-rs/src/main.rs
git commit -m "cli: add tree subcommand with bridge topology"
```

---

### Task 4: Validation and finish

**Files:** none (verification only), plus progress doc.

**Interfaces:**
- Consumes: completed branch binary; myece and dev48 access.
- Produces: color behavior evidence, topology comparison, handoff doc.

- [ ] **Step 1: Build and validate color behavior on myece**

```bash
# in container
cd /workspace && cargo build -p lspci-rs --target x86_64-unknown-linux-gnu
# color auto in a TTY (interactive run) shows colors
cargo run -q -p lspci-rs --target x86_64-unknown-linux-gnu -- list
# pipe auto-disables colors
cargo run -q -p lspci-rs --target x86_64-unknown-linux-gnu -- list | grep -c $'\x1b'
# --color always keeps codes through a pipe
cargo run -q -p lspci-rs --target x86_64-unknown-linux-gnu -- list --color always | grep -c $'\x1b'
# --color never disables colors in a TTY
cargo run -q -p lspci-rs --target x86_64-unknown-linux-gnu -- list --color never
# JSON never has codes
cargo run -q -p lspci-rs --target x86_64-unknown-linux-gnu -- show 0000:00:05.0 --format json --color always | grep -c $'\x1b'
```

Expected: pipe auto → 0 escapes; `--color always` pipe → >0 escapes; JSON always → 0 escapes.

- [ ] **Step 2: Transfer to dev48 and compare topology**

```bash
# on myece host
podman cp 95c90e05ab1a:/workspace/target/x86_64-unknown-linux-gnu/debug/lspci-rs /tmp/lspci-rs
# locally
sftp myece <<< "get /tmp/lspci-rs <local-staging-path>"
sftp dev48 <<< "put <local-staging-path> /tmp/lspci-rs"
ssh dev48 'sudo chmod +x /tmp/lspci-rs; sudo /tmp/lspci-rs tree; sudo lspci -t'
```

Compare: same parent/child relationships between bridges and devices (the ASCII layout may differ; the hierarchy must match).

- [ ] **Step 3: Regression on myece**

```bash
cd /workspace
cargo fmt --all -- --check
cargo check --workspace --target x86_64-unknown-linux-gnu
cargo run -q -p lspci-rs --target x86_64-unknown-linux-gnu -- list --format text --color never | wc -l
cargo run -q -p lspci-rs --target x86_64-unknown-linux-gnu -- show 0000:00:05.0 --config standard --format text --color never
git diff --check
```

Expected: 9 devices; config dump and capability output content unchanged.

- [ ] **Step 4: Record the handoff**

Create `docs/superpowers/progress/2026-08-11-cli-enhancement-progress.md` recording: commit list, color behavior evidence, dev48 topology comparison result, any fixes made. Commit:

```bash
git add docs/superpowers/progress/2026-08-11-cli-enhancement-progress.md
git commit -m "docs: record CLI enhancement validation results"
```

- [ ] **Step 5: Finish the branch**

Use superpowers:finishing-a-development-branch to merge `sdd/cli-enhancement` into `main` (or follow the user's chosen option).
