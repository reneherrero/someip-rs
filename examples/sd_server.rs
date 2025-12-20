//! SOME/IP-SD server example.
//!
//! This example demonstrates how to offer a service using SOME/IP-SD.
//! Run this first, then run the sd_client example.

use std::time::Duration;

use someip_rs::sd::{Endpoint, OfferedService, SdRequest, SdServer, InstanceId};
use someip_rs::ServiceId;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("SOME/IP-SD Server Example");
    println!("=========================\n");

    // Create SD server
    let mut server = SdServer::new()?;
    println!("SD server started on {:?}", server.local_addr()?);

    // Define the service we want to offer
    let service = OfferedService {
        service_id: ServiceId(0x1234),
        instance_id: InstanceId(0x0001),
        major_version: 1,
        minor_version: 0,
        endpoint: Endpoint::tcp("127.0.0.1:30500".parse()?),
        ttl: 10, // 10 seconds TTL
    };

    // Start offering the service
    server.offer_service(service)?;
    println!("Offering service 0x1234 instance 0x0001 on tcp://127.0.0.1:30500\n");

    println!("Waiting for requests... (Press Ctrl+C to stop)\n");

    loop {
        // Check if we should send periodic offers
        if server.should_send_offers() {
            server.send_offers()?;
            println!("Sent periodic offer announcement");
        }

        // Poll for incoming requests
        match server.poll()? {
            Some(SdRequest::FindService {
                service_id,
                instance_id,
                from,
                ..
            }) => {
                println!(
                    "Received FindService for {:?} instance {:?} from {}",
                    service_id, instance_id, from
                );
                // Response is sent automatically if we offer the service
            }
            Some(SdRequest::Subscribe {
                service_id,
                instance_id,
                eventgroup_id,
                endpoint,
                counter,
                ttl,
                from,
                ..
            }) => {
                println!(
                    "Received Subscribe for {:?} instance {:?} eventgroup {:?} from {}",
                    service_id, instance_id, eventgroup_id, from
                );
                println!("  Endpoint: {}", endpoint);
                println!("  TTL: {} seconds", ttl);

                // Accept the subscription
                server.accept_subscription(
                    service_id,
                    instance_id,
                    eventgroup_id,
                    counter,
                    from,
                    endpoint,
                    ttl,
                    None, // No multicast endpoint
                )?;
                println!("  -> Subscription accepted\n");
            }
            Some(SdRequest::Unsubscribe {
                service_id,
                instance_id,
                eventgroup_id,
                from,
            }) => {
                println!(
                    "Received Unsubscribe for {:?} instance {:?} eventgroup {:?} from {}",
                    service_id, instance_id, eventgroup_id, from
                );
            }
            None => {}
        }

        // Clean up expired subscriptions periodically
        let expired = server.cleanup_expired();
        for (service_id, instance_id, eventgroup_id, addr) in expired {
            println!(
                "Subscription expired: {:?} {:?} {:?} from {}",
                service_id, instance_id, eventgroup_id, addr
            );
        }

        std::thread::sleep(Duration::from_millis(100));
    }
}
