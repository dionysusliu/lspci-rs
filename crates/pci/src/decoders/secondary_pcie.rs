use super::read_dword;
use crate::ConfigSpaceSnapshot;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SecondaryPcieCapability {
    pub perform_equalization: bool,
    pub equalization_request_interrupt: bool,
    pub lane_equalization_control: u32,
}

pub fn decode_secondary_pcie(
    snapshot: &ConfigSpaceSnapshot,
    offset: u16,
) -> Option<SecondaryPcieCapability> {
    let base = u32::from(offset);
    let link_control_3 = read_dword(snapshot, base + 4).ok()?;
    let lane_equalization_control = read_dword(snapshot, base + 8).ok()?;

    Some(SecondaryPcieCapability {
        perform_equalization: link_control_3 & 0x0000_0001 != 0,
        equalization_request_interrupt: link_control_3 & 0x0000_0002 != 0,
        lane_equalization_control,
    })
}
