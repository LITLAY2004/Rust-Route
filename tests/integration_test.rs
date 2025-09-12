use std::net::IpAddr;
use std::time::Duration;
use tokio::time::sleep;
use rust_route::{Router, RouterConfig, InterfaceConfig};

#[tokio::test]
async fn test_basic_routing() {
    // Create router configuration
    let config = RouterConfig {
        router_id: "test-router-1".to_string(),
        update_interval: 5,
        holddown_timer: 30,
        garbage_collection_timer: 60,
        max_hop_count: 15,
        split_horizon: true,
        poison_reverse: false,
    };

    let interface = InterfaceConfig {
        name: "test0".to_string(),
        ip_address: "192.168.1.1".parse().unwrap(),
        subnet_mask: "255.255.255.0".parse().unwrap(),
        multicast_address: "224.0.0.9".parse().unwrap(),
        port: 5200,
        mtu: 1500,
    };

    // Create router instance
    let router = Router::new(config, vec![interface]).await.unwrap();
    
    // Test basic functionality
    assert!(router.is_running().await);
    
    // Add a test route
    router.add_static_route(
        "10.0.0.0".parse().unwrap(),
        "255.255.255.0".parse().unwrap(),
        Some("192.168.1.2".parse().unwrap()),
        1
    ).await.unwrap();
    
    // Verify route was added
    let routes = router.get_routing_table().await;
    assert!(routes.len() > 0);
    
    println!("✅ Basic routing test passed");
}

#[tokio::test]
async fn test_multi_router_convergence() {
    // This test would simulate multiple routers and test convergence
    // For now, we'll create a simple simulation
    
    println!("🔄 Testing multi-router convergence...");
    
    // Router 1: 192.168.1.1/24
    let config1 = RouterConfig {
        router_id: "router-1".to_string(),
        update_interval: 2,
        holddown_timer: 10,
        garbage_collection_timer: 20,
        max_hop_count: 15,
        split_horizon: true,
        poison_reverse: false,
    };
    
    let interface1 = InterfaceConfig {
        name: "eth0".to_string(),
        ip_address: "192.168.1.1".parse().unwrap(),
        subnet_mask: "255.255.255.0".parse().unwrap(),
        multicast_address: "224.0.0.9".parse().unwrap(),
        port: 5201,
        mtu: 1500,
    };
    
    // Router 2: 192.168.2.1/24
    let config2 = RouterConfig {
        router_id: "router-2".to_string(),
        update_interval: 2,
        holddown_timer: 10,
        garbage_collection_timer: 20,
        max_hop_count: 15,
        split_horizon: true,
        poison_reverse: false,
    };
    
    let interface2 = InterfaceConfig {
        name: "eth1".to_string(),
        ip_address: "192.168.2.1".parse().unwrap(),
        subnet_mask: "255.255.255.0".parse().unwrap(),
        multicast_address: "224.0.0.9".parse().unwrap(),
        port: 5202,
        mtu: 1500,
    };
    
    // Create routers (in real test environment, these would be separate processes)
    let router1 = Router::new(config1, vec![interface1]).await.unwrap();
    let router2 = Router::new(config2, vec![interface2]).await.unwrap();
    
    // Wait for initial setup
    sleep(Duration::from_secs(1)).await;
    
    // Add some static routes to simulate network topology
    router1.add_static_route(
        "10.1.0.0".parse().unwrap(),
        "255.255.0.0".parse().unwrap(),
        None,
        1
    ).await.unwrap();
    
    router2.add_static_route(
        "10.2.0.0".parse().unwrap(),
        "255.255.0.0".parse().unwrap(),
        None,
        1
    ).await.unwrap();
    
    // Wait for route propagation
    sleep(Duration::from_secs(5)).await;
    
    // Check if routes have been exchanged
    let routes1 = router1.get_routing_table().await;
    let routes2 = router2.get_routing_table().await;
    
    println!("Router 1 has {} routes", routes1.len());
    println!("Router 2 has {} routes", routes2.len());
    
    println!("✅ Multi-router convergence test completed");
}
