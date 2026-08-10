use pci::{
    AER_CE_BITS, AER_UE_BITS, AerCapability, CommandRegister, ConfigSpaceSnapshot, PciAddress,
    PciBarKind, PciBarType, PciCapability, PciCapabilityChainStatus, PciCapabilityContent,
    PciCapabilityKind, PciCapabilityReport, PciField, PciInspection, PciResource, PciSnapshot,
    StatusRegister, capability_name,
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

    match &details.command {
        PciField::Available(command) => {
            writeln!(output, "  control: {}", render_command_text(command)).unwrap();
        }
        PciField::Unavailable { reason } => {
            writeln!(output, "  control: <unavailable: {reason:?}>").unwrap();
        }
        PciField::NotApplicable => {
            writeln!(output, "  control: <not-applicable>").unwrap();
        }
    }

    match &details.status {
        PciField::Available(status) => {
            writeln!(output, "  status: {}", render_status_text(status)).unwrap();
        }
        PciField::Unavailable { reason } => {
            writeln!(output, "  status: <unavailable: {reason:?}>").unwrap();
        }
        PciField::NotApplicable => {
            writeln!(output, "  status: <not-applicable>").unwrap();
        }
    }

    match &details.resources {
        PciField::Available(resources) => {
            writeln!(output, "  resources:").unwrap();

            for resource in resources {
                let bar_type = resource
                    .bar_type
                    .as_ref()
                    .map(render_bar_type)
                    .unwrap_or_else(|| "unknown".to_owned());
                writeln!(
                    output,
                    "    BAR{} start=0x{:x} size=0x{:x} type={} flags=0x{:x}",
                    resource.index, resource.start, resource.size, bar_type, resource.flags
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
            capability_name(&capability.kind, capability.id),
            capability.id,
            capability.offset,
            render_next_pointer(&capability.next),
            capability.state
        )
        .unwrap();

        if let Some(content) = &capability.content {
            writeln!(
                output,
                "        content: {}",
                render_capability_content(content)
            )
            .unwrap();
        }
    }
}

fn render_capability_content(content: &PciCapabilityContent) -> String {
    match content {
        PciCapabilityContent::Pm(pm) => format!(
            "version={} pme_support=0x{:02x} power_state=D{} pme_enable={} pme_status={} no_soft_reset={}",
            pm.version,
            pm.pme_support,
            pm.power_state,
            pm.pme_enable,
            pm.pme_status,
            pm.no_soft_reset
        ),
        PciCapabilityContent::Msi(msi) => format!(
            "enable={} count={}/{} 64bit={} maskable={} address=0x{:x} data=0x{:x}",
            msi.enable,
            1u32 << msi.multiple_message_enable,
            1u32 << msi.multiple_message_capable,
            msi.is_64_bit,
            msi.per_vector_masking,
            msi.address,
            msi.data
        ),
        PciCapabilityContent::MsiX(msix) => format!(
            "enable={} count={} masked={} table=BAR{}+0x{:x} pba=BAR{}+0x{:x}",
            msix.enable,
            msix.count,
            msix.masked,
            msix.table_bar,
            msix.table_offset,
            msix.pba_bar,
            msix.pba_offset
        ),
        PciCapabilityContent::Pcie(pcie) => format!(
            "version={} type={} slot={} link_max={}GT/s x{} link={}GT/s x{} training={}",
            pcie.version,
            render_pcie_device_type(pcie.device_type),
            pcie.slot_implemented,
            render_pcie_speed(pcie.link_max_speed),
            pcie.link_max_width,
            render_pcie_speed(pcie.link_current_speed),
            pcie.link_current_width,
            pcie.link_training
        ),
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
        PciCapabilityContent::VendorSpecific(vendor) => {
            let data: Vec<String> = vendor
                .data
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect();
            format!("len={} data={}", vendor.length, data.join(" "))
        }
        PciCapabilityContent::Dsn(dsn) => {
            let serial: Vec<String> = dsn
                .serial
                .iter()
                .rev()
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
            "initial_vfs={} total_vfs={} num_vfs={} vf_offset={} vf_stride={} vf_device_id=0x{:04x} control=0x{:04x}",
            sriov.initial_vfs,
            sriov.total_vfs,
            sriov.num_vfs,
            sriov.vf_offset,
            sriov.vf_stride,
            sriov.vf_device_id,
            sriov.control
        ),
        PciCapabilityContent::Aer(aer) => render_aer_text(aer),
        PciCapabilityContent::Ltr(ltr) => format!(
            "snoop={}:{} no_snoop={}:{}",
            ltr.snoop_value, ltr.snoop_scale, ltr.no_snoop_value, ltr.no_snoop_scale
        ),
        PciCapabilityContent::Ats(ats) => format!(
            "queue_depth={} enable={} page_aligned={} stu={}",
            ats.invalidate_queue_depth, ats.enable, ats.page_aligned, ats.smallest_translation_unit
        ),
        PciCapabilityContent::Pri(pri) => format!(
            "enable={} stopped={} capacity={} allocation={}",
            pri.enable, pri.stopped, pri.outstanding_capacity, pri.outstanding_allocation
        ),
        PciCapabilityContent::Pasid(pasid) => format!(
            "width={} exec_supported={} priv_supported={} enable={}",
            pasid.max_pasid_width,
            pasid.execute_supported,
            pasid.privileged_supported,
            pasid.enable
        ),
        PciCapabilityContent::Ptm(ptm) => format!(
            "requester={} responder={} root={} granularity={}ns enable={} root_select={}",
            ptm.requester_capable,
            ptm.responder_capable,
            ptm.root_capable,
            ptm.clock_granularity,
            ptm.enable,
            ptm.root_select
        ),
        PciCapabilityContent::Dpc(dpc) => format!(
            "trigger_enable={} trigger_status={} reason={} interrupt_enable={} rp_busy={} err_ptr={} source=0x{:04x}",
            dpc.trigger_enable,
            dpc.trigger_status,
            dpc.trigger_reason,
            dpc.interrupt_enable,
            dpc.rp_busy,
            dpc.rp_pio_error_pointer,
            dpc.error_source_id
        ),
        PciCapabilityContent::Tph(tph) => format!(
            "device_specific={} interrupt_vector={} extended_requester={} location={} size={} mode_select={}",
            tph.device_specific_supported,
            tph.interrupt_vector_supported,
            tph.extended_requester_supported,
            tph.st_table_location,
            tph.st_table_size,
            tph.st_mode_select
        ),
        PciCapabilityContent::VendorExt(vendor_ext) => format!(
            "vendor=0x{:04x} rev={} len={} data={}",
            vendor_ext.vendor_id,
            vendor_ext.revision,
            vendor_ext.length,
            vendor_ext
                .data
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<Vec<_>>()
                .join("")
        ),
        PciCapabilityContent::Dvsec(dvsec) => format!(
            "vendor=0x{:04x} rev={} id=0x{:04x} len={} data={}",
            dvsec.vendor_id,
            dvsec.revision,
            dvsec.dvsec_id,
            dvsec.length,
            dvsec
                .data
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<Vec<_>>()
                .join("")
        ),
        PciCapabilityContent::Vc(vc) => format!(
            "lpevc={} evc_count={} ref_clock={} pat_entry_bits={} port_control=0x{:04x} port_status=0x{:04x} resources={}",
            vc.lpevc,
            vc.evc_count,
            vc.reference_clock,
            vc.pat_entry_bits,
            vc.port_control,
            vc.port_status,
            vc.resources.len()
        ),
        PciCapabilityContent::SecondaryPcie(secondary) => format!(
            "perform_eq={} eq_interrupt={} lane_eq=0x{:08x}",
            secondary.perform_equalization,
            secondary.equalization_request_interrupt,
            secondary.lane_equalization_control
        ),
    }
}

fn render_pcie_device_type(device_type: u8) -> &'static str {
    match device_type {
        0x0 => "endpoint",
        0x1 => "legacy-endpoint",
        0x4 => "root-port",
        0x5 => "upstream-switch-port",
        0x6 => "downstream-switch-port",
        0x7 => "pcie-to-pci-bridge",
        0x8 => "pci-to-pcie-bridge",
        0x9 => "rc-integrated-endpoint",
        0xa => "rc-event-collector",
        _ => "unknown",
    }
}

fn render_pcie_speed(speed: u8) -> &'static str {
    match speed {
        1 => "2.5",
        2 => "5.0",
        3 => "8.0",
        4 => "16.0",
        5 => "32.0",
        6 => "64.0",
        _ => "?",
    }
}

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
    output
}

