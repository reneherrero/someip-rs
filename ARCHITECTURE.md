# Architecture

This document describes the internal architecture of the `someip-rs` library.

## Design Principles

1. **Pure Rust** - No C dependencies, built entirely on `std::net` and optionally `tokio`

2. **Type Safety** - Newtype wrappers (`ServiceId`, `MethodId`, etc.) prevent ID mixing at compile time

3. **Layered Design** - Clean separation between header/message, codec, and transport layers

4. **Sync-first** - Synchronous API by default, async available via `tokio` feature

5. **Protocol Compliance** - Follows AUTOSAR SOME/IP specification

## Feature Flags

| Feature | Default | Requires | Provides |
|---------|---------|----------|----------|
| `tokio` | No | Tokio runtime | Async transport (`TcpClientAsync`, `UdpClientAsync`) |

**Dependency graph:**
```
default ─────────────────► std::net (sync transport)
                │
                └────────► bytes, thiserror

tokio ───────────────────► Async transport (tokio::net)
```

## Module Structure

```
src/
├── lib.rs              # Public API re-exports
├── error.rs            # Error types (SomeIpError, Result)
├── types.rs            # Core types (MessageType, ReturnCode, PROTOCOL_VERSION)
├── header.rs           # SomeIpHeader, ID newtypes (ServiceId, MethodId, etc.)
├── message.rs          # SomeIpMessage, MessageBuilder
├── codec.rs            # Serialization/deserialization (sync)
├── codec_async.rs      # Async codec [tokio feature]
│
├── transport/          # Synchronous transport layer
│   ├── mod.rs          # Re-exports
│   ├── tcp.rs          # TcpClient, TcpServer, TcpConnection
│   └── udp.rs          # UdpClient, UdpServer
│
├── transport_async/    # Async transport layer [tokio feature]
│   ├── mod.rs          # Re-exports
│   ├── tcp.rs          # TcpClientAsync, TcpServerAsync
│   └── udp.rs          # UdpClientAsync, UdpServerAsync
│
├── connection/         # Connection management
│   ├── mod.rs          # Re-exports
│   ├── config.rs       # ConnectionConfig
│   ├── state.rs        # ConnectionState
│   ├── managed_tcp.rs  # ManagedTcpConnection (reconnection)
│   ├── managed_tcp_async.rs  # Async variant [tokio feature]
│   ├── pool.rs         # ConnectionPool (sync)
│   └── pool_async.rs   # ConnectionPoolAsync [tokio feature]
│
├── sd/                 # SOME/IP Service Discovery
│   ├── mod.rs          # Re-exports
│   ├── types.rs        # SD constants and types (InstanceId, EventgroupId)
│   ├── entry.rs        # SdEntry, ServiceEntry, EventgroupEntry
│   ├── option.rs       # SdOption, IPv4EndpointOption, IPv6EndpointOption
│   ├── message.rs      # SdMessage, SdFlags
│   ├── client.rs       # SdClient (find/subscribe)
│   └── server.rs       # SdServer (offer/publish)
│
└── tp/                 # SOME/IP Transport Protocol (large messages)
    ├── mod.rs          # Re-exports
    ├── header.rs       # TP header (offset, more flag)
    ├── segment.rs      # TpSegment
    ├── reassembly.rs   # TpReassembler
    ├── client.rs       # TpUdpClient
    └── server.rs       # TpUdpServer
```

## SOME/IP Protocol Overview

SOME/IP messages consist of a 16-byte header followed by an optional payload:

```
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

**Key concepts:**
- **Message ID** (Service ID + Method ID): Identifies the service and operation
- **Request ID** (Client ID + Session ID): Correlates requests with responses
- **Length**: Payload length + 8 bytes (covers Request ID through Return Code)

## Message Types

| Type | Value | Description |
|------|-------|-------------|
| Request | 0x00 | Expects response |
| RequestNoReturn | 0x01 | Fire-and-forget |
| Notification | 0x02 | Event/cyclic |
| Response | 0x80 | Response to Request |
| Error | 0x81 | Error response |
| TpRequest | 0x20 | Segmented request |
| TpResponse | 0xA0 | Segmented response |

## Transport Layer

### TCP Transport

TCP provides reliable, connection-oriented message delivery:

```
TcpClient::connect()
    │
    ├──► TcpStream (std::net)
    │
    └──► call(request) ──► write_message() ──► read_message() ──► response
```

Messages are length-prefixed using the header's length field.

### UDP Transport

UDP provides connectionless datagram delivery:

```
UdpClient::new()
    │
    ├──► UdpSocket (std::net)
    │
    └──► call_to(addr, request) ──► send_to() ──► recv_from() ──► response
```

For messages exceeding UDP MTU, use SOME/IP-TP segmentation.

## Service Discovery (SD)

SOME/IP-SD enables dynamic service discovery using:
- **Service ID**: 0xFFFF
- **Method ID**: 0x8100
- **Transport**: UDP multicast (224.224.224.245:30490)

### Entry Types

| Type | Description |
|------|-------------|
| FindService | Client searching for a service |
| OfferService | Server announcing availability |
| StopOfferService | Server going offline |
| SubscribeEventgroup | Client subscribing to events |
| SubscribeEventgroupAck | Server confirming subscription |

### Options

Options carry endpoint information:
- **IPv4/IPv6 Endpoint**: Transport address and port
- **Configuration**: Key-value service configuration

## Transport Protocol (TP)

SOME/IP-TP handles messages larger than a single UDP datagram:

```
Large Message (> 1400 bytes)
    │
    ▼
TpSegment::segment(message, max_size)
    │
    ├──► [Segment 1: offset=0, more=true]
    ├──► [Segment 2: offset=N, more=true]
    └──► [Segment N: offset=M, more=false]
    │
    ▼
TpReassembler::add(segment)
    │
    └──► Complete message when all segments received
```

TP header adds 4 bytes to identify segment position:
- **Offset** (28 bits): Byte offset of this segment
- **More flag** (1 bit): More segments follow

## Error Handling

```rust
pub enum SomeIpError {
    Io(std::io::Error),
    InvalidHeader(String),
    InvalidMessage(String),
    Timeout,
    ConnectionClosed,
    InvalidMessageType(u8),
    InvalidReturnCode(u8),
    SegmentationError(String),
    ServiceDiscoveryError(String),
}
```

All fallible operations return `Result<T, SomeIpError>`.

## Testing Strategy

```
tests/
├── header_tests.rs     # Header serialization/parsing
├── message_tests.rs    # Message building and encoding
├── transport_tests.rs  # TCP/UDP integration tests
├── sd_tests.rs         # Service Discovery tests
└── tp_tests.rs         # Transport Protocol tests
```

Unit tests are co-located with implementation (`#[cfg(test)] mod tests`).

## Performance Considerations

1. **Zero-copy parsing** - Header parsed directly from bytes without allocation
2. **Buffer reuse** - Connection pools reuse buffers across requests
3. **Lazy serialization** - Messages serialized only when sent
4. **Async I/O** - `tokio` feature enables non-blocking multiplexed connections
