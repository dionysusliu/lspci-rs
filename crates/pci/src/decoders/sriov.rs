use super::{read_dword, read_word};
use crate::ConfigSpaceSnapshot;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SriovCapability {
    pub capabilities: u32,
    pub control: u16,
    pub status: u16,
    pub initial_vfs: u16,
    pub total_vfs: u16,
    pub num_vfs: u16,
    pub function_dependency_link: u16,
    pub vf_device_id: u16,
    pub supported_page_sizes: u32,
    pub system_page_size: u32,
    pub vf_bars: [u32; 6],
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
    let vf_device_id = read_word(snapshot, base + 0x14).ok()?;
    let supported_page_sizes = read_dword(snapshot, base + 0x18).ok()?;
    let system_page_size = read_dword(snapshot, base + 0x1c).ok()?;

    let mut vf_bars = [0u32; 6];
    for (index, bar) in vf_bars.iter_mut().enumerate() {
        *bar = read_dword(snapshot, base + 0x20 + (index as u32) * 4).ok()?;
    }

    let migration_state_array_offset = read_dword(snapshot, base + 0x38).ok()?;
    let migration_state_array_size = read_dword(snapshot, base + 0x3c).ok()?;

    Some(SriovCapability {
        capabilities,
        control,
        status,
        initial_vfs,
        total_vfs,
        num_vfs,
        function_dependency_link,
        vf_device_id,
        supported_page_sizes,
        system_page_size,
        vf_bars,
        migration_state_array_offset,
        migration_state_array_size,
    })
}
