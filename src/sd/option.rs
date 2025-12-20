//! SOME/IP-SD option types.

use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6};

use crate::error::{Result, SomeIpError};

use super::types::{OptionType, TransportProtocol, SD_OPTION_HEADER_SIZE};

/// IPv4 endpoint option.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IPv4EndpointOption {
    /// IPv4 address.
    pub address: Ipv4Addr,
    /// Transport protocol (TCP or UDP).
    pub protocol: TransportProtocol,
    /// Port number.
    pub port: u16,
}

impl IPv4EndpointOption {
    /// Size of an IPv4 endpoint option (excluding header).
    pub const DATA_SIZE: usize = 9;

    /// Create a new IPv4 endpoint option.
    pub fn new(address: Ipv4Addr, protocol: TransportProtocol, port: u16) -> Self {
        Self {
            address,
            protocol,
            port,
        }
    }

    /// Create from a socket address.
    pub fn from_socket_addr(addr: SocketAddrV4, protocol: TransportProtocol) -> Self {
        Self {
            address: *addr.ip(),
            protocol,
            port: addr.port(),
        }
    }

    /// Convert to a socket address.
    pub fn to_socket_addr(&self) -> SocketAddrV4 {
        SocketAddrV4::new(self.address, self.port)
    }

    /// Parse from bytes (excluding the option header).
    pub fn from_bytes(data: &[u8]) -> Result<Self> {
        if data.len() < Self::DATA_SIZE {
            return Err(SomeIpError::MessageTooShort {
                expected: Self::DATA_SIZE,
                actual: data.len(),
            });
        }

        let address = Ipv4Addr::new(data[0], data[1], data[2], data[3]);
        // data[4] is reserved
        let protocol = TransportProtocol::from_u8(data[5])
            .ok_or_else(|| SomeIpError::invalid_header(format!("Unknown protocol: 0x{:02X}", data[5])))?;
        let port = u16::from_be_bytes([data[6], data[7]]);

        Ok(Self {
            address,
            protocol,
            port,
        })
    }

    /// Serialize to bytes (excluding the option header).
    pub fn to_bytes(&self) -> [u8; Self::DATA_SIZE] {
        let mut buf = [0u8; Self::DATA_SIZE];
        let octets = self.address.octets();
        buf[0] = octets[0];
        buf[1] = octets[1];
        buf[2] = octets[2];
        buf[3] = octets[3];
        buf[4] = 0; // Reserved
        buf[5] = self.protocol as u8;
        buf[6..8].copy_from_slice(&self.port.to_be_bytes());
        buf[8] = 0; // Padding for alignment
        buf
    }
}

/// IPv6 endpoint option.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IPv6EndpointOption {
    /// IPv6 address.
    pub address: Ipv6Addr,
    /// Transport protocol (TCP or UDP).
    pub protocol: TransportProtocol,
    /// Port number.
    pub port: u16,
}

impl IPv6EndpointOption {
    /// Size of an IPv6 endpoint option (excluding header).
    pub const DATA_SIZE: usize = 21;

    /// Create a new IPv6 endpoint option.
    pub fn new(address: Ipv6Addr, protocol: TransportProtocol, port: u16) -> Self {
        Self {
            address,
            protocol,
            port,
        }
    }

    /// Create from a socket address.
    pub fn from_socket_addr(addr: SocketAddrV6, protocol: TransportProtocol) -> Self {
        Self {
            address: *addr.ip(),
            protocol,
            port: addr.port(),
        }
    }

    /// Convert to a socket address.
    pub fn to_socket_addr(&self) -> SocketAddrV6 {
        SocketAddrV6::new(self.address, self.port, 0, 0)
    }

    /// Parse from bytes (excluding the option header).
    pub fn from_bytes(data: &[u8]) -> Result<Self> {
        if data.len() < Self::DATA_SIZE {
            return Err(SomeIpError::MessageTooShort {
                expected: Self::DATA_SIZE,
                actual: data.len(),
            });
        }

        let mut addr_bytes = [0u8; 16];
        addr_bytes.copy_from_slice(&data[0..16]);
        let address = Ipv6Addr::from(addr_bytes);
        // data[16] is reserved
        let protocol = TransportProtocol::from_u8(data[17])
            .ok_or_else(|| SomeIpError::invalid_header(format!("Unknown protocol: 0x{:02X}", data[17])))?;
        let port = u16::from_be_bytes([data[18], data[19]]);

        Ok(Self {
            address,
            protocol,
            port,
        })
    }

    /// Serialize to bytes (excluding the option header).
    pub fn to_bytes(&self) -> [u8; Self::DATA_SIZE] {
        let mut buf = [0u8; Self::DATA_SIZE];
        buf[0..16].copy_from_slice(&self.address.octets());
        buf[16] = 0; // Reserved
        buf[17] = self.protocol as u8;
        buf[18..20].copy_from_slice(&self.port.to_be_bytes());
        buf[20] = 0; // Padding
        buf
    }
}

