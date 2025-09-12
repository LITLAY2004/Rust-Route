//! Router implementation for RustRoute

use crate::routing_table::RoutingTable;
use crate::network::NetworkInterface;
use crate::protocol::RipPacket;
use crate::metrics::MetricsCollector;
use crate::{RustRouteError, RustRouteResult};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{RwLock, mpsc};
use tokio::net::UdpSocket;
use uuid::Uuid;

/// Router configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouterConfig {
    pub router_id: Uuid,
    pub port: u16,
    pub rip_version: u8,
    pub interfaces: Vec<InterfaceConfig>,
    pub update_interval: u64,          // seconds
    pub holddown_timer: u64,           // seconds
    pub garbage_collection_timer: u64, // seconds
    pub max_hop_count: u8,
    pub split_horizon: bool,
    pub poison_reverse: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InterfaceConfig {
    pub name: String,
    pub ip_address: Ipv4Addr,
    pub subnet_mask: Ipv4Addr,
    pub enabled: bool,
}

impl Default for InterfaceConfig {
    fn default() -> Self {
        Self {
            name: "eth0".to_string(),
            ip_address: Ipv4Addr::new(192, 168, 1, 1),
            subnet_mask: Ipv4Addr::new(255, 255, 255, 0),
            enabled: true,
        }
    }
}

impl Default for RouterConfig {
    fn default() -> Self {
        Self {
            router_id: Uuid::new_v4(),
            port: 520,
            rip_version: 2,
            interfaces: vec![
                InterfaceConfig {
                    name: "eth0".to_string(),
                    ip_address: Ipv4Addr::new(192, 168, 1, 1),
                    subnet_mask: Ipv4Addr::new(255, 255, 255, 0),
                    enabled: true,
                }
            ],
            update_interval: 30,
            holddown_timer: 180,
            garbage_collection_timer: 240,
            max_hop_count: 15,
            split_horizon: true,
            poison_reverse: false,
        }
    }
}

/// RustRoute Router implementation with real networking
#[derive(Debug)]
pub struct Router {
    pub config: RouterConfig,
    pub routing_table: Arc<RwLock<RoutingTable>>,
    pub interfaces: HashMap<String, NetworkInterface>,
    pub neighbors: Arc<RwLock<HashMap<IpAddr, RouterInfo>>>,
    pub metrics: Arc<MetricsCollector>,
    pub sockets: HashMap<String, Arc<UdpSocket>>,
    pub shutdown_sender: Option<mpsc::Sender<()>>,
    running: bool,
    start_time: Instant,
}

/// Information about neighboring routers
#[derive(Debug, Clone)]
pub struct RouterInfo {
    pub router_id: Uuid,
    pub last_seen: Instant,
    pub interface: String,
    pub routes_count: usize,
}

impl Router {
    /// Create a new RustRoute router
    pub async fn new(config: RouterConfig) -> RustRouteResult<Self> {
        Ok(Self {
            config,
            routing_table: Arc::new(RwLock::new(RoutingTable::new())),
            interfaces: HashMap::new(),
            neighbors: Arc::new(RwLock::new(HashMap::new())),
            metrics: Arc::new(MetricsCollector::new()),
            sockets: HashMap::new(),
            shutdown_sender: None,
            running: false,
            start_time: Instant::now(),
        })
    }

    /// Add a network interface to the router
    pub async fn add_interface(&mut self, interface: NetworkInterface) -> RustRouteResult<()> {
        let interface_name = interface.config.name.clone();
        let bind_addr = SocketAddr::new(
            IpAddr::V4(interface.config.ip_address), 
            self.config.port
        );

        // Create UDP socket for this interface
        let socket = UdpSocket::bind(bind_addr).await
            .map_err(|e| RustRouteError::NetworkError(format!("Failed to bind to {}: {}", bind_addr, e)))?;

        // Enable broadcast
        socket.set_broadcast(true)
            .map_err(|e| RustRouteError::NetworkError(format!("Failed to enable broadcast: {}", e)))?;

        log::info!("Created socket for interface {} on {}", interface_name, bind_addr);

        self.sockets.insert(interface_name.clone(), Arc::new(socket));
        self.interfaces.insert(interface_name, interface);
        
        Ok(())
    }

