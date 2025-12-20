//! SOME/IP Service Discovery (SD) implementation.
//!
//! This module provides types and utilities for SOME/IP-SD, which enables
//! dynamic service discovery and event subscription in automotive networks.
//!
//! # Overview
//!
//! SOME/IP-SD uses special SOME/IP messages (Service ID 0xFFFF, Method ID 0x8100)
//! to discover services and manage event subscriptions. It typically runs over
//! UDP multicast (224.224.224.245:30490).
//!
//! # Example
//!
//! ```no_run
//! use someip_rs::sd::{SdClient, SdMessage, InstanceId};
//! use someip_rs::ServiceId;
//!
//! // Create an SD client
//! let mut client = SdClient::new().unwrap();
//!
//! // Find a service
//! client.find_service(ServiceId(0x1234), InstanceId::ANY).unwrap();
//! ```

mod client;
mod entry;
mod message;
mod option;
mod server;
mod types;

pub use client::{SdClient, SdClientConfig, SdEvent, ServiceInfo};
pub use entry::{EventgroupEntry, SdEntry, ServiceEntry};
pub use message::{SdFlags, SdMessage};
pub use option::{ConfigurationOption, Endpoint, IPv4EndpointOption, IPv6EndpointOption, SdOption};
pub use server::{OfferedService, SdRequest, SdServer};
pub use types::{
    EntryType, EventgroupId, InstanceId, OptionType, TransportProtocol, SD_DEFAULT_PORT,
    SD_ENTRY_SIZE, SD_METHOD_ID, SD_MULTICAST_ADDR, SD_SERVICE_ID,
};
