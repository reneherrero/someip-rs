//! TCP echo server example.
//!
//! This example demonstrates a simple SOME/IP server that echoes back
//! any payload it receives.
//!
//! Run with: cargo run --example tcp_server
//! Then connect with: cargo run --example tcp_client

use someip_rs::transport::TcpServer;
use someip_rs::{MessageType, ReturnCode, ServiceId};
use std::thread;

const SERVICE_ID: u16 = 0x1234;
const BIND_ADDR: &str = "127.0.0.1:30490";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Starting SOME/IP TCP server on {BIND_ADDR}...");

    let server = TcpServer::bind(BIND_ADDR)?;
    println!("Server listening on {}", server.local_addr());

    for connection in server.incoming() {
        match connection {
            Ok(mut conn) => {
                let peer = conn.peer_addr();
                println!("New connection from {peer}");

                // Handle each connection in a separate thread
                thread::spawn(move || {
                    loop {
                        match conn.read_message() {
                            Ok(request) => {
                                println!(
                                    "Received: service={}, method={}, type={:?}, payload={} bytes",
                                    request.header.service_id,
                                    request.header.method_id,
                                    request.header.message_type,
                                    request.payload.len()
                                );

                                // Check if it's our service
                                if request.header.service_id != ServiceId(SERVICE_ID) {
                                    println!("Unknown service, sending error response");
                                    let error = request
                                        .create_error_response(ReturnCode::UnknownService)
                                        .build();
                                    let _ = conn.write_message(&error);
                                    continue;
                                }

                                // Only respond to requests (not notifications)
                                if request.header.message_type == MessageType::Request {
                                    // Echo back the payload
                                    let response = request
                                        .create_response()
                                        .payload(request.payload.clone())
                                        .build();

                                    if let Err(e) = conn.write_message(&response) {
                                        eprintln!("Failed to send response: {e}");
                                        break;
                                    }
                                    println!("Sent response");
                                }
                            }
                            Err(e) => {
                                eprintln!("Connection error: {e}");
                                break;
                            }
                        }
                    }
                    println!("Connection closed: {peer}");
                });
            }
            Err(e) => {
                eprintln!("Accept error: {e}");
            }
        }
    }

    Ok(())
}