/// Configuration string option.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigurationOption {
    /// Configuration string.
    pub config_string: String,
}

impl ConfigurationOption {
    /// Create a new configuration option.
    pub fn new(config_string: impl Into<String>) -> Self {
        Self {
            config_string: config_string.into(),
        }
    }

    /// Parse from bytes (excluding the option header).
    pub fn from_bytes(data: &[u8]) -> Result<Self> {
        let config_string = String::from_utf8(data.to_vec())
            .map_err(|_| SomeIpError::invalid_header("Invalid UTF-8 in configuration string"))?;
        Ok(Self { config_string })
    }

    /// Serialize to bytes (excluding the option header).
    pub fn to_bytes(&self) -> Vec<u8> {
        self.config_string.as_bytes().to_vec()
    }
}

/// An SD option.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SdOption {
    /// IPv4 endpoint option.
    IPv4Endpoint(IPv4EndpointOption),
    /// IPv6 endpoint option.
    IPv6Endpoint(IPv6EndpointOption),
    /// IPv4 multicast option.
    IPv4Multicast(IPv4EndpointOption),
    /// IPv6 multicast option.
    IPv6Multicast(IPv6EndpointOption),
    /// Configuration string option.
    Configuration(ConfigurationOption),
    /// Unknown option (preserved for round-tripping).
    Unknown { option_type: u8, data: Vec<u8> },
}

impl SdOption {
    /// Parse an option from bytes (including the header).
    pub fn from_bytes(data: &[u8]) -> Result<(Self, usize)> {
        if data.len() < SD_OPTION_HEADER_SIZE {
            return Err(SomeIpError::MessageTooShort {
                expected: SD_OPTION_HEADER_SIZE,
                actual: data.len(),
            });
        }

        let length = u16::from_be_bytes([data[0], data[1]]) as usize;
        let option_type_byte = data[2];
        // data[3] is reserved

        let total_size = SD_OPTION_HEADER_SIZE + length;
        if data.len() < total_size {
            return Err(SomeIpError::MessageTooShort {
                expected: total_size,
                actual: data.len(),
            });
        }

        let option_data = &data[SD_OPTION_HEADER_SIZE..total_size];

        let option = match OptionType::from_u8(option_type_byte) {
            Some(OptionType::IPv4Endpoint) => {
                SdOption::IPv4Endpoint(IPv4EndpointOption::from_bytes(option_data)?)
            }
            Some(OptionType::IPv6Endpoint) => {
                SdOption::IPv6Endpoint(IPv6EndpointOption::from_bytes(option_data)?)
            }
            Some(OptionType::IPv4Multicast) => {
                SdOption::IPv4Multicast(IPv4EndpointOption::from_bytes(option_data)?)
            }
            Some(OptionType::IPv6Multicast) => {
                SdOption::IPv6Multicast(IPv6EndpointOption::from_bytes(option_data)?)
            }
            Some(OptionType::Configuration) => {
                SdOption::Configuration(ConfigurationOption::from_bytes(option_data)?)
            }
            _ => SdOption::Unknown {
                option_type: option_type_byte,
                data: option_data.to_vec(),
            },
        };

        Ok((option, total_size))
    }

    /// Serialize the option to bytes (including the header).
    pub fn to_bytes(&self) -> Vec<u8> {
        let (option_type, data) = match self {
            SdOption::IPv4Endpoint(opt) => (OptionType::IPv4Endpoint as u8, opt.to_bytes().to_vec()),
            SdOption::IPv6Endpoint(opt) => (OptionType::IPv6Endpoint as u8, opt.to_bytes().to_vec()),
            SdOption::IPv4Multicast(opt) => (OptionType::IPv4Multicast as u8, opt.to_bytes().to_vec()),
            SdOption::IPv6Multicast(opt) => (OptionType::IPv6Multicast as u8, opt.to_bytes().to_vec()),
            SdOption::Configuration(opt) => (OptionType::Configuration as u8, opt.to_bytes()),
            SdOption::Unknown { option_type, data } => (*option_type, data.clone()),
        };

        let length = data.len() as u16;
        let mut buf = Vec::with_capacity(SD_OPTION_HEADER_SIZE + data.len());
        buf.extend_from_slice(&length.to_be_bytes());
        buf.push(option_type);
        buf.push(0); // Reserved
        buf.extend_from_slice(&data);

        buf
    }

    /// Get the option type.
    pub fn option_type(&self) -> Option<OptionType> {
        match self {
            SdOption::IPv4Endpoint(_) => Some(OptionType::IPv4Endpoint),
            SdOption::IPv6Endpoint(_) => Some(OptionType::IPv6Endpoint),
            SdOption::IPv4Multicast(_) => Some(OptionType::IPv4Multicast),
            SdOption::IPv6Multicast(_) => Some(OptionType::IPv6Multicast),
            SdOption::Configuration(_) => Some(OptionType::Configuration),
            SdOption::Unknown { .. } => None,
        }
    }
}

