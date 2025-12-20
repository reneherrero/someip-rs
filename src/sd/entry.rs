//! SOME/IP-SD entry types.

use crate::error::{Result, SomeIpError};
use crate::header::ServiceId;

use super::types::{EntryType, EventgroupId, InstanceId, SD_ENTRY_SIZE};

/// A service entry (FindService or OfferService).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceEntry {
    /// Entry type (FindService or OfferService).
    pub entry_type: EntryType,
    /// Index of first option run.
    pub index_first_option: u8,
    /// Index of second option run.
    pub index_second_option: u8,
    /// Number of options in first run (4 bits).
    pub num_options_1: u8,
    /// Number of options in second run (4 bits).
    pub num_options_2: u8,
    /// Service ID.
    pub service_id: ServiceId,
    /// Instance ID.
    pub instance_id: InstanceId,
    /// Major version.
    pub major_version: u8,
    /// Time-to-live in seconds (0 = stop offer/find).
    pub ttl: u32,
    /// Minor version.
    pub minor_version: u32,
}

impl ServiceEntry {
    /// Create a new FindService entry.
    pub fn find_service(
        service_id: ServiceId,
        instance_id: InstanceId,
        major_version: u8,
        minor_version: u32,
    ) -> Self {
        Self {
            entry_type: EntryType::FindService,
            index_first_option: 0,
            index_second_option: 0,
            num_options_1: 0,
            num_options_2: 0,
            service_id,
            instance_id,
            major_version,
            ttl: 0xFFFFFF, // Max TTL for find
            minor_version,
        }
    }

    /// Create a new OfferService entry.
    pub fn offer_service(
        service_id: ServiceId,
        instance_id: InstanceId,
        major_version: u8,
        minor_version: u32,
        ttl: u32,
    ) -> Self {
        Self {
            entry_type: EntryType::OfferService,
            index_first_option: 0,
            index_second_option: 0,
            num_options_1: 0,
            num_options_2: 0,
            service_id,
            instance_id,
            major_version,
            ttl: ttl & 0xFFFFFF, // 24 bits only
            minor_version,
        }
    }

    /// Create a StopOfferService entry (OfferService with TTL=0).
    pub fn stop_offer_service(
        service_id: ServiceId,
        instance_id: InstanceId,
        major_version: u8,
        minor_version: u32,
    ) -> Self {
        Self::offer_service(service_id, instance_id, major_version, minor_version, 0)
    }

    /// Check if this is a stop offer (TTL = 0).
    pub fn is_stop_offer(&self) -> bool {
        self.entry_type == EntryType::OfferService && self.ttl == 0
    }

    /// Parse a service entry from bytes.
    pub fn from_bytes(data: &[u8]) -> Result<Self> {
        if data.len() < SD_ENTRY_SIZE {
            return Err(SomeIpError::MessageTooShort {
                expected: SD_ENTRY_SIZE,
                actual: data.len(),
            });
        }

        let entry_type = EntryType::from_u8(data[0])
            .ok_or_else(|| SomeIpError::invalid_header(format!("Unknown entry type: 0x{:02X}", data[0])))?;

        if !entry_type.is_service_entry() {
            return Err(SomeIpError::invalid_header("Expected service entry type"));
        }

        let index_first_option = data[1];
        let index_second_option = data[2];
        let num_options_1 = (data[3] >> 4) & 0x0F;
        let num_options_2 = data[3] & 0x0F;

        let service_id = ServiceId(u16::from_be_bytes([data[4], data[5]]));
        let instance_id = InstanceId(u16::from_be_bytes([data[6], data[7]]));
        let major_version = data[8];
        let ttl = u32::from_be_bytes([0, data[9], data[10], data[11]]);
        let minor_version = u32::from_be_bytes([data[12], data[13], data[14], data[15]]);

        Ok(Self {
            entry_type,
            index_first_option,
            index_second_option,
            num_options_1,
            num_options_2,
            service_id,
            instance_id,
            major_version,
            ttl,
            minor_version,
        })
    }

    /// Serialize the entry to bytes.
    pub fn to_bytes(&self) -> [u8; SD_ENTRY_SIZE] {
        let mut buf = [0u8; SD_ENTRY_SIZE];

        buf[0] = self.entry_type as u8;
        buf[1] = self.index_first_option;
        buf[2] = self.index_second_option;
        buf[3] = ((self.num_options_1 & 0x0F) << 4) | (self.num_options_2 & 0x0F);
        buf[4..6].copy_from_slice(&self.service_id.0.to_be_bytes());
        buf[6..8].copy_from_slice(&self.instance_id.0.to_be_bytes());
        buf[8] = self.major_version;
        let ttl_bytes = self.ttl.to_be_bytes();
        buf[9] = ttl_bytes[1];
        buf[10] = ttl_bytes[2];
        buf[11] = ttl_bytes[3];
        buf[12..16].copy_from_slice(&self.minor_version.to_be_bytes());

        buf
    }
}