fn render_command_text(command: &CommandRegister) -> String {
    let flag = |enabled: bool| if enabled { "+" } else { "-" };
    format!(
        "I/O{} Mem{} BusMaster{} SpecCycle{} MemWINV{} VGASnoop{} ParErr{} Stepping{} SERR{} FastB2B{} DisINTx{}",
        flag(command.io_space),
        flag(command.memory_space),
        flag(command.bus_master),
        flag(command.special_cycle),
        flag(command.mem_write_invalidate),
        flag(command.vga_palette_snoop),
        flag(command.parity_error_response),
        flag(command.stepping),
        flag(command.serr_enable),
        flag(command.fast_back_to_back),
        flag(command.interrupt_disable),
    )
}

fn render_status_text(status: &StatusRegister) -> String {
    let flag = |enabled: bool| if enabled { "+" } else { "-" };
    let devsel = match status.devsel_timing {
        0 => "fast",
        1 => "medium",
        2 => "slow",
        _ => "unknown",
    };
    format!(
        "Cap{} 66MHz{} UDF{} FastB2B{} ParErr{} DEVSEL={} >TAbort{} <TAbort{} <MAbort{} >SERR{} <PERR{} INTx{}",
        flag(status.capabilities_list),
        flag(status.capable_66mhz),
        flag(status.udf),
        flag(status.capable_fast_back_to_back),
        flag(status.master_parity_error),
        devsel,
        flag(status.signaled_target_abort),
        flag(status.received_target_abort),
        flag(status.received_master_abort),
        flag(status.signaled_system_error),
        flag(status.detected_parity_error),
        flag(status.interrupt_status),
    )
}

