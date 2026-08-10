use pci::{
    ConfigSpaceSnapshot, PciAddress, PciCapability, PciCapabilityChainStatus, PciCapabilityKind,
    PciCapabilityReport, PciField, PciInspection, PciResource, PciSnapshot,
};
use serde::Serialize;
use std::fmt::{Display, LowerHex, Write as _};

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
pub fn render_inspection_text(
    inspection: &PciInspection,
    config: Option<&ConfigSpaceSnapshot>,
) -> String {
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
        render_hex_field(&details.subsystem_vendor_id)
    )
    .unwrap();

    writeln!(
        output,
        "  subsystem device: {}",
        render_hex_field(&details.subsystem_device_id)
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

    match &details.capabilities {
        PciField::Available(report) => {
            writeln!(output, "  capabilities:").unwrap();

            render_capability_group_text(
                &mut output,
                "standard",
                &report.standard,
                &report.standard_status,
            );
            render_capability_group_text(
                &mut output,
                "extended",
                &report.extended,
                &report.extended_status,
            );
        }

        PciField::Unavailable { reason } => {
            writeln!(output, "  capabilities: <unavailable: {reason:?}>").unwrap();
        }

        PciField::NotApplicable => {
            writeln!(output, "  capabilities: <not-applicable>").unwrap();
        }
    }

    if let Some(snapshot) = config {
        render_config_space_text(&mut output, snapshot);
    }

    output
}

fn render_capability_group_text(
    output: &mut String,
    label: &str,
    capabilities: &[PciCapability],
    status: &PciCapabilityChainStatus,
) {
    writeln!(output, "    {label}: chain={}", render_chain_status(status)).unwrap();

    for capability in capabilities {
        writeln!(
            output,
            "      {} id=0x{:04x} offset=0x{:03x} next={} state={:?}",
            render_capability_kind(&capability.kind),
            capability.id,
            capability.offset,
            render_next_pointer(&capability.next),
            capability.state
        )
        .unwrap();
    }
}

fn render_next_pointer(next: &Option<u16>) -> String {
    match next {
        Some(next) => format!("0x{next:03x}"),
        None => "none".to_owned(),
    }
}

fn render_chain_status(status: &PciCapabilityChainStatus) -> String {
    match status {
        PciCapabilityChainStatus::NotPresent => "not-present".to_owned(),
        PciCapabilityChainStatus::Complete => "complete".to_owned(),
        PciCapabilityChainStatus::Truncated => "truncated".to_owned(),
        PciCapabilityChainStatus::Unavailable(reason) => format!("unavailable: {reason:?}"),
        PciCapabilityChainStatus::Malformed(reason) => format!("malformed: {reason:?}"),
    }
}

fn render_config_space_text(output: &mut String, snapshot: &ConfigSpaceSnapshot) {
    writeln!(output, "config-space:").unwrap();
    writeln!(
        output,
        "  requested: 0x{:03x}..0x{:03x}",
        snapshot.requested.start, snapshot.requested.end
    )
    .unwrap();

    for segment in &snapshot.segments {
        for (row_index, chunk) in segment.bytes.chunks(16).enumerate() {
            let row_offset = segment.offset + (row_index as u32) * 16;
            let bytes: Vec<String> = chunk.iter().map(|byte| format!("{byte:02x}")).collect();
            writeln!(output, "  {row_offset:04x}: {}", bytes.join(" ")).unwrap();
        }
    }

    for failure in &snapshot.failures {
        writeln!(
            output,
            "  unavailable: 0x{:03x}..0x{:03x} <{:?}>",
            failure.offset,
            failure.offset + failure.length,
            failure.reason
        )
        .unwrap();
    }
}

/// render a PciField to text
fn render_field<T: Display>(field: &PciField<T>) -> String {
    match field {
        PciField::Available(value) => value.to_string(),
        PciField::Unavailable { reason } => {
            format!("<unavailable: {reason:?}>")
        }
        PciField::NotApplicable => "<not_available>".to_owned(),
    }
}

fn render_hex_field<T: LowerHex>(field: &PciField<T>) -> String {
    match field {
        PciField::Available(value) => format!("0x{value:x}"),

        PciField::Unavailable { reason } => {
            format!("<unavailable: {reason:?}>")
        }

        PciField::NotApplicable => "<not-applicable>".to_owned(),
    }
}

fn render_capability_kind(kind: &PciCapabilityKind) -> &'static str {
    match kind {
        PciCapabilityKind::Standard => "standard",
        PciCapabilityKind::Extended => "extended",
        PciCapabilityKind::Unknown(_) => "unknown",
    }
}

