use super::read_word;
use crate::ConfigSpaceSnapshot;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PmCapability {
    pub version: u8,
    pub pme_clock: bool,
    pub dsi: bool,
    pub aux_current: u8,
    pub d1_support: bool,
    pub d2_support: bool,
    /// bitmask: bit 0 = D0 ... bit 4 = D3cold
    pub pme_support: u8,
    /// 0 = D0, 1 = D1, 2 = D2, 3 = D3hot
    pub power_state: u8,
    pub no_soft_reset: bool,
    pub pme_enable: bool,
    pub data_select: u8,
    pub data_scale: u8,
    pub pme_status: bool,
}

pub fn decode_pm(snapshot: &ConfigSpaceSnapshot, offset: u16) -> Option<PmCapability> {
    let base = u32::from(offset);
    let pmc = read_word(snapshot, base + 2).ok()?;
    let pmcsr = read_word(snapshot, base + 4).ok()?;

    Some(PmCapability {
        version: (pmc & 0x0007) as u8,
        pme_clock: pmc & 0x0008 != 0,
        dsi: pmc & 0x0010 != 0,
        aux_current: ((pmc >> 6) & 0x0003) as u8,
        d1_support: pmc & 0x0200 != 0,
        d2_support: pmc & 0x0400 != 0,
        pme_support: ((pmc >> 11) & 0x001f) as u8,
        power_state: (pmcsr & 0x0003) as u8,
        no_soft_reset: pmcsr & 0x0008 != 0,
        pme_enable: pmcsr & 0x0100 != 0,
        data_select: ((pmcsr >> 9) & 0x000f) as u8,
        data_scale: ((pmcsr >> 13) & 0x0003) as u8,
        pme_status: pmcsr & 0x8000 != 0,
    })
}
