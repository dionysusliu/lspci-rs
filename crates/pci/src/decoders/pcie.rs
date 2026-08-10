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
