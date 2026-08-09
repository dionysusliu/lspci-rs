#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PciField<T> {
    Available(T),
    Unavailable { reason: PciFieldUnavailableReason },
    NotApplicable,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PciFieldUnavailableReason {
    PermissionDenied,
    UnsupportedByBackend,
    UnsupportedByLibrary,
    DeviceUnavailable,
    NotBound,
    ReadError,
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PciCapabilityState {
    Valid,
    Truncated,
    Unavailable(PciFieldUnavailableReason),
    Malformed(PciCapabilityMalformedReason),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PciCapabilityMalformedReason {
    MisalignedOffset,
    OffsetOutOfRange,
    CycleDetected,
    InvalidNextPointer,
    MissingHeader,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PciCapabilityChainStatus {
    NotPresent,
    Complete,
    Truncated,
    Unavailable(PciFieldUnavailableReason),
    Malformed(PciCapabilityMalformedReason),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PciCapabilityReport {
    pub standard: Vec<PciCapability>,
    pub extended: Vec<PciCapability>,
    pub standard_status: PciCapabilityChainStatus,
    pub extended_status: PciCapabilityChainStatus,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PciCapabilityKind {
    Standard,
    Extended,
    Unknown(u16),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PciCapability {
    pub id: u16,
    pub kind: PciCapabilityKind,
    pub offset: u16,
    pub next: Option<u16>,
    pub state: PciCapabilityState,
}
