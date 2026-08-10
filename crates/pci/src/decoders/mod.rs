pub mod pm;

pub use pm::PmCapability;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PciCapabilityContent {
    Pm(PmCapability),
}
