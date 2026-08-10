use crate::ConfigSpaceSnapshot;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VendorSpecificCapability {
    pub length: u8,
    pub data: Vec<u8>,
}

pub fn decode_vendor_specific(
    snapshot: &ConfigSpaceSnapshot,
    offset: u16,
) -> Option<VendorSpecificCapability> {
    let base = u32::from(offset);
    let length = snapshot.read(base + 2, 1).ok()?[0];
    let data = if length == 0 {
        Vec::new()
    } else {
        snapshot.read(base + 3, u32::from(length)).ok()?
    };

    Some(VendorSpecificCapability { length, data })
}
