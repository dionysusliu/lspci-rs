use super::{read_dword, read_word};
use crate::ConfigSpaceSnapshot;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DpcCapability {
    pub interrupt_message_number: u8,
    pub rp_pio_extensions: bool,
    pub rp_pio_log_size: u8,
    pub trigger_enable: u8,
    pub completion_control: bool,
    pub interrupt_enable: bool,
    pub err_cor_enable: bool,
    pub software_trigger: bool,
    pub trigger_status: bool,
    pub trigger_reason: u8,
    pub interrupt_status: bool,
    pub reason_extension: bool,
    pub error_source_id: u16,
    pub rp_pio_first_error_pointer: Option<u8>,
    pub rp_pio_status: Option<u32>,
}

pub fn decode_dpc(snapshot: &ConfigSpaceSnapshot, offset: u16) -> Option<DpcCapability> {
    let base = u32::from(offset);
    let capability = read_word(snapshot, base + 4).ok()?;
    let control = read_word(snapshot, base + 6).ok()?;
    let status = read_word(snapshot, base + 8).ok()?;
    let error_source_id = read_word(snapshot, base + 10).ok()?;

    let rp_pio_extensions = capability & 0x0010 != 0;
    let (rp_pio_first_error_pointer, rp_pio_status) = if rp_pio_extensions {
        let first = read_dword(snapshot, base + 12).ok()?;
        let status = read_dword(snapshot, base + 16).ok()?;
        (Some((first & 0x0000_003f) as u8), Some(status))
    } else {
        (None, None)
    };

    Some(DpcCapability {
        interrupt_message_number: (capability & 0x0007) as u8,
        rp_pio_extensions,
        rp_pio_log_size: ((capability >> 8) & 0x001f) as u8,
        trigger_enable: (control & 0x0003) as u8,
        completion_control: control & 0x0004 != 0,
        interrupt_enable: control & 0x0008 != 0,
        err_cor_enable: control & 0x0010 != 0,
        software_trigger: control & 0x0040 != 0,
        trigger_status: status & 0x0001 != 0,
        trigger_reason: ((status >> 1) & 0x0003) as u8,
        interrupt_status: status & 0x0008 != 0,
        reason_extension: status & 0x0010 != 0,
        error_source_id,
        rp_pio_first_error_pointer,
        rp_pio_status,
    })
}
