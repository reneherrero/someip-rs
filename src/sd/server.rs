//! SOME/IP-SD server for offering services.

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

/// An offered service.
#[derive(Debug, Clone)]
pub struct OfferedService {
    /// Service ID.
    pub service_id: ServiceId,
    /// Instance ID.
    pub instance_id: InstanceId,
    /// Major version.
    pub major_version: u8,
    /// Minor version.
    pub minor_version: u32,
    /// Endpoint where the service is available.
    pub endpoint: Endpoint,
    /// TTL in seconds for offer announcements.
    pub ttl: u32,
}

/// A subscription from a client.
#[derive(Debug, Clone)]
#[allow(dead_code)]
struct Subscription {
    /// Subscriber's address.
    client_addr: SocketAddr,
    /// Subscriber's endpoint for events.
    client_endpoint: Endpoint,
    /// Counter value from subscription.
    counter: u8,
    /// When the subscription expires.
    expires_at: Instant,
}

/// Requests received by the SD server.
#[derive(Debug, Clone)]
pub enum SdRequest {
    /// A client is looking for a service.
    FindService {
        /// Service ID being searched.
        service_id: ServiceId,
        /// Instance ID being searched.
        instance_id: InstanceId,
        /// Major version requested.
        major_version: u8,
        /// Minor version requested.
        minor_version: u32,
        /// Source address of the request.
        from: SocketAddr,
    },
    /// A client wants to subscribe to an eventgroup.
    Subscribe {
        /// Service ID.
        service_id: ServiceId,
        /// Instance ID.
        instance_id: InstanceId,
        /// Eventgroup ID.
        eventgroup_id: EventgroupId,
        /// Major version.
        major_version: u8,
        /// TTL requested.
        ttl: u32,
        /// Counter for tracking.
        counter: u8,
        /// Client's endpoint for receiving events.
        endpoint: Endpoint,
        /// Source address of the request.
        from: SocketAddr,
    },
    /// A client wants to unsubscribe from an eventgroup.
    Unsubscribe {
        /// Service ID.
        service_id: ServiceId,
        /// Instance ID.
        instance_id: InstanceId,
        /// Eventgroup ID.
        eventgroup_id: EventgroupId,
        /// Source address of the request.
        from: SocketAddr,
    },
}

/// SD server configuration.
#[derive(Debug, Clone)]
pub struct SdServerConfig {
    /// Local address to bind to.
    pub bind_addr: SocketAddr,
    /// Multicast address for SD.
    pub multicast_addr: SocketAddr,
    /// Interface address for multicast (None = any).
    pub multicast_interface: Option<Ipv4Addr>,
    /// Interval for cyclic offer announcements.
    pub offer_interval: Duration,
}

impl Default for SdServerConfig {
    fn default() -> Self {
        Self {
            bind_addr: SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, SD_DEFAULT_PORT)),
            multicast_addr: SocketAddr::V4(SocketAddrV4::new(SD_MULTICAST_ADDR, SD_DEFAULT_PORT)),
            multicast_interface: None,
            offer_interval: Duration::from_secs(1),
        }
    }
}

/// Key for identifying a subscription.
type SubscriptionKey = (ServiceId, InstanceId, EventgroupId, SocketAddr);

/// SOME/IP-SD server for offering services and handling subscriptions.
pub struct SdServer {
    socket: UdpSocket,
    multicast_addr: SocketAddr,
    offered_services: HashMap<(ServiceId, InstanceId), OfferedService>,
    subscriptions: HashMap<SubscriptionKey, Subscription>,
    recv_buffer: Vec<u8>,
    last_offer_time: Option<Instant>,
    offer_interval: Duration,
}

impl SdServer {
    /// Create a new SD server with default configuration.
    pub fn new() -> Result<Self> {
        Self::with_config(SdServerConfig::default())
    }

    /// Create a new SD server with custom configuration.
    pub fn with_config(config: SdServerConfig) -> Result<Self> {
        let socket = UdpSocket::bind(config.bind_addr).map_err(SomeIpError::io)?;

        // Join multicast group
        if let SocketAddr::V4(multicast) = config.multicast_addr {
            let interface = config.multicast_interface.unwrap_or(Ipv4Addr::UNSPECIFIED);
            socket
                .join_multicast_v4(multicast.ip(), &interface)
                .map_err(SomeIpError::io)?;
        }

        // Enable sending to multicast
        socket.set_multicast_loop_v4(true).ok();

        // Set non-blocking for poll operations
        socket.set_nonblocking(true).map_err(SomeIpError::io)?;

        Ok(Self {
            socket,
            multicast_addr: config.multicast_addr,
            offered_services: HashMap::new(),
            subscriptions: HashMap::new(),
            recv_buffer: vec![0u8; 65535],
            last_offer_time: None,
            offer_interval: config.offer_interval,
        })
    }

