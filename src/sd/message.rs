//! SOME/IP-SD message handling.

use bytes::Bytes;

use crate::error::{Result, SomeIpError};
use crate::header::{MethodId, ServiceId};
use crate::message::SomeIpMessage;

use super::entry::{EventgroupEntry, SdEntry, ServiceEntry};
use super::option::{Endpoint, SdOption};
use super::types::{EventgroupId, InstanceId, SD_ENTRY_SIZE, SD_METHOD_ID, SD_SERVICE_ID};

/// SD message flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SdFlags {
    /// Reboot flag - set when the sender has rebooted.
    pub reboot: bool,
    /// Unicast flag - set when the message should be answered via unicast.
    pub unicast: bool,
    /// Explicit initial data control flag.
    pub explicit_initial_data: bool,
}

impl SdFlags {
    /// Parse flags from a byte.
    pub fn from_u8(byte: u8) -> Self {
        Self {
            reboot: (byte & 0x80) != 0,
            unicast: (byte & 0x40) != 0,
            explicit_initial_data: (byte & 0x20) != 0,
        }
    }

    /// Serialize flags to a byte.
    pub fn to_u8(&self) -> u8 {
        let mut byte = 0u8;
        if self.reboot {
            byte |= 0x80;
        }
        if self.unicast {
            byte |= 0x40;
        }
        if self.explicit_initial_data {
            byte |= 0x20;
        }
        byte
    }
}

/// A SOME/IP-SD message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SdMessage {
    /// Message flags.
    pub flags: SdFlags,
    /// List of entries.
    pub entries: Vec<SdEntry>,
    /// List of options.
    pub options: Vec<SdOption>,
}

impl SdMessage {
    /// Create a new empty SD message.
    pub fn new() -> Self {
        Self {
            flags: SdFlags::default(),
            entries: Vec::new(),
            options: Vec::new(),
        }
    }

    /// Create a FindService message.
    pub fn find_service(
        service_id: ServiceId,
        instance_id: InstanceId,
        major_version: u8,
        minor_version: u32,
    ) -> Self {
        let entry = ServiceEntry::find_service(service_id, instance_id, major_version, minor_version);
        Self {
            flags: SdFlags::default(),
            entries: vec![SdEntry::Service(entry)],
            options: Vec::new(),
        }
    }

    /// Create an OfferService message.
    pub fn offer_service(
        service_id: ServiceId,
        instance_id: InstanceId,
        major_version: u8,
        minor_version: u32,
        ttl: u32,
        endpoint: Endpoint,
    ) -> Self {
        let mut entry =
            ServiceEntry::offer_service(service_id, instance_id, major_version, minor_version, ttl);
        entry.index_first_option = 0;
        entry.num_options_1 = 1;

        Self {
            flags: SdFlags::default(),
            entries: vec![SdEntry::Service(entry)],
            options: vec![endpoint.to_option()],
        }
    }

    /// Create a StopOfferService message.
    pub fn stop_offer_service(
        service_id: ServiceId,
        instance_id: InstanceId,
        major_version: u8,
        minor_version: u32,
    ) -> Self {
        let entry =
            ServiceEntry::stop_offer_service(service_id, instance_id, major_version, minor_version);
        Self {
            flags: SdFlags::default(),
            entries: vec![SdEntry::Service(entry)],
            options: Vec::new(),
        }
    }

    /// Create a SubscribeEventgroup message.
    pub fn subscribe_eventgroup(
        service_id: ServiceId,
        instance_id: InstanceId,
        major_version: u8,
        eventgroup_id: EventgroupId,
        ttl: u32,
        endpoint: Endpoint,
    ) -> Self {
        let mut entry = EventgroupEntry::subscribe(
            service_id,
            instance_id,
            major_version,
            eventgroup_id,
            ttl,
        );
        entry.index_first_option = 0;
        entry.num_options_1 = 1;

        Self {
            flags: SdFlags::default(),
            entries: vec![SdEntry::Eventgroup(entry)],
            options: vec![endpoint.to_option()],
        }
    }

    /// Create a StopSubscribeEventgroup message.
    pub fn stop_subscribe_eventgroup(
        service_id: ServiceId,
        instance_id: InstanceId,
        major_version: u8,
        eventgroup_id: EventgroupId,
    ) -> Self {
        let entry = EventgroupEntry::unsubscribe(
            service_id,
            instance_id,
            major_version,
            eventgroup_id,
        );
        Self {
            flags: SdFlags::default(),
            entries: vec![SdEntry::Eventgroup(entry)],
            options: Vec::new(),
        }
    }

