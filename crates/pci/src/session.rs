use std::{ffi::CStr, ptr};

use pci_sys::bindings::{
    PCI_FILL_BASES, PCI_FILL_CLASS, PCI_FILL_CLASS_EXT, PCI_FILL_DRIVER, PCI_FILL_IDENT,
    PCI_FILL_IO_FLAGS, PCI_FILL_IRQ, PCI_FILL_PARENT, PCI_FILL_SIZES, PCI_FILL_SUBSYS, pci_alloc,
    pci_cleanup, pci_fill_info, pci_get_string_property, pci_init,
    pci_lookup_mode_PCI_LOOKUP_NO_NUMBERS, pci_scan_bus,
};

use crate::{
    ConfigReadLevel, ConfigSpaceReader, ConfigSpaceSnapshot, PciAddress, PciCapabilityChainStatus,
    PciCapabilityReport, PciCapabilityState, PciDevice, PciDeviceDetails, PciError, PciField,
    PciFieldUnavailableReason, PciResource, PciSnapshot, capability, decoders,
    details::PciInspection,
};

/// all fields we would like to fill into pci_access using libpci
const INSPECT_FIELDS: u32 = PCI_FILL_IDENT
    | PCI_FILL_CLASS
    | PCI_FILL_IRQ
    | PCI_FILL_BASES
    | PCI_FILL_SIZES
    | PCI_FILL_IO_FLAGS
    | PCI_FILL_CLASS_EXT
    | PCI_FILL_SUBSYS
    | PCI_FILL_PARENT
    | PCI_FILL_DRIVER;

/// all necessary fields to fill into
const REQUIRED_INSPECT_FIELDS: u32 = PCI_FILL_IDENT | PCI_FILL_CLASS;
const RESOURCE_FIELDS: u32 = PCI_FILL_BASES | PCI_FILL_SIZES | PCI_FILL_IO_FLAGS;

pub struct PciSession {
    access: *mut pci_sys::bindings::pci_access,
}

impl PciSession {
    pub fn new() -> Result<Self, PciError> {
        let access = unsafe { pci_alloc() };

        if access.is_null() {
            return Err(PciError::Allocation);
        }

        unsafe {
            pci_init(access);
        }

        Ok(Self { access })
    }

    pub fn inspect(&mut self, address: PciAddress) -> Result<PciInspection, PciError> {
        let raw = unsafe { self.find_raw_device(address)? };

        unsafe {
            let result = pci_fill_info(raw, INSPECT_FIELDS as std::os::raw::c_int);

            if result < 0 {
                return Err(PciError::DeviceInfo {
                    address,
                    known_fields: result as u32,
                    requested_fields: INSPECT_FIELDS,
                });
            }

            let known_fields = result as u32;
            if known_fields & REQUIRED_INSPECT_FIELDS != REQUIRED_INSPECT_FIELDS {
                return Err(PciError::DeviceInfo {
                    address,
                    known_fields,
                    requested_fields: REQUIRED_INSPECT_FIELDS,
                });
            }

            let device = Self::device_from_raw(self.access, raw);
            let capabilities = {
                let mut reader = ConfigSpaceReader::new(raw, 0x000..0x1000);
                let header_readable = reader.read(0x000, 0x040).is_ok();
                let mut report = capability::discover(&mut reader);

                if header_readable {
                    for capability in report.standard.iter() {
                        if matches!(capability.state, PciCapabilityState::Valid) {
                            let start = u32::from(capability.offset);
                            let end = (start + 0x40).min(0x100);
                            let _ = reader.fetch(start, end - start);
                        }
                    }

                    // vendor-specific payloads can exceed the 64-byte prefetch
                    for capability in report.standard.iter() {
                        if capability.id == 0x09
                            && matches!(capability.state, PciCapabilityState::Valid)
                        {
                            let length_offset = u32::from(capability.offset) + 2;
                            if let Ok(bytes) = reader.snapshot().read(length_offset, 1) {
                                let start = u32::from(capability.offset) + 3;
                                let end = (start + u32::from(bytes[0])).min(0x100);
                                if end > start {
                                    let _ = reader.fetch(start, end - start);
                                }
                            }
                        }
                    }

                    let snapshot = reader.snapshot();
                    for capability in report.standard.iter_mut() {
                        decoders::decode_content(snapshot, capability);
                    }
                }

                Self::capabilities_from_report(report, header_readable)
            };
            let details = Self::details_from_raw(raw, known_fields, capabilities);

            Ok(PciInspection { device, details })
        }
    }

    pub fn read_config(
        &mut self,
        address: PciAddress,
        level: ConfigReadLevel,
    ) -> Result<ConfigSpaceSnapshot, PciError> {
        let raw = unsafe { self.find_raw_device(address)? };
        let requested = level.range();
        let length = requested.end - requested.start;

        unsafe {
            let mut reader = ConfigSpaceReader::new(raw, requested.clone());
            let _ = reader.fetch(requested.start, length);
            Ok(reader.snapshot().clone())
        }
    }