    /// Get the local address of the socket.
    pub fn local_addr(&self) -> Result<SocketAddr> {
        self.socket.local_addr().map_err(SomeIpError::io)
    }

    /// Start offering a service.
    pub fn offer_service(&mut self, service: OfferedService) -> Result<()> {
        let key = (service.service_id, service.instance_id);
        self.offered_services.insert(key, service.clone());

        // Send initial offer
        let msg = SdMessage::offer_service(
            service.service_id,
            service.instance_id,
            service.major_version,
            service.minor_version,
            service.ttl,
            service.endpoint,
        );
        self.send_multicast(&msg)
    }

    /// Stop offering a service.
    pub fn stop_offer_service(
        &mut self,
        service_id: ServiceId,
        instance_id: InstanceId,
    ) -> Result<()> {
        let key = (service_id, instance_id);
        if let Some(service) = self.offered_services.remove(&key) {
            // Send stop offer
            let msg = SdMessage::stop_offer_service(
                service_id,
                instance_id,
                service.major_version,
                service.minor_version,
            );
            self.send_multicast(&msg)?;
        }
        Ok(())
    }

    /// Get all offered services.
    pub fn offered_services(&self) -> impl Iterator<Item = &OfferedService> {
        self.offered_services.values()
    }

    /// Send cyclic offer announcements for all services.
    pub fn send_offers(&mut self) -> Result<()> {
        for service in self.offered_services.values() {
            let msg = SdMessage::offer_service(
                service.service_id,
                service.instance_id,
                service.major_version,
                service.minor_version,
                service.ttl,
                service.endpoint.clone(),
            );
            self.send_multicast(&msg)?;
        }
        self.last_offer_time = Some(Instant::now());
        Ok(())
    }

    /// Check if it's time to send cyclic offers.
    pub fn should_send_offers(&self) -> bool {
        match self.last_offer_time {
            Some(last) => Instant::now().duration_since(last) >= self.offer_interval,
            None => true,
        }
    }

    /// Accept a subscription request.
    pub fn accept_subscription(
        &mut self,
        service_id: ServiceId,
        instance_id: InstanceId,
        eventgroup_id: EventgroupId,
        counter: u8,
        client_addr: SocketAddr,
        client_endpoint: Endpoint,
        ttl: u32,
        multicast_endpoint: Option<Endpoint>,
    ) -> Result<()> {
        // Store subscription
        let key = (service_id, instance_id, eventgroup_id, client_addr);
        self.subscriptions.insert(
            key,
            Subscription {
                client_addr,
                client_endpoint,
                counter,
                expires_at: Instant::now() + Duration::from_secs(ttl as u64),
            },
        );

        // Get major version from offered service
        let major_version = self
            .offered_services
            .get(&(service_id, instance_id))
            .map(|s| s.major_version)
            .unwrap_or(0xFF);

        // Send ACK
        let msg = SdMessage::subscribe_eventgroup_ack(
            service_id,
            instance_id,
            major_version,
            eventgroup_id,
            ttl,
            counter,
            multicast_endpoint,
        );
        self.send_to(&msg, client_addr)
    }

    /// Reject a subscription request.
    pub fn reject_subscription(
        &mut self,
        service_id: ServiceId,
        instance_id: InstanceId,
        eventgroup_id: EventgroupId,
        counter: u8,
        client_addr: SocketAddr,
    ) -> Result<()> {
        // Get major version from offered service
        let major_version = self
            .offered_services
            .get(&(service_id, instance_id))
            .map(|s| s.major_version)
            .unwrap_or(0xFF);

        // Send NACK
        let msg = SdMessage::subscribe_eventgroup_nack(
            service_id,
            instance_id,
            major_version,
            eventgroup_id,
            counter,
        );
        self.send_to(&msg, client_addr)
    }