fn render_bar_type(bar_type: &PciBarType) -> String {
    match bar_type.kind {
        PciBarKind::Io => "io".to_owned(),
        PciBarKind::Memory => {
            let width = if bar_type.is_64_bit { "64" } else { "32" };
            if bar_type.prefetchable {
                format!("memory-{width}-prefetch")
            } else {
                format!("memory-{width}")
            }
        }
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
            command: json_command(&details.command),
            status: json_status(&details.status),
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
    command: JsonField<JsonCommand>,
    status: JsonField<JsonStatus>,
}

#[derive(Debug, Serialize)]
struct JsonResource {
    index: u8,
    start: String,
    size: String,
    flags: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    bar_type: Option<String>,
}

#[derive(Debug, Serialize)]
struct JsonCommand {
    io_space: bool,
    memory_space: bool,
    bus_master: bool,
    special_cycle: bool,
    mem_write_invalidate: bool,
    vga_palette_snoop: bool,
    parity_error_response: bool,
    stepping: bool,
    serr_enable: bool,
    fast_back_to_back: bool,
    interrupt_disable: bool,
}

#[derive(Debug, Serialize)]
struct JsonStatus {
    interrupt_status: bool,
    capabilities_list: bool,
    capable_66mhz: bool,
    udf: bool,
    capable_fast_back_to_back: bool,
    master_parity_error: bool,
    devsel: String,
    signaled_target_abort: bool,
    received_target_abort: bool,
    received_master_abort: bool,
    signaled_system_error: bool,
    detected_parity_error: bool,
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
    name: String,
    kind: String,
    offset: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    next: Option<String>,

    state: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    content: Option<JsonCapabilityContent>,
}

#[derive(Debug, Serialize)]
#[serde(tag = "type")]
enum JsonCapabilityContent {
    #[serde(rename = "pm")]
    Pm(JsonPm),
    #[serde(rename = "msi")]
    Msi(JsonMsi),
    #[serde(rename = "msix")]
    MsiX(JsonMsiX),
    #[serde(rename = "pcie")]
    Pcie(JsonPcie),
    #[serde(rename = "vendor_specific")]
    VendorSpecific(JsonVendorSpecific),
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
    #[serde(rename = "ltr")]
    Ltr(JsonLtr),
    #[serde(rename = "ats")]
    Ats(JsonAts),
    #[serde(rename = "pri")]
    Pri(JsonPri),
    #[serde(rename = "pasid")]
    Pasid(JsonPasid),
    #[serde(rename = "ptm")]
    Ptm(JsonPtm),
    #[serde(rename = "dpc")]
    Dpc(JsonDpc),
    #[serde(rename = "tph")]
    Tph(JsonTph),
    #[serde(rename = "vendor_ext")]
    VendorExt(JsonVendorExt),
    #[serde(rename = "dvsec")]
    Dvsec(JsonDvsec),
    #[serde(rename = "vc")]
    Vc(JsonVc),
    #[serde(rename = "secondary_pcie")]
    SecondaryPcie(JsonSecondaryPcie),

    #[serde(rename = "slot_id")]
    SlotId(JsonSlotId),
    #[serde(rename = "hot_plug")]
    HotPlug(JsonHotPlug),
    #[serde(rename = "vpd")]
    Vpd(JsonVpd),
    #[serde(rename = "pci_x")]
    PciX(JsonPciX),
}

#[derive(Debug, Serialize)]
struct JsonPm {
    version: u8,
    pme_clock: bool,
    dsi: bool,
    aux_current: u8,
    d1_support: bool,
    d2_support: bool,
    pme_support: String,
    power_state: String,
    no_soft_reset: bool,
    pme_enable: bool,
    data_select: u8,
    data_scale: u8,
    pme_status: bool,
}

#[derive(Debug, Serialize)]
struct JsonMsi {
    enable: bool,
    vectors_capable: u32,
    vectors_enabled: u32,
    is_64_bit: bool,
    per_vector_masking: bool,
    address: String,
    data: String,
}

#[derive(Debug, Serialize)]
struct JsonMsiX {
    enable: bool,
    count: u16,
    masked: bool,
    table: JsonBarRef,
    pba: JsonBarRef,
}

#[derive(Debug, Serialize)]
struct JsonBarRef {
    bar: u8,
    offset: String,
}

#[derive(Debug, Serialize)]
struct JsonPcie {
    version: u8,
    device_type: String,
    slot_implemented: bool,
    interrupt_message_number: u8,
    dev_ctl: String,
    dev_sta: String,
    link_max_speed: String,
    link_max_width: u8,
    link_target_speed: String,
    link_current_speed: String,
    link_current_width: u8,
    link_training: bool,

    #[serde(skip_serializing_if = "Option::is_none")]
    slot_ctl: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    slot_sta: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    root_ctl: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    root_sta: Option<String>,
}

#[derive(Debug, Serialize)]
struct JsonVendorSpecific {
    length: u8,
    data: String,
}

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
    vf_offset: u16,
    vf_stride: u16,
    vf_device_id: String,
    supported_page_sizes: String,
    system_page_size: String,
    vf_bars: Vec<String>,
    migration_state_array_offset: String,
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
}

