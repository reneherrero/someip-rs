//! TCP client example.
//!
//! This example demonstrates a SOME/IP client that sends requests
//! to a server and receives responses.
//!
//! Run the server first: cargo run --example tcp_server
//! Then run: cargo run --example tcp_client

use someip_rs::transport::TcpClient;
use someip_rs::{ClientId, MethodId, ServiceId, SomeIpMessage};
use std::time::Duration;

const SERVICE_ID: u16 = 0x1234;
const METHOD_ECHO: u16 = 0x0001;
const METHOD_GREET: u16 = 0x0002;
const SERVER_ADDR: &str = "127.0.0.1:30490";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Connecting to SOME/IP server at {SERVER_ADDR}...");

    let mut client = TcpClient::connect(SERVER_ADDR)?;
    client.set_client_id(ClientId(0x0100));
    client.set_read_timeout(Some(Duration::from_secs(5)))?;

    println!("Connected!");

    // Example 1: Simple echo request
    println!("\n--- Example 1: Echo Request ---");
    let request = SomeIpMessage::request(ServiceId(SERVICE_ID), MethodId(METHOD_ECHO))
        .payload(b"Hello, SOME/IP!".as_slice())
        .build();

    println!(
        "Sending request: service={}, method={}, payload={:?}",
        request.header.service_id,
        request.header.method_id,
        String::from_utf8_lossy(&request.payload)
    );

    let response = client.call(request)?;
    println!(
        "Received response: return_code={:?}, payload={:?}",
        response.header.return_code,
        String::from_utf8_lossy(&response.payload)
    );

    // Example 2: Multiple requests
    println!("\n--- Example 2: Multiple Requests ---");
    for i in 1..=3 {
        let payload = format!("Message #{i}");
        let request = SomeIpMessage::request(ServiceId(SERVICE_ID), MethodId(METHOD_GREET))
            .payload(payload.as_bytes().to_vec())
            .build();

        println!("Request {i}: session_id={}", request.header.session_id);
        let response = client.call(request)?;
        println!(
            "Response {i}: session_id={}, payload={:?}",
            response.header.session_id,
            String::from_utf8_lossy(&response.payload)
        );
    }

    // Example 3: Fire-and-forget (notification)
    println!("\n--- Example 3: Fire-and-Forget ---");
    let notification =
        SomeIpMessage::request_no_return(ServiceId(SERVICE_ID), MethodId(METHOD_ECHO))
            .payload(b"This is a notification".as_slice())
            .build();

    println!("Sending notification (no response expected)...");
    client.send(notification)?;
    println!("Notification sent!");

    println!("\nDone!");
    Ok(())
}
