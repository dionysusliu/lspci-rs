use pci::{ConfigReadLevel, PciAddress, PciDevice, PciSession, PciSnapshot};

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
        if device.class_id == 0x0604 {
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

fn owner_bridge(windows: &[(PciAddress, BridgeWindow)], device: &PciDevice) -> Option<PciAddress> {
    // innermost bridge whose [secondary, subordinate] contains the device bus
    let mut best: Option<&(PciAddress, BridgeWindow)> = None;
    for entry in windows {
        if entry.0 == device.address {
            continue;
        }
        let window = &entry.1;
        if device.address.bus >= window.secondary && device.address.bus <= window.subordinate {
            match best {
                Some(current)
                    if window.subordinate - window.secondary
                        >= current.1.subordinate - current.1.secondary => {}
                _ => best = Some(entry),
            }
        }
    }
    best.map(|entry| entry.0)
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
            format!(
                "-[{:04x}:{:02x}]-+- ",
                device.address.domain, device.address.bus
            )
        } else {
            format!("{prefix}+- ")
        };
        let bridge_label = windows
            .iter()
            .find(|(address, _)| *address == device.address)
            .map(|(_, window)| format!("-[{:02x}-{:02x}]", window.secondary, window.subordinate));

        let address_text = format!(
            "{:02x}:{:02x}.{}",
            device.address.bus, device.address.slot, device.address.function
        );
        match bridge_label {
            Some(label) => {
                output.push_str(&format!(
                    "{connector}{} {} {}\n",
                    palette.address(&address_text),
                    palette.dim(&label),
                    device.device_name
                ));
                if let Some((_, window)) = windows
                    .iter()
                    .find(|(address, _)| *address == device.address)
                {
                    let child_prefix = format!("{prefix}|  ");
                    for child_bus in window.secondary..=window.subordinate {
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