/// A network endpoint (address + port + protocol).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Endpoint {
    /// Socket address.
    pub address: SocketAddr,
    /// Transport protocol.
    pub protocol: TransportProtocol,
}

impl Endpoint {
    /// Create a new endpoint.
    pub fn new(address: SocketAddr, protocol: TransportProtocol) -> Self {
        Self { address, protocol }
    }

    /// Create a TCP endpoint.
    pub fn tcp(address: SocketAddr) -> Self {
        Self::new(address, TransportProtocol::Tcp)
    }

    /// Create a UDP endpoint.
    pub fn udp(address: SocketAddr) -> Self {
        Self::new(address, TransportProtocol::Udp)
    }

    /// Create from a string address (e.g., "192.168.1.1:30490").
    pub fn from_str_tcp(addr: &str) -> Result<Self> {
        let socket_addr: SocketAddr = addr
            .parse()
            .map_err(|_| SomeIpError::invalid_header(format!("Invalid address: {}", addr)))?;
        Ok(Self::tcp(socket_addr))
    }

    /// Create from a string address (e.g., "192.168.1.1:30490").
    pub fn from_str_udp(addr: &str) -> Result<Self> {
        let socket_addr: SocketAddr = addr
            .parse()
            .map_err(|_| SomeIpError::invalid_header(format!("Invalid address: {}", addr)))?;
        Ok(Self::udp(socket_addr))
    }

    /// Convert to an SD option.
    pub fn to_option(&self) -> SdOption {
        match self.address {
            SocketAddr::V4(addr) => SdOption::IPv4Endpoint(IPv4EndpointOption::from_socket_addr(
                addr,
                self.protocol,
            )),
            SocketAddr::V6(addr) => SdOption::IPv6Endpoint(IPv6EndpointOption::from_socket_addr(
                addr,
                self.protocol,
            )),
        }
    }

    /// Create from an SD option.
    pub fn from_option(option: &SdOption) -> Option<Self> {
        match option {
            SdOption::IPv4Endpoint(opt) => Some(Self {
                address: SocketAddr::V4(opt.to_socket_addr()),
                protocol: opt.protocol,
            }),
            SdOption::IPv6Endpoint(opt) => Some(Self {
                address: SocketAddr::V6(opt.to_socket_addr()),
                protocol: opt.protocol,
            }),
            _ => None,
        }
    }
}

impl std::fmt::Display for Endpoint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let proto = match self.protocol {
            TransportProtocol::Tcp => "tcp",
            TransportProtocol::Udp => "udp",
        };
        write!(f, "{}://{}", proto, self.address)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ipv4_endpoint_roundtrip() {
        let opt = IPv4EndpointOption::new(
            Ipv4Addr::new(192, 168, 1, 100),
            TransportProtocol::Tcp,
            30490,
        );

        let bytes = opt.to_bytes();
        let parsed = IPv4EndpointOption::from_bytes(&bytes).unwrap();

        assert_eq!(opt, parsed);
    }

    #[test]
    fn test_ipv6_endpoint_roundtrip() {
        let opt = IPv6EndpointOption::new(
            Ipv6Addr::new(0x2001, 0x0db8, 0, 0, 0, 0, 0, 1),
            TransportProtocol::Udp,
            30490,
        );

        let bytes = opt.to_bytes();
        let parsed = IPv6EndpointOption::from_bytes(&bytes).unwrap();

        assert_eq!(opt, parsed);
    }

    #[test]
    fn test_sd_option_roundtrip() {
        let opt = SdOption::IPv4Endpoint(IPv4EndpointOption::new(
            Ipv4Addr::new(192, 168, 1, 100),
            TransportProtocol::Tcp,
            30490,
        ));

        let bytes = opt.to_bytes();
        let (parsed, size) = SdOption::from_bytes(&bytes).unwrap();

        assert_eq!(opt, parsed);
        assert_eq!(size, bytes.len());
    }

    #[test]
    fn test_endpoint_display() {
        let endpoint = Endpoint::tcp("192.168.1.100:30490".parse().unwrap());
        assert_eq!(format!("{}", endpoint), "tcp://192.168.1.100:30490");

        let endpoint = Endpoint::udp("192.168.1.100:30490".parse().unwrap());
        assert_eq!(format!("{}", endpoint), "udp://192.168.1.100:30490");
    }

    #[test]
    fn test_configuration_option() {
        let opt = ConfigurationOption::new("key=value");
        let bytes = opt.to_bytes();
        let parsed = ConfigurationOption::from_bytes(&bytes).unwrap();
        assert_eq!(opt, parsed);
    }
}
