# someip-rs

A Rust library for the SOME/IP (Scalable service-Oriented MiddlewarE over IP) protocol, built on `std::net`.

[![Crates.io](https://img.shields.io/crates/v/someip-rs.svg)](https://crates.io/crates/someip-rs)
[![Documentation](https://docs.rs/someip-rs/badge.svg)](https://docs.rs/someip-rs)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](LICENSING.md)

## Features

- **Pure Rust** - No C dependencies, built on `std::net`
- **Full SOME/IP support** - Complete 16-byte header with serialization
- **Type-safe IDs** - `ServiceId`, `MethodId`, `ClientId`, `SessionId` newtypes
- **TCP & UDP** - Both transport protocols with client/server support
- **SOME/IP-SD** - Service Discovery for dynamic service registration
- **SOME/IP-TP** - Transport Protocol for large message segmentation
- **Async support** - Optional Tokio integration via `tokio` feature

## Quick Start

```toml
[dependencies]
someip-rs = "0.1"
```

### Send a Request (TCP)

```rust
use someip_rs::{SomeIpMessage, ServiceId, MethodId};
use someip_rs::transport::TcpClient;

let mut client = TcpClient::connect("127.0.0.1:30490")?;

let request = SomeIpMessage::request(ServiceId(0x1234), MethodId(0x0001))
    .payload(b"Hello, SOME/IP!".as_slice())
    .build();

let response = client.call(request)?;
println!("Response: {:?}", response.payload);
```

### Handle Requests (TCP Server)

```rust
use someip_rs::transport::TcpServer;
use someip_rs::MessageType;

let server = TcpServer::bind("127.0.0.1:30490")?;

for connection in server.incoming() {
    let mut conn = connection?;
    let request = conn.read_message()?;

    if request.header.message_type == MessageType::Request {
        let response = request.create_response()
            .payload(b"Hello back!".as_slice())
            .build();
        conn.write_message(&response)?;
    }
}
```

### UDP Client/Server

```rust
use someip_rs::{SomeIpMessage, ServiceId, MethodId};
use someip_rs::transport::{UdpClient, UdpServer};

// Client
let mut client = UdpClient::new()?;
let request = SomeIpMessage::request(ServiceId(0x1234), MethodId(0x0001))
    .payload(b"ping".as_slice())
    .build();
let response = client.call_to("127.0.0.1:30491", request)?;

// Server
let mut server = UdpServer::bind("127.0.0.1:30491")?;
let (request, client_addr) = server.receive()?;
server.respond(&request, b"pong".as_slice(), client_addr)?;
```

## Protocol Overview

SOME/IP messages consist of a 16-byte header followed by an optional payload:

```text
+----------------+----------------+----------------+----------------+
|         Service ID (16)         |         Method ID (16)          |
+----------------+----------------+----------------+----------------+
|                          Length (32)                              |
+----------------+----------------+----------------+----------------+
|         Client ID (16)          |         Session ID (16)         |
+----------------+----------------+----------------+----------------+
| Proto Ver (8)  | Iface Ver (8)  | Msg Type (8)   | Return Code(8) |
+----------------+----------------+----------------+----------------+
|                         Payload (variable)                        |
+----------------+----------------+----------------+----------------+
```

## Feature Flags

| Feature | Description | Default |
|---------|-------------|---------|
| `tokio` | Async transport with Tokio runtime | No |

## Examples

See [`examples/`](./examples/) for complete working examples:

- `message_basics.rs` - Message creation, serialization, and parsing
- `tcp_server.rs` - TCP echo server
- `tcp_client.rs` - TCP client with request/response
- `udp_server.rs` - UDP server with responses
- `udp_client.rs` - UDP client with request/response
- `sd_server.rs` - Service Discovery server (offer services)
- `sd_client.rs` - Service Discovery client (find services)

Run examples:

```bash
# Message basics (standalone)
cargo run --example message_basics

# TCP (run server first, then client)
cargo run --example tcp_server
cargo run --example tcp_client

# UDP (run server first, then client)
cargo run --example udp_server
cargo run --example udp_client

# Service Discovery (run server first, then client)
cargo run --example sd_server
cargo run --example sd_client
```

## Documentation

- [API Reference](https://docs.rs/someip-rs)
- [ARCHITECTURE.md](./ARCHITECTURE.md) - Internal design

## License

MIT OR Apache-2.0. See [LICENSING.md](./LICENSING.md).