#[derive(Debug, Serialize)]
struct JsonLtr {
    snoop_value: u16,
    snoop_scale: u8,
    no_snoop_value: u16,
    no_snoop_scale: u8,
}

#[derive(Debug, Serialize)]
struct JsonAts {
    invalidate_queue_depth: u8,
    enable: bool,
    page_aligned: bool,
    smallest_translation_unit: u8,
}

#[derive(Debug, Serialize)]
struct JsonPri {
    enable: bool,
    reset: bool,
    response_failure: bool,
    unexpected_group_index: bool,
    stopped: bool,
    outstanding_capacity: u32,
    outstanding_allocation: u32,
}

#[derive(Debug, Serialize)]
struct JsonPasid {
    execute_supported: bool,
    privileged_supported: bool,
    max_pasid_width: u8,
    enable: bool,
    execute_enable: bool,
    privileged_enable: bool,
}

#[derive(Debug, Serialize)]
struct JsonPtm {
    requester_capable: bool,
    responder_capable: bool,
    root_capable: bool,
    clock_granularity: u8,
    enable: bool,
    root_select: bool,
}

#[derive(Debug, Serialize)]
struct JsonDpc {
    interrupt_message_number: u8,
    rp_pio_extensions: bool,
    poisoned_tlp_blocking_capable: bool,
    software_trigger_capable: bool,
    rp_pio_log_size: u8,
    dl_active_error_capable: bool,
    trigger_enable: u8,
    completion_control: bool,
    interrupt_enable: bool,
    err_cor_enable: bool,
    poisoned_tlp_blocking_enable: bool,
    software_trigger: bool,
    dl_active_error_enable: bool,
    trigger_status: bool,
    trigger_reason: u8,
    interrupt_status: bool,
    rp_busy: bool,
    trigger_extension: u8,
    rp_pio_error_pointer: u8,
    error_source_id: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    rp_pio_first_error_pointer: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    rp_pio_status: Option<String>,
}

