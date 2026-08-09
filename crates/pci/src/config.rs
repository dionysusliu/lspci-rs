use std::{ffi::c_int, ops::Range};

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

pub(crate) struct ConfigSpaceReader {
    raw: *mut pci_sys::bindings::pci_dev,
    snapshot: ConfigSpaceSnapshot,
}

impl ConfigSpaceReader {
    pub(crate) unsafe fn new(raw: *mut pci_sys::bindings::pci_dev, requested: Range<u32>) -> Self {
        Self {
            raw,
            snapshot: ConfigSpaceSnapshot::new(requested),
        }
    }

    pub(crate) fn snapshot(&self) -> &ConfigSpaceSnapshot {
        &self.snapshot
    }

    pub(crate) fn fetch(&mut self, offset: u32, length: u32) -> Result<(), ConfigReadFailure> {
        let end = match offset.checked_add(length) {
            Some(end) if length != 0 => end,
            _ => {
                let failure = self.record_failure(offset, length);
                return Err(failure);
            }
        };

        let mut cursor = offset;
        let mut first_failure = None;

        while cursor < end {
            if let Some(segment_end) = self.covering_segment_end(cursor) {
                cursor = segment_end.min(end);
                continue;
            }

            let gap_end = self.next_segment_start(cursor).unwrap_or(end).min(end);
            let gap_length = gap_end - cursor;

            if let Err(failure) = self.read_range(cursor, gap_length) {
                first_failure.get_or_insert(failure);
            }

            cursor = gap_end;
        }

        match first_failure {
            Some(failure) => Err(failure),
            None => Ok(()),
        }
    }

    pub(crate) fn read(&mut self, offset: u32, length: u32) -> Result<Vec<u8>, ConfigReadFailure> {
        self.fetch(offset, length)?;

        let end = match offset.checked_add(length) {
            Some(end) if length != 0 => end,
            _ => return Err(self.record_failure(offset, length)),
        };

        let mut bytes = Vec::with_capacity(length as usize);
        let mut cursor = offset;

        while cursor < end {
            let segment = match self.segment_covering(cursor) {
                Some(segment) => segment,
                None => return Err(self.failure_for_range(offset, length)),
            };

            let segment_end = segment_end(segment);
            let take_end = segment_end.min(end);
            let start = (cursor - segment.offset) as usize;
            let len = (take_end - cursor) as usize;
            bytes.extend_from_slice(&segment.bytes[start..start + len]);
            cursor = take_end;
        }

        Ok(bytes)
    }

    fn read_range(&mut self, offset: u32, length: u32) -> Result<(), ConfigReadFailure> {
        if length == 0 {
            let failure = self.record_failure(offset, length);
            return Err(failure);
        }

        let len = match c_int::try_from(length) {
            Ok(len) => len,
            Err(_) => {
                let failure = self.record_failure(offset, length);
                return Err(failure);
            }
        };
        let raw_offset = match c_int::try_from(offset) {
            Ok(offset) => offset,
            Err(_) => {
                let failure = self.record_failure(offset, length);
                return Err(failure);
            }
        };

        let mut bytes = vec![0u8; length as usize];
        let read = unsafe {
            pci_sys::bindings::pci_read_block(self.raw, raw_offset, bytes.as_mut_ptr(), len)
        };

        if read != len {
            let failure = self.record_failure(offset, length);
            return Err(failure);
        }

        self.insert_segment(offset, bytes);
        Ok(())
    }

    fn insert_segment(&mut self, offset: u32, bytes: Vec<u8>) {
        self.snapshot.segments.push(ConfigSegment { offset, bytes });
        self.snapshot.segments.sort_by_key(|segment| segment.offset);

        let mut merged = Vec::with_capacity(self.snapshot.segments.len());

        for segment in self.snapshot.segments.drain(..) {
            match merged.last_mut() {
                Some(last) => {
                    let last_end = segment_end(last);
                    if segment.offset <= last_end {
                        let segment_end = segment_end(&segment);
                        if segment_end > last_end {
                            let start = (last_end - segment.offset) as usize;
                            last.bytes.extend_from_slice(&segment.bytes[start..]);
                        }
                    } else {
                        merged.push(segment);
                    }
                }
                None => merged.push(segment),
            }
        }

        self.snapshot.segments = merged;
    }

    fn segment_covering(&self, offset: u32) -> Option<&ConfigSegment> {
        self.snapshot
            .segments
            .iter()
            .find(|segment| segment.offset <= offset && offset < segment_end(segment))
    }

    fn covering_segment_end(&self, offset: u32) -> Option<u32> {
        self.segment_covering(offset).map(segment_end)
    }

    fn next_segment_start(&self, offset: u32) -> Option<u32> {
        self.snapshot
            .segments
            .iter()
            .find(|segment| segment.offset > offset)
            .map(|segment| segment.offset)
    }

    fn record_failure(&mut self, offset: u32, length: u32) -> ConfigReadFailure {
        let failure = ConfigReadFailure {
            offset,
            length,
            reason: PciFieldUnavailableReason::ReadError,
        };
        self.snapshot.failures.push(failure.clone());
        failure
    }

    fn failure_for_range(&self, offset: u32, length: u32) -> ConfigReadFailure {
        self.snapshot
            .failures
            .iter()
            .find(|failure| {
                let failure_end = failure.offset.saturating_add(failure.length);
                let range_end = offset.saturating_add(length);
                failure.offset < range_end && offset < failure_end
            })
            .cloned()
            .unwrap_or(ConfigReadFailure {
                offset,
                length,
                reason: PciFieldUnavailableReason::ReadError,
            })
    }
}

fn segment_end(segment: &ConfigSegment) -> u32 {
    segment
        .offset
        .checked_add(segment.bytes.len() as u32)
        .expect("config segment end overflow")
}
