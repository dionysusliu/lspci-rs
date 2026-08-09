use std::ops::Range;

use crate::PciFieldUnavailableReason;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConfigReadLevel {
    Header,
    Standard,
    Extended,
}

impl ConfigReadLevel {
    pub fn range(self) -> Range<u32> {
        match self {
            Self::Header => 0x000..0x040,
            Self::Standard => 0x000..0x100,
            Self::Extended => 0x000..0x1000,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConfigSegment {
    pub offset: u32,
    pub bytes: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConfigReadFailure {
    pub offset: u32,
    pub length: u32,
    pub reason: PciFieldUnavailableReason,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConfigSpaceSnapshot {
    pub requested: Range<u32>,
    pub segments: Vec<ConfigSegment>,
    pub failures: Vec<ConfigReadFailure>,
}

impl ConfigSpaceSnapshot {
    pub(crate) fn new(requested: Range<u32>) -> Self {
        Self {
            requested,
            segments: Vec::new(),
            failures: Vec::new(),
        }
    }
}
