use std::collections::HashSet;

use crate::{
    ConfigSpaceReader, PciCapability, PciCapabilityChainStatus, PciCapabilityKind,
    PciCapabilityMalformedReason, PciCapabilityReport, PciCapabilityState,
};

pub(crate) fn discover(reader: &mut ConfigSpaceReader) -> PciCapabilityReport {
    let standard = discover_standard(reader);
    let extended = discover_extended(reader);

    PciCapabilityReport {
        standard: standard.capabilities,
        extended: extended.capabilities,
        standard_status: standard.status,
        extended_status: extended.status,
    }
}

struct ChainDiscovery {
    capabilities: Vec<PciCapability>,
    status: PciCapabilityChainStatus,
}

fn discover_standard(reader: &mut ConfigSpaceReader) -> ChainDiscovery {
    let header = match reader.read(0, 0x40) {
        Ok(bytes) => bytes,
        Err(failure) => {
            return ChainDiscovery {
                capabilities: Vec::new(),
                status: PciCapabilityChainStatus::Unavailable(failure.reason),
            };
        }
    };

    let status = u16::from_le_bytes([header[0x06], header[0x07]]);
    if status & 0x0010 == 0 {
        return ChainDiscovery {
            capabilities: Vec::new(),
            status: PciCapabilityChainStatus::NotPresent,
        };
    }

    let pointer = u16::from(header[0x34]);
    if pointer == 0 {
        return ChainDiscovery {
            capabilities: Vec::new(),
            status: PciCapabilityChainStatus::Malformed(
                PciCapabilityMalformedReason::InvalidNextPointer,
            ),
        };
    }

    walk_standard_chain(reader, pointer)
}

fn walk_standard_chain(reader: &mut ConfigSpaceReader, start: u16) -> ChainDiscovery {
    walk_chain(
        reader,
        start,
        0x40..0x100,
        48,
        2,
        PciCapabilityKind::Standard,
        |bytes| u16::from(bytes[0]),
        |bytes| u16::from(bytes[1]),
        ChainKind::Standard,
    )
}

fn discover_extended(reader: &mut ConfigSpaceReader) -> ChainDiscovery {
    walk_chain(
        reader,
        0x100,
        0x100..0x1000,
        256,
        4,
        PciCapabilityKind::Extended,
        |bytes| u16::from_le_bytes([bytes[0], bytes[1]]),
        |bytes| ((u16::from(bytes[2]) | (u16::from(bytes[3]) << 8)) >> 4) & 0x0fff,
        ChainKind::Extended,
    )
}

#[derive(Clone, Copy)]
enum ChainKind {
    Standard,
    Extended,
}

fn walk_chain(
    reader: &mut ConfigSpaceReader,
    start: u16,
    valid_range: std::ops::Range<u16>,
    max_nodes: usize,
    header_len: u32,
    kind: PciCapabilityKind,
    read_id: impl Fn(&[u8]) -> u16,
    read_next: impl Fn(&[u8]) -> u16,
    chain_kind: ChainKind,
) -> ChainDiscovery {
    if !valid_range.contains(&start) || start % 4 != 0 {
        return ChainDiscovery {
            capabilities: Vec::new(),
            status: malformed_start(start, &valid_range),
        };
    }

    let mut capabilities = Vec::new();
    let mut visited = HashSet::new();
    let mut current = start;

    loop {
        if capabilities.len() >= max_nodes {
            return ChainDiscovery {
                capabilities,
                status: PciCapabilityChainStatus::Truncated,
            };
        }

        if !visited.insert(current) {
            return ChainDiscovery {
                capabilities,
                status: PciCapabilityChainStatus::Malformed(
                    PciCapabilityMalformedReason::CycleDetected,
                ),
            };
        }

        let header = match reader.read(u32::from(current), header_len) {
            Ok(bytes) => bytes,
            Err(failure) => {
                return ChainDiscovery {
                    capabilities,
                    status: if capabilities.is_empty() {
                        PciCapabilityChainStatus::Unavailable(failure.reason)
                    } else {
                        PciCapabilityChainStatus::Truncated
                    },
                };
            }
        };

        if header.iter().all(|byte| *byte == 0) {
            return if capabilities.is_empty()
                && matches!(chain_kind, ChainKind::Extended)
                && current == 0x100
            {
                ChainDiscovery {
                    capabilities,
                    status: PciCapabilityChainStatus::NotPresent,
                }
            } else {
                ChainDiscovery {
                    capabilities,
                    status: PciCapabilityChainStatus::Malformed(
                        PciCapabilityMalformedReason::MissingHeader,
                    ),
                }
            };
        }

        let id = read_id(&header);
        let next = read_next(&header);
        let next = if next == 0 { None } else { Some(next) };

        let node = PciCapability {
            id,
            kind,
            offset: current,
            next,
            state: PciCapabilityState::Valid,
        };

        capabilities.push(node);

        let Some(next) = next else {
            return ChainDiscovery {
                capabilities,
                status: PciCapabilityChainStatus::Complete,
            };
        };

        if !valid_range.contains(&next) {
            return ChainDiscovery {
                capabilities,
                status: PciCapabilityChainStatus::Malformed(
                    PciCapabilityMalformedReason::OffsetOutOfRange,
                ),
            };
        }

        if next % 4 != 0 {
            return ChainDiscovery {
                capabilities,
                status: PciCapabilityChainStatus::Malformed(
                    PciCapabilityMalformedReason::MisalignedOffset,
                ),
            };
        }

        current = next;
    }
}

fn malformed_start(start: u16, valid_range: &std::ops::Range<u16>) -> PciCapabilityChainStatus {
    if start == 0 {
        PciCapabilityChainStatus::Malformed(PciCapabilityMalformedReason::InvalidNextPointer)
    } else if !valid_range.contains(&start) {
        PciCapabilityChainStatus::Malformed(PciCapabilityMalformedReason::OffsetOutOfRange)
    } else {
        PciCapabilityChainStatus::Malformed(PciCapabilityMalformedReason::MisalignedOffset)
    }
}
