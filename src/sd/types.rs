//! SOME/IP-SD type definitions.

use std::net::Ipv4Addr;

/// SD Service ID (always 0xFFFF).
pub const SD_SERVICE_ID: u16 = 0xFFFF;

/// SD Method ID (always 0x8100).
pub const SD_METHOD_ID: u16 = 0x8100;

/// Default SD multicast address.
pub const SD_MULTICAST_ADDR: Ipv4Addr = Ipv4Addr::new(224, 224, 224, 245);

/// Default SD port.
pub const SD_DEFAULT_PORT: u16 = 30490;

/// Size of an SD entry in bytes.
pub const SD_ENTRY_SIZE: usize = 16;

/// Size of an SD option header in bytes.
pub const SD_OPTION_HEADER_SIZE: usize = 4;

/// Instance ID for a service instance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct InstanceId(pub u16);

impl InstanceId {
    /// Wildcard instance ID that matches any instance.
    pub const ANY: InstanceId = InstanceId(0xFFFF);

    /// Check if this is the wildcard instance ID.
    pub fn is_any(&self) -> bool {
        self.0 == 0xFFFF
    }
}

impl std::fmt::Display for InstanceId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "0x{:04X}", self.0)
    }
}

/// Eventgroup ID for event subscriptions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct EventgroupId(pub u16);

impl std::fmt::Display for EventgroupId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "0x{:04X}", self.0)
    }
}

/// SD entry types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum EntryType {
    /// Find a service.
    FindService = 0x00,
    /// Offer a service (TTL > 0) or stop offering (TTL = 0).
    OfferService = 0x01,
    /// Subscribe to an eventgroup (TTL > 0) or unsubscribe (TTL = 0).
    SubscribeEventgroup = 0x06,
    /// Acknowledge (TTL > 0) or reject (TTL = 0) a subscription.
    SubscribeEventgroupAck = 0x07,
}

impl EntryType {
    /// Create an EntryType from a raw byte value.
    pub fn from_u8(value: u8) -> Option<Self> {
        match value {
            0x00 => Some(Self::FindService),
            0x01 => Some(Self::OfferService),
            0x06 => Some(Self::SubscribeEventgroup),
            0x07 => Some(Self::SubscribeEventgroupAck),
            _ => None,
        }
    }

    /// Check if this is a service entry type.
    pub fn is_service_entry(&self) -> bool {
        matches!(self, Self::FindService | Self::OfferService)
    }

    /// Check if this is an eventgroup entry type.
    pub fn is_eventgroup_entry(&self) -> bool {
        matches!(self, Self::SubscribeEventgroup | Self::SubscribeEventgroupAck)
    }
}

/// SD option types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum OptionType {
    /// Configuration string option.
    Configuration = 0x01,
    /// Load balancing option.
    LoadBalancing = 0x02,
    /// IPv4 endpoint option.
    IPv4Endpoint = 0x04,
    /// IPv6 endpoint option.
    IPv6Endpoint = 0x06,
    /// IPv4 multicast option.
    IPv4Multicast = 0x14,
    /// IPv6 multicast option.
    IPv6Multicast = 0x16,
    /// IPv4 SD endpoint option.
    IPv4SdEndpoint = 0x24,
    /// IPv6 SD endpoint option.
    IPv6SdEndpoint = 0x26,
}

impl OptionType {
    /// Create an OptionType from a raw byte value.
    pub fn from_u8(value: u8) -> Option<Self> {
        match value {
            0x01 => Some(Self::Configuration),
            0x02 => Some(Self::LoadBalancing),
            0x04 => Some(Self::IPv4Endpoint),
            0x06 => Some(Self::IPv6Endpoint),
            0x14 => Some(Self::IPv4Multicast),
            0x16 => Some(Self::IPv6Multicast),
            0x24 => Some(Self::IPv4SdEndpoint),
            0x26 => Some(Self::IPv6SdEndpoint),
            _ => None,
        }
    }

    /// Check if this is an IPv4 option.
    pub fn is_ipv4(&self) -> bool {
        matches!(
            self,
            Self::IPv4Endpoint | Self::IPv4Multicast | Self::IPv4SdEndpoint
        )
    }

    /// Check if this is an IPv6 option.
    pub fn is_ipv6(&self) -> bool {
        matches!(
            self,
            Self::IPv6Endpoint | Self::IPv6Multicast | Self::IPv6SdEndpoint
        )
    }
}

/// Transport protocol used for endpoints.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum TransportProtocol {
    /// TCP protocol.
    Tcp = 0x06,
    /// UDP protocol.
    Udp = 0x11,
}

impl TransportProtocol {
    /// Create a TransportProtocol from a raw byte value.
    pub fn from_u8(value: u8) -> Option<Self> {
        match value {
            0x06 => Some(Self::Tcp),
            0x11 => Some(Self::Udp),
            _ => None,
        }
    }
}

impl Default for TransportProtocol {
    fn default() -> Self {
        Self::Udp
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_entry_type_from_u8() {
        assert_eq!(EntryType::from_u8(0x00), Some(EntryType::FindService));
        assert_eq!(EntryType::from_u8(0x01), Some(EntryType::OfferService));
        assert_eq!(EntryType::from_u8(0x06), Some(EntryType::SubscribeEventgroup));
        assert_eq!(EntryType::from_u8(0x07), Some(EntryType::SubscribeEventgroupAck));
        assert_eq!(EntryType::from_u8(0xFF), None);
    }

    #[test]
    fn test_option_type_from_u8() {
        assert_eq!(OptionType::from_u8(0x04), Some(OptionType::IPv4Endpoint));
        assert_eq!(OptionType::from_u8(0x06), Some(OptionType::IPv6Endpoint));
        assert_eq!(OptionType::from_u8(0xFF), None);
    }

    #[test]
    fn test_instance_id_any() {
        assert!(InstanceId::ANY.is_any());
        assert!(!InstanceId(0x0001).is_any());
    }

    #[test]
    fn test_transport_protocol() {
        assert_eq!(TransportProtocol::from_u8(0x06), Some(TransportProtocol::Tcp));
        assert_eq!(TransportProtocol::from_u8(0x11), Some(TransportProtocol::Udp));
        assert_eq!(TransportProtocol::from_u8(0xFF), None);
    }
}
