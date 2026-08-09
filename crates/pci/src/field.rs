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