#[derive(Debug, Serialize)]
struct JsonTph {
    interrupt_vector_supported: bool,
    device_specific_supported: bool,
    extended_requester_supported: bool,
    st_table_location: u8,
    st_table_size: u16,
    st_mode_select: u8,
    st_table: Vec<String>,
}

#[derive(Debug, Serialize)]
struct JsonVendorExt {
    vendor_id: String,
    revision: u8,
    length: u16,
    data: String,
}

#[derive(Debug, Serialize)]
struct JsonDvsec {
    vendor_id: String,
    revision: u8,
    dvsec_id: String,
    length: u16,
    data: String,
}

#[derive(Debug, Serialize)]
struct JsonVcResource {
    control: String,
    status: String,
    capability: String,
}

#[derive(Debug, Serialize)]
struct JsonVc {
    evc_count: u8,
    lpevc: u8,
    reference_clock: u8,
    pat_entry_bits: u8,
    arbitration_table_position: u8,
    port_control: String,
    port_status: String,
    resources: Vec<JsonVcResource>,
}

#[derive(Debug, Serialize)]
struct JsonSecondaryPcie {
    perform_equalization: bool,
    equalization_request_interrupt: bool,
    lane_equalization_control: String,
}

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
                        bar_type: resource.bar_type.as_ref().map(render_bar_type),
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
            name: capability_name(&capability.kind, capability.id).to_owned(),
            kind: render_capability_kind(&capability.kind).to_owned(),
            offset: format!("0x{:03x}", capability.offset),
            next: capability.next.map(|next| format!("0x{next:03x}")),
            state: format!("{:?}", capability.state),
            content: capability.content.as_ref().map(json_capability_content),
        })
        .collect()
}

