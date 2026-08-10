use super::read_dword;
use crate::ConfigSpaceSnapshot;

pub const AER_UE_BITS: &[(u8, &str)] = &[
    (4, "DLP"),
    (5, "SDES"),
    (8, "TLP"),
    (9, "FCP"),
    (10, "CmpltTO"),
    (11, "CmpltAbrt"),
    (12, "UnxCmplt"),
    (13, "RxOF"),
    (14, "MalfTLP"),
    (15, "ECRC"),
    (16, "UnsupReq"),
    (17, "ACSViol"),
    (18, "UncorrIntErr"),
    (19, "BlockedTLP"),
    (20, "AtomicOpBlocked"),
    (21, "TLPPrefixBlocked"),
];

pub const AER_CE_BITS: &[(u8, &str)] = &[
    (0, "RxErr"),
    (6, "BadTLP"),
    (7, "BadDLLP"),
    (8, "Rollover"),
    (9, "Timeout"),
    (13, "AdvNonFatalErr"),
    (14, "CorrIntErr"),
    (15, "HeaderOF"),
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AerCapability {
    pub version: u8,
    pub ue_status: u32,
    pub ue_mask: u32,
    pub ue_severity: u32,
    pub ce_status: u32,
    pub ce_mask: u32,
    pub capabilities_control: u32,
    pub first_error_pointer: u8,
    pub header_log: [u32; 4],
    pub root_command: Option<u32>,
    pub root_status: Option<u32>,
    pub error_source_id: Option<u32>,
    pub tlp_prefix_log: [u32; 4],
}

pub fn decode_aer(snapshot: &ConfigSpaceSnapshot, offset: u16) -> Option<AerCapability> {
    let base = u32::from(offset);

    let header = read_dword(snapshot, base).ok()?;
    let version = ((header >> 16) & 0x000f) as u8;

    let ue_status = read_dword(snapshot, base + 0x04).ok()?;
    let ue_mask = read_dword(snapshot, base + 0x08).ok()?;
    let ue_severity = read_dword(snapshot, base + 0x0c).ok()?;
    let ce_status = read_dword(snapshot, base + 0x10).ok()?;
    let ce_mask = read_dword(snapshot, base + 0x14).ok()?;
    let capabilities_control = read_dword(snapshot, base + 0x18).ok()?;

    let mut header_log = [0u32; 4];
    for (index, entry) in header_log.iter_mut().enumerate() {
        *entry = read_dword(snapshot, base + 0x1c + (index as u32) * 4).ok()?;
    }

    let is_bridge = snapshot
        .read(0x0e, 1)
        .ok()
        .map(|bytes| bytes[0] & 0x7f == 1)
        .unwrap_or(false);

    let (root_command, root_status, error_source_id) = if is_bridge {
        (
            Some(read_dword(snapshot, base + 0x2c).ok()?),
            Some(read_dword(snapshot, base + 0x30).ok()?),
            Some(read_dword(snapshot, base + 0x34).ok()?),
        )
    } else {
        (None, None, None)
    };

    let mut tlp_prefix_log = [0u32; 4];
    for (index, entry) in tlp_prefix_log.iter_mut().enumerate() {
        *entry = read_dword(snapshot, base + 0x38 + (index as u32) * 4).ok()?;
    }

    Some(AerCapability {
        version,
        ue_status,
        ue_mask,
        ue_severity,
        ce_status,
        ce_mask,
        capabilities_control,
        first_error_pointer: ((capabilities_control >> 8) & 0x001f) as u8,
        header_log,
        root_command,
        root_status,
        error_source_id,
        tlp_prefix_log,
    })
}
