//! Routing table implementation for RIP protocol

use std::collections::HashMap;
use std::net::Ipv4Addr;
use std::time::{Duration, Instant};
// use serde::{Serialize, Deserialize};
use crate::RustRouteResult;

/// A single route entry in the routing table
#[derive(Debug, Clone, PartialEq)]
pub struct Route {
    pub destination: Ipv4Addr,
    pub subnet_mask: Ipv4Addr,
    pub next_hop: Ipv4Addr,
    pub metric: u32,
    pub interface: String,
    pub learned_from: Option<Ipv4Addr>,
    pub last_updated: Option<Instant>,
    pub is_directly_connected: bool,
}

impl Route {
    /// Create a new route entry
    pub fn new(
        destination: Ipv4Addr,
        subnet_mask: Ipv4Addr,
        next_hop: Ipv4Addr,
        metric: u32,
        interface: String,
    ) -> Self {
        Self {
            destination,
            subnet_mask,
            next_hop,
            metric,
            interface,
            learned_from: None,
            last_updated: Some(Instant::now()),
            is_directly_connected: false,
        }
    }

    /// Create a directly connected route
    pub fn new_direct(
        destination: Ipv4Addr,
        subnet_mask: Ipv4Addr,
        interface: String,
    ) -> Self {
        Self {
            destination,
            subnet_mask,
            next_hop: Ipv4Addr::new(0, 0, 0, 0),
            metric: 0,
            interface,
            learned_from: None,
            last_updated: Some(Instant::now()),
            is_directly_connected: true,
        }
    }

    /// Check if this route is expired (not updated for timeout period)
    pub fn is_expired(&self, timeout: Duration) -> bool {
        if let Some(last_updated) = self.last_updated {
            last_updated.elapsed() > timeout
        } else {
            false
        }
    }

    /// Get the network address for this route
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
}

/// Routing table that manages all routes
#[derive(Debug)]
pub struct RoutingTable {
    routes: HashMap<String, Route>,
    route_timeout: Duration,
    garbage_collection_timeout: Duration,
}

impl RoutingTable {
    /// Create a new routing table
    pub fn new() -> Self {
        Self {
            routes: HashMap::new(),
            route_timeout: Duration::from_secs(180), // 3 minutes
            garbage_collection_timeout: Duration::from_secs(120), // 2 minutes
        }
    }

    /// Add or update a route in the table
    pub fn add_route(&mut self, route: Route) -> RustRouteResult<()> {
        let key = format!("{}/{}", route.destination, route.subnet_mask);
        
        // Check if this is a better route
        if let Some(existing_route) = self.routes.get(&key) {
            if route.metric < existing_route.metric {
                log::info!("Updating route to {} with better metric {}", 
                          route.destination, route.metric);
                self.routes.insert(key, route);
            } else if route.metric == existing_route.metric && 
                     route.next_hop == existing_route.next_hop {
                // Same route, just update timestamp
                let mut updated_route = route;
                updated_route.last_updated = Some(Instant::now());
                self.routes.insert(key, updated_route);
            }
        } else {
            log::info!("Adding new route to {} via {} with metric {}", 
                      route.destination, route.next_hop, route.metric);
            self.routes.insert(key, route);
        }
        
        Ok(())
    }

    /// Remove a route from the table
    pub fn remove_route(&mut self, destination: Ipv4Addr, subnet_mask: Ipv4Addr) -> RustRouteResult<()> {
        let key = format!("{}/{}", destination, subnet_mask);
        if self.routes.remove(&key).is_some() {
            log::info!("Removed route to {}", destination);
        }
        Ok(())
    }

    /// Get a route for a specific destination
    pub fn get_route(&self, destination: Ipv4Addr) -> Option<&Route> {
        // Find the best matching route (longest prefix match)
        let mut best_route: Option<&Route> = None;
        let mut best_prefix_len = 0;

        for route in self.routes.values() {
            if self.matches_network(destination, route) {
                let prefix_len = self.prefix_length(route.subnet_mask);
                if prefix_len > best_prefix_len {
                    best_route = Some(route);
                    best_prefix_len = prefix_len;
                }
            }
        }

        best_route
    }

    /// Get all routes in the table
    pub fn get_all_routes(&self) -> Vec<&Route> {
        self.routes.values().collect()
    }

