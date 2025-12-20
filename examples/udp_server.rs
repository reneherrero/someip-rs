//! UDP server example.
//!
//! This example demonstrates a SOME/IP UDP server that handles
//! requests and sends responses.
//!
//! Run with: cargo run --example udp_server
//! Then connect with: cargo run --example udp_client

use someip_rs::transport::UdpServer;
use someip_rs::{MessageType, ReturnCode, ServiceId};

const SERVICE_ID: u16 = 0x4321;
const BIND_ADDR: &str = "127.0.0.1:30491";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Starting SOME/IP UDP server on {BIND_ADDR}...");

    let mut server = UdpServer::bind(BIND_ADDR)?;
    println!("Server listening on {}", server.local_addr());

    loop {
        match server.receive() {
            Ok((request, client_addr)) => {
                println!(
                    "Received from {}: service={}, method={}, type={:?}, payload={} bytes",
                    client_addr,
                    request.header.service_id,
                    request.header.method_id,
                    request.header.message_type,
                    request.payload.len()
                );

                // Check if it's our service
                if request.header.service_id != ServiceId(SERVICE_ID) {
                    println!("Unknown service {}", request.header.service_id);
                    server.respond_error(&request, ReturnCode::UnknownService, client_addr)?;
                    continue;
                }

                // Handle based on message type
                match request.header.message_type {
                    MessageType::Request => {
                        // Process the request and create a response
                        let response_payload = process_request(&request.payload);
                        server.respond(&request, response_payload, client_addr)?;
                        println!("Sent response to {client_addr}");
                    }
                    MessageType::RequestNoReturn | MessageType::Notification => {
                        // Fire-and-forget, no response needed
                        println!(
                            "Received notification: {:?}",
                            String::from_utf8_lossy(&request.payload)
                        );
                    }
                    _ => {
                        println!("Unexpected message type: {:?}", request.header.message_type);
                    }
                }
            }
            Err(e) => {
                eprintln!("Receive error: {e}");
            }
        }
    }
}

fn process_request(payload: &[u8]) -> Vec<u8> {
    // Simple example: reverse the payload
    let mut result = payload.to_vec();
    result.reverse();
    result
}
