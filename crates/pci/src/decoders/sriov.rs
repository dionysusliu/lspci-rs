use super::{read_dword, read_word};
use crate::ConfigSpaceSnapshot;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SriovVfBarKind {
    Io,
    Memory,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SriovVfBar {
    pub kind: SriovVfBarKind,
    pub is_64_bit: bool,
    pub prefetchable: bool,
    pub address: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SriovCapability {
    pub capabilities: u32,
    pub control: u16,
    pub status: u16,
    pub initial_vfs: u16,
    pub total_vfs: u16,
    pub num_vfs: u16,
    pub function_dependency_link: u16,
    pub vf_offset: u16,
    pub vf_stride: u16,
    pub vf_device_id: u16,
    pub supported_page_sizes: u32,
    pub system_page_size: u32,
    pub vf_bars: [Option<SriovVfBar>; 6],
    pub migration_state_array_offset: u32,
    pub migration_state_array_size: u32,
}

pub fn decode_sriov(snapshot: &ConfigSpaceSnapshot, offset: u16) -> Option<SriovCapability> {
    let base = u32::from(offset);

    let capabilities = read_dword(snapshot, base + 0x04).ok()?;
    let control = read_word(snapshot, base + 0x08).ok()?;
    let status = read_word(snapshot, base + 0x0a).ok()?;
    let initial_vfs = read_word(snapshot, base + 0x0c).ok()?;
    let total_vfs = read_word(snapshot, base + 0x0e).ok()?;
    let num_vfs = read_word(snapshot, base + 0x10).ok()?;
    let function_dependency_link = read_word(snapshot, base + 0x12).ok()?;
    let vf_offset = read_word(snapshot, base + 0x14).ok()?;
    let vf_stride = read_word(snapshot, base + 0x16).ok()?;
    let vf_device_id = read_word(snapshot, base + 0x1a).ok()?;
    let supported_page_sizes = read_dword(snapshot, base + 0x1c).ok()?;
    let system_page_size = read_dword(snapshot, base + 0x20).ok()?;

    let mut raw_bars = [0u32; 6];
    for (index, bar) in raw_bars.iter_mut().enumerate() {
        *bar = read_dword(snapshot, base + 0x24 + (index as u32) * 4).ok()?;
    }

    let mut vf_bars: [Option<SriovVfBar>; 6] = Default::default();
    let mut index = 0;
    while index < 6 {
        let raw = raw_bars[index];
        if raw & 0x1 != 0 {
            vf_bars[index] = Some(SriovVfBar {
                kind: SriovVfBarKind::Io,
                is_64_bit: false,
                prefetchable: false,
                address: u64::from(raw & 0xffff_fffc),
            });
            index += 1;
        } else {
            let is_64_bit = (raw >> 1) & 0x3 == 0x2;
            let prefetchable = raw & 0x8 != 0;
            let mut address = u64::from(raw & 0xffff_fff0);
            if is_64_bit && index + 1 < 6 {
                address |= u64::from(raw_bars[index + 1]) << 32;
                vf_bars[index] = Some(SriovVfBar {
                    kind: SriovVfBarKind::Memory,
                    is_64_bit,
                    prefetchable,
                    address,
                });
                index += 2;
            } else {
                vf_bars[index] = Some(SriovVfBar {
                    kind: SriovVfBarKind::Memory,
                    is_64_bit,
                    prefetchable,
                    address,
                });
                index += 1;
            }
        }
    }

    let migration_state_array_offset = read_dword(snapshot, base + 0x40).ok()?;
    let migration_state_array_size = read_dword(snapshot, base + 0x44).ok()?;

    Some(SriovCapability {
        capabilities,
        control,
        status,
        initial_vfs,
        total_vfs,
        num_vfs,
        function_dependency_link,
        vf_offset,
        vf_stride,
        vf_device_id,
        supported_page_sizes,
        system_page_size,
        vf_bars,
        migration_state_array_offset,
        migration_state_array_size,
    })
}