fn json_capability_content(content: &PciCapabilityContent) -> JsonCapabilityContent {
    match content {
        PciCapabilityContent::Pm(pm) => JsonCapabilityContent::Pm(JsonPm {
            version: pm.version,
            pme_clock: pm.pme_clock,
            dsi: pm.dsi,
            aux_current: pm.aux_current,
            d1_support: pm.d1_support,
            d2_support: pm.d2_support,
            pme_support: format!("0x{:02x}", pm.pme_support),
            power_state: format!("D{}", pm.power_state),
            no_soft_reset: pm.no_soft_reset,
            pme_enable: pm.pme_enable,
            data_select: pm.data_select,
            data_scale: pm.data_scale,
            pme_status: pm.pme_status,
        }),
        PciCapabilityContent::Msi(msi) => JsonCapabilityContent::Msi(JsonMsi {
            enable: msi.enable,
            vectors_capable: 1u32 << msi.multiple_message_capable,
            vectors_enabled: 1u32 << msi.multiple_message_enable,
            is_64_bit: msi.is_64_bit,
            per_vector_masking: msi.per_vector_masking,
            address: format!("0x{:x}", msi.address),
            data: format!("0x{:x}", msi.data),
        }),
        PciCapabilityContent::MsiX(msix) => JsonCapabilityContent::MsiX(JsonMsiX {
            enable: msix.enable,
            count: msix.count,
            masked: msix.masked,
            table: JsonBarRef {
                bar: msix.table_bar,
                offset: format!("0x{:x}", msix.table_offset),
            },
            pba: JsonBarRef {
                bar: msix.pba_bar,
                offset: format!("0x{:x}", msix.pba_offset),
            },
        }),
        PciCapabilityContent::Pcie(pcie) => JsonCapabilityContent::Pcie(JsonPcie {
            version: pcie.version,
            device_type: render_pcie_device_type(pcie.device_type).to_owned(),
            slot_implemented: pcie.slot_implemented,
            interrupt_message_number: pcie.interrupt_message_number,
            dev_ctl: format!("0x{:04x}", pcie.dev_ctl),
            dev_sta: format!("0x{:04x}", pcie.dev_sta),
            link_max_speed: render_pcie_speed(pcie.link_max_speed).to_owned(),
            link_max_width: pcie.link_max_width,
            link_target_speed: render_pcie_speed(pcie.link_target_speed).to_owned(),
            link_current_speed: render_pcie_speed(pcie.link_current_speed).to_owned(),
            link_current_width: pcie.link_current_width,
            link_training: pcie.link_training,
            slot_ctl: pcie.slot_ctl.map(|value| format!("0x{value:04x}")),
            slot_sta: pcie.slot_sta.map(|value| format!("0x{value:04x}")),
            root_ctl: pcie.root_ctl.map(|value| format!("0x{value:04x}")),
            root_sta: pcie.root_sta.map(|value| format!("0x{value:08x}")),
        }),
        PciCapabilityContent::SlotId(slot_id) => JsonCapabilityContent::SlotId(JsonSlotId {
            slots: slot_id.slots,
            first: slot_id.first,
            chassis: format!("0x{:02x}", slot_id.chassis),
        }),
        PciCapabilityContent::HotPlug(hot_plug) => JsonCapabilityContent::HotPlug(JsonHotPlug {
            hot_plug_capable: hot_plug.hot_plug_capable,
        }),
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
        PciCapabilityContent::VendorSpecific(vendor) => {
            JsonCapabilityContent::VendorSpecific(JsonVendorSpecific {
                length: vendor.length,
                data: vendor
                    .data
                    .iter()
                    .map(|byte| format!("{byte:02x}"))
                    .collect::<Vec<_>>()
                    .join(" "),
            })
        }
        PciCapabilityContent::Dsn(dsn) => JsonCapabilityContent::Dsn(JsonDsn {
            serial: dsn
                .serial
                .iter()
                .rev()
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
            vf_offset: sriov.vf_offset,
            vf_stride: sriov.vf_stride,
            vf_device_id: format!("0x{:04x}", sriov.vf_device_id),
            supported_page_sizes: format!("0x{:08x}", sriov.supported_page_sizes),
            system_page_size: format!("0x{:08x}", sriov.system_page_size),
            vf_bars: sriov
                .vf_bars
                .iter()
                .map(|bar| format!("0x{bar:08x}"))
                .collect(),
            migration_state_array_offset: format!("0x{:08x}", sriov.migration_state_array_offset),
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
        }),
        PciCapabilityContent::Ltr(ltr) => JsonCapabilityContent::Ltr(JsonLtr {
            snoop_value: ltr.snoop_value,
            snoop_scale: ltr.snoop_scale,
            no_snoop_value: ltr.no_snoop_value,
            no_snoop_scale: ltr.no_snoop_scale,
        }),
        PciCapabilityContent::Ats(ats) => JsonCapabilityContent::Ats(JsonAts {
            invalidate_queue_depth: ats.invalidate_queue_depth,
            enable: ats.enable,
            page_aligned: ats.page_aligned,
            smallest_translation_unit: ats.smallest_translation_unit,
        }),
        PciCapabilityContent::Pri(pri) => JsonCapabilityContent::Pri(JsonPri {
            enable: pri.enable,
            reset: pri.reset,
            response_failure: pri.response_failure,
            unexpected_group_index: pri.unexpected_group_index,
            stopped: pri.stopped,
            outstanding_capacity: pri.outstanding_capacity,
            outstanding_allocation: pri.outstanding_allocation,
        }),
        PciCapabilityContent::Pasid(pasid) => JsonCapabilityContent::Pasid(JsonPasid {
            execute_supported: pasid.execute_supported,
            privileged_supported: pasid.privileged_supported,
            max_pasid_width: pasid.max_pasid_width,
            enable: pasid.enable,
            execute_enable: pasid.execute_enable,
            privileged_enable: pasid.privileged_enable,
        }),
        PciCapabilityContent::Ptm(ptm) => JsonCapabilityContent::Ptm(JsonPtm {
            requester_capable: ptm.requester_capable,
            responder_capable: ptm.responder_capable,
            root_capable: ptm.root_capable,
            clock_granularity: ptm.clock_granularity,
            enable: ptm.enable,
            root_select: ptm.root_select,
        }),
        PciCapabilityContent::Dpc(dpc) => JsonCapabilityContent::Dpc(JsonDpc {
            interrupt_message_number: dpc.interrupt_message_number,
            rp_pio_extensions: dpc.rp_pio_extensions,
            poisoned_tlp_blocking_capable: dpc.poisoned_tlp_blocking_capable,
            software_trigger_capable: dpc.software_trigger_capable,
            rp_pio_log_size: dpc.rp_pio_log_size,
            dl_active_error_capable: dpc.dl_active_error_capable,
            trigger_enable: dpc.trigger_enable,
            completion_control: dpc.completion_control,
            interrupt_enable: dpc.interrupt_enable,
            err_cor_enable: dpc.err_cor_enable,
            poisoned_tlp_blocking_enable: dpc.poisoned_tlp_blocking_enable,
            software_trigger: dpc.software_trigger,
            dl_active_error_enable: dpc.dl_active_error_enable,
            trigger_status: dpc.trigger_status,
            trigger_reason: dpc.trigger_reason,
            interrupt_status: dpc.interrupt_status,
            rp_busy: dpc.rp_busy,
            trigger_extension: dpc.trigger_extension,
            rp_pio_error_pointer: dpc.rp_pio_error_pointer,
            error_source_id: format!("0x{:04x}", dpc.error_source_id),
            rp_pio_first_error_pointer: dpc
                .rp_pio_first_error_pointer
                .map(|value| format!("0x{value:02x}")),
            rp_pio_status: dpc.rp_pio_status.map(|value| format!("0x{value:08x}")),
        }),
        PciCapabilityContent::Tph(tph) => JsonCapabilityContent::Tph(JsonTph {
            interrupt_vector_supported: tph.interrupt_vector_supported,
            device_specific_supported: tph.device_specific_supported,
            extended_requester_supported: tph.extended_requester_supported,
            st_table_location: tph.st_table_location,
            st_table_size: tph.st_table_size,
            st_mode_select: tph.st_mode_select,
            st_table: tph
                .st_table
                .iter()
                .map(|entry| format!("0x{entry:04x}"))
                .collect(),
        }),
        PciCapabilityContent::VendorExt(vendor_ext) => {
            JsonCapabilityContent::VendorExt(JsonVendorExt {
                vendor_id: format!("0x{:04x}", vendor_ext.vendor_id),
                revision: vendor_ext.revision,
                length: vendor_ext.length,
                data: vendor_ext
                    .data
                    .iter()
                    .map(|byte| format!("{byte:02x}"))
                    .collect::<Vec<_>>()
                    .join(""),
            })
        }
        PciCapabilityContent::Dvsec(dvsec) => JsonCapabilityContent::Dvsec(JsonDvsec {
            vendor_id: format!("0x{:04x}", dvsec.vendor_id),
            revision: dvsec.revision,
            dvsec_id: format!("0x{:04x}", dvsec.dvsec_id),
            length: dvsec.length,
            data: dvsec
                .data
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<Vec<_>>()
                .join(""),
        }),
        PciCapabilityContent::Vc(vc) => JsonCapabilityContent::Vc(JsonVc {
            evc_count: vc.evc_count,
            lpevc: vc.lpevc,
            reference_clock: vc.reference_clock,
            pat_entry_bits: vc.pat_entry_bits,
            arbitration_table_position: vc.arbitration_table_position,
            port_control: format!("0x{:04x}", vc.port_control),
            port_status: format!("0x{:04x}", vc.port_status),
            resources: vc
                .resources
                .iter()
                .map(|resource| JsonVcResource {
                    control: format!("0x{:08x}", resource.control),
                    status: format!("0x{:08x}", resource.status),
                    capability: format!("0x{:08x}", resource.capability),
                })
                .collect(),
        }),
        PciCapabilityContent::SecondaryPcie(secondary) => {
            JsonCapabilityContent::SecondaryPcie(JsonSecondaryPcie {
                perform_equalization: secondary.perform_equalization,
                equalization_request_interrupt: secondary.equalization_request_interrupt,
                lane_equalization_control: format!("0x{:08x}", secondary.lane_equalization_control),
            })
        }
    }
}

