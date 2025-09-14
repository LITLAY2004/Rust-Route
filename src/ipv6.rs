use std::net::{Ipv6Addr, SocketAddrV6};
use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use tokio::net::UdpSocket;
use ipnet::Ipv6Net;

use crate::protocol::RipMessage;

/// IPv6 RIP configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RipV6Config {
    pub enabled: bool,
    pub port: u16,
    pub multicast_address: Ipv6Addr,
    pub update_interval: u64,
    pub garbage_collection_timeout: u64,
    pub infinity_metric: u32,
}

impl Default for RipV6Config {
    fn default() -> Self {
        Self {
            enabled: false,
            port: 521, // RIPng port
            multicast_address: "ff02::9".parse().unwrap(), // RIPng multicast
            update_interval: 30,
            garbage_collection_timeout: 120,
            infinity_metric: 16,
        }
    }
}

/// IPv6 Route entry
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RipV6Route {
    pub prefix: Ipv6Net,
    pub next_hop: Ipv6Addr,
    pub metric: u32,
    pub route_tag: u16,
    pub interface: String,
    pub learned_from: Ipv6Addr,
    pub last_updated: std::time::SystemTime,
    pub garbage_collection_timer: Option<std::time::SystemTime>,
}

impl RipV6Route {
    pub fn new(
        prefix: Ipv6Net,
        next_hop: Ipv6Addr,
        metric: u32,
        interface: String,
        learned_from: Ipv6Addr,
    ) -> Self {
        Self {
            prefix,
            next_hop,
            metric,
            route_tag: 0,
            interface,
            learned_from,
            last_updated: std::time::SystemTime::now(),
            garbage_collection_timer: None,
        }
    }

    pub fn age_seconds(&self) -> u64 {
        self.last_updated
            .elapsed()
            .unwrap_or_default()
            .as_secs()
    }

    pub fn is_expired(&self, timeout: u64) -> bool {
        self.age_seconds() > timeout
    }

    pub fn is_garbage(&self) -> bool {
        self.garbage_collection_timer.is_some()
    }

    pub fn mark_for_garbage_collection(&mut self) {
        self.garbage_collection_timer = Some(std::time::SystemTime::now());
        self.metric = 16; // Mark as unreachable
    }

    pub fn should_be_deleted(&self, gc_timeout: u64) -> bool {
        if let Some(gc_time) = self.garbage_collection_timer {
            gc_time.elapsed().unwrap_or_default().as_secs() > gc_timeout
        } else {
            false
        }
    }
}

/// IPv6 RIP routing table
#[derive(Debug, Clone)]
pub struct RipV6RoutingTable {
    routes: HashMap<Ipv6Net, RipV6Route>,
    config: RipV6Config,
}

impl RipV6RoutingTable {
    pub fn new(config: RipV6Config) -> Self {
        Self {
            routes: HashMap::new(),
            config,
        }
    }

    pub fn add_route(&mut self, route: RipV6Route) -> bool {
        let prefix = route.prefix;
        
        // Check if we should update the existing route
        if let Some(existing) = self.routes.get(&prefix) {
            // Update if better metric or same source with newer timestamp
            if route.metric < existing.metric || 
               (route.learned_from == existing.learned_from && route.metric <= existing.metric) {
                self.routes.insert(prefix, route);
                return true;
            }
            return false;
        }

        // Add new route
        self.routes.insert(prefix, route);
        true
    }

    pub fn remove_route(&mut self, prefix: &Ipv6Net) -> Option<RipV6Route> {
        self.routes.remove(prefix)
    }

    pub fn get_route(&self, prefix: &Ipv6Net) -> Option<&RipV6Route> {
        self.routes.get(prefix)
    }

    pub fn get_all_routes(&self) -> Vec<&RipV6Route> {
        self.routes.values().collect()
    }

    pub fn find_best_route(&self, destination: &Ipv6Addr) -> Option<&RipV6Route> {
        self.routes
            .values()
            .filter(|route| route.prefix.contains(destination) && route.metric < self.config.infinity_metric)
            .min_by_key(|route| route.metric)
    }

