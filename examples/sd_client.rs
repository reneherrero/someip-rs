//! SOME/IP-SD client example.
//!
//! This example demonstrates how to find services using SOME/IP-SD.
//! Run the sd_server example first, then run this.

use std::time::Duration;

use someip_rs::sd::{Endpoint, SdClient, SdClientConfig, SdEvent, EventgroupId, InstanceId};
use someip_rs::ServiceId;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("SOME/IP-SD Client Example");
    println!("=========================\n");

    // Create SD client with custom config
    let config = SdClientConfig {
        subscribe_ttl: 30, // 30 seconds subscription TTL
        ..Default::default()
    };
    let mut client = SdClient::with_config(config)?;
    println!("SD client started on {:?}", client.local_addr()?);

    // Set local endpoint for receiving events (required for subscriptions)
    client.set_local_endpoint(Endpoint::udp("127.0.0.1:30501".parse()?));

    // Find a specific service
    let service_id = ServiceId(0x1234);
    let instance_id = InstanceId(0x0001);

    println!("\nSearching for service 0x1234 instance 0x0001...");

    // Wait for the service to become available
    match client.wait_for_service(service_id, instance_id, Duration::from_secs(5))? {
        Some(info) => {
            println!("\nService found!");
            println!("  Service ID: {:?}", info.service_id);
            println!("  Instance ID: {:?}", info.instance_id);
            println!("  Major version: {}", info.major_version);
            println!("  Minor version: {}", info.minor_version);
            println!("  Endpoints:");
            for ep in &info.endpoints {
                println!("    - {}", ep);
            }
            println!("  TTL remaining: {} seconds", info.remaining_ttl());
            println!("  Source: {}", info.source_addr);
        }
        None => {
            println!("Service not found within timeout.");
            println!("Make sure sd_server is running.");
            return Ok(());
        }
    }

    // Subscribe to an eventgroup
    let eventgroup_id = EventgroupId(0x0001);
    println!("\nSubscribing to eventgroup 0x0001...");

    client.subscribe(service_id, instance_id, eventgroup_id, 1)?;
    println!("Subscription request sent.");

    // Wait for subscription acknowledgement
    println!("\nWaiting for subscription response...");
    let deadline = std::time::Instant::now() + Duration::from_secs(5);

    while std::time::Instant::now() < deadline {
        match client.poll()? {
            Some(SdEvent::SubscriptionAck {
                service_id,
                instance_id,
                eventgroup_id,
                multicast_endpoint,
            }) => {
                println!("\nSubscription acknowledged!");
                println!("  Service: {:?}", service_id);
                println!("  Instance: {:?}", instance_id);
                println!("  Eventgroup: {:?}", eventgroup_id);
                if let Some(ep) = multicast_endpoint {
                    println!("  Multicast endpoint: {}", ep);
                }
                break;
            }
            Some(SdEvent::SubscriptionNack {
                service_id,
                instance_id,
                eventgroup_id,
            }) => {
                println!("\nSubscription rejected!");
                println!("  Service: {:?}", service_id);
                println!("  Instance: {:?}", instance_id);
                println!("  Eventgroup: {:?}", eventgroup_id);
                break;
            }
            Some(SdEvent::ServiceAvailable(info)) => {
                println!("Service update: {:?}", info.service_id);
            }
            Some(SdEvent::ServiceUnavailable { service_id, instance_id }) => {
                println!("Service unavailable: {:?} {:?}", service_id, instance_id);
            }
            None => {}
        }
        std::thread::sleep(Duration::from_millis(100));
    }

    // Unsubscribe
    println!("\nUnsubscribing...");
    client.unsubscribe(service_id, instance_id, eventgroup_id, 1)?;
    println!("Unsubscribe request sent.");

    // List all known services
    println!("\nKnown services:");
    for service in client.services() {
        println!(
            "  - {:?} instance {:?} at {:?}",
            service.service_id, service.instance_id, service.endpoints
        );
    }

    println!("\nDone!");
    Ok(())
}