/// An eventgroup entry (Subscribe or SubscribeAck).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventgroupEntry {
    /// Entry type (SubscribeEventgroup or SubscribeEventgroupAck).
    pub entry_type: EntryType,
    /// Index of first option run.
    pub index_first_option: u8,
    /// Index of second option run.
    pub index_second_option: u8,
    /// Number of options in first run (4 bits).
    pub num_options_1: u8,
    /// Number of options in second run (4 bits).
    pub num_options_2: u8,
    /// Service ID.
    pub service_id: ServiceId,
    /// Instance ID.
    pub instance_id: InstanceId,
    /// Major version.
    pub major_version: u8,
    /// Time-to-live in seconds (0 = unsubscribe/nack).
    pub ttl: u32,
    /// Counter for subscription tracking.
    pub counter: u8,
    /// Eventgroup ID.
    pub eventgroup_id: EventgroupId,
}

impl EventgroupEntry {
    /// Create a new SubscribeEventgroup entry.
    pub fn subscribe(
        service_id: ServiceId,
        instance_id: InstanceId,
        major_version: u8,
        eventgroup_id: EventgroupId,
        ttl: u32,
    ) -> Self {
        Self {
            entry_type: EntryType::SubscribeEventgroup,
            index_first_option: 0,
            index_second_option: 0,
            num_options_1: 0,
            num_options_2: 0,
            service_id,
            instance_id,
            major_version,
            ttl: ttl & 0xFFFFFF,
            counter: 0,
            eventgroup_id,
        }
    }

    /// Create an unsubscribe entry (Subscribe with TTL=0).
    pub fn unsubscribe(
        service_id: ServiceId,
        instance_id: InstanceId,
        major_version: u8,
        eventgroup_id: EventgroupId,
    ) -> Self {
        Self::subscribe(service_id, instance_id, major_version, eventgroup_id, 0)
    }

    /// Create a SubscribeEventgroupAck entry.
    pub fn subscribe_ack(
        service_id: ServiceId,
        instance_id: InstanceId,
        major_version: u8,
        eventgroup_id: EventgroupId,
        ttl: u32,
        counter: u8,
    ) -> Self {
        Self {
            entry_type: EntryType::SubscribeEventgroupAck,
            index_first_option: 0,
            index_second_option: 0,
            num_options_1: 0,
            num_options_2: 0,
            service_id,
            instance_id,
            major_version,
            ttl: ttl & 0xFFFFFF,
            counter,
            eventgroup_id,
        }
    }

    /// Create a SubscribeEventgroupNack entry (Ack with TTL=0).
    pub fn subscribe_nack(
        service_id: ServiceId,
        instance_id: InstanceId,
        major_version: u8,
        eventgroup_id: EventgroupId,
        counter: u8,
    ) -> Self {
        Self::subscribe_ack(service_id, instance_id, major_version, eventgroup_id, 0, counter)
    }

    /// Check if this is an unsubscribe or nack (TTL = 0).
    pub fn is_negative(&self) -> bool {
        self.ttl == 0
    }

    /// Parse an eventgroup entry from bytes.
    pub fn from_bytes(data: &[u8]) -> Result<Self> {
        if data.len() < SD_ENTRY_SIZE {
            return Err(SomeIpError::MessageTooShort {
                expected: SD_ENTRY_SIZE,
                actual: data.len(),
            });
        }

        let entry_type = EntryType::from_u8(data[0])
            .ok_or_else(|| SomeIpError::invalid_header(format!("Unknown entry type: 0x{:02X}", data[0])))?;

        if !entry_type.is_eventgroup_entry() {
            return Err(SomeIpError::invalid_header("Expected eventgroup entry type"));
        }

        let index_first_option = data[1];
        let index_second_option = data[2];
        let num_options_1 = (data[3] >> 4) & 0x0F;
        let num_options_2 = data[3] & 0x0F;

        let service_id = ServiceId(u16::from_be_bytes([data[4], data[5]]));
        let instance_id = InstanceId(u16::from_be_bytes([data[6], data[7]]));
        let major_version = data[8];
        let ttl = u32::from_be_bytes([0, data[9], data[10], data[11]]);

        // Bytes 12-15: Reserved (4 bits) | Counter (4 bits) | Eventgroup ID (16 bits)
        let counter = data[12] & 0x0F;
        let eventgroup_id = EventgroupId(u16::from_be_bytes([data[14], data[15]]));

        Ok(Self {
            entry_type,
            index_first_option,
            index_second_option,
            num_options_1,
            num_options_2,
            service_id,
            instance_id,
            major_version,
            ttl,
            counter,
            eventgroup_id,
        })
    }