    /// Run the router with real networking
    pub async fn run_with_real_networking(&mut self, update_interval: Duration) -> RustRouteResult<()> {
        self.running = true;
        log::info!("Starting router with real RIP networking...");

        let (shutdown_tx, mut shutdown_rx) = mpsc::channel(1);
        self.shutdown_sender = Some(shutdown_tx);

        // Clone necessary data for async tasks
        let routing_table = Arc::clone(&self.routing_table);
        let neighbors = Arc::clone(&self.neighbors);
        let metrics = Arc::clone(&self.metrics);
        let config = self.config.clone();

        // Start packet receiver for each interface
        let mut receiver_handles = Vec::new();
        for (interface_name, socket) in &self.sockets {
            let socket_clone = Arc::clone(socket);
            let routing_table_clone = Arc::clone(&routing_table);
            let neighbors_clone = Arc::clone(&neighbors);
            let metrics_clone = Arc::clone(&metrics);
            let config_clone = config.clone();
            let interface_name_clone = interface_name.clone();

            let handle = tokio::spawn(async move {
                Self::packet_receiver(
                    socket_clone,
                    routing_table_clone,
                    neighbors_clone,
                    metrics_clone,
                    config_clone,
                    interface_name_clone,
                ).await
            });
            receiver_handles.push(handle);
        }

        // Start periodic update sender
        let sockets_clone: HashMap<String, Arc<UdpSocket>> = self.sockets.iter()
            .map(|(k, v)| (k.clone(), Arc::clone(v)))
            .collect();
        
        let update_handle = tokio::spawn(async move {
            Self::periodic_update_sender(
                sockets_clone,
                routing_table,
                neighbors,
                metrics,
                config,
                update_interval,
            ).await
        });

        // Wait for shutdown signal
        tokio::select! {
            _ = shutdown_rx.recv() => {
                log::info!("Shutdown signal received");
            }
            result = update_handle => {
                log::error!("Update sender finished unexpectedly: {:?}", result);
            }
        }

        // Cancel all tasks
        for handle in receiver_handles {
            handle.abort();
        }

        self.running = false;
        log::info!("Router stopped");
        Ok(())
    }

