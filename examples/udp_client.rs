//! UDP client example.
//!
//! This example demonstrates a SOME/IP UDP client that sends
//! requests and receives responses.
//!
//! Run the server first: cargo run --example udp_server
//! Then run: cargo run --example udp_client

use someip_rs::transport::UdpClient;
use someip_rs::{ClientId, MethodId, ServiceId, SomeIpMessage};
use std::time::Duration;

const SERVICE_ID: u16 = 0x4321;
const METHOD_REVERSE: u16 = 0x0001;
const SERVER_ADDR: &str = "127.0.0.1:30491";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Creating SOME/IP UDP client...");

    let mut client = UdpClient::new()?;
    client.set_client_id(ClientId(0x0200));
    client.set_read_timeout(Some(Duration::from_secs(5)))?;

    println!("Client bound to {}", client.local_addr()?);

    // Example 1: Request with call_to (specify address per-request)
    println!("\n--- Example 1: Request/Response ---");
    let request = SomeIpMessage::request(ServiceId(SERVICE_ID), MethodId(METHOD_REVERSE))
        .payload(b"Hello UDP!".as_slice())
        .build();

    println!(
        "Sending: {:?}",
        String::from_utf8_lossy(&request.payload)
    );

    let response = client.call_to(SERVER_ADDR, request)?;
    println!(
        "Received: {:?} (reversed)",
        String::from_utf8_lossy(&response.payload)
    );

    // Example 2: Connect and use call()
    println!("\n--- Example 2: Connected Mode ---");
    client.connect(SERVER_ADDR)?;
    println!("Connected to {SERVER_ADDR}");

    for word in ["Rust", "SOME/IP", "Automotive"] {
        let request = SomeIpMessage::request(ServiceId(SERVICE_ID), MethodId(METHOD_REVERSE))
            .payload(word.as_bytes().to_vec())
            .build();

        let response = client.call(request)?;
        println!(
            "{} -> {}",
            word,
            String::from_utf8_lossy(&response.payload)
        );
    }

    // Example 3: Send notification (fire-and-forget)
    println!("\n--- Example 3: Notification ---");
    let notification = SomeIpMessage::notification(ServiceId(SERVICE_ID), MethodId(0x8001))
        .payload(b"Event occurred!".as_slice())
        .build();

    client.send(notification)?;
    println!("Notification sent!");

    println!("\nDone!");
    Ok(())
}
