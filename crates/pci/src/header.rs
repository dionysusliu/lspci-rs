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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PciHeaderKind {
    Device,
    Bridge,
    CardBus,
    Unknown(u8),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PciHeaderType {
    pub kind: PciHeaderKind,
    pub multifunction: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PciBist {
    pub capable: bool,
    pub start: bool,
    pub completion_code: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PciExpansionRom {
    pub enable: bool,
    pub address: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PciInterruptPin {
    None,
    IntA,
    IntB,
    IntC,
    IntD,
    Unknown(u8),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PciBridgeHeader {
    pub primary_bus: u8,
    pub secondary_bus: u8,
    pub subordinate_bus: u8,
    pub secondary_latency_timer: u8,
    pub io_base: u32,
    pub io_limit: u32,
    pub io_enabled: bool,
    pub secondary_status: u16,
    pub memory_base: u32,
    pub memory_limit: u32,
    pub memory_enabled: bool,
    pub prefetchable_base: u64,
    pub prefetchable_limit: u64,
    pub prefetchable_64_bit: bool,
    pub prefetchable_enabled: bool,
    pub bridge_control: u16,
}

fn read_byte_field(
    snapshot: &ConfigSpaceSnapshot,
    offset: u32,
) -> Result<u8, PciFieldUnavailableReason> {
    let bytes = snapshot
        .read(offset, 1)
        .map_err(|_| PciFieldUnavailableReason::ReadError)?;
    Ok(bytes[0])
}

fn read_dword_field(
    snapshot: &ConfigSpaceSnapshot,
    offset: u32,
) -> Result<u32, PciFieldUnavailableReason> {
    let bytes = snapshot
        .read(offset, 4)
        .map_err(|_| PciFieldUnavailableReason::ReadError)?;
    Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}

pub(crate) fn cache_line_size_field(snapshot: &ConfigSpaceSnapshot) -> PciField<u8> {
    match read_byte_field(snapshot, 0x0c) {
        Ok(value) => PciField::Available(value),
        Err(reason) => PciField::Unavailable { reason },
    }
}

pub(crate) fn latency_timer_field(snapshot: &ConfigSpaceSnapshot) -> PciField<u8> {
    match read_byte_field(snapshot, 0x0d) {
        Ok(value) => PciField::Available(value),
        Err(reason) => PciField::Unavailable { reason },
    }
}

pub(crate) fn header_type_field(snapshot: &ConfigSpaceSnapshot) -> PciField<PciHeaderType> {
    match read_byte_field(snapshot, 0x0e) {
        Ok(raw) => {
            let kind = match raw & 0x7f {
                0 => PciHeaderKind::Device,
                1 => PciHeaderKind::Bridge,
                2 => PciHeaderKind::CardBus,
                other => PciHeaderKind::Unknown(other),
            };
            PciField::Available(PciHeaderType {
                kind,
                multifunction: raw & 0x80 != 0,
            })
        }
        Err(reason) => PciField::Unavailable { reason },
    }
}

pub(crate) fn bist_field(snapshot: &ConfigSpaceSnapshot) -> PciField<PciBist> {
    match read_byte_field(snapshot, 0x0f) {
        Ok(raw) => PciField::Available(PciBist {
            capable: raw & 0x80 != 0,
            start: raw & 0x40 != 0,
            completion_code: raw & 0x0f,
        }),
        Err(reason) => PciField::Unavailable { reason },
    }
}

pub(crate) fn expansion_rom_field(
    snapshot: &ConfigSpaceSnapshot,
    is_bridge: bool,
) -> PciField<PciExpansionRom> {
    let offset = if is_bridge { 0x38 } else { 0x30 };
    match read_dword_field(snapshot, offset) {
        Ok(raw) => PciField::Available(PciExpansionRom {
            enable: raw & 0x1 != 0,
            address: raw & 0xfffff800,
        }),
        Err(reason) => PciField::Unavailable { reason },
    }
}

pub(crate) fn interrupt_line_field(snapshot: &ConfigSpaceSnapshot) -> PciField<u8> {
    match read_byte_field(snapshot, 0x3c) {
        Ok(value) => PciField::Available(value),
        Err(reason) => PciField::Unavailable { reason },
    }
}

pub(crate) fn interrupt_pin_field(snapshot: &ConfigSpaceSnapshot) -> PciField<PciInterruptPin> {
    match read_byte_field(snapshot, 0x3d) {
        Ok(raw) => {
            let pin = match raw {
                0 => PciInterruptPin::None,
                1 => PciInterruptPin::IntA,
                2 => PciInterruptPin::IntB,
                3 => PciInterruptPin::IntC,
                4 => PciInterruptPin::IntD,
                other => PciInterruptPin::Unknown(other),
            };
            PciField::Available(pin)
        }
        Err(reason) => PciField::Unavailable { reason },
    }
}

pub(crate) fn cardbus_cis_field(snapshot: &ConfigSpaceSnapshot) -> PciField<u32> {
    match read_dword_field(snapshot, 0x28) {
        Ok(value) => PciField::Available(value),
        Err(reason) => PciField::Unavailable { reason },
    }
}

pub(crate) fn bridge_header_field(snapshot: &ConfigSpaceSnapshot) -> PciField<PciBridgeHeader> {
    // 0x18..0x40 covers every Type 1 register decoded here.
    let bytes = match snapshot.read(0x18, 0x28) {
        Ok(bytes) => bytes,
        Err(_) => {
            return PciField::Unavailable {
                reason: PciFieldUnavailableReason::ReadError,
            };
        }
    };

    let word = |offset: usize| u16::from_le_bytes([bytes[offset], bytes[offset + 1]]);
    let dword = |offset: usize| {
        u32::from_le_bytes([
            bytes[offset],
            bytes[offset + 1],
            bytes[offset + 2],
            bytes[offset + 3],
        ])
    };

    let primary_bus = bytes[0x00];
    let secondary_bus = bytes[0x01];
    let subordinate_bus = bytes[0x02];
    let secondary_latency_timer = bytes[0x03];
    let io_base_raw = bytes[0x04]; // 0x1c
    let io_limit_raw = bytes[0x05]; // 0x1d
    let secondary_status = word(0x06); // 0x1e
    let memory_base_raw = word(0x08); // 0x20
    let memory_limit_raw = word(0x0a); // 0x22
    let pref_base_lo = word(0x0c); // 0x24
    let pref_limit_lo = word(0x0e); // 0x26
    let pref_base_hi = dword(0x10); // 0x28
    let pref_limit_hi = dword(0x14); // 0x2c
    let io_base_upper = word(0x18); // 0x30
    let io_limit_upper = word(0x1a); // 0x32
    let bridge_control = word(0x26); // 0x3e

    let io_base = u32::from(io_base_raw & 0xf0) << 8 | u32::from(io_base_upper) << 16;
    let io_limit = u32::from(io_limit_raw & 0xf0) << 8 | u32::from(io_limit_upper) << 16 | 0xfff;
    let memory_base = u32::from(memory_base_raw) << 16;
    let memory_limit = (u32::from(memory_limit_raw) << 16) | 0x000f_ffff;
    let prefetchable_64_bit = pref_base_lo & 0x1 != 0;
    let prefetchable_base =
        (u64::from(pref_base_hi) << 32) | (u64::from(pref_base_lo & 0xfff0) << 16);
    let prefetchable_limit =
        (u64::from(pref_limit_hi) << 32) | ((u64::from(pref_limit_lo) << 16) | 0x000f_ffff);

    PciField::Available(PciBridgeHeader {
        primary_bus,
        secondary_bus,
        subordinate_bus,
        secondary_latency_timer,
        io_base,
        io_limit,
        io_enabled: io_base <= io_limit,
        secondary_status,
        memory_base,
        memory_limit,
        memory_enabled: memory_base <= memory_limit,
        prefetchable_base,
        prefetchable_limit,
        prefetchable_64_bit,
        prefetchable_enabled: prefetchable_base <= prefetchable_limit,
        bridge_control,
    })
}
