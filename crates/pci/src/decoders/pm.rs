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
