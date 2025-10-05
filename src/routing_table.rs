//! Routing table implementation for RIP protocol

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::Ipv4Addr;
use std::time::{Duration, Instant};

const DEFAULT_ROUTE_TIMEOUT: Duration = Duration::from_secs(180);
const DEFAULT_GC_TIMEOUT: Duration = Duration::from_secs(240);

/// Indicates where a route originated from
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RouteSource {
    Direct,
    Static,
    Dynamic,
}

impl RouteSource {
    pub fn as_str(&self) -> &'static str {
        match self {
            RouteSource::Direct => "direct",
            RouteSource::Static => "static",
            RouteSource::Dynamic => "dynamic",
        }
    }

    fn priority(&self) -> u8 {
        match self {
            RouteSource::Direct => 3,
            RouteSource::Static => 2,
            RouteSource::Dynamic => 1,
        }
    }
}

/// Snapshot of a route suitable for serialization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteSnapshot {
    pub destination: String,
    pub subnet_mask: String,
    pub next_hop: String,
    pub metric: u32,
    pub interface: String,
    pub learned_from: Option<String>,
    pub age_seconds: u64,
    pub source: RouteSource,
}

/// A single route entry in the routing table
#[derive(Debug, Clone)]
pub struct Route {
    pub destination: Ipv4Addr,
    pub subnet_mask: Ipv4Addr,
    pub next_hop: Ipv4Addr,
    pub metric: u32,
    pub interface: String,
    pub learned_from: Option<Ipv4Addr>,
    pub last_updated: Instant,
    pub created_at: Instant,
    pub source: RouteSource,
}

impl Route {
    /// Create a new route entry
    pub fn new(
        destination: Ipv4Addr,
        subnet_mask: Ipv4Addr,
        next_hop: Ipv4Addr,
        metric: u32,
        interface: String,
        source: RouteSource,
        learned_from: Option<Ipv4Addr>,
    ) -> Self {
        let now = Instant::now();
        Self {
            destination,
            subnet_mask,
            next_hop,
            metric,
            interface,
            learned_from,
            last_updated: now,
            created_at: now,
            source,
        }
    }

    /// Create a directly connected route from interface configuration
    pub fn new_direct(destination: Ipv4Addr, subnet_mask: Ipv4Addr, interface: String) -> Self {
        Self::new(
            destination,
            subnet_mask,
            Ipv4Addr::UNSPECIFIED,
            1,
            interface,
            RouteSource::Direct,
            None,
        )
    }

    pub fn age_seconds(&self) -> u64 {
        self.last_updated.elapsed().as_secs()
    }

    pub fn learned_from_display(&self) -> Option<String> {
        self.learned_from.map(|ip| ip.to_string())
    }

    pub fn network_address(&self) -> Ipv4Addr {
        let dest_octets = self.destination.octets();
        let mask_octets = self.subnet_mask.octets();

        Ipv4Addr::new(
            dest_octets[0] & mask_octets[0],
            dest_octets[1] & mask_octets[1],
            dest_octets[2] & mask_octets[2],
            dest_octets[3] & mask_octets[3],
        )
    }

    pub fn prefix_length(&self) -> u32 {
        u32::from(self.subnet_mask).count_ones()
    }

    pub fn mark_unreachable(&mut self) {
        self.metric = 16;
        self.last_updated = Instant::now();
    }

    pub fn update_from(&mut self, other: &Route) {
        self.metric = other.metric;
        self.next_hop = other.next_hop;
        self.interface = other.interface.clone();
        self.learned_from = other.learned_from;
        self.last_updated = Instant::now();
        self.source = other.source;
    }

    pub fn to_snapshot(&self) -> RouteSnapshot {
        RouteSnapshot {
            destination: self.destination.to_string(),
            subnet_mask: self.subnet_mask.to_string(),
            next_hop: self.next_hop.to_string(),
            metric: self.metric,
            interface: self.interface.clone(),
            learned_from: self.learned_from_display(),
            age_seconds: self.age_seconds(),
            source: self.source,
        }
    }
}

/// Routing table that manages all routes
#[derive(Debug, Clone)]
pub struct RoutingTable {
    routes: HashMap<String, Route>,
    route_timeout: Duration,
    garbage_collection_timeout: Duration,
}

impl RoutingTable {
    /// Create a new routing table with default timers
    pub fn new() -> Self {
        Self::with_timeouts(DEFAULT_ROUTE_TIMEOUT, DEFAULT_GC_TIMEOUT)
    }

    pub fn with_timeouts(route_timeout: Duration, garbage_collection_timeout: Duration) -> Self {
        Self {
            routes: HashMap::new(),
            route_timeout,
            garbage_collection_timeout,
        }
    }

    fn key(destination: Ipv4Addr, subnet_mask: Ipv4Addr) -> String {
        format!("{}/{}", destination, subnet_mask)
    }

    /// Add or replace a route entry based on source priority and metric
    pub fn add_or_replace(&mut self, route: Route) -> bool {
        let key = Self::key(route.destination, route.subnet_mask);

        match self.routes.get_mut(&key) {
            Some(existing) => {
                // Prefer higher priority sources (direct > static > dynamic)
                if route.source.priority() > existing.source.priority() {
                    *existing = route;
                    return true;
                }

                // For same source priority, keep better metric or update timestamp if same path
                if route.metric < existing.metric
                    || (route.metric == existing.metric
                        && route.next_hop == existing.next_hop
                        && route.interface == existing.interface)
                {
                    existing.update_from(&route);
                    return true;
                }

                false
            }
            None => {
                self.routes.insert(key, route);
                true
            }
        }
    }