    /// Get routes for advertising (split horizon)
    pub fn get_routes_for_advertising(&self, outgoing_interface: &str) -> Vec<&Route> {
        self.routes.values()
            .filter(|route| {
                // Split horizon: don't advertise routes learned from this interface
                route.interface != outgoing_interface
            })
            .collect()
    }

    /// Clean up expired routes
    pub fn cleanup_expired_routes(&mut self) -> RustRouteResult<()> {
        let expired_keys: Vec<String> = self.routes
            .iter()
            .filter(|(_, route)| route.is_expired(self.route_timeout))
            .map(|(key, _)| key.clone())
            .collect();

        for key in expired_keys {
            if let Some(route) = self.routes.remove(&key) {
                log::info!("Removed expired route to {}", route.destination);
            }
        }

        Ok(())
    }

    /// Check if destination matches a route's network
    fn matches_network(&self, destination: Ipv4Addr, route: &Route) -> bool {
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

    /// Calculate prefix length from subnet mask
    fn prefix_length(&self, subnet_mask: Ipv4Addr) -> u32 {
        let mask_u32 = u32::from(subnet_mask);
        mask_u32.count_ones()
    }

    /// Get routing table statistics
    pub fn get_statistics(&self) -> RoutingTableStatistics {
        let total_routes = self.routes.len();
        let direct_routes = self.routes.values()
            .filter(|r| r.is_directly_connected)
            .count();
        let learned_routes = total_routes - direct_routes;

        RoutingTableStatistics {
            total_routes,
            direct_routes,
            learned_routes,
        }
    }

    /// Update an existing route or add a new one
    pub fn update_route(
        &mut self,
        destination: Ipv4Addr,
        subnet_mask: Ipv4Addr,
        next_hop: Ipv4Addr,
        metric: u32,
        interface: String,
        learned_from: Option<Ipv4Addr>,
    ) -> bool {
        let key = format!("{}/{}", destination, self.prefix_length(subnet_mask));
        
        if let Some(existing_route) = self.routes.get_mut(&key) {
            // Update existing route if new metric is better
            if metric < existing_route.metric {
                existing_route.next_hop = next_hop;
                existing_route.metric = metric;
                existing_route.interface = interface;
                existing_route.learned_from = learned_from;
                existing_route.last_updated = Some(Instant::now());
                return true;
            }
        } else {
            // Add new route
            let route = Route {
                destination,
                subnet_mask,
                next_hop,
                metric,
                interface,
                learned_from,
                last_updated: Some(Instant::now()),
                is_directly_connected: learned_from.is_none(),
            };
            self.routes.insert(key, route);
            return true;
        }
        
        false
    }

    /// Process timeouts for route entries
    pub fn process_timeouts(&mut self) {
        let now = Instant::now();
        let timeout_duration = Duration::from_secs(180); // 3 minutes
        
        let mut routes_to_remove = Vec::new();
        
        for (key, route) in &mut self.routes {
            if let Some(last_updated) = route.last_updated {
                if now.duration_since(last_updated) > timeout_duration && !route.is_directly_connected {
                    route.metric = 16; // Mark as unreachable
                    routes_to_remove.push(key.clone());
                }
            }
        }
        
        // Remove timed out routes
        for key in routes_to_remove {
            self.routes.remove(&key);
        }
    }

    /// Garbage collect expired routes
    pub fn garbage_collect(&mut self) {
        let now = Instant::now();
        let gc_duration = Duration::from_secs(240); // 4 minutes
        
        self.routes.retain(|_, route| {
            if let Some(last_updated) = route.last_updated {
                now.duration_since(last_updated) <= gc_duration || route.is_directly_connected
            } else {
                route.is_directly_connected
            }
        });
    }

    /// Get routing table statistics (alias for get_statistics)
    pub fn get_stats(&self) -> RoutingTableStatistics {
        self.get_statistics()
    }

    /// Print routing table (for debugging)
    pub fn print_table(&self) {
        println!("=== Routing Table ===");
        println!("Destination\t\tMask\t\t\tNext Hop\t\tMetric\tInterface");
        println!("--------------------------------------------------------------------------");
        
        for route in self.routes.values() {
            println!("{}\t{}\t{}\t{}\t{}", 
                route.destination, 
                route.subnet_mask, 
                route.next_hop, 
                route.metric, 
                route.interface
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
#[derive(Debug)]
pub struct RoutingTableStatistics {
    pub total_routes: usize,
    pub direct_routes: usize,
    pub learned_routes: usize,
}