pub(crate) mod capability;
mod config;
pub(crate) mod decoders;
mod details;
mod device;
mod error;
mod field;
mod names;
mod session;

pub(crate) use config::ConfigSpaceReader;
pub use config::{ConfigReadFailure, ConfigReadLevel, ConfigSegment, ConfigSpaceSnapshot};
pub use decoders::aer::{AER_CE_BITS, AER_UE_BITS};
pub use decoders::{
    AcsCapability, AerCapability, AriCapability, AtsCapability, DpcCapability, DsnCapability,
    DvsecCapability, HotPlugCapability, LtrCapability, MsiCapability, MsiXCapability,
    PasidCapability, PciCapabilityContent, PciXCapability, PcieCapability, PmCapability,
    PriCapability, PtmCapability, SlotIdCapability, SriovCapability, TphCapability,
    VendorExtCapability, VendorSpecificCapability, VpdCapability,
};
pub use details::{PciDeviceDetails, PciInspection, PciResource};
pub use device::{PciAddress, PciAddressParseError, PciDevice, PciSnapshot};
pub use error::PciError;
pub use field::{
    PciCapability, PciCapabilityChainStatus, PciCapabilityKind, PciCapabilityMalformedReason,
    PciCapabilityReport, PciCapabilityState, PciField, PciFieldUnavailableReason,
};
pub use names::capability_name;
pub use session::PciSession;
