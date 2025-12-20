//! SOME/IP-SD client for service discovery.

use std::collections::HashMap;
use std::io;
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4, UdpSocket};
use std::time::{Duration, Instant};

use crate::error::{Result, SomeIpError};
use crate::header::ServiceId;

use super::entry::SdEntry;
use super::message::SdMessage;
use super::option::Endpoint;
use super::types::{
    EntryType, EventgroupId, InstanceId, SD_DEFAULT_PORT, SD_MULTICAST_ADDR,
};

/// Information about a discovered service.
#[derive(Debug, Clone)]
pub struct ServiceInfo {
    /// Service ID.
    pub service_id: ServiceId,
    /// Instance ID.
    pub instance_id: InstanceId,
    /// Major version.
    pub major_version: u8,
    /// Minor version.
    pub minor_version: u32,
    /// Available endpoints for connecting to the service.
    pub endpoints: Vec<Endpoint>,
    /// When the service offer expires.
    pub expires_at: Instant,
    /// Source address of the service offer.
    pub source_addr: SocketAddr,
}

impl ServiceInfo {
    /// Check if the service offer has expired.
    pub fn is_expired(&self) -> bool {
        Instant::now() >= self.expires_at
    }

    /// Get remaining TTL in seconds.
    pub fn remaining_ttl(&self) -> u32 {
        self.expires_at
            .saturating_duration_since(Instant::now())
            .as_secs() as u32
    }
}

/// Events received by the SD client.
#[derive(Debug, Clone)]
pub enum SdEvent {
    /// A service became available.
    ServiceAvailable(ServiceInfo),
    /// A service became unavailable.
    ServiceUnavailable {
        /// Service ID.
        service_id: ServiceId,
        /// Instance ID.
        instance_id: InstanceId,
    },
    /// Subscription was acknowledged.
    SubscriptionAck {
        /// Service ID.
        service_id: ServiceId,
        /// Instance ID.
        instance_id: InstanceId,
        /// Eventgroup ID.
        eventgroup_id: EventgroupId,
        /// Multicast endpoint if provided.
        multicast_endpoint: Option<Endpoint>,
    },
    /// Subscription was rejected.
    SubscriptionNack {
        /// Service ID.
        service_id: ServiceId,
        /// Instance ID.
        instance_id: InstanceId,
        /// Eventgroup ID.
        eventgroup_id: EventgroupId,
    },
}

/// SD client configuration.
#[derive(Debug, Clone)]
pub struct SdClientConfig {
    /// Local address to bind to.
    pub bind_addr: SocketAddr,
    /// Multicast address for SD.
    pub multicast_addr: SocketAddr,
    /// Interface address for multicast (None = any).
    pub multicast_interface: Option<Ipv4Addr>,
    /// Default TTL for find requests.
    pub find_ttl: u32,
    /// Default TTL for subscriptions.
    pub subscribe_ttl: u32,
}

impl Default for SdClientConfig {
    fn default() -> Self {
        Self {
            bind_addr: SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, SD_DEFAULT_PORT)),
            multicast_addr: SocketAddr::V4(SocketAddrV4::new(SD_MULTICAST_ADDR, SD_DEFAULT_PORT)),
            multicast_interface: None,
            find_ttl: 0xFFFFFF,
            subscribe_ttl: 0xFFFFFF,
        }
    }
}

/// SOME/IP-SD client for discovering services and subscribing to events.
pub struct SdClient {
    socket: UdpSocket,
    multicast_addr: SocketAddr,
    services: HashMap<(ServiceId, InstanceId), ServiceInfo>,
    recv_buffer: Vec<u8>,
    subscribe_ttl: u32,
    local_endpoint: Option<Endpoint>,
}

impl SdClient {
    /// Create a new SD client with default configuration.
    pub fn new() -> Result<Self> {
        Self::with_config(SdClientConfig::default())
    }

    /// Create a new SD client with custom configuration.
    pub fn with_config(config: SdClientConfig) -> Result<Self> {
        let socket = UdpSocket::bind(config.bind_addr).map_err(SomeIpError::io)?;

        // Join multicast group
        if let SocketAddr::V4(multicast) = config.multicast_addr {
            let interface = config.multicast_interface.unwrap_or(Ipv4Addr::UNSPECIFIED);
            socket
                .join_multicast_v4(multicast.ip(), &interface)
                .map_err(SomeIpError::io)?;
        }

        // Set non-blocking for poll operations
        socket.set_nonblocking(true).map_err(SomeIpError::io)?;

        Ok(Self {
            socket,
            multicast_addr: config.multicast_addr,
            services: HashMap::new(),
            recv_buffer: vec![0u8; 65535],
            subscribe_ttl: config.subscribe_ttl,
            local_endpoint: None,
        })
    }

