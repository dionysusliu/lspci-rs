use pci::{
    AER_CE_BITS, AER_UE_BITS, AerCapability, CommandRegister, ConfigSpaceSnapshot, PciAddress,
    PciBarKind, PciBarType, PciCapability, PciCapabilityChainStatus, PciCapabilityContent,
    PciCapabilityKind, PciCapabilityReport, PciField, PciInspection, PciResource, PciSnapshot,
    PcieCapability, StatusRegister, capability_name,
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
        PciCapabilityContent::Pcie(pcie) => render_pcie_text(pcie),
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

fn pcie_retimer_text(presence: u8) -> &'static str {
    match presence {
        1 => "Retimer+",
        2 => "2Retimers+",
        _ => "Retimer-",
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
    let speed_note = if speed_downgraded {
        " (downgraded)"
    } else {
        ""
    };
    let width_note = if width_downgraded {
        " (downgraded)"
    } else {
        ""
    };

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
            "\n          DevCap2: Completion Timeout: {:02x}, TimeoutDis{} NROPrPrP{} ARI{} AtomicOpsRouting{} LTR{} 10BitTagComp{} 10BitTagReq{} OBFF {} ExtFmt{} EETLPPrefix{}",
            dev_cap2.completion_timeout_ranges,
            pcie_flag(dev_cap2.completion_timeout_disable),
            pcie_flag(dev_cap2.no_ro_pr_pr_passing),
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
            "\n          DevCtl2: Completion Timeout: {:02x}, TimeoutDis{} ARI{} LTR{} 10BitTagReq{} OBFF {}",
            dev_ctl2.completion_timeout,
            pcie_flag(dev_ctl2.completion_timeout_disable),
            pcie_flag(dev_ctl2.ari),
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
            "\n          LnkSta2: Current De-emphasis: {}, EqualizationComplete{} EqualizationPhase1{} EqualizationPhase2{} EqualizationPhase3{} LinkEqualizationRequest{} {}",
            if lnk_sta2.current_de_emphasis { "-3.5dB" } else { "-6dB" },
            pcie_flag(lnk_sta2.equalization_complete),
            pcie_flag(lnk_sta2.equalization_phase1),
            pcie_flag(lnk_sta2.equalization_phase2),
            pcie_flag(lnk_sta2.equalization_phase3),
            pcie_flag(lnk_sta2.equalization_request),
            pcie_retimer_text(lnk_sta2.retimer_presence)
        ));
    }

    output
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