    /// helper function: find raw libpci device matching the address
    unsafe fn find_raw_device(
        &mut self,
        address: PciAddress,
    ) -> Result<*mut pci_sys::bindings::pci_dev, PciError> {
        unsafe {
            pci_scan_bus(self.access);

            let mut raw = (*self.access).devices;

            while !raw.is_null() {
                if Self::address_from_raw(raw) == address {
                    return Ok(raw);
                }

                raw = (*raw).next;
            }
        }

        Err(PciError::DeviceNotFound { address })
    }

    /// helper function: extract PciAddress from raw libpci context
    unsafe fn address_from_raw(raw: *mut pci_sys::bindings::pci_dev) -> PciAddress {
        unsafe {
            PciAddress {
                domain: (*raw).domain_16,
                bus: (*raw).bus,
                slot: (*raw).dev,
                function: (*raw).func,
            }
        }
    }

    /// helper function: extract PciDevice from raw libpci context
    unsafe fn device_from_raw(
        access: *mut pci_sys::bindings::pci_access,
        raw: *mut pci_sys::bindings::pci_dev,
    ) -> PciDevice {
        unsafe {
            let address = Self::address_from_raw(raw);
            let vendor_id = (*raw).vendor_id;
            let device_id = (*raw).device_id;
            let class_id = (*raw).device_class;

            PciDevice {
                address,
                vendor_id,
                device_id,
                class_id,
                vendor_name: Self::lookup_vendor(access, vendor_id),
                device_name: Self::lookup_device(access, vendor_id, device_id),
                class_name: Self::lookup_class(access, class_id),
            }
        }
    }

    /// try collecting all device details from raw pci_dev context
    unsafe fn details_from_raw(
        raw: *mut pci_sys::bindings::pci_dev,
        known_fields: u32,
        capabilities: PciField<PciCapabilityReport>,
    ) -> PciDeviceDetails {
        unsafe {
            let revision = if known_fields & PCI_FILL_CLASS_EXT != 0 {
                PciField::Available((*raw).rev_id)
            } else {
                unavailable()
            };

            let programming_interface = if known_fields & PCI_FILL_CLASS_EXT != 0 {
                PciField::Available((*raw).prog_if)
            } else {
                unavailable()
            };

            let subsystem_vendor_id = if known_fields & PCI_FILL_SUBSYS != 0 {
                PciField::Available((*raw).subsys_vendor_id)
            } else {
                unavailable()
            };

            let subsystem_device_id = if known_fields & PCI_FILL_SUBSYS != 0 {
                PciField::Available((*raw).subsys_id)
            } else {
                unavailable()
            };

            let parent = if known_fields & PCI_FILL_PARENT == 0 {
                unavailable()
            } else if (*raw).parent.is_null() {
                PciField::NotApplicable
            } else {
                PciField::Available(Self::address_from_raw((*raw).parent))
            };

            let irq = if known_fields & PCI_FILL_IRQ == 0 {
                unavailable()
            } else if (*raw).irq < 0 {
                PciField::NotApplicable
            } else {
                PciField::Available((*raw).irq as u32)
            };

            let driver = if known_fields & PCI_FILL_DRIVER == 0 {
                unavailable()
            } else {
                let driver_ptr = pci_get_string_property(raw, PCI_FILL_DRIVER);

                if driver_ptr.is_null() {
                    PciField::Unavailable {
                        reason: crate::PciFieldUnavailableReason::NotBound,
                    }
                } else {
                    let driver = CStr::from_ptr(driver_ptr).to_string_lossy().into_owned();
                    PciField::Available(driver)
                }
            };

            let resources = if known_fields & RESOURCE_FIELDS != RESOURCE_FIELDS {
                unavailable()
            } else {
                let mut values = Vec::new();

                for index in 0..6 {
                    let raw_base = (*raw).base_addr[index] as u64;
                    let start = if raw_base & 0x1 != 0 {
                        // I/O BAR, last 2 bits are attributes
                        raw_base & !0x3
                    } else {
                        // Memory BAR, last 4bites are attributes
                        raw_base & !0xf
                    };

                    let size = (*raw).size[index] as u64;
                    let flags = (*raw).flags[index] as u64;

                    if start != 0 || size != 0 || flags != 0 {
                        values.push(PciResource {
                            index: index as u8,
                            start,
                            size,
                            flags,
                        });
                    }
                }

                if values.is_empty() {
                    PciField::NotApplicable
                } else {
                    PciField::Available(values)
                }
            };

            PciDeviceDetails {
                revision,
                programming_interface,
                subsystem_vendor_id,
                subsystem_device_id,
                parent,
                irq,
                driver,
                resources,
                capabilities,
            }
        }
    }

