//! SOME/IP Transport Protocol (TP) for large message segmentation.
//!
//! SOME/IP-TP enables sending messages larger than the maximum UDP datagram size
//! by segmenting them into multiple smaller packets and reassembling on the receiver.
//!
//! # Overview
//!
//! - Messages are split into segments of up to ~1392 bytes each
//! - Each segment includes a 4-byte TP header after the SOME/IP header
//! - The TP header contains offset (in 16-byte units) and a "more segments" flag
//! - Message type has the TP flag (0x20) OR'd in
//!
//! # Example
//!
//! ```no_run
//! use someip_rs::tp::{TpUdpClient, TpUdpServer};
//! use someip_rs::{SomeIpMessage, ServiceId, MethodId};
//!
//! // Client automatically segments large messages
//! let mut client = TpUdpClient::new().unwrap();
//!
//! let request = SomeIpMessage::request(ServiceId(0x1234), MethodId(0x0001))
//!     .payload_vec(vec![0u8; 5000]) // Larger than single UDP packet
//!     .build();
//!
//! // Message is automatically segmented and sent
//! client.send_to("127.0.0.1:30490", request).unwrap();
//! ```

mod client;
mod header;
mod reassembly;
mod segment;
mod server;

pub use client::TpUdpClient;
pub use header::{TpHeader, TP_HEADER_SIZE};
pub use reassembly::{ReassemblyKey, TpReassembler};
pub use segment::{needs_segmentation, segment_message, TpSegment, DEFAULT_MAX_SEGMENT_PAYLOAD};
pub use server::TpUdpServer;