    /// Create a SubscribeEventgroupAck message.
    pub fn subscribe_eventgroup_ack(
        service_id: ServiceId,
        instance_id: InstanceId,
        major_version: u8,
        eventgroup_id: EventgroupId,
        ttl: u32,
        counter: u8,
        endpoint: Option<Endpoint>,
    ) -> Self {
        let mut entry = EventgroupEntry::subscribe_ack(
            service_id,
            instance_id,
            major_version,
            eventgroup_id,
            ttl,
            counter,
        );

        let options = if let Some(ep) = endpoint {
            entry.index_first_option = 0;
            entry.num_options_1 = 1;
            vec![ep.to_option()]
        } else {
            Vec::new()
        };

        Self {
            flags: SdFlags::default(),
            entries: vec![SdEntry::Eventgroup(entry)],
            options,
        }
    }

    /// Create a SubscribeEventgroupNack message.
    pub fn subscribe_eventgroup_nack(
        service_id: ServiceId,
        instance_id: InstanceId,
        major_version: u8,
        eventgroup_id: EventgroupId,
        counter: u8,
    ) -> Self {
        let entry = EventgroupEntry::subscribe_nack(
            service_id,
            instance_id,
            major_version,
            eventgroup_id,
            counter,
        );
        Self {
            flags: SdFlags::default(),
            entries: vec![SdEntry::Eventgroup(entry)],
            options: Vec::new(),
        }
    }

    /// Parse an SD message from bytes (SD payload only, not including SOME/IP header).
    pub fn from_bytes(data: &[u8]) -> Result<Self> {
        if data.len() < 12 {
            return Err(SomeIpError::MessageTooShort {
                expected: 12,
                actual: data.len(),
            });
        }

        let flags = SdFlags::from_u8(data[0]);
        // data[1..4] is reserved

        let entries_length = u32::from_be_bytes([data[4], data[5], data[6], data[7]]) as usize;

        if data.len() < 8 + entries_length + 4 {
            return Err(SomeIpError::MessageTooShort {
                expected: 8 + entries_length + 4,
                actual: data.len(),
            });
        }

        // Parse entries
        let entries_data = &data[8..8 + entries_length];
        let mut entries = Vec::new();
        let mut offset = 0;
        while offset + SD_ENTRY_SIZE <= entries_data.len() {
            let entry = SdEntry::from_bytes(&entries_data[offset..])?;
            entries.push(entry);
            offset += SD_ENTRY_SIZE;
        }

        // Parse options
        let options_offset = 8 + entries_length;
        let options_length =
            u32::from_be_bytes([data[options_offset], data[options_offset + 1], data[options_offset + 2], data[options_offset + 3]]) as usize;

        let options_data = &data[options_offset + 4..];
        if options_data.len() < options_length {
            return Err(SomeIpError::MessageTooShort {
                expected: options_length,
                actual: options_data.len(),
            });
        }

        let mut options = Vec::new();
        let mut opt_offset = 0;
        while opt_offset < options_length {
            let (option, size) = SdOption::from_bytes(&options_data[opt_offset..])?;
            options.push(option);
            opt_offset += size;
        }

        Ok(Self {
            flags,
            entries,
            options,
        })
    }

    /// Parse an SD message from a SOME/IP message.
    pub fn from_someip_message(msg: &SomeIpMessage) -> Result<Self> {
        if msg.header.service_id != ServiceId(SD_SERVICE_ID) {
            return Err(SomeIpError::invalid_header(format!(
                "Expected SD service ID 0x{:04X}, got {}",
                SD_SERVICE_ID, msg.header.service_id
            )));
        }
        if msg.header.method_id != MethodId(SD_METHOD_ID) {
            return Err(SomeIpError::invalid_header(format!(
                "Expected SD method ID 0x{:04X}, got {}",
                SD_METHOD_ID, msg.header.method_id
            )));
        }

        Self::from_bytes(&msg.payload)
    }

    /// Serialize the SD message to bytes (SD payload only).
    pub fn to_bytes(&self) -> Vec<u8> {
        // Calculate sizes
        let entries_length = self.entries.len() * SD_ENTRY_SIZE;
        let options_bytes: Vec<Vec<u8>> = self.options.iter().map(|o| o.to_bytes()).collect();
        let options_length: usize = options_bytes.iter().map(|b| b.len()).sum();

        let mut buf = Vec::with_capacity(8 + entries_length + 4 + options_length);

        // Flags + reserved
        buf.push(self.flags.to_u8());
        buf.extend_from_slice(&[0, 0, 0]); // Reserved

        // Entries length
        buf.extend_from_slice(&(entries_length as u32).to_be_bytes());

        // Entries
        for entry in &self.entries {
            buf.extend_from_slice(&entry.to_bytes());
        }

        // Options length
        buf.extend_from_slice(&(options_length as u32).to_be_bytes());

        // Options
        for option_bytes in options_bytes {
            buf.extend_from_slice(&option_bytes);
        }

        buf
    }

    /// Convert to a SOME/IP message.
    pub fn to_someip_message(&self) -> SomeIpMessage {
        let payload = self.to_bytes();
        SomeIpMessage::notification(ServiceId(SD_SERVICE_ID), MethodId(SD_METHOD_ID))
            .payload(Bytes::from(payload))
            .build()
    }

