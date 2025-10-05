//! Router implementation for RustRoute

use crate::config_manager::{InterfaceConfig, RipConfig, RouterConfig};
use crate::metrics::Metrics;
use crate::network::{InterfaceConfig as NetInterfaceConfig, NetworkInterface};
use crate::protocol::RipPacket;
use crate::routing_table::{Route, RouteSource, RoutingTable, RoutingTableStatistics};
use crate::{RustRouteError, RustRouteResult};
use ipnet::{IpNet, Ipv4Net};
use log::{debug, warn};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct NeighborInfo {
    pub address: IpAddr,
    pub interface: Option<String>,
    pub last_seen: Instant,
    pub learned_routes: usize,
}

/// Router runtime responsible for managing configuration, routing table and metrics
#[derive(Debug)]
pub struct Router {
    config: RouterConfig,
    routing_table: Arc<RwLock<RoutingTable>>,
    metrics: Metrics,
    neighbors: Arc<RwLock<HashMap<IpAddr, NeighborInfo>>>,
    start_time: Instant,
    router_uuid: Uuid,
    interfaces: HashMap<String, Arc<NetworkInterface>>,
}

impl Router {
    pub async fn new(
        config: RouterConfig,
        routing_table: Arc<RwLock<RoutingTable>>,
        metrics: Metrics,
    ) -> RustRouteResult<Self> {
        let router_uuid = Self::derive_router_uuid(&config.router_id);

        let interfaces = if config.rip.enabled {
            Self::initialize_network_interfaces(&config).await?
        } else {
            HashMap::new()
        };

        let mut router = Self {
            config,
            routing_table,
            metrics,
            neighbors: Arc::new(RwLock::new(HashMap::new())),
            start_time: Instant::now(),
            router_uuid,
            interfaces,
        };

        router.rebuild_routing_table().await?;
        Ok(router)
    }

    pub fn router_id(&self) -> &str {
        &self.config.router_id
    }

    pub fn router_uuid(&self) -> Uuid {
        self.router_uuid
    }

    pub fn metrics(&self) -> Metrics {
        self.metrics.clone()
    }

    pub fn routing_table(&self) -> Arc<RwLock<RoutingTable>> {
        Arc::clone(&self.routing_table)
    }

    pub fn config(&self) -> &RouterConfig {
        &self.config
    }

    pub fn config_snapshot(&self) -> RouterConfig {
        self.config.clone()
    }

    pub fn neighbors(&self) -> Arc<RwLock<HashMap<IpAddr, NeighborInfo>>> {
        Arc::clone(&self.neighbors)
    }

    pub fn network_interfaces(&self) -> Vec<Arc<NetworkInterface>> {
        self.interfaces.values().cloned().collect()
    }

    pub fn rip_config(&self) -> &RipConfig {
        &self.config.rip
    }

    pub fn rip_enabled(&self) -> bool {
        self.config.rip.enabled && !self.interfaces.is_empty()
    }

    pub fn uptime(&self) -> Duration {
        self.start_time.elapsed()
    }

    pub async fn apply_config(&mut self, config: RouterConfig) -> RustRouteResult<()> {
        self.config = config;
        self.router_uuid = Self::derive_router_uuid(&self.config.router_id);

        if self.config.rip.enabled {
            warn!(
                "Runtime interface reconfiguration is only partially supported; please restart after interface changes."
            );
        }
        self.rebuild_routing_table().await
    }

    pub async fn restart(&mut self) -> RustRouteResult<()> {
        self.metrics.reset();
        self.start_time = Instant::now();
        self.rebuild_routing_table().await
    }

    pub async fn statistics(&self) -> RouterStatistics {
        let routing_table = self.routing_table.read().await;
        let table_stats = routing_table.get_stats();
        let neighbor_count = self.neighbors.read().await.len();
        let metrics_snapshot = self
            .metrics
            .snapshot(neighbor_count, table_stats.total_routes);

        RouterStatistics {
            uptime: format_duration(metrics_snapshot.uptime_seconds),
            packets_sent: metrics_snapshot.packets_sent,
            packets_received: metrics_snapshot.packets_received,
            route_count: table_stats.total_routes,
            neighbor_count,
            memory_usage: current_process_memory_bytes(),
            table_breakdown: table_stats,
        }
    }

    async fn rebuild_routing_table(&mut self) -> RustRouteResult<()> {
        let mut table = self.routing_table.write().await;

        // Remove previously derived direct routes before re-applying
        table.clear_source(RouteSource::Direct);

        for iface in &self.config.interfaces {
            if !iface.enabled {
                continue;
            }

            if let Some(net) = parse_ipv4_net(iface)? {
                table.install_direct_route(net.network(), net.netmask(), iface.name.clone());
            }
        }

        self.metrics.update_route_count(table.route_count());

        Ok(())
    }

    pub async fn learn_neighbor(&self, address: IpAddr, interface: Option<String>, routes: usize) {
        let mut neighbors = self.neighbors.write().await;
        neighbors.insert(
            address,
            NeighborInfo {
                address,
                interface,
                last_seen: Instant::now(),
                learned_routes: routes,
            },
        );
    }