    /// Set the local endpoint to use for subscriptions.
    pub fn set_local_endpoint(&mut self, endpoint: Endpoint) {
        self.local_endpoint = Some(endpoint);
    }

    /// Get the local address of the socket.
    pub fn local_addr(&self) -> Result<SocketAddr> {
        self.socket.local_addr().map_err(SomeIpError::io)
    }

    /// Send a FindService message for a specific service.
    pub fn find_service(
        &mut self,
        service_id: ServiceId,
        instance_id: InstanceId,
    ) -> Result<()> {
        self.find_service_version(service_id, instance_id, 0xFF, 0xFFFFFFFF)
    }

    /// Send a FindService message for a specific service version.
    pub fn find_service_version(
        &mut self,
        service_id: ServiceId,
        instance_id: InstanceId,
        major_version: u8,
        minor_version: u32,
    ) -> Result<()> {
        let msg = SdMessage::find_service(service_id, instance_id, major_version, minor_version);
        self.send_message(&msg)
    }

    /// Subscribe to an eventgroup.
    pub fn subscribe(
        &mut self,
        service_id: ServiceId,
        instance_id: InstanceId,
        eventgroup_id: EventgroupId,
        major_version: u8,
    ) -> Result<()> {
        let endpoint = self.local_endpoint.clone().ok_or_else(|| {
            SomeIpError::invalid_header("Local endpoint not set for subscription")
        })?;

        let msg = SdMessage::subscribe_eventgroup(
            service_id,
            instance_id,
            major_version,
            eventgroup_id,
            self.subscribe_ttl,
            endpoint,
        );
        self.send_message(&msg)
    }

    /// Unsubscribe from an eventgroup.
    pub fn unsubscribe(
        &mut self,
        service_id: ServiceId,
        instance_id: InstanceId,
        eventgroup_id: EventgroupId,
        major_version: u8,
    ) -> Result<()> {
        let msg = SdMessage::stop_subscribe_eventgroup(
            service_id,
            instance_id,
            major_version,
            eventgroup_id,
        );
        self.send_message(&msg)
    }

    /// Send an SD message.
    fn send_message(&self, msg: &SdMessage) -> Result<()> {
        let someip_msg = msg.to_someip_message();
        let mut buf = Vec::with_capacity(16 + someip_msg.payload.len());
        buf.extend_from_slice(&someip_msg.header.to_bytes());
        buf.extend_from_slice(&someip_msg.payload);

        self.socket
            .send_to(&buf, self.multicast_addr)
            .map_err(SomeIpError::io)?;

        Ok(())
    }

    /// Poll for incoming SD messages (non-blocking).
    pub fn poll(&mut self) -> Result<Option<SdEvent>> {
        match self.socket.recv_from(&mut self.recv_buffer) {
            Ok((size, src_addr)) => {
                // Copy data to avoid borrow issues
                let data = self.recv_buffer[..size].to_vec();
                self.process_message(&data, src_addr)
            }
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => Ok(None),
            Err(e) => Err(SomeIpError::io(e)),
        }
    }

    /// Wait for a specific service to become available.
    pub fn wait_for_service(
        &mut self,
        service_id: ServiceId,
        instance_id: InstanceId,
        timeout: Duration,
    ) -> Result<Option<ServiceInfo>> {
        let deadline = Instant::now() + timeout;

        // Check if already known
        if let Some(info) = self.get_service(service_id, instance_id) {
            if !info.is_expired() {
                return Ok(Some(info.clone()));
            }
        }

        // Send find request
        self.find_service(service_id, instance_id)?;

        // Poll until found or timeout
        while Instant::now() < deadline {
            if let Some(event) = self.poll()? {
                if let SdEvent::ServiceAvailable(info) = event {
                    if info.service_id == service_id
                        && (instance_id.is_any() || info.instance_id == instance_id)
                    {
                        return Ok(Some(info));
                    }
                }
            }

            // Small sleep to avoid busy waiting
            std::thread::sleep(Duration::from_millis(10));
        }

        Ok(None)
    }