    /// Get subscribers for an eventgroup.
    pub fn get_subscribers(
        &self,
        service_id: ServiceId,
        instance_id: InstanceId,
        eventgroup_id: EventgroupId,
    ) -> Vec<&Endpoint> {
        self.subscriptions
            .iter()
            .filter(|((sid, iid, egid, _), sub)| {
                *sid == service_id
                    && *iid == instance_id
                    && *egid == eventgroup_id
                    && Instant::now() < sub.expires_at
            })
            .map(|(_, sub)| &sub.client_endpoint)
            .collect()
    }

    /// Remove expired subscriptions.
    pub fn cleanup_expired(&mut self) -> Vec<SubscriptionKey> {
        let expired: Vec<_> = self
            .subscriptions
            .iter()
            .filter(|(_, sub)| Instant::now() >= sub.expires_at)
            .map(|(key, _)| *key)
            .collect();

        for key in &expired {
            self.subscriptions.remove(key);
        }

        expired
    }

    /// Poll for incoming SD requests (non-blocking).
    pub fn poll(&mut self) -> Result<Option<SdRequest>> {
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

    /// Send a message to the multicast address.
    fn send_multicast(&self, msg: &SdMessage) -> Result<()> {
        self.send_to(msg, self.multicast_addr)
    }

    /// Send a message to a specific address.
    fn send_to(&self, msg: &SdMessage, addr: SocketAddr) -> Result<()> {
        let someip_msg = msg.to_someip_message();
        let mut buf = Vec::with_capacity(16 + someip_msg.payload.len());
        buf.extend_from_slice(&someip_msg.header.to_bytes());
        buf.extend_from_slice(&someip_msg.payload);

        self.socket.send_to(&buf, addr).map_err(SomeIpError::io)?;

        Ok(())
    }

    /// Process a received message.
    fn process_message(&mut self, data: &[u8], src_addr: SocketAddr) -> Result<Option<SdRequest>> {
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
                    if service_entry.entry_type == EntryType::FindService {
                        // Check if we offer this service
                        let key = (service_entry.service_id, service_entry.instance_id);
                        if let Some(offered) = self.offered_services.get(&key) {
                            // Send unicast offer response
                            let msg = SdMessage::offer_service(
                                offered.service_id,
                                offered.instance_id,
                                offered.major_version,
                                offered.minor_version,
                                offered.ttl,
                                offered.endpoint.clone(),
                            );
                            self.send_to(&msg, src_addr)?;
                        }

                        return Ok(Some(SdRequest::FindService {
                            service_id: service_entry.service_id,
                            instance_id: service_entry.instance_id,
                            major_version: service_entry.major_version,
                            minor_version: service_entry.minor_version,
                            from: src_addr,
                        }));
                    }
                }
                SdEntry::Eventgroup(eg_entry) => {
                    if eg_entry.entry_type == EntryType::SubscribeEventgroup {
                        let endpoints = sd_msg.get_endpoints_for_entry(entry);
                        let endpoint = endpoints.into_iter().next();

                        if eg_entry.ttl == 0 {
                            // Unsubscribe
                            let key = (
                                eg_entry.service_id,
                                eg_entry.instance_id,
                                eg_entry.eventgroup_id,
                                src_addr,
                            );
                            self.subscriptions.remove(&key);

                            return Ok(Some(SdRequest::Unsubscribe {
                                service_id: eg_entry.service_id,
                                instance_id: eg_entry.instance_id,
                                eventgroup_id: eg_entry.eventgroup_id,
                                from: src_addr,
                            }));
                        } else if let Some(ep) = endpoint {
                            // Subscribe
                            return Ok(Some(SdRequest::Subscribe {
                                service_id: eg_entry.service_id,
                                instance_id: eg_entry.instance_id,
                                eventgroup_id: eg_entry.eventgroup_id,
                                major_version: eg_entry.major_version,
                                ttl: eg_entry.ttl,
                                counter: eg_entry.counter,
                                endpoint: ep,
                                from: src_addr,
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
    fn test_offered_service() {
        let service = OfferedService {
            service_id: ServiceId(0x1234),
            instance_id: InstanceId(0x0001),
            major_version: 1,
            minor_version: 0,
            endpoint: Endpoint::tcp("192.168.1.100:30490".parse().unwrap()),
            ttl: 3600,
        };

        assert_eq!(service.service_id, ServiceId(0x1234));
        assert_eq!(service.ttl, 3600);
    }

    #[test]
    fn test_sd_server_config_default() {
        let config = SdServerConfig::default();
        assert_eq!(config.offer_interval, Duration::from_secs(1));
    }
}