    /// Serialize the entry to bytes.
    pub fn to_bytes(&self) -> [u8; SD_ENTRY_SIZE] {
        let mut buf = [0u8; SD_ENTRY_SIZE];

        buf[0] = self.entry_type as u8;
        buf[1] = self.index_first_option;
        buf[2] = self.index_second_option;
        buf[3] = ((self.num_options_1 & 0x0F) << 4) | (self.num_options_2 & 0x0F);
        buf[4..6].copy_from_slice(&self.service_id.0.to_be_bytes());
        buf[6..8].copy_from_slice(&self.instance_id.0.to_be_bytes());
        buf[8] = self.major_version;
        let ttl_bytes = self.ttl.to_be_bytes();
        buf[9] = ttl_bytes[1];
        buf[10] = ttl_bytes[2];
        buf[11] = ttl_bytes[3];
        buf[12] = self.counter & 0x0F;
        buf[13] = 0; // Reserved
        buf[14..16].copy_from_slice(&self.eventgroup_id.0.to_be_bytes());

        buf
    }
}

/// An SD entry (either Service or Eventgroup).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SdEntry {
    /// Service entry (Find/Offer).
    Service(ServiceEntry),
    /// Eventgroup entry (Subscribe/Ack).
    Eventgroup(EventgroupEntry),
}

impl SdEntry {
    /// Parse an entry from bytes.
    pub fn from_bytes(data: &[u8]) -> Result<Self> {
        if data.is_empty() {
            return Err(SomeIpError::MessageTooShort {
                expected: 1,
                actual: 0,
            });
        }

        let entry_type = EntryType::from_u8(data[0]);

        match entry_type {
            Some(t) if t.is_service_entry() => Ok(SdEntry::Service(ServiceEntry::from_bytes(data)?)),
            Some(t) if t.is_eventgroup_entry() => {
                Ok(SdEntry::Eventgroup(EventgroupEntry::from_bytes(data)?))
            }
            _ => Err(SomeIpError::invalid_header(format!(
                "Unknown entry type: 0x{:02X}",
                data[0]
            ))),
        }
    }

    /// Serialize the entry to bytes.
    pub fn to_bytes(&self) -> [u8; SD_ENTRY_SIZE] {
        match self {
            SdEntry::Service(e) => e.to_bytes(),
            SdEntry::Eventgroup(e) => e.to_bytes(),
        }
    }

    /// Get the service ID from this entry.
    pub fn service_id(&self) -> ServiceId {
        match self {
            SdEntry::Service(e) => e.service_id,
            SdEntry::Eventgroup(e) => e.service_id,
        }
    }

    /// Get the instance ID from this entry.
    pub fn instance_id(&self) -> InstanceId {
        match self {
            SdEntry::Service(e) => e.instance_id,
            SdEntry::Eventgroup(e) => e.instance_id,
        }
    }

    /// Get the TTL from this entry.
    pub fn ttl(&self) -> u32 {
        match self {
            SdEntry::Service(e) => e.ttl,
            SdEntry::Eventgroup(e) => e.ttl,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_service_entry_roundtrip() {
        let entry = ServiceEntry::offer_service(
            ServiceId(0x1234),
            InstanceId(0x0001),
            1,
            0,
            3600,
        );

        let bytes = entry.to_bytes();
        let parsed = ServiceEntry::from_bytes(&bytes).unwrap();

        assert_eq!(entry, parsed);
    }

    #[test]
    fn test_find_service_entry() {
        let entry = ServiceEntry::find_service(
            ServiceId(0x1234),
            InstanceId::ANY,
            0xFF, // Any major version
            0xFFFFFFFF, // Any minor version
        );

        assert_eq!(entry.entry_type, EntryType::FindService);
        assert_eq!(entry.instance_id, InstanceId::ANY);
    }

    #[test]
    fn test_eventgroup_entry_roundtrip() {
        let entry = EventgroupEntry::subscribe(
            ServiceId(0x1234),
            InstanceId(0x0001),
            1,
            EventgroupId(0x0001),
            3600,
        );

        let bytes = entry.to_bytes();
        let parsed = EventgroupEntry::from_bytes(&bytes).unwrap();

        assert_eq!(entry, parsed);
    }

    #[test]
    fn test_subscribe_ack_nack() {
        let ack = EventgroupEntry::subscribe_ack(
            ServiceId(0x1234),
            InstanceId(0x0001),
            1,
            EventgroupId(0x0001),
            3600,
            0,
        );
        assert!(!ack.is_negative());

        let nack = EventgroupEntry::subscribe_nack(
            ServiceId(0x1234),
            InstanceId(0x0001),
            1,
            EventgroupId(0x0001),
            0,
        );
        assert!(nack.is_negative());
    }

    #[test]
    fn test_sd_entry_dispatch() {
        let service = ServiceEntry::offer_service(ServiceId(0x1234), InstanceId(0x0001), 1, 0, 3600);
        let bytes = service.to_bytes();

        let entry = SdEntry::from_bytes(&bytes).unwrap();
        assert!(matches!(entry, SdEntry::Service(_)));

        let eventgroup = EventgroupEntry::subscribe(
            ServiceId(0x1234),
            InstanceId(0x0001),
            1,
            EventgroupId(0x0001),
            3600,
        );
        let bytes = eventgroup.to_bytes();

        let entry = SdEntry::from_bytes(&bytes).unwrap();
        assert!(matches!(entry, SdEntry::Eventgroup(_)));
    }
}