    fn capabilities_from_report(
        report: PciCapabilityReport,
        header_readable: bool,
    ) -> PciField<PciCapabilityReport> {
        if !header_readable {
            let reason = match report.standard_status {
                PciCapabilityChainStatus::Unavailable(reason) => reason,
                _ => PciFieldUnavailableReason::ReadError,
            };
            return PciField::Unavailable { reason };
        }

        if matches!(report.standard_status, PciCapabilityChainStatus::NotPresent)
            && matches!(report.extended_status, PciCapabilityChainStatus::NotPresent)
        {
            PciField::NotApplicable
        } else {
            PciField::Available(report)
        }
    }

    pub fn scan(&mut self) -> Result<PciSnapshot, PciError> {
        let mut devices = Vec::new();

        unsafe {
            pci_scan_bus(self.access);

            let requested_fields = (PCI_FILL_IDENT | PCI_FILL_CLASS) as u32;
            let mut raw = (*self.access).devices;

            while !raw.is_null() {
                let known_fields =
                    pci_fill_info(raw, requested_fields as std::os::raw::c_int) as u32;

                let address = PciAddress {
                    domain: (*raw).domain_16,
                    bus: (*raw).bus,
                    slot: (*raw).dev,
                    function: (*raw).func,
                };

                if known_fields & requested_fields != requested_fields {
                    return Err(PciError::DeviceInfo {
                        address,
                        known_fields,
                        requested_fields,
                    });
                }

                let vendor_id = (*raw).vendor_id;
                let device_id = (*raw).device_id;
                let class_id = (*raw).device_class;

                let vendor_name = Self::lookup_vendor(self.access, vendor_id);

                let device_name = Self::lookup_device(self.access, vendor_id, device_id);

                let class_name = Self::lookup_class(self.access, class_id);

                devices.push(PciDevice {
                    address,
                    vendor_id,
                    device_id,
                    class_id,
                    vendor_name,
                    device_name,
                    class_name,
                });

                raw = (*raw).next;
            }
        }

        Ok(PciSnapshot::from_devices(devices))
    }

    fn lookup_vendor(access: *mut pci_sys::bindings::pci_access, vendor_id: u16) -> String {
        let mut buffer = [0 as std::os::raw::c_char; 256];

        let flags = (pci_sys::bindings::pci_lookup_mode_PCI_LOOKUP_VENDOR
            | pci_lookup_mode_PCI_LOOKUP_NO_NUMBERS) as std::os::raw::c_int;
        let result = unsafe {
            pci_sys::bindings::pci_lookup_name(
                access,
                buffer.as_mut_ptr(),
                buffer.len() as std::os::raw::c_int,
                flags,
                vendor_id as std::os::raw::c_int,
            )
        };

        name_from_ptr(result)
    }

    fn lookup_device(
        access: *mut pci_sys::bindings::pci_access,
        vendor_id: u16,
        device_id: u16,
    ) -> String {
        let mut buffer = [0 as std::os::raw::c_char; 256];

        let flags = (pci_sys::bindings::pci_lookup_mode_PCI_LOOKUP_DEVICE
            | pci_lookup_mode_PCI_LOOKUP_NO_NUMBERS) as std::os::raw::c_int;

        let result = unsafe {
            pci_sys::bindings::pci_lookup_name(
                access,
                buffer.as_mut_ptr(),
                buffer.len() as std::os::raw::c_int,
                flags,
                vendor_id as std::os::raw::c_int,
                device_id as std::os::raw::c_int,
            )
        };

        name_from_ptr(result)
    }

    fn lookup_class(access: *mut pci_sys::bindings::pci_access, class_id: u16) -> String {
        let mut buffer = [0 as std::os::raw::c_char; 256];

        let flags = (pci_sys::bindings::pci_lookup_mode_PCI_LOOKUP_CLASS
            | pci_lookup_mode_PCI_LOOKUP_NO_NUMBERS) as std::os::raw::c_int;

        let result = unsafe {
            pci_sys::bindings::pci_lookup_name(
                access,
                buffer.as_mut_ptr(),
                buffer.len() as std::os::raw::c_int,
                flags,
                class_id as std::os::raw::c_int,
            )
        };

        name_from_ptr(result)
    }
}

impl Drop for PciSession {
    fn drop(&mut self) {
        if !self.access.is_null() {
            unsafe {
                pci_cleanup(self.access);
            }
        }

        self.access = ptr::null_mut()
    }
}

/// helper function, for parsing result of pci_lookup_name()
fn name_from_ptr(ptr: *mut std::os::raw::c_char) -> String {
    if ptr.is_null() {
        return "<unknown>".to_owned();
    }

    unsafe {
        std::ffi::CStr::from_ptr(ptr)
            .to_str()
            .map(str::to_owned)
            .unwrap_or_else(|_| "<unknown>".to_owned())
    }
}

/// helper function,  return an unavailable field
fn unavailable<T>() -> PciField<T> {
    PciField::Unavailable {
        reason: crate::PciFieldUnavailableReason::Unknown,
    }
}