    /// Get a known service by ID.
    pub fn get_service(&self, service_id: ServiceId, instance_id: InstanceId) -> Option<&ServiceInfo> {
        self.services.get(&(service_id, instance_id))
    }

    /// Get all known services.
    pub fn services(&self) -> impl Iterator<Item = &ServiceInfo> {
        self.services.values()
    }

    /// Remove expired services.
    pub fn cleanup_expired(&mut self) -> Vec<(ServiceId, InstanceId)> {
        let expired: Vec<_> = self
            .services
            .iter()
            .filter(|(_, info)| info.is_expired())
            .map(|(key, _)| *key)
            .collect();

        for key in &expired {
            self.services.remove(key);
        }

        expired
    }

    /// Process a received message.
    fn process_message(&mut self, data: &[u8], src_addr: SocketAddr) -> Result<Option<SdEvent>> {
        // Skip SOME/IP header (16 bytes)
        if data.len() < 16 {
            return Ok(None);
        }

        let sd_payload = &data[16..];
        let sd_msg = match SdMessage::from_bytes(sd_payload) {
            Ok(msg) => msg,
            Err(_) => return Ok(None),
        };

        // Process each entry
        for entry in &sd_msg.entries {
            match entry {
                SdEntry::Service(service_entry) => {
                    match service_entry.entry_type {
                        EntryType::OfferService => {
                            if service_entry.ttl == 0 {
                                // Stop offer
                                let key = (service_entry.service_id, service_entry.instance_id);
                                self.services.remove(&key);
                                return Ok(Some(SdEvent::ServiceUnavailable {
                                    service_id: service_entry.service_id,
                                    instance_id: service_entry.instance_id,
                                }));
                            } else {
                                // New or updated offer
                                let endpoints = sd_msg.get_endpoints_for_entry(entry);
                                let info = ServiceInfo {
                                    service_id: service_entry.service_id,
                                    instance_id: service_entry.instance_id,
                                    major_version: service_entry.major_version,
                                    minor_version: service_entry.minor_version,
                                    endpoints,
                                    expires_at: Instant::now()
                                        + Duration::from_secs(service_entry.ttl as u64),
                                    source_addr: src_addr,
                                };
                                let key = (service_entry.service_id, service_entry.instance_id);
                                self.services.insert(key, info.clone());
                                return Ok(Some(SdEvent::ServiceAvailable(info)));
                            }
                        }
                        EntryType::FindService => {
                            // Ignore find requests (we're a client)
                        }
                        _ => {}
                    }
                }
                SdEntry::Eventgroup(eg_entry) => {
                    if eg_entry.entry_type == EntryType::SubscribeEventgroupAck {
                        if eg_entry.ttl == 0 {
                            // NACK
                            return Ok(Some(SdEvent::SubscriptionNack {
                                service_id: eg_entry.service_id,
                                instance_id: eg_entry.instance_id,
                                eventgroup_id: eg_entry.eventgroup_id,
                            }));
                        } else {
                            // ACK
                            let endpoints = sd_msg.get_endpoints_for_entry(entry);
                            let multicast_endpoint = endpoints.into_iter().next();
                            return Ok(Some(SdEvent::SubscriptionAck {
                                service_id: eg_entry.service_id,
                                instance_id: eg_entry.instance_id,
                                eventgroup_id: eg_entry.eventgroup_id,
                                multicast_endpoint,
                            }));
                        }
                    }
                }
            }
        }

        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_service_info_expiry() {
        let info = ServiceInfo {
            service_id: ServiceId(0x1234),
            instance_id: InstanceId(0x0001),
            major_version: 1,
            minor_version: 0,
            endpoints: vec![],
            expires_at: Instant::now() + Duration::from_secs(10),
            source_addr: "192.168.1.1:30490".parse().unwrap(),
        };

        assert!(!info.is_expired());
        assert!(info.remaining_ttl() > 0);
    }

    #[test]
    fn test_sd_client_config_default() {
        let config = SdClientConfig::default();
        assert_eq!(config.find_ttl, 0xFFFFFF);
        assert_eq!(config.subscribe_ttl, 0xFFFFFF);
    }
}
