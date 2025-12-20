//! SOME/IP protocol implementation built on std::net.
//!
//! This crate provides a synchronous implementation of the SOME/IP
//! (Scalable service-Oriented MiddlewarE over IP) protocol, commonly
//! used in automotive applications.
//!
//! # Features
//!
//! - Complete SOME/IP message header support
//! - TCP and UDP transport layers
//! - Type-safe service, method, client, and session IDs
//! - Request/response pattern support
//! - Fire-and-forget (notification) messages
//! - SOME/IP-SD (Service Discovery) for dynamic service discovery
//!
//! # Example
//!
//! ```no_run
//! use someip_rs::{SomeIpMessage, ServiceId, MethodId, ClientId, SessionId};
//! use someip_rs::transport::TcpClient;
//!
//! // Create a request message
//! let request = SomeIpMessage::request(ServiceId(0x1234), MethodId(0x0001))
//!     .client_id(ClientId(0x0100))
//!     .payload(b"hello".as_slice())
//!     .build();
//!
//! // Send via TCP and receive response
//! let mut client = TcpClient::connect("127.0.0.1:30490").unwrap();
//! let response = client.call(request).unwrap();
//!
//! println!("Response: {:?}", response.payload);
//! ```
//!
//! # Protocol Overview
//!
//! SOME/IP messages consist of a 16-byte header followed by an optional payload:
//!
//! ```text
//! +--------+--------+--------+--------+
//! |    Service ID   |   Method ID     |  (4 bytes)
//! +--------+--------+--------+--------+
//! |           Length                  |  (4 bytes)
//! +--------+--------+--------+--------+
//! |    Client ID    |   Session ID    |  (4 bytes)
//! +--------+--------+--------+--------+
//! |Proto|Iface|MsgType|RetCode|        (4 bytes)
//! +--------+--------+--------+--------+
//! |           Payload ...             |  (variable)
//! +--------+--------+--------+--------+
//! ```

pub mod codec;
pub mod connection;
pub mod error;
pub mod header;
pub mod message;
pub mod sd;
pub mod tp;
pub mod transport;
pub mod types;

// Async modules (require tokio feature)
#[cfg(feature = "tokio")]
pub mod codec_async;
#[cfg(feature = "tokio")]
pub mod transport_async;

// Re-export commonly used types at the crate root
pub use error::{Result, SomeIpError};
pub use header::{ClientId, MethodId, ServiceId, SessionId, SomeIpHeader, HEADER_SIZE};
pub use message::{MessageBuilder, SomeIpMessage};
pub use tp::{TpReassembler, TpSegment, TpUdpClient, TpUdpServer};
pub use types::{MessageType, ReturnCode, PROTOCOL_VERSION};
