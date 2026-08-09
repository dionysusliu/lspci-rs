use pci::PciSnapshot;
use serde::Serialize;
use std::fmt::Write as _;

pub fn render_text(snapshot: &PciSnapshot) -> String {
    let mut output = String::new();

    let mut devices: Vec<_> = snapshot.devices().iter().collect();
    devices.sort_by_key(|device| {
        (
            device.address.domain,
            device.address.bus,
            device.address.slot,
            device.address.function,
        )
    });

    for device in devices {
        writeln!(
            output,
            "{} vendor=0x{:04x} device=0x{:04x} class=0x{:04x} {} / {} / {}",
            device.address,
            device.vendor_id,
            device.device_id,
            device.class_id,
            device.vendor_name,
            device.device_name,
            device.class_name,
        )
        .expect("writing PCI text otuput to String cannot fail");
    }

    output
}

#[derive(Debug, Serialize)]
struct JsonSnapshot<'a> {
    devices: Vec<JsonDevice<'a>>,
}

#[derive(Debug, Serialize)]
struct JsonDevice<'a> {
    address: JsonAddress,
    vendor_id: String,
    device_id: String,
    class_id: String,
    vendor_name: &'a str,
    device_name: &'a str,
    class_name: &'a str,
}

#[derive(Debug, Serialize)]
struct JsonAddress {
    domain: String,
    bus: String,
    slot: String,
    function: String,
    display: String,
}

pub fn render_json(snapshot: &PciSnapshot) -> Result<String, serde_json::Error> {
    let mut devices: Vec<_> = snapshot.devices().iter().collect();
    devices.sort_by_key(|device| {
        (
            device.address.domain,
            device.address.bus,
            device.address.slot,
            device.address.function,
        )
    });

    let json_snapshot = JsonSnapshot {
        devices: devices
            .into_iter()
            .map(|device| JsonDevice {
                address: JsonAddress {
                    domain: format!("0x{:04x}", device.address.domain),
                    bus: format!("0x{:04x}", device.address.bus),
                    slot: format!("0x{:04x}", device.address.slot),
                    function: device.address.function.to_string(),
                    display: device.address.to_string(),
                },
                vendor_id: format!("0x{:04x}", device.vendor_id),
                device_id: format!("0x{:04x}", device.device_id),
                class_id: format!("0x{:04x}", device.class_id),
                vendor_name: &device.vendor_name,
                device_name: &device.device_name,
                class_name: &device.class_name,
            })
            .collect(),
    };

    serde_json::to_string_pretty(&json_snapshot)
}
