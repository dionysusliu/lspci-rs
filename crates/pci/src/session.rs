use std::ptr;

use pci_sys::bindings::{
    PCI_FILL_CLASS, PCI_FILL_IDENT, pci_access, pci_alloc, pci_cleanup, pci_fill_info, pci_init,
    pci_lookup_mode_PCI_LOOKUP_CLASS, pci_lookup_mode_PCI_LOOKUP_DEVICE,
    pci_lookup_mode_PCI_LOOKUP_NO_NUMBERS, pci_lookup_mode_PCI_LOOKUP_VENDOR, pci_lookup_name,
    pci_scan_bus,
};

use crate::{PciAddress, PciDevice, PciError, PciSnapshot};

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
            | pci_sys::bindings::pci_lookup_mode_PCI_LOOKUP_NO_NUMBERS)
            as std::os::raw::c_int;

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
            | pci_sys::bindings::pci_lookup_mode_PCI_LOOKUP_NO_NUMBERS)
            as std::os::raw::c_int;

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
