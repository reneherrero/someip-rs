//! Async transport layer for SOME/IP using Tokio.
//!
//! This module provides async versions of the TCP and UDP transport types.
//!
//! # Example
//!
//! ```no_run
//! use someip_rs::transport_async::{AsyncTcpClient, AsyncTcpServer};
//! use someip_rs::{SomeIpMessage, ServiceId, MethodId};
//!
//! #[tokio::main(flavor = "current_thread")]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let mut client = AsyncTcpClient::connect("127.0.0.1:30490").await?;
//!
//!     let request = SomeIpMessage::request(ServiceId(0x1234), MethodId(0x0001))
//!         .payload(b"hello".as_slice())
//!         .build();
//!
//!     let response = client.call(request).await?;
//!     println!("Response: {:?}", response.payload);
//!
//!     Ok(())
//! }
//! ```

mod tcp;
mod udp;

pub use tcp::{AsyncTcpClient, AsyncTcpConnection, AsyncTcpServer};
pub use udp::{AsyncUdpClient, AsyncUdpServer};
