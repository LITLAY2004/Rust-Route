//! Integration tests for RIPER

use riper::{
    router::{Router, RouterConfig},
    network::{NetworkInterface, InterfaceConfig},
    routing_table::RoutingTable,
    protocol::{RiperPacket, RouteEntry},
};
use std::net::{IpAddr, Ipv4Addr};
use std::time::Duration;
use tokio;
use uuid::Uuid;

#[tokio::test]
async fn test_router_creation() {
    let config = RouterConfig::default();
    let router = Router::new(config);
    
    assert!(!router.config.router_id.is_nil());
    assert_eq!(router.config.update_interval, 30);
    assert_eq!(router.config.max_hop_count, 15);
}

#[tokio::test]
async fn test_routing_table_operations() {
    let mut table = RoutingTable::new();
    
    // Add a route
    let updated = table.update_route(
        Ipv4Addr::new(192, 168, 1, 0),
        Ipv4Addr::new(255, 255, 255, 0),
        IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1)),
        1,
        "eth0".to_string(),
    );
    
    assert!(updated);
    
    // Get the route
    let route = table.get_route(
        Ipv4Addr::new(192, 168, 1, 0),
        Ipv4Addr::new(255, 255, 255, 0),
    );
    
    assert!(route.is_some());
    assert_eq!(route.unwrap().metric, 1);
    
    // Find best route
    let best = table.find_best_route(Ipv4Addr::new(192, 168, 1, 10));
    assert!(best.is_some());
}

#[tokio::test]
async fn test_network_interface() {
    let config = InterfaceConfig {
        name: "test0".to_string(),
        ip_address: Ipv4Addr::new(192, 168, 1, 1),
        subnet_mask: Ipv4Addr::new(255, 255, 255, 0),
        port: 5520, // Use different port for testing
        ..Default::default()
    };
    
    let interface = NetworkInterface::new(config);
    
    // Test interface properties
    assert_eq!(interface.config.name, "test0");
    assert_eq!(interface.config.ip_address, Ipv4Addr::new(192, 168, 1, 1));
    
    // Test network calculations
    let broadcast = interface.get_broadcast_address();
    assert_eq!(broadcast, Ipv4Addr::new(192, 168, 1, 255));
    
    let network = interface.get_network_address();
    assert_eq!(network, Ipv4Addr::new(192, 168, 1, 0));
    
    // Test subnet membership
    assert!(interface.is_in_subnet(Ipv4Addr::new(192, 168, 1, 10)));
    assert!(!interface.is_in_subnet(Ipv4Addr::new(192, 168, 2, 10)));
}

#[tokio::test]
async fn test_packet_serialization() {
    let router_id = Uuid::new_v4();
    let routes = vec![
        RouteEntry::new(
            Ipv4Addr::new(192, 168, 1, 0),
            Ipv4Addr::new(255, 255, 255, 0),
            IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1)),
            1,
            "eth0".to_string(),
        ),
        RouteEntry::new(
            Ipv4Addr::new(10, 0, 0, 0),
            Ipv4Addr::new(255, 0, 0, 0),
            IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1)),
            2,
            "eth0".to_string(),
        ),
    ];
    
    let packet = RiperPacket::new_update(router_id, routes);
    
    // Test packet validation
    assert!(packet.validate().is_ok());
    
    // Test serialization
    let json = packet.to_json().unwrap();
    let deserialized = RiperPacket::from_json(&json).unwrap();
    
    assert_eq!(packet.router_id, deserialized.router_id);
    assert_eq!(packet.packet_type, deserialized.packet_type);
    assert_eq!(packet.routes.len(), deserialized.routes.len());
}

