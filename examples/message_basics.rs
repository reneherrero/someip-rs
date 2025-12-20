//! Message basics example.
//!
//! This example demonstrates how to create, serialize, and parse
//! SOME/IP messages without network transport.
//!
//! Run with: cargo run --example message_basics

use someip_rs::{
    ClientId, MethodId, ReturnCode, ServiceId, SessionId, SomeIpHeader, SomeIpMessage,
    HEADER_SIZE,
};

fn main() {
    println!("=== SOME/IP Message Basics ===\n");

    // Example 1: Create a request message using the builder
    println!("--- Example 1: Building Messages ---");
    let request = SomeIpMessage::request(ServiceId(0x1234), MethodId(0x0001))
        .client_id(ClientId(0x0100))
        .session_id(SessionId(0x0001))
        .interface_version(2)
        .payload(b"Hello, World!".as_slice())
        .build();

    println!("Request message:");
    print_message(&request);

    // Example 2: Create a response from the request
    println!("\n--- Example 2: Creating Responses ---");
    let response = request
        .create_response()
        .payload(b"Response data".as_slice())
        .build();

    println!("Response message:");
    print_message(&response);

    // Example 3: Create an error response
    println!("\n--- Example 3: Error Response ---");
    let error = request
        .create_error_response(ReturnCode::UnknownMethod)
        .build();

    println!("Error response:");
    print_message(&error);

    // Example 4: Serialize and deserialize
    println!("\n--- Example 4: Serialization ---");
    let bytes = request.to_bytes();
    println!("Serialized to {} bytes", bytes.len());
    println!("Header bytes: {:02X?}", &bytes[..HEADER_SIZE]);
    println!("Payload bytes: {:02X?}", &bytes[HEADER_SIZE..]);

    let parsed = SomeIpMessage::from_bytes(&bytes).expect("Failed to parse");
    println!("\nParsed message matches original: {}", parsed == request);

    // Example 5: Working with headers directly
    println!("\n--- Example 5: Header Details ---");
    let header = SomeIpHeader::new(ServiceId(0xFFFF), MethodId(0x8001));
    println!("Service ID: {}", header.service_id);
    println!("Method ID: {} (is_event: {})", header.method_id, header.method_id.is_event());
    println!("Message ID: 0x{:08X}", header.message_id());
    println!("Request ID: 0x{:08X}", header.request_id());

    // Example 6: Different message types
    println!("\n--- Example 6: Message Types ---");

    let notification = SomeIpMessage::notification(ServiceId(0x1234), MethodId::event(0x0001))
        .payload(b"Event data".as_slice())
        .build();
    println!("Notification: type={:?}, method_id={} (is_event: {})",
        notification.header.message_type,
        notification.header.method_id,
        notification.header.method_id.is_event()
    );

    let fire_and_forget = SomeIpMessage::request_no_return(ServiceId(0x1234), MethodId(0x0002))
        .payload(b"Fire and forget".as_slice())
        .build();
    println!("Fire-and-forget: type={:?}, expects_response: {}",
        fire_and_forget.header.message_type,
        fire_and_forget.expects_response()
    );

    // Example 7: Return codes
    println!("\n--- Example 7: Return Codes ---");
    for code in [
        ReturnCode::Ok,
        ReturnCode::NotOk,
        ReturnCode::UnknownService,
        ReturnCode::UnknownMethod,
        ReturnCode::Timeout,
    ] {
        println!("  {:?}: is_ok={}, value=0x{:02X}", code, code.is_ok(), code as u8);
    }

    println!("\n=== Done! ===");
}

fn print_message(msg: &SomeIpMessage) {
    println!("  Service ID:        {}", msg.header.service_id);
    println!("  Method ID:         {}", msg.header.method_id);
    println!("  Client ID:         {}", msg.header.client_id);
    println!("  Session ID:        {}", msg.header.session_id);
    println!("  Protocol Version:  0x{:02X}", msg.header.protocol_version);
    println!("  Interface Version: {}", msg.header.interface_version);
    println!("  Message Type:      {:?}", msg.header.message_type);
    println!("  Return Code:       {:?}", msg.header.return_code);
    println!("  Length:            {} (payload: {} bytes)",
        msg.header.length,
        msg.payload.len()
    );
    if !msg.payload.is_empty() {
        println!("  Payload:           {:?}", String::from_utf8_lossy(&msg.payload));
    }
}