    pub fn update_timers(&mut self) {
        let now = std::time::SystemTime::now();
        let mut to_remove = Vec::new();
        let mut to_garbage_collect = Vec::new();

        for (prefix, route) in &mut self.routes {
            // Check for garbage collection timeout
            if route.should_be_deleted(self.config.garbage_collection_timeout) {
                to_remove.push(*prefix);
                continue;
            }

            // Check for route expiration
            if route.is_expired(self.config.update_interval * 6) && !route.is_garbage() {
                to_garbage_collect.push(*prefix);
            }
        }

        // Remove expired routes
        for prefix in to_remove {
            self.routes.remove(&prefix);
        }

        // Mark routes for garbage collection
        for prefix in to_garbage_collect {
            if let Some(route) = self.routes.get_mut(&prefix) {
                route.mark_for_garbage_collection();
            }
        }
    }

    pub fn route_count(&self) -> usize {
        self.routes.len()
    }

    pub fn active_route_count(&self) -> usize {
        self.routes
            .values()
            .filter(|route| route.metric < self.config.infinity_metric)
            .count()
    }
}

/// IPv6 RIP packet structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RipV6Packet {
    pub command: u8,
    pub version: u8,
    pub reserved: u16,
    pub entries: Vec<RipV6Entry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RipV6Entry {
    pub prefix: Ipv6Net,
    pub route_tag: u16,
    pub prefix_length: u8,
    pub metric: u8,
}

impl RipV6Packet {
    pub fn new_request() -> Self {
        Self {
            command: 1, // Request
            version: 1, // RIPng version
            reserved: 0,
            entries: vec![],
        }
    }

    pub fn new_response(routes: Vec<&RipV6Route>) -> Self {
        let entries = routes
            .into_iter()
            .map(|route| RipV6Entry {
                prefix: route.prefix,
                route_tag: route.route_tag,
                prefix_length: route.prefix.prefix_len(),
                metric: route.metric.min(16) as u8,
            })
            .collect();

        Self {
            command: 2, // Response
            version: 1,
            reserved: 0,
            entries,
        }
    }

    pub fn to_bytes(&self) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        // Simplified serialization - in practice, would need proper RIPng format
        serde_json::to_vec(self).map_err(Into::into)
    }

    pub fn from_bytes(data: &[u8]) -> Result<Self, Box<dyn std::error::Error>> {
        // Simplified deserialization - in practice, would need proper RIPng parsing
        serde_json::from_slice(data).map_err(Into::into)
    }
}

/// IPv6 RIP router implementation
pub struct RipV6Router {
    config: RipV6Config,
    routing_table: RipV6RoutingTable,
    socket: Option<UdpSocket>,
    interfaces: HashMap<String, Ipv6Addr>,
}

impl RipV6Router {
    pub fn new(config: RipV6Config) -> Self {
        let routing_table = RipV6RoutingTable::new(config.clone());
        
        Self {
            config,
            routing_table,
            socket: None,
            interfaces: HashMap::new(),
        }
    }

    pub async fn start(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if !self.config.enabled {
            log::info!("IPv6 RIP is disabled");
            return Ok(());
        }

        log::info!("🚀 Starting IPv6 RIP router on port {}", self.config.port);

        // Bind to IPv6 multicast address
        let addr = SocketAddrV6::new(
            Ipv6Addr::UNSPECIFIED,
            self.config.port,
            0,
            0,
        );

        let socket = UdpSocket::bind(addr).await?;
        
        // Join multicast group
        // socket.join_multicast_v6(&self.config.multicast_address, 0)?; // Interface index 0 for all interfaces
        
        self.socket = Some(socket);
        
        // Start periodic tasks
        self.start_periodic_tasks().await;

        Ok(())
    }

