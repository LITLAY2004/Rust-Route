//! Network interface and communication handling for RustRoute

use crate::protocol::RipPacket;
use crate::{RustRouteError, RustRouteResult};
use serde::{Deserialize, Serialize};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use tokio::net::UdpSocket as TokioUdpSocket;

/// Network interface configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InterfaceConfig {
    pub name: String,
    pub ip_address: Ipv4Addr,
    pub subnet_mask: Ipv4Addr,
    pub multicast_address: Ipv4Addr,
    pub port: u16,
    pub mtu: u16,
    pub enabled: bool,
}

impl Default for InterfaceConfig {
    fn default() -> Self {
        Self {
            name: "eth0".to_string(),
            ip_address: Ipv4Addr::new(192, 168, 1, 1),
            subnet_mask: Ipv4Addr::new(255, 255, 255, 0),
            multicast_address: Ipv4Addr::new(224, 0, 0, 9), // RIP multicast address
            port: 520, // Standard RIP port
            mtu: 1500,
            enabled: true,
        }
    }
}

/// Network interface for RustRoute communication
#[derive(Debug)]
pub struct NetworkInterface {
    pub config: InterfaceConfig,
    socket: Option<TokioUdpSocket>,
}

impl NetworkInterface {
    /// Create a new network interface
    pub fn new(config: InterfaceConfig) -> Self {
        Self {
            config,
            socket: None,
        }
    }

    /// Initialize the network interface
    pub async fn initialize(&mut self) -> RustRouteResult<()> {
        let bind_addr = SocketAddr::new(
            IpAddr::V4(self.config.ip_address),
            self.config.port,
        );

        let socket = TokioUdpSocket::bind(bind_addr)
            .await
            .map_err(|e| RustRouteError::NetworkError(format!("Failed to bind socket: {}", e)))?;

        // Enable broadcast for RIP communication
        socket
            .set_broadcast(true)
            .map_err(|e| RustRouteError::NetworkError(format!("Failed to set broadcast: {}", e)))?;

        self.socket = Some(socket);
        
        log::info!(
            "Network interface {} initialized on {}:{}",
            self.config.name,
            self.config.ip_address,
            self.config.port
        );

        Ok(())
    }

    /// Send a RIPER packet
    pub async fn send_packet(&self, packet: &RipPacket) -> RustRouteResult<()> {
        let socket = self.socket.as_ref()
            .ok_or_else(|| RustRouteError::NetworkError("Interface not initialized".to_string()))?;

        // Serialize packet to JSON
        let json_data = packet.to_json()
            .map_err(|e| RustRouteError::ProtocolError(format!("Failed to serialize packet: {}", e)))?;

        // Send to broadcast address
        let broadcast_addr = self.get_broadcast_address();
        let target = SocketAddr::new(IpAddr::V4(broadcast_addr), self.config.port);

        socket.send_to(json_data.as_bytes(), target)
            .await
            .map_err(|e| RustRouteError::NetworkError(format!("Failed to send packet: {}", e)))?;

        log::debug!("Sent packet to {} on interface {}", target, self.config.name);
        Ok(())
    }

    /// Send a packet to a specific destination
    pub async fn send_packet_to(&self, packet: &RipPacket, destination: SocketAddr) -> RustRouteResult<()> {
        let socket = self.socket.as_ref()
            .ok_or_else(|| RustRouteError::NetworkError("Interface not initialized".to_string()))?;

        let json_data = packet.to_json()
            .map_err(|e| RustRouteError::ProtocolError(format!("Failed to serialize packet: {}", e)))?;

        socket.send_to(json_data.as_bytes(), destination)
            .await
            .map_err(|e| RustRouteError::NetworkError(format!("Failed to send packet: {}", e)))?;

        log::debug!("Sent packet to {} on interface {}", destination, self.config.name);
        Ok(())
    }

    /// Receive a RIPER packet
    pub async fn receive_packet(&self) -> RustRouteResult<(RipPacket, SocketAddr)> {
        let socket = self.socket.as_ref()
            .ok_or_else(|| RustRouteError::NetworkError("Interface not initialized".to_string()))?;

        let mut buffer = vec![0u8; self.config.mtu as usize];
        let (bytes_received, sender_addr) = socket.recv_from(&mut buffer)
            .await
            .map_err(|e| RustRouteError::NetworkError(format!("Failed to receive packet: {}", e)))?;

        buffer.truncate(bytes_received);
        let json_str = String::from_utf8(buffer)
            .map_err(|e| RustRouteError::ProtocolError(format!("Invalid UTF-8 in packet: {}", e)))?;

        let packet = RipPacket::from_json(&json_str)
            .map_err(|e| RustRouteError::ProtocolError(format!("Failed to deserialize packet: {}", e)))?;

        // Validate packet
        packet.validate()
            .map_err(|e| RustRouteError::ProtocolError(format!("Invalid packet: {}", e)))?;

        log::debug!("Received packet from {} on interface {}", sender_addr, self.config.name);
        Ok((packet, sender_addr))
    }

    /// Get the broadcast address for this interface
    pub fn get_broadcast_address(&self) -> Ipv4Addr {
        let ip = u32::from(self.config.ip_address);
        let mask = u32::from(self.config.subnet_mask);
        let network = ip & mask;
        let broadcast = network | (!mask);
        Ipv4Addr::from(broadcast)
    }

