use pci::{PciAddress, PciField, PciInspection, PciResource, PciSnapshot};
use serde::Serialize;
use std::fmt::{Display, Write as _};

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

/// render PciInspection result to text
pub fn render_inspection_text(inspection: &PciInspection) -> String {
    let device = &inspection.device;
    let details = &inspection.details;

    let mut output = String::new();

    writeln!(output, "PCI device {}", device.address).unwrap();

    writeln!(
        output,
        "  vendor: 0x{:04x} {}",
        device.vendor_id, device.vendor_name
    )
    .unwrap();

    writeln!(
        output,
        "  device: 0x{:04x} {}",
        device.device_id, device.device_name
    )
    .unwrap();

    writeln!(
        output,
        "  class: 0x{:04x} {}",
        device.class_id, device.class_name
    )
    .unwrap();

    writeln!(output, "  revision: {}", render_field(&details.revision)).unwrap();

    writeln!(
        output,
        "  programming interface: {}",
        render_field(&details.programming_interface)
    )
    .unwrap();

    writeln!(
        output,
        "  subsystem vendor: {}",
        render_field(&details.subsystem_vendor_id)
    )
    .unwrap();

    writeln!(
        output,
        "  subsystem device: {}",
        render_field(&details.subsystem_device_id)
    )
    .unwrap();

    writeln!(output, "  parent: {}", render_field(&details.parent)).unwrap();

    writeln!(output, "  irq: {}", render_field(&details.irq)).unwrap();

    writeln!(output, "  driver: {}", render_field(&details.driver)).unwrap();

    match &details.resources {
        PciField::Available(resources) => {
            writeln!(output, "  resources:").unwrap();

            for resource in resources {
                writeln!(
                    output,
                    "    BAR{} start=0x{:x} size=0x{:x} flags=0x{:x}",
                    resource.index, resource.start, resource.size, resource.flags
                )
                .unwrap();
            }
        }

        PciField::Unavailable { reason } => {
            writeln!(output, "  resources: <unavailable: {reason:?}>").unwrap();
        }

        PciField::NotApplicable => {
            writeln!(output, "  resources: <not-applicable>").unwrap();
        }
    }

    output
}

/// render a PciField to text
fn render_field<T: Display>(field: &PciField<T>) -> String {
    match field {
        PciField::Available(value) => value.to_string(),
        PciField::Unavailable { reason } => {
            format!("<unavailable: {reason:?}>")
        }
        PciField::NotApplicable => "<not-applicable>".to_owned(),
    }
}

pub fn render_inspection_json(inspection: &PciInspection) -> Result<String, serde_json::Error> {
    let device = &inspection.device;
    let details = &inspection.details;

    let json = JsonInspection {
        device: JsonDevice {
            address: JsonAddress {
                domain: format!("0x{:04x}", device.address.domain),
                bus: format!("0x{:02x}", device.address.bus),
                slot: format!("0x{:02x}", device.address.slot),
                function: device.address.function.to_string(),
                display: device.address.to_string(),
            },
            vendor_id: format!("0x{:04x}", device.vendor_id),
            device_id: format!("0x{:04x}", device.device_id),
            class_id: format!("0x{:04x}", device.class_id),
            vendor_name: &device.vendor_name,
            device_name: &device.device_name,
            class_name: &device.class_name,
        },

        details: JsonDetails {
            revision: json_field(&details.revision),
            programming_interface: json_field(&details.programming_interface),
            subsystem_vendor_id: json_field(&details.subsystem_vendor_id),
            subsystem_device_id: json_field(&details.subsystem_device_id),
            parent: json_parent(&details.parent),
            irq: json_field(&details.irq),
            driver: json_field(&details.driver),
            resources: json_resources(&details.resources),
        },
    };

    serde_json::to_string_pretty(&json)
}

#[derive(Debug, Serialize)]
struct JsonInspection<'a> {
    device: JsonDevice<'a>,
    details: JsonDetails,
}

#[derive(Debug, Serialize)]
struct JsonDetails {
    revision: JsonField<u8>,
    programming_interface: JsonField<u8>,
    subsystem_vendor_id: JsonField<u16>,
    subsystem_device_id: JsonField<u16>,
    parent: JsonField<String>,
    irq: JsonField<u32>,
    driver: JsonField<String>,
    resources: JsonField<Vec<JsonResource>>,
}

#[derive(Debug, Serialize)]
struct JsonResource {
    index: u8,
    start: String,
    size: String,
    flags: String,
}

#[derive(Debug, Serialize)]
struct JsonField<T> {
    state: &'static str,

    #[serde(skip_serializing_if = "Option::is_none")]
    value: Option<T>,

    #[serde(skip_serializing_if = "Option::is_none")]
    reason: Option<String>,
}

fn json_field<T: Clone>(field: &PciField<T>) -> JsonField<T> {
    match field {
        PciField::Available(value) => JsonField {
            state: "available",
            value: Some(value.clone()),
            reason: None,
        },
        PciField::Unavailable { reason } => JsonField {
            state: "unavailable",
            value: None,
            reason: Some(format!("{reason:?}")),
        },
        PciField::NotApplicable => JsonField {
            state: "not_available",
            value: None,
            reason: None,
        },
    }
}

fn json_parent(field: &PciField<PciAddress>) -> JsonField<String> {
    match field {
        PciField::Available(address) => JsonField {
            state: "available",
            value: Some(address.to_string()),
            reason: None,
        },

        PciField::Unavailable { reason } => JsonField {
            state: "unavailable",
            value: None,
            reason: Some(format!("{reason:?}")),
        },

        PciField::NotApplicable => JsonField {
            state: "not_applicable",
            value: None,
            reason: None,
        },
    }
}

fn json_resources(field: &PciField<Vec<PciResource>>) -> JsonField<Vec<JsonResource>> {
    match field {
        PciField::Available(resources) => JsonField {
            state: "available",
            value: Some(
                resources
                    .iter()
                    .map(|resource| JsonResource {
                        index: resource.index,
                        start: format!("0x{:x}", resource.start),
                        size: format!("0x{:x}", resource.size),
                        flags: format!("0x{:x}", resource.flags),
                    })
                    .collect(),
            ),
            reason: None,
        },

        PciField::Unavailable { reason } => JsonField {
            state: "unavailable",
            value: None,
            reason: Some(format!("{reason:?}")),
        },

        PciField::NotApplicable => JsonField {
            state: "not_applicable",
            value: None,
            reason: None,
        },
    }
}