    async fn start_periodic_tasks(&self) {
        // Update timer task
        let config = self.config.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(
                std::time::Duration::from_secs(config.update_interval)
            );

            loop {
                interval.tick().await;
                // Update timers logic would go here
                log::debug!("IPv6 RIP: Timer update");
            }
        });

        // Periodic route advertisement
        let config = self.config.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(
                std::time::Duration::from_secs(config.update_interval)
            );

            loop {
                interval.tick().await;
                // Send route updates
                log::debug!("IPv6 RIP: Sending periodic updates");
            }
        });
    }

    pub async fn send_routes(&self, destination: SocketAddrV6) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if let Some(socket) = &self.socket {
            let routes = self.routing_table.get_all_routes();
            let packet = RipV6Packet::new_response(routes);
            let data = packet.to_bytes()?;
            
            socket.send_to(&data, destination).await?;
            log::debug!("Sent {} IPv6 routes to {}", packet.entries.len(), destination);
        }
        
        Ok(())
    }

    pub async fn process_received_packet(&mut self, data: &[u8], source: SocketAddrV6) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let packet = RipV6Packet::from_bytes(data)?;
        
        match packet.command {
            1 => self.handle_request(source).await?,
            2 => self.handle_response(packet, source).await?,
            _ => log::warn!("Unknown IPv6 RIP command: {}", packet.command),
        }
        
        Ok(())
    }

    async fn handle_request(&self, source: SocketAddrV6) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        log::debug!("Received IPv6 RIP request from {}", source);
        self.send_routes(source).await
    }

    async fn handle_response(&mut self, packet: RipV6Packet, source: SocketAddrV6) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        log::debug!("Received IPv6 RIP response with {} entries from {}", packet.entries.len(), source);
        
        for entry in packet.entries {
            let route = RipV6Route::new(
                entry.prefix,
                source.ip().clone(),
                entry.metric as u32,
                "unknown".to_string(), // Would need to determine actual interface
                source.ip().clone(),
            );
            
            if self.routing_table.add_route(route) {
                log::info!("Added/updated IPv6 route: {} via {}", entry.prefix, source.ip());
            }
        }
        
        Ok(())
    }

    pub fn add_interface(&mut self, name: String, address: Ipv6Addr) {
        self.interfaces.insert(name, address);
        log::info!("Added IPv6 interface: {} with address {}", name, address);
    }

    pub fn routing_table(&self) -> &RipV6RoutingTable {
        &self.routing_table
    }

    pub fn routing_table_mut(&mut self) -> &mut RipV6RoutingTable {
        &mut self.routing_table
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ipv6_route_creation() {
        let prefix: Ipv6Net = "2001:db8::/32".parse().unwrap();
        let next_hop: Ipv6Addr = "fe80::1".parse().unwrap();
        let learned_from: Ipv6Addr = "fe80::2".parse().unwrap();
        
        let route = RipV6Route::new(
            prefix,
            next_hop,
            5,
            "eth0".to_string(),
            learned_from,
        );
        
        assert_eq!(route.prefix, prefix);
        assert_eq!(route.next_hop, next_hop);
        assert_eq!(route.metric, 5);
        assert_eq!(route.interface, "eth0");
        assert!(!route.is_garbage());
    }

    #[test]
    fn test_ipv6_routing_table() {
        let config = RipV6Config::default();
        let mut table = RipV6RoutingTable::new(config);
        
        let prefix: Ipv6Net = "2001:db8::/32".parse().unwrap();
        let route = RipV6Route::new(
            prefix,
            "fe80::1".parse().unwrap(),
            5,
            "eth0".to_string(),
            "fe80::2".parse().unwrap(),
        );
        
        assert!(table.add_route(route));
        assert_eq!(table.route_count(), 1);
        
        let retrieved = table.get_route(&prefix).unwrap();
        assert_eq!(retrieved.metric, 5);
    }

    #[test]
    fn test_best_route_selection() {
        let config = RipV6Config::default();
        let mut table = RipV6RoutingTable::new(config);
        
        // Add routes with different metrics
        let prefix1: Ipv6Net = "2001:db8::/48".parse().unwrap();
        let prefix2: Ipv6Net = "2001:db8::/32".parse().unwrap();
        
        let route1 = RipV6Route::new(
            prefix1,
            "fe80::1".parse().unwrap(),
            10,
            "eth0".to_string(),
            "fe80::2".parse().unwrap(),
        );
        
        let route2 = RipV6Route::new(
            prefix2,
            "fe80::3".parse().unwrap(),
            5,
            "eth1".to_string(),
            "fe80::4".parse().unwrap(),
        );
        
        table.add_route(route1);
        table.add_route(route2);
        
        let destination: Ipv6Addr = "2001:db8::1".parse().unwrap();
        let best_route = table.find_best_route(&destination).unwrap();
        
        // Should prefer the more specific route (longer prefix)
        assert_eq!(best_route.prefix, prefix1);
    }

    #[tokio::test]
    async fn test_ipv6_router_creation() {
        let config = RipV6Config::default();
        let router = RipV6Router::new(config);
        
        assert_eq!(router.routing_table().route_count(), 0);
    }
}
