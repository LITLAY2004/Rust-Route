use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, SocketAddrV4};
use std::sync::Arc;

use rust_route::config_manager::RouterConfig;
use rust_route::metrics::Metrics;
use rust_route::protocol::{RipCommand, RipEntry, RipPacket};
use rust_route::router::{handle_rip_response, NeighborInfo, Router};
use rust_route::routing_table::{RouteSource, RoutingTable};
use tokio::sync::RwLock;

#[tokio::test]
async fn handle_rip_response_adds_dynamic_route() {
    let routing_table = Arc::new(RwLock::new(RoutingTable::new()));
    let neighbors: Arc<RwLock<HashMap<IpAddr, NeighborInfo>>> =
        Arc::new(RwLock::new(HashMap::new()));
    let metrics = Metrics::new();
    let base_config = RouterConfig::default();
    let rip_config = Arc::new(base_config.rip.clone());

    let packet = RipPacket {
        command: RipCommand::Response,
        version: 2,
        reserved: 0,
        entries: vec![RipEntry {
            address_family: 2,
            route_tag: 0,
            ip_address: Ipv4Addr::new(10, 1, 0, 0),
            subnet_mask: Ipv4Addr::new(255, 255, 255, 0),
            next_hop: Ipv4Addr::UNSPECIFIED,
            metric: 1,
        }],
    };

    let sender = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(192, 168, 10, 1), 520));

    let routes = handle_rip_response(
        Arc::clone(&routing_table),
        Arc::clone(&neighbors),
        metrics.clone(),
        rip_config,
        "eth0".to_string(),
        packet,
        sender,
    )
    .await
    .expect("processing response succeeds");

    assert_eq!(routes.len(), 1);

    let table = routing_table.read().await;
    let route = table
        .find_best_route(&Ipv4Addr::new(10, 1, 0, 42))
        .expect("route installed");
    assert_eq!(route.metric, 2); // incoming metric + 1
    assert_eq!(route.source, RouteSource::Dynamic);
    assert_eq!(route.learned_from, Some(Ipv4Addr::new(192, 168, 10, 1)));

    let metrics_snapshot = metrics.snapshot(1, table.route_count());
    assert_eq!(metrics_snapshot.routing_updates_received, 1);
    drop(table);

    let neighbor_map = neighbors.read().await;
    let neighbor = neighbor_map
        .get(&IpAddr::V4(Ipv4Addr::new(192, 168, 10, 1)))
        .expect("neighbor recorded");
    assert_eq!(neighbor.learned_routes, 1);
    assert_eq!(neighbor.interface.as_deref(), Some("eth0"));
}

#[tokio::test]
async fn router_initializes_without_rip() {
    let mut config = RouterConfig::default();
    config.rip.enabled = false;

    let routing_table = Arc::new(RwLock::new(RoutingTable::new()));
    let metrics = Metrics::new();

    let router = Router::new(config, Arc::clone(&routing_table), metrics)
        .await
        .expect("router constructed");

    assert!(!router.rip_enabled());
    assert!(router.network_interfaces().is_empty());
}
