use crate::ConfigSpaceSnapshot;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HotPlugCapability {
    pub hot_plug_capable: bool,
}

pub fn decode_hot_plug(_snapshot: &ConfigSpaceSnapshot, _offset: u16) -> Option<HotPlugCapability> {
    // lspci treats the presence of this capability as sufficient evidence
    Some(HotPlugCapability {
        hot_plug_capable: true,
    })
}