fn aer_flag_bit_names(value: u32, bits: &[(u8, &str)]) -> Vec<String> {
    bits.iter()
        .filter(|(bit, _)| value & (1u32 << bit) != 0)
        .map(|(_, name)| (*name).to_owned())
        .collect()
}

fn json_command(field: &PciField<CommandRegister>) -> JsonField<JsonCommand> {
    match field {
        PciField::Available(command) => JsonField {
            state: "available",
            value: Some(JsonCommand {
                io_space: command.io_space,
                memory_space: command.memory_space,
                bus_master: command.bus_master,
                special_cycle: command.special_cycle,
                mem_write_invalidate: command.mem_write_invalidate,
                vga_palette_snoop: command.vga_palette_snoop,
                parity_error_response: command.parity_error_response,
                stepping: command.stepping,
                serr_enable: command.serr_enable,
                fast_back_to_back: command.fast_back_to_back,
                interrupt_disable: command.interrupt_disable,
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

fn json_status(field: &PciField<StatusRegister>) -> JsonField<JsonStatus> {
    match field {
        PciField::Available(status) => JsonField {
            state: "available",
            value: Some(JsonStatus {
                interrupt_status: status.interrupt_status,
                capabilities_list: status.capabilities_list,
                capable_66mhz: status.capable_66mhz,
                udf: status.udf,
                capable_fast_back_to_back: status.capable_fast_back_to_back,
                master_parity_error: status.master_parity_error,
                devsel: match status.devsel_timing {
                    0 => "fast".to_owned(),
                    1 => "medium".to_owned(),
                    2 => "slow".to_owned(),
                    _ => "unknown".to_owned(),
                },
                signaled_target_abort: status.signaled_target_abort,
                received_target_abort: status.received_target_abort,
                received_master_abort: status.received_master_abort,
                signaled_system_error: status.signaled_system_error,
                detected_parity_error: status.detected_parity_error,
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