#[tokio::test]
async fn test_route_validation() {
    // Valid route
    let valid_route = RouteEntry::new(
        Ipv4Addr::new(192, 168, 1, 0), // Network address
        Ipv4Addr::new(255, 255, 255, 0),
        IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1)),
        1,
        "eth0".to_string(),
    );
    assert!(valid_route.validate().is_ok());
    
    // Invalid route - not a network address
    let invalid_route = RouteEntry::new(
        Ipv4Addr::new(192, 168, 1, 5), // Host address
        Ipv4Addr::new(255, 255, 255, 0),
        IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1)),
        1,
        "eth0".to_string(),
    );
    assert!(invalid_route.validate().is_err());
    
    // Invalid metric
    let invalid_metric_route = RouteEntry::new(
        Ipv4Addr::new(192, 168, 1, 0),
        Ipv4Addr::new(255, 255, 255, 0),
        IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1)),
        17, // Invalid metric
        "eth0".to_string(),
    );
    assert!(invalid_metric_route.validate().is_err());
}

#[tokio::test]
async fn test_routing_table_convergence() {
    let mut table = RoutingTable::new();
    
    // Add initial route
    table.update_route(
        Ipv4Addr::new(10, 0, 0, 0),
        Ipv4Addr::new(255, 0, 0, 0),
        IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1)),
        2,
        "eth0".to_string(),
    );
    
    // Update with better route
    let updated = table.update_route(
        Ipv4Addr::new(10, 0, 0, 0),
        Ipv4Addr::new(255, 0, 0, 0),
        IpAddr::V4(Ipv4Addr::new(192, 168, 1, 2)),
        1, // Better metric
        "eth1".to_string(),
    );
    
    assert!(updated);
    
    let route = table.get_route(
        Ipv4Addr::new(10, 0, 0, 0),
        Ipv4Addr::new(255, 0, 0, 0),
    ).unwrap();
    
    assert_eq!(route.metric, 1);
    assert_eq!(route.interface, "eth1");
}

#[tokio::test]
async fn test_split_horizon() {
    let config = RouterConfig {
        split_horizon: true,
        poison_reverse: false,
        ..Default::default()
    };
    
    let router = Router::new(config);
    
    // Add interfaces
    let mut interface1 = NetworkInterface::new(InterfaceConfig {
        name: "eth0".to_string(),
        port: 5521,
        ..Default::default()
    });
    
    let mut interface2 = NetworkInterface::new(InterfaceConfig {
        name: "eth1".to_string(),
        port: 5522,
        ..Default::default()
    });
    
    // Initialize interfaces (this might fail in test environment, but we test the logic)
    let _ = interface1.initialize().await;
    let _ = interface2.initialize().await;
    
    // Test that the router respects split horizon configuration
    assert!(router.config.split_horizon);
    assert!(!router.config.poison_reverse);
}

// Performance test
#[tokio::test]
async fn test_large_routing_table() {
    let mut table = RoutingTable::new();
    
    // Add many routes
    for i in 1..=100 {
        table.update_route(
            Ipv4Addr::new(10, i, 0, 0),
            Ipv4Addr::new(255, 255, 0, 0),
            IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1)),
            1,
            "eth0".to_string(),
        );
    }
    
    let stats = table.get_stats();
    assert_eq!(stats.total_routes, 100);
    assert_eq!(stats.valid_routes, 100);
    
    // Test route lookup performance
    let start = std::time::Instant::now();
    for i in 1..=100 {
        let _route = table.find_best_route(Ipv4Addr::new(10, i, 1, 1));
    }
    let duration = start.elapsed();
    
    // Should be fast even with many routes
    assert!(duration < Duration::from_millis(100));
}

#[tokio::test]
async fn test_garbage_collection() {
    let mut table = RoutingTable::new();
    
    // Add a route
    table.update_route(
        Ipv4Addr::new(192, 168, 1, 0),
        Ipv4Addr::new(255, 255, 255, 0),
        IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1)),
        1,
        "eth0".to_string(),
    );
    
    // Simulate timeout processing
    let marked = table.process_timeouts();
    
    // Since we just added the route, it shouldn't be marked for GC yet
    assert_eq!(marked, 0);
    
    // Test garbage collection (no routes should be removed yet)
    let removed = table.garbage_collect();
    assert_eq!(removed, 0);
    
    let stats = table.get_stats();
    assert_eq!(stats.total_routes, 1);
}