    /// Get the network address for this interface
    pub fn get_network_address(&self) -> Ipv4Addr {
        let ip = u32::from(self.config.ip_address);
        let mask = u32::from(self.config.subnet_mask);
        let network = ip & mask;
        Ipv4Addr::from(network)
    }

    /// Check if an IP address is in the same subnet
    pub fn is_in_subnet(&self, addr: Ipv4Addr) -> bool {
        let ip = u32::from(self.config.ip_address);
        let mask = u32::from(self.config.subnet_mask);
        let addr_u32 = u32::from(addr);
        
        (ip & mask) == (addr_u32 & mask)
    }

    /// Get interface statistics
    pub fn get_stats(&self) -> InterfaceStats {
        InterfaceStats {
            name: self.config.name.clone(),
            ip_address: self.config.ip_address,
            subnet_mask: self.config.subnet_mask,
            is_active: self.socket.is_some(),
            mtu: self.config.mtu,
        }
    }
}

/// Network interface statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InterfaceStats {
    pub name: String,
    pub ip_address: Ipv4Addr,
    pub subnet_mask: Ipv4Addr,
    pub is_active: bool,
    pub mtu: u16,
}

/// Network manager for handling multiple interfaces
#[derive(Debug)]
pub struct NetworkManager {
    interfaces: std::collections::HashMap<String, NetworkInterface>,
}

impl NetworkManager {
    /// Create a new network manager
    pub fn new() -> Self {
        Self {
            interfaces: std::collections::HashMap::new(),
        }
    }

    /// Add a network interface
    pub fn add_interface(&mut self, interface: NetworkInterface) {
        let name = interface.config.name.clone();
        self.interfaces.insert(name, interface);
    }

    /// Initialize all interfaces
    pub async fn initialize_all(&mut self) -> RustRouteResult<()> {
        for (name, interface) in &mut self.interfaces {
            interface.initialize().await
                .map_err(|e| RustRouteError::NetworkError(format!("Failed to initialize interface {}: {}", name, e)))?;
        }
        Ok(())
    }

    /// Get interface by name
    pub fn get_interface(&self, name: &str) -> Option<&NetworkInterface> {
        self.interfaces.get(name)
    }

    /// Get mutable interface by name
    pub fn get_interface_mut(&mut self, name: &str) -> Option<&mut NetworkInterface> {
        self.interfaces.get_mut(name)
    }

    /// Get all interface names
    pub fn get_interface_names(&self) -> Vec<String> {
        self.interfaces.keys().cloned().collect()
    }

    /// Broadcast packet to all interfaces
    pub async fn broadcast_packet(&self, packet: &RipPacket) -> RustRouteResult<()> {
        for (name, interface) in &self.interfaces {
            if let Err(e) = interface.send_packet(packet).await {
                log::error!("Failed to send packet on interface {}: {}", name, e);
            }
        }
        Ok(())
    }

    /// Get statistics for all interfaces
    pub fn get_all_stats(&self) -> Vec<InterfaceStats> {
        self.interfaces.values().map(|iface| iface.get_stats()).collect()
    }
}

impl Default for NetworkManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Utility functions for network operations
pub mod utils {
    use super::*;

    /// Convert subnet mask to prefix length
    pub fn mask_to_prefix_length(mask: Ipv4Addr) -> u8 {
        u32::from(mask).count_ones() as u8
    }

    /// Convert prefix length to subnet mask
    pub fn prefix_length_to_mask(prefix_len: u8) -> Ipv4Addr {
        if prefix_len == 0 {
            Ipv4Addr::new(0, 0, 0, 0)
        } else if prefix_len >= 32 {
            Ipv4Addr::new(255, 255, 255, 255)
        } else {
            let mask = !((1u32 << (32 - prefix_len)) - 1);
            Ipv4Addr::from(mask)
        }
    }

    /// Check if two IP addresses are in the same subnet
    pub fn in_same_subnet(ip1: Ipv4Addr, ip2: Ipv4Addr, mask: Ipv4Addr) -> bool {
        let ip1_u32 = u32::from(ip1);
        let ip2_u32 = u32::from(ip2);
        let mask_u32 = u32::from(mask);
        
        (ip1_u32 & mask_u32) == (ip2_u32 & mask_u32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::utils::*;

    #[test]
    fn test_broadcast_address() {
        let config = InterfaceConfig {
            ip_address: Ipv4Addr::new(192, 168, 1, 10),
            subnet_mask: Ipv4Addr::new(255, 255, 255, 0),
            ..Default::default()
        };
        
        let interface = NetworkInterface::new(config);
        let broadcast = interface.get_broadcast_address();
        
        assert_eq!(broadcast, Ipv4Addr::new(192, 168, 1, 255));
    }

    #[test]
    fn test_subnet_check() {
        let config = InterfaceConfig {
            ip_address: Ipv4Addr::new(192, 168, 1, 10),
            subnet_mask: Ipv4Addr::new(255, 255, 255, 0),
            ..Default::default()
        };
        
        let interface = NetworkInterface::new(config);
        
        assert!(interface.is_in_subnet(Ipv4Addr::new(192, 168, 1, 20)));
        assert!(!interface.is_in_subnet(Ipv4Addr::new(192, 168, 2, 20)));
    }

    #[test]
    fn test_prefix_conversion() {
        assert_eq!(mask_to_prefix_length(Ipv4Addr::new(255, 255, 255, 0)), 24);
        assert_eq!(prefix_length_to_mask(24), Ipv4Addr::new(255, 255, 255, 0));
    }
}
