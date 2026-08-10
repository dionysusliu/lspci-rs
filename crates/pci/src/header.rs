use crate::{ConfigSpaceSnapshot, PciField, PciFieldUnavailableReason};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommandRegister {
    pub io_space: bool,
    pub memory_space: bool,
    pub bus_master: bool,
    pub special_cycle: bool,
    pub mem_write_invalidate: bool,
    pub vga_palette_snoop: bool,
    pub parity_error_response: bool,
    pub stepping: bool,
    pub serr_enable: bool,
    pub fast_back_to_back: bool,
    pub interrupt_disable: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StatusRegister {
    pub interrupt_status: bool,
    pub capabilities_list: bool,
    pub capable_66mhz: bool,
    pub udf: bool,
    pub capable_fast_back_to_back: bool,
    pub master_parity_error: bool,
    /// 0 = fast, 1 = medium, 2 = slow
    pub devsel_timing: u8,
    pub signaled_target_abort: bool,
    pub received_target_abort: bool,
    pub received_master_abort: bool,
    pub signaled_system_error: bool,
    pub detected_parity_error: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PciBarKind {
    Io,
    Memory,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PciBarType {
    pub kind: PciBarKind,
    pub is_64_bit: bool,
    pub prefetchable: bool,
}

pub fn decode_command(word: u16) -> CommandRegister {
    CommandRegister {
        io_space: word & 0x0001 != 0,
        memory_space: word & 0x0002 != 0,
        bus_master: word & 0x0004 != 0,
        special_cycle: word & 0x0008 != 0,
        mem_write_invalidate: word & 0x0010 != 0,
        vga_palette_snoop: word & 0x0020 != 0,
        parity_error_response: word & 0x0040 != 0,
        stepping: word & 0x0080 != 0,
        serr_enable: word & 0x0100 != 0,
        fast_back_to_back: word & 0x0200 != 0,
        interrupt_disable: word & 0x0400 != 0,
    }
}

pub fn decode_status(word: u16) -> StatusRegister {
    StatusRegister {
        interrupt_status: word & 0x0008 != 0,
        capabilities_list: word & 0x0010 != 0,
        capable_66mhz: word & 0x0020 != 0,
        udf: word & 0x0040 != 0,
        capable_fast_back_to_back: word & 0x0080 != 0,
        master_parity_error: word & 0x0100 != 0,
        devsel_timing: ((word >> 9) & 0x0003) as u8,
        signaled_target_abort: word & 0x0800 != 0,
        received_target_abort: word & 0x1000 != 0,
        received_master_abort: word & 0x2000 != 0,
        signaled_system_error: word & 0x4000 != 0,
        detected_parity_error: word & 0x8000 != 0,
    }
}

pub fn decode_bar_type(bar: u32) -> PciBarType {
    if bar & 0x1 != 0 {
        PciBarType {
            kind: PciBarKind::Io,
            is_64_bit: false,
            prefetchable: false,
        }
    } else {
        PciBarType {
            kind: PciBarKind::Memory,
            is_64_bit: (bar >> 1) & 0x3 == 0x2,
            prefetchable: bar & 0x8 != 0,
        }
    }
}

fn read_word_field(
    snapshot: &ConfigSpaceSnapshot,
    offset: u32,
) -> Result<u16, PciFieldUnavailableReason> {
    let bytes = snapshot
        .read(offset, 2)
        .map_err(|_| PciFieldUnavailableReason::ReadError)?;
    Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
}

pub(crate) fn command_field(snapshot: &ConfigSpaceSnapshot) -> PciField<CommandRegister> {
    match read_word_field(snapshot, 0x04) {
        Ok(word) => PciField::Available(decode_command(word)),
        Err(reason) => PciField::Unavailable { reason },
    }
}

pub(crate) fn status_field(snapshot: &ConfigSpaceSnapshot) -> PciField<StatusRegister> {
    match read_word_field(snapshot, 0x06) {
        Ok(word) => PciField::Available(decode_status(word)),
        Err(reason) => PciField::Unavailable { reason },
    }
}

pub(crate) fn bar_type_field(snapshot: &ConfigSpaceSnapshot, index: u8) -> Option<PciBarType> {
    let bytes = snapshot.read(0x10 + u32::from(index) * 4, 4).ok()?;
    let bar = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
    Some(decode_bar_type(bar))
}
