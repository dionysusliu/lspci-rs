use crate::ConfigSpaceSnapshot;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HotPlugCapability {
    pub hot_plug_capable: bool,
}

pub fn decode_hot_plug(snapshot: &ConfigSpaceSnapshot, offset: u16) -> Option<HotPlugCapability> {
    let base = u32::from(offset);
    let flags = snapshot.read(base + 2, 1).ok()?[0];

    Some(HotPlugCapability {
        hot_plug_capable: flags & 0x01 != 0,
    })
}