    pub fn install_direct_route(
        &mut self,
        destination: Ipv4Addr,
        subnet_mask: Ipv4Addr,
        interface: String,
    ) -> bool {
        self.add_or_replace(Route::new_direct(destination, subnet_mask, interface))
    }

    pub fn add_static_route(
        &mut self,
        destination: Ipv4Addr,
        subnet_mask: Ipv4Addr,
        next_hop: Ipv4Addr,
        metric: u32,
        interface: String,
    ) -> bool {
        let route = Route::new(
            destination,
            subnet_mask,
            next_hop,
            metric,
            interface,
            RouteSource::Static,
            Some(next_hop),
        );
        self.add_or_replace(route)
    }

    pub fn remove_route(&mut self, destination: Ipv4Addr, subnet_mask: Ipv4Addr) -> bool {
        let key = Self::key(destination, subnet_mask);
        self.routes.remove(&key).is_some()
    }

    pub fn get_route(&self, destination: Ipv4Addr) -> Option<&Route> {
        self.find_best_route(&destination)
    }

    pub fn get_all_routes(&self) -> Vec<&Route> {
        self.routes.values().collect()
    }

    pub fn snapshot(&self) -> Vec<RouteSnapshot> {
        self.routes.values().map(Route::to_snapshot).collect()
    }

    pub fn get_routes_for_advertising(&self, outgoing_interface: &str) -> Vec<&Route> {
        self.routes
            .values()
            .filter(|route| route.interface != outgoing_interface)
            .collect()
    }

    /// Longest prefix match
    pub fn find_best_route(&self, destination: &Ipv4Addr) -> Option<&Route> {
        self.routes
            .values()
            .filter(|route| self.matches_network(destination, route))
            .max_by_key(|route| route.prefix_length())
    }

    fn matches_network(&self, destination: &Ipv4Addr, route: &Route) -> bool {
        let dest_octets = destination.octets();
        let route_octets = route.destination.octets();
        let mask_octets = route.subnet_mask.octets();

        for i in 0..4 {
            if (dest_octets[i] & mask_octets[i]) != (route_octets[i] & mask_octets[i]) {
                return false;
            }
        }

        true
    }

    pub fn route_count(&self) -> usize {
        self.routes.len()
    }

    pub fn get_stats(&self) -> RoutingTableStatistics {
        let mut stats = RoutingTableStatistics::default();
        stats.total_routes = self.routes.len();

        for route in self.routes.values() {
            match route.source {
                RouteSource::Direct => stats.direct_routes += 1,
                RouteSource::Static => stats.static_routes += 1,
                RouteSource::Dynamic => stats.learned_routes += 1,
            }
        }

        stats
    }

    pub fn clear_source(&mut self, source: RouteSource) {
        self.routes.retain(|_, route| route.source != source);
    }

    /// Update dynamic routes based on timeouts
    pub fn process_timeouts(&mut self) {
        let now = Instant::now();
        for route in self.routes.values_mut() {
            if route.source == RouteSource::Dynamic {
                if now.duration_since(route.last_updated) > self.route_timeout {
                    route.mark_unreachable();
                }
            }
        }
    }

    pub fn garbage_collect(&mut self) {
        let now = Instant::now();
        self.routes.retain(|_, route| {
            if route.source == RouteSource::Dynamic && route.metric >= 16 {
                now.duration_since(route.last_updated) <= self.garbage_collection_timeout
            } else {
                true
            }
        });
    }

    pub fn print_table(&self) {
        println!("=== Routing Table ===");
        println!("Destination\tMask\t\t\tNext Hop\t\tMetric\tInterface\tSource");
        println!(
            "--------------------------------------------------------------------------------"
        );

        for route in self.routes.values() {
            println!(
                "{}\t{}\t{}\t{}\t{}\t{}",
                route.destination,
                route.subnet_mask,
                route.next_hop,
                route.metric,
                route.interface,
                route.source.as_str()
            );
        }
        println!();
    }
}

impl Default for RoutingTable {
    fn default() -> Self {
        Self::new()
    }
}

/// Statistics about the routing table
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct RoutingTableStatistics {
    pub total_routes: usize,
    pub direct_routes: usize,
    pub static_routes: usize,
    pub learned_routes: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;

    #[test]
    fn direct_route_priority() {
        let mut table = RoutingTable::new();
        let dest = Ipv4Addr::new(192, 168, 1, 0);
        let mask = Ipv4Addr::new(255, 255, 255, 0);

        table.add_or_replace(Route::new(
            dest,
            mask,
            Ipv4Addr::new(192, 168, 1, 1),
            2,
            "eth0".to_string(),
            RouteSource::Dynamic,
            Some(Ipv4Addr::new(10, 0, 0, 1)),
        ));

        assert_eq!(table.route_count(), 1);

        // Installing a direct route should override dynamic
        table.install_direct_route(dest, mask, "eth0".to_string());
        let route = table.get_route(dest).unwrap();
        assert_eq!(route.source, RouteSource::Direct);
        assert_eq!(route.metric, 1);
    }

    #[test]
    fn snapshot_contains_expected_fields() {
        let mut table = RoutingTable::new();
        let dest = Ipv4Addr::new(10, 0, 0, 0);
        let mask = Ipv4Addr::new(255, 255, 255, 0);
        table.add_static_route(
            dest,
            mask,
            Ipv4Addr::new(10, 0, 0, 1),
            3,
            "eth1".to_string(),
        );

        let snapshot = table.snapshot();
        assert_eq!(snapshot.len(), 1);
        let entry = &snapshot[0];
        assert_eq!(entry.destination, "10.0.0.0");
        assert_eq!(entry.source, RouteSource::Static);
    }
}