pub fn render_inspection_json(
    inspection: &PciInspection,
    config: Option<&ConfigSpaceSnapshot>,
) -> Result<String, serde_json::Error> {
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
            subsystem_vendor_id: json_hex_field(&details.subsystem_vendor_id),
            subsystem_device_id: json_hex_field(&details.subsystem_device_id),
            parent: json_parent(&details.parent),
            irq: json_field(&details.irq),
            driver: json_field(&details.driver),
            resources: json_resources(&details.resources),
            capabilities: json_capabilities(&details.capabilities),
        },

        config: config.map(json_config_space),
    };

    serde_json::to_string_pretty(&json)
}

#[derive(Debug, Serialize)]
struct JsonInspection<'a> {
    device: JsonDevice<'a>,
    details: JsonDetails,

    #[serde(skip_serializing_if = "Option::is_none")]
    config: Option<JsonConfigSpace>,
}

#[derive(Debug, Serialize)]
struct JsonDetails {
    revision: JsonField<u8>,
    programming_interface: JsonField<u8>,
    subsystem_vendor_id: JsonField<String>,
    subsystem_device_id: JsonField<String>,
    parent: JsonField<String>,
    irq: JsonField<u32>,
    driver: JsonField<String>,
    resources: JsonField<Vec<JsonResource>>,
    capabilities: JsonField<JsonCapabilities>,
}

#[derive(Debug, Serialize)]
struct JsonResource {
    index: u8,
    start: String,
    size: String,
    flags: String,
}

#[derive(Debug, Serialize)]
struct JsonCapabilities {
    standard: Vec<JsonCapability>,
    extended: Vec<JsonCapability>,
    standard_status: String,
    extended_status: String,
}

#[derive(Debug, Serialize)]
struct JsonCapability {
    id: String,
    kind: String,
    offset: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    next: Option<String>,

    state: String,
}

#[derive(Debug, Serialize)]
struct JsonConfigSpace {
    requested: JsonRange,
    segments: Vec<JsonConfigSegment>,
    failures: Vec<JsonConfigFailure>,
}

#[derive(Debug, Serialize)]
struct JsonRange {
    start: String,
    end: String,
}

#[derive(Debug, Serialize)]
struct JsonConfigSegment {
    offset: String,
    bytes: String,
}

#[derive(Debug, Serialize)]
struct JsonConfigFailure {
    offset: String,
    length: String,
    reason: String,
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
            state: "not_applicable",
            value: None,
            reason: None,
        },
    }
}

fn json_hex_field<T: LowerHex>(field: &PciField<T>) -> JsonField<String> {
    match field {
        PciField::Available(value) => JsonField {
            state: "available",
            value: Some(format!("0x{value:x}")),
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

fn json_capabilities(field: &PciField<PciCapabilityReport>) -> JsonField<JsonCapabilities> {
    match field {
        PciField::Available(report) => JsonField {
            state: "available",
            value: Some(JsonCapabilities {
                standard: json_capability_list(&report.standard),
                extended: json_capability_list(&report.extended),
                standard_status: json_chain_status(&report.standard_status),
                extended_status: json_chain_status(&report.extended_status),
            }),
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

fn json_capability_list(capabilities: &[PciCapability]) -> Vec<JsonCapability> {
    capabilities
        .iter()
        .map(|capability| JsonCapability {
            id: format!("0x{:04x}", capability.id),
            kind: render_capability_kind(&capability.kind).to_owned(),
            offset: format!("0x{:03x}", capability.offset),
            next: capability.next.map(|next| format!("0x{next:03x}")),
            state: format!("{:?}", capability.state),
        })
        .collect()
}

fn json_chain_status(status: &PciCapabilityChainStatus) -> String {
    match status {
        PciCapabilityChainStatus::NotPresent => "not_present".to_owned(),
        PciCapabilityChainStatus::Complete => "complete".to_owned(),
        PciCapabilityChainStatus::Truncated => "truncated".to_owned(),
        PciCapabilityChainStatus::Unavailable(reason) => format!("unavailable: {reason:?}"),
        PciCapabilityChainStatus::Malformed(reason) => format!("malformed: {reason:?}"),
    }
}

fn json_config_space(snapshot: &ConfigSpaceSnapshot) -> JsonConfigSpace {
    JsonConfigSpace {
        requested: JsonRange {
            start: format!("0x{:03x}", snapshot.requested.start),
            end: format!("0x{:03x}", snapshot.requested.end),
        },
        segments: snapshot
            .segments
            .iter()
            .map(|segment| JsonConfigSegment {
                offset: format!("0x{:03x}", segment.offset),
                bytes: segment
                    .bytes
                    .iter()
                    .map(|byte| format!("{byte:02x}"))
                    .collect(),
            })
            .collect(),
        failures: snapshot
            .failures
            .iter()
            .map(|failure| JsonConfigFailure {
                offset: format!("0x{:03x}", failure.offset),
                length: format!("0x{:x}", failure.length),
                reason: format!("{:?}", failure.reason),
            })
            .collect(),
    }
}