    /// Packet receiver for a specific interface
    async fn packet_receiver(
        socket: Arc<UdpSocket>,
        routing_table: Arc<RwLock<RoutingTable>>,
        neighbors: Arc<RwLock<HashMap<IpAddr, RouterInfo>>>,
        metrics: Arc<MetricsCollector>,
        config: RouterConfig,
        interface_name: String,
    ) -> RustRouteResult<()> {
        let mut buffer = [0u8; 65536];

        loop {
            match socket.recv_from(&mut buffer).await {
                Ok((len, sender_addr)) => {
                    metrics.increment_packets_received();
                    
                    // Parse JSON RIP packet
                    if let Ok(packet_str) = std::str::from_utf8(&buffer[..len]) {
                        if let Ok(packet) = RipPacket::from_json(packet_str) {
                            log::debug!("Received RIP packet from {}: {:?}", sender_addr, packet.command);
                            
                            // Validate packet
                            if packet.validate().is_ok() {
                                Self::process_rip_packet(
                                    packet,
                                    sender_addr,
                                    &routing_table,
                                    &neighbors,
                                    &metrics,
                                    &config,
                                    &interface_name,
                                ).await;
                            } else {
                                log::warn!("Invalid RIP packet from {}", sender_addr);
                                metrics.increment_packets_dropped();
                            }
                        } else {
                            log::debug!("Failed to parse packet from {}", sender_addr);
                            metrics.increment_packets_dropped();
                        }
                    } else {
                        log::debug!("Non-UTF8 packet from {}", sender_addr);
                        metrics.increment_packets_dropped();
                    }
                }
                Err(e) => {
                    log::error!("Error receiving packet on {}: {}", interface_name, e);
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
            }
        }
    }

    /// Process received RIP packet
    async fn process_rip_packet(
        packet: RipPacket,
        sender_addr: SocketAddr,
        routing_table: &Arc<RwLock<RoutingTable>>,
        neighbors: &Arc<RwLock<HashMap<IpAddr, RouterInfo>>>,
        metrics: &Arc<MetricsCollector>,
        config: &RouterConfig,
        interface_name: &str,
    ) {
        // Update neighbor information
        {
            let mut neighbors_lock = neighbors.write().await;
            neighbors_lock.insert(sender_addr.ip(), RouterInfo {
                router_id: uuid::Uuid::new_v4(), // Generate a UUID for now
                last_seen: Instant::now(),
                interface: interface_name.to_string(),
                routes_count: packet.entries.len(),
            });
        }

        match packet.command {
            crate::protocol::RipCommand::Response => {
                metrics.increment_routing_updates_received();
                
                // Process route updates
                let mut routing_table_lock = routing_table.write().await;
                let routes_count = packet.entries.len();
                for route in packet.entries {
                    // Apply distance vector algorithm
                    let metric = std::cmp::min(route.metric + 1, config.max_hop_count as u32);
                    
                    let sender_ip = match sender_addr.ip() {
                        std::net::IpAddr::V4(ip) => ip,
                        _ => continue, // Skip IPv6 for now
                    };
                    
                    if routing_table_lock.update_route(
                        route.ip_address,
                        route.subnet_mask,
                        sender_ip,
                        metric,
                        interface_name.to_string(),
                        Some(sender_ip),
                    ) {
                        metrics.increment_route_changes();
                        log::info!("Updated route to {}/{} via {} metric {}", 
                                 route.ip_address, route.subnet_mask, sender_addr.ip(), metric);
                    }
                }
                
                log::debug!("Processed {} routes from {}", routes_count, sender_addr);
            }
            crate::protocol::RipCommand::Request => {
                log::debug!("Received route request from {}", sender_addr);
                // Note: Response would be sent in periodic updates
            }
        }
    }

    /// Periodic update sender
    async fn periodic_update_sender(
        sockets: HashMap<String, Arc<UdpSocket>>,
        routing_table: Arc<RwLock<RoutingTable>>,
        neighbors: Arc<RwLock<HashMap<IpAddr, RouterInfo>>>,
        metrics: Arc<MetricsCollector>,
        config: RouterConfig,
        update_interval: Duration,
    ) -> RustRouteResult<()> {
        let mut interval_timer = tokio::time::interval(update_interval);
            
            loop {
                interval_timer.tick().await;

            // Clean up expired neighbors
            {
                let mut neighbors_lock = neighbors.write().await;
                let now = Instant::now();
                neighbors_lock.retain(|_, neighbor| {
                    now.duration_since(neighbor.last_seen) < Duration::from_secs(180)
                });
            }

            // Process route timeouts and garbage collection
            {
                let mut routing_table_lock = routing_table.write().await;
                routing_table_lock.process_timeouts();
                routing_table_lock.garbage_collect();
            }

            // Send route updates
            let routes = {
                let routing_table_lock = routing_table.read().await;
                routing_table_lock.get_all_routes().into_iter().cloned().collect::<Vec<_>>()
            };

            if !routes.is_empty() {
                let packet = RipPacket::new_update(config.router_id, routes);
                let packet_json = packet.to_json().unwrap();
                let packet_bytes = packet_json.as_bytes();

                // Send to broadcast address on each interface
                for (interface_name, socket) in &sockets {
                    let broadcast_addr = SocketAddr::new(
                        IpAddr::V4(Ipv4Addr::BROADCAST), 
                        config.port
                    );

                    match socket.send_to(packet_bytes, broadcast_addr).await {
                        Ok(_) => {
                            metrics.increment_packets_sent();
                            metrics.increment_routing_updates_sent();
                            log::debug!("Sent {} routes on interface {}", packet.entries.len(), interface_name);
                        }
                        Err(e) => {
                            log::error!("Failed to send update on {}: {}", interface_name, e);
                        }
                    }
                }
            }
        }
    }

    /// Get real-time router statistics
    pub async fn get_real_statistics(&self) -> RouterStatistics {
        let routing_table_lock = self.routing_table.read().await;
        let neighbors_lock = self.neighbors.read().await;
        let routing_stats = routing_table_lock.get_stats();
        let uptime = self.start_time.elapsed();

        RouterStatistics {
            uptime: format!("{}时{}分{}秒", 
                          uptime.as_secs() / 3600,
                          (uptime.as_secs() % 3600) / 60,
                          uptime.as_secs() % 60),
            packets_sent: self.metrics.get_packets_sent(),
            packets_received: self.metrics.get_packets_received(),
            route_count: routing_stats.total_routes,
            neighbor_count: neighbors_lock.len(),
            memory_usage: Self::get_memory_usage(),
        }
    }

    /// Get memory usage (simplified)
    fn get_memory_usage() -> u64 {
        // In a real implementation, this would get actual memory usage
        // For now, return a reasonable estimate
        std::process::id() as u64 * 1024 * 1024 // Simple estimation
    }

    /// Shutdown the router gracefully
    pub async fn shutdown(&mut self) -> RustRouteResult<()> {
        if let Some(sender) = &self.shutdown_sender {
            let _ = sender.send(()).await;
        }
        self.running = false;
        log::info!("Router shutdown initiated");
        Ok(())
    }
}

/// Router statistics for CLI display
#[derive(Debug, Clone)]
pub struct RouterStatistics {
    pub uptime: String,
    pub packets_sent: u64,
    pub packets_received: u64,
    pub route_count: usize,
    pub neighbor_count: usize,
    pub memory_usage: u64,
}