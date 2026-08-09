# PCI Configuration Space and Capability Design

Date: 2026-08-09
Status: Approved design
Scope: read-only configuration-space inspection and generic capability discovery

## 1. Goal

Add a Rust configuration-space layer that follows the observable semantics of
pciutils/lspci while preserving the project's boundaries:

- libpci remains the system-interaction backend;
- reads are lazy and may succeed only for a prefix or for disjoint segments;
- partial reads and their reasons remain observable;
- standard and extended PCI capability chains are discovered generically;
- configuration-space writes and protocol-specific decoders are out of scope.

The implementation must work against real PCI devices. It must not use fixtures
that pretend to be hardware responses.

## 2. Design decisions

### 2.1 Segment-based cache

A configuration space is modeled as an observed byte range with possible holes,
not as a fixed `[u8; 4096]` whose unread bytes are zero-filled.

```rust
pub struct ConfigSegment {
    pub offset: u32,
    pub bytes: Vec<u8>,
}

pub struct ConfigReadFailure {
    pub offset: u32,
    pub length: u32,
    pub reason: PciFieldUnavailableReason,
}

pub struct ConfigSpaceSnapshot {
    pub requested: Range<u32>,
    pub segments: Vec<ConfigSegment>,
    pub failures: Vec<ConfigReadFailure>,
}
```

`offset` is retained because reads may be non-contiguous, capability pointers
are offsets into the PCI configuration space, and partial failures must not be
represented by invented bytes.

The internal reader owns the snapshot while a device is being inspected:

```rust
struct ConfigSpaceReader<'a> {
    raw: *mut pci_sys::bindings::pci_dev,
    snapshot: ConfigSpaceSnapshot,
    _session: PhantomData<&'a mut PciSession>,
}
```

Its `fetch(offset, length)` operation avoids already-covered ranges, reads only
missing ranges through `pci_read_block`, appends or merges successful segments,
and records failures without discarding previous successes.

### 2.2 Read levels

The public read level is explicit rather than based on repeated `-x` flags:

```rust
pub enum ConfigReadLevel {
    Header,   // 0x000..0x040
    Standard, // 0x000..0x100
    Extended, // 0x000..0x1000
}
```

These ranges correspond to the useful `lspci` targets for header, standard
configuration space, and extended configuration space. A failed suffix does
not invalidate an already-read prefix.

### 2.3 Capability discovery

A capability walker consumes the reader abstraction rather than calling FFI
directly. It first obtains the smallest required header and then follows the
device's linked list.

Standard capabilities begin at the header pointer at `0x34`; each standard
header contains an ID and a next pointer. Extended capabilities begin at
`0x100`; each extended header contains an ID, version, and next pointer.

The generic result contains identity and chain metadata, not protocol-specific
fields:

```rust
pub struct PciCapability {
    pub id: u16,
    pub kind: PciCapabilityKind,
    pub offset: u16,
    pub next: Option<u16>,
    pub state: PciCapabilityState,
}

pub struct PciCapabilityReport {
    pub standard: Vec<PciCapability>,
    pub extended: Vec<PciCapability>,
    pub standard_status: PciCapabilityChainStatus,
    pub extended_status: PciCapabilityChainStatus,
}
```

The walker detects misalignment, out-of-range pointers, cycles, missing
headers, and excessive traversal. It preserves entries discovered before a
later failure. Unknown IDs are data, not parser errors.

MSI, MSI-X, PCI Express, AER, SR-IOV, and other protocol-specific decoders are
a later layer over this generic report.

### 2.4 Inspection versus raw configuration dump

A normal inspection returns semantic information only:

```rust
pub struct PciInspection {
    pub device: PciDevice,
    pub details: PciDeviceDetails,
}
```

`PciDeviceDetails` retains the decoded `PciCapabilityReport`, but does not own
the complete raw configuration space.

Raw bytes are exposed only through an explicit API:

```rust
impl PciSession {
    pub fn read_config(
        &mut self,
        address: PciAddress,
        level: ConfigReadLevel,
    ) -> Result<ConfigSpaceSnapshot, PciError>;
}
```

This keeps ordinary inspection small while still supporting hex dumps, JSON
diagnostics, and a future TUI. A future interactive mode may add a longer-lived
cache, but that is not required by this read-only slice.

### 2.5 Session and FFI ownership

`PciSession` owns the libpci context. Raw `pci_dev` pointers are used only
inside the session's active scan and never escape into public result types.
`ConfigSpaceSnapshot` owns its `Vec<u8>` data and is safe to retain after the
libpci context is gone.

All calls to `pci_read_block` are concentrated in one unsafe helper. No
configuration write operation is exposed in this phase.

## 3. Error semantics

A summary/identity failure remains a `PciError::DeviceInfo` failure. A failure
to read one configuration-space range is a `ConfigReadFailure` and does not
discard successful ranges or fail the whole inspection.

Reasons must reflect evidence:

- use `PermissionDenied` only when libpci or the underlying system explicitly
  indicates denied access;
- use `ReadError` when `pci_read_block` fails without a more specific cause;
- use `UnsupportedByBackend` when the backend cannot provide the requested
  range;
- use `DeviceUnavailable` when the device disappears or becomes unavailable;
- do not infer permission failure from an unexplained zero return value.

If the configuration header cannot be read, the capability field is
`Unavailable`. If the header is readable but no capability list exists, it is
`NotApplicable`. If a chain is partially readable, the report remains
available and its chain status records truncation or the specific malformed
condition.

## 4. CLI and output

`list` remains summary-only and does not read configuration space.
`show` keeps its current behavior and gains an explicit configuration option:

```text
lspci-rs show 0000:00:05.0 --config header
lspci-rs show 0000:00:05.0 --config standard
lspci-rs show 0000:00:05.0 --config extended
```

Text output separates raw bytes from decoded capability metadata. It reports
requested ranges, successful segments, unavailable ranges, and reasons. JSON
uses explicit `segments` and `failures` arrays and never zero-fills unread
bytes.

## 5. Implementation order

1. Remove raw `config_space` from `PciDeviceDetails` and introduce the final
   capability report types.
2. Add the segment cache and `ConfigSpaceReader::fetch`.
3. Add standard and extended capability walkers.
4. Integrate temporary config reads into `PciSession::inspect`.
5. Add `PciSession::read_config` for explicit raw dumps.
6. Add the `show --config` CLI option.
7. Update text and JSON renderers.
8. Verify formatting, workspace compilation, and real ECS output against
   `lspci`.

No fixture-based hardware tests are part of this slice. Validation uses the
real ECS PCI environment and compiler/static checks.