    pub async fn cleanup_neighbors(&self, max_age: Duration) {
        let mut neighbors = self.neighbors.write().await;
        neighbors.retain(|_, info| info.last_seen.elapsed() <= max_age);
    }

    fn derive_router_uuid(router_id: &str) -> Uuid {
        if let Ok(uuid) = Uuid::parse_str(router_id) {
            uuid
        } else {
            Uuid::new_v4()
        }
    }

    async fn initialize_network_interfaces(
        config: &RouterConfig,
    ) -> RustRouteResult<HashMap<String, Arc<NetworkInterface>>> {
        let mut map = HashMap::new();

        for iface in &config.interfaces {
            if !iface.enabled {
                continue;
            }

            let Some(net) = parse_ipv4_net(iface)? else {
                continue;
            };

            let host_ip = net.addr();
            let subnet_mask = net.netmask();

            let mut interface = NetworkInterface::new(NetInterfaceConfig {
                name: iface.name.clone(),
                ip_address: host_ip,
                subnet_mask,
                multicast_address: Ipv4Addr::new(224, 0, 0, 9),
                port: config.rip.port,
                mtu: 1500,
                enabled: true,
            });

            match interface.initialize().await {
                Ok(_) => {
                    map.insert(iface.name.clone(), Arc::new(interface));
                }
                Err(err) => {
                    warn!(
                        "Skipping interface {} ({}): {}",
                        iface.name, iface.address, err
                    );
                }
            }
        }

        Ok(map)
    }
}

fn parse_ipv4_net(interface: &InterfaceConfig) -> RustRouteResult<Option<Ipv4Net>> {
    let cidr = interface.address.trim();
    if cidr.is_empty() {
        return Err(RustRouteError::InvalidInput(format!(
            "Interface {} is missing an address",
            interface.name
        )));
    }

    match cidr.parse::<IpNet>() {
        Ok(IpNet::V4(v4)) => Ok(Some(v4)),
        Ok(IpNet::V6(_)) => Ok(None), // IPv6 handled separately
        Err(err) => Err(RustRouteError::InvalidInput(format!(
            "Invalid interface address {}: {}",
            interface.address, err
        ))),
    }
}

fn format_duration(seconds: u64) -> String {
    let hours = seconds / 3600;
    let minutes = (seconds % 3600) / 60;
    let secs = seconds % 60;
    format!("{}时{}分{}秒", hours, minutes, secs)
}

fn current_process_memory_bytes() -> u64 {
    if let Ok(status) = std::fs::read_to_string("/proc/self/status") {
        for line in status.lines() {
            if let Some(rest) = line.strip_prefix("VmRSS:") {
                if let Some(value) = rest.split_whitespace().next() {
                    if let Ok(kb) = value.parse::<u64>() {
                        return kb * 1024;
                    }
                }
            }
        }
    }
    0
}

pub async fn handle_rip_response(
    routing_table: Arc<RwLock<RoutingTable>>,
    neighbors: Arc<RwLock<HashMap<IpAddr, NeighborInfo>>>,
    metrics: Metrics,
    rip_config: Arc<RipConfig>,
    interface_name: String,
    packet: RipPacket,
    sender: SocketAddr,
) -> RustRouteResult<Vec<Route>> {
    let sender_ip = match sender.ip() {
        IpAddr::V4(ip) => ip,
        _ => {
            warn!("Ignoring non-IPv4 RIP response from {}", sender);
            return Ok(Vec::new());
        }
    };

    let entries = packet.entries;
    let learned_count = entries.len();
    let mut updated = false;
    let mut updated_routes = Vec::new();

    {
        let mut table = routing_table.write().await;

        for entry in entries {
            let mut metric = entry.metric.saturating_add(1);
            if metric > rip_config.infinity_metric {
                metric = rip_config.infinity_metric;
            }

            if metric >= rip_config.infinity_metric {
                continue;
            }

            let next_hop = if entry.next_hop.is_unspecified() {
                sender_ip
            } else {
                entry.next_hop
            };

            let route = Route::new(
                entry.ip_address,
                entry.subnet_mask,
                next_hop,
                metric,
                interface_name.clone(),
                RouteSource::Dynamic,
                Some(sender_ip),
            );

            if table.add_or_replace(route.clone()) {
                updated = true;
                updated_routes.push(route);
            }
        }

        if updated {
            metrics.increment_routing_updates_received();
        }

        metrics.update_route_count(table.route_count());
    }

    {
        let mut neighbor_map = neighbors.write().await;
        neighbor_map.insert(
            IpAddr::V4(sender_ip),
            NeighborInfo {
                address: IpAddr::V4(sender_ip),
                interface: Some(interface_name.clone()),
                last_seen: Instant::now(),
                learned_routes: learned_count,
            },
        );
    }

    if updated {
        debug!(
            "Updated routes from neighbor {} via {}",
            sender_ip, interface_name
        );
    }

    Ok(updated_routes)
}

/// Router statistics for CLI display
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouterStatistics {
    pub uptime: String,
    pub packets_sent: u64,
    pub packets_received: u64,
    pub route_count: usize,
    pub neighbor_count: usize,
    pub memory_usage: u64,
    pub table_breakdown: RoutingTableStatistics,
}
