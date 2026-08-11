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
        if device.class_id == 0x0604
            && let Ok(config) = session.read_config(device.address, ConfigReadLevel::Header)
            && let Ok(bytes) = config.read(0x19, 2)
        {
            windows.push((
                device.address,
                BridgeWindow {
                    secondary: bytes[0],
                    subordinate: bytes[1],
                },
            ));
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
                "",
            );
        }
    }
    Ok(output)
}

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
        .collect();
    sorted.sort_by_key(|device| (device.address.slot, device.address.function));

    for (position, device) in sorted.iter().enumerate() {
        let connector = if depth == 0 {
            if position == 0 {
                format!(
                    "-[{:04x}:{:02x}]-+- ",
                    device.address.domain, device.address.bus
                )
            } else {
                "           +- ".to_owned()
            }
        } else {
            format!("{prefix}+- ")
        };
        let window = windows
            .iter()
            .find(|(address, _)| *address == device.address)
            .map(|(_, window)| window);

        let address_text = format!(
            "{:02x}:{:02x}.{}",
            device.address.bus, device.address.slot, device.address.function
        );
        let device_text = format!("{} {}", device.vendor_name, device.device_name);
        match window {
            Some(window) => {
                output.push_str(&format!(
                    "{connector}{} {} {}\n",
                    palette.address(&address_text),
                    palette.dim(&format!(
                        "-[{:02x}-{:02x}]",
                        window.secondary, window.subordinate
                    )),
                    device_text
                ));
                let child_prefix = if depth == 0 {
                    "           |  ".to_owned()
                } else {
                    format!("{prefix}|  ")
                };
                render_bus(
                    output,
                    palette,
                    devices,
                    windows,
                    window.secondary,
                    depth + 1,
                    &child_prefix,
                );
            }
            None => {
                output.push_str(&format!(
                    "{connector}{} {}\n",
                    palette.address(&address_text),
                    device_text
                ));
            }
        }
    }
}
