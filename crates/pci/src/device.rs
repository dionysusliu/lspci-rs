use std::{
    fmt::{self, Display},
    str::FromStr,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PciAddress {
    pub domain: u16,
    pub bus: u8,
    pub slot: u8,
    pub function: u8,
}

impl Display for PciAddress {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{:04x}:{:02x}:{:02x}.{}",
            self.domain, self.bus, self.slot, self.function
        )
    }
}

impl FromStr for PciAddress {
    type Err = PciAddressParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let bytes = s.as_bytes();

        if bytes.len() != 12 || bytes[4] != b':' || bytes[7] != b':' || bytes[10] != b'.' {
            return Err(PciAddressParseError);
        }

        let domain = parse_hex(&bytes[0..4])?;
        let bus = parse_hex(&bytes[5..7])?;
        let slot = parse_hex(&bytes[8..10])?;
        let function = parse_hex(&bytes[11..12])?;

        // PCI slot maximum is 0x1f (31), PCI function maximum is 0x7
        if slot > 0x1f || function > 0x07 {
            return Err(PciAddressParseError);
        }

        Ok(Self {
            domain,
            bus: bus as u8,
            slot: slot as u8,
            function: function as u8,
        })
    }
}

/// helper function parsing hex byte string to u16
fn parse_hex(bytes: &[u8]) -> Result<u16, PciAddressParseError> {
    let mut value = 0u16;

    for byte in bytes {
        let digit = match byte {
            b'0'..=b'9' => byte - b'0',
            b'a'..=b'f' => byte - b'a' + 10,
            b'A'..=b'F' => byte - b'A' + 10,
            _ => return Err(PciAddressParseError),
        };

        value = value * 16 + u16::from(digit);
    }

    Ok(value)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PciAddressParseError;

impl Display for PciAddressParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Invalid PCI address; expected dddd:bb:ss.f")
    }
}

impl std::error::Error for PciAddressParseError {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PciDevice {
    pub address: PciAddress,

    pub vendor_id: u16,
    pub device_id: u16,
    pub class_id: u16,

    pub vendor_name: String,
    pub device_name: String,
    pub class_name: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PciSnapshot {
    devices: Vec<PciDevice>,
}

impl PciSnapshot {
    pub fn devices(&self) -> &[PciDevice] {
        self.devices.as_ref()
    }

    pub(crate) fn from_devices(devices: Vec<PciDevice>) -> Self {
        Self { devices }
    }
}