    /// Check if this is a FindService message.
    pub fn is_find_service(&self) -> bool {
        self.entries.iter().any(|e| {
            matches!(
                e,
                SdEntry::Service(s) if s.entry_type == super::types::EntryType::FindService
            )
        })
    }

    /// Check if this is an OfferService message.
    pub fn is_offer_service(&self) -> bool {
        self.entries.iter().any(|e| {
            matches!(
                e,
                SdEntry::Service(s) if s.entry_type == super::types::EntryType::OfferService && s.ttl > 0
            )
        })
    }

    /// Check if this is a StopOfferService message.
    pub fn is_stop_offer_service(&self) -> bool {
        self.entries.iter().any(|e| {
            matches!(
                e,
                SdEntry::Service(s) if s.entry_type == super::types::EntryType::OfferService && s.ttl == 0
            )
        })
    }

    /// Get the options for an entry by index.
    pub fn get_options_for_entry(&self, entry: &SdEntry) -> Vec<&SdOption> {
        let (index1, num1, index2, num2) = match entry {
            SdEntry::Service(e) => (
                e.index_first_option as usize,
                e.num_options_1 as usize,
                e.index_second_option as usize,
                e.num_options_2 as usize,
            ),
            SdEntry::Eventgroup(e) => (
                e.index_first_option as usize,
                e.num_options_1 as usize,
                e.index_second_option as usize,
                e.num_options_2 as usize,
            ),
        };

        let mut options = Vec::new();

        // First option run
        for i in index1..index1 + num1 {
            if let Some(opt) = self.options.get(i) {
                options.push(opt);
            }
        }

        // Second option run
        for i in index2..index2 + num2 {
            if let Some(opt) = self.options.get(i) {
                options.push(opt);
            }
        }

        options
    }

    /// Get endpoints from options for an entry.
    pub fn get_endpoints_for_entry(&self, entry: &SdEntry) -> Vec<Endpoint> {
        self.get_options_for_entry(entry)
            .iter()
            .filter_map(|opt| Endpoint::from_option(opt))
            .collect()
    }
}

impl Default for SdMessage {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::MessageType;

    #[test]
    fn test_sd_flags_roundtrip() {
        let flags = SdFlags {
            reboot: true,
            unicast: true,
            explicit_initial_data: false,
        };

        let byte = flags.to_u8();
        let parsed = SdFlags::from_u8(byte);

        assert_eq!(flags, parsed);
    }

    #[test]
    fn test_find_service_message() {
        let msg = SdMessage::find_service(
            ServiceId(0x1234),
            InstanceId::ANY,
            0xFF,
            0xFFFFFFFF,
        );

        assert!(msg.is_find_service());
        assert_eq!(msg.entries.len(), 1);
        assert!(msg.options.is_empty());
    }

    #[test]
    fn test_offer_service_message() {
        let endpoint = Endpoint::tcp("192.168.1.100:30490".parse().unwrap());
        let msg = SdMessage::offer_service(
            ServiceId(0x1234),
            InstanceId(0x0001),
            1,
            0,
            3600,
            endpoint,
        );

        assert!(msg.is_offer_service());
        assert_eq!(msg.entries.len(), 1);
        assert_eq!(msg.options.len(), 1);
    }

    #[test]
    fn test_sd_message_roundtrip() {
        let endpoint = Endpoint::tcp("192.168.1.100:30490".parse().unwrap());
        let original = SdMessage::offer_service(
            ServiceId(0x1234),
            InstanceId(0x0001),
            1,
            0,
            3600,
            endpoint,
        );

        let bytes = original.to_bytes();
        let parsed = SdMessage::from_bytes(&bytes).unwrap();

        assert_eq!(original.flags, parsed.flags);
        assert_eq!(original.entries.len(), parsed.entries.len());
        assert_eq!(original.options.len(), parsed.options.len());
    }

    #[test]
    fn test_to_someip_message() {
        let msg = SdMessage::find_service(
            ServiceId(0x1234),
            InstanceId::ANY,
            0xFF,
            0xFFFFFFFF,
        );

        let someip = msg.to_someip_message();

        assert_eq!(someip.header.service_id, ServiceId(SD_SERVICE_ID));
        assert_eq!(someip.header.method_id, MethodId(SD_METHOD_ID));
        assert_eq!(someip.header.message_type, MessageType::Notification);
    }

    #[test]
    fn test_get_endpoints_for_entry() {
        let endpoint = Endpoint::tcp("192.168.1.100:30490".parse().unwrap());
        let msg = SdMessage::offer_service(
            ServiceId(0x1234),
            InstanceId(0x0001),
            1,
            0,
            3600,
            endpoint.clone(),
        );

        let entry = &msg.entries[0];
        let endpoints = msg.get_endpoints_for_entry(entry);

        assert_eq!(endpoints.len(), 1);
        assert_eq!(endpoints[0], endpoint);
    }
}
