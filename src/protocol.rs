//! RIP protocol implementation

use crate::RustRouteResult;
use serde::{Deserialize, Serialize};
use std::net::Ipv4Addr;

/// RIP packet types
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum RipCommand {
    Request = 1,
    Response = 2,
}

/// RIP route entry
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RipEntry {
    pub address_family: u16,
    pub route_tag: u16,
    pub ip_address: Ipv4Addr,
    pub subnet_mask: Ipv4Addr,
    pub next_hop: Ipv4Addr,
    pub metric: u32,
}

/// RIP packet structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RipPacket {
    pub command: RipCommand,
    pub version: u8,
    pub reserved: u16,
    pub entries: Vec<RipEntry>,
}

impl RipPacket {
    /// Create a new RIP request packet
    pub fn new_request() -> Self {
        Self {
            command: RipCommand::Request,
            version: 2,
            reserved: 0,
            entries: vec![RipEntry {
                address_family: 0,
                route_tag: 0,
                ip_address: Ipv4Addr::new(0, 0, 0, 0),
                subnet_mask: Ipv4Addr::new(0, 0, 0, 0),
                next_hop: Ipv4Addr::new(0, 0, 0, 0),
                metric: 16,
            }],
        }
    }

    /// Create a new RIP response packet
    pub fn new_response(entries: Vec<RipEntry>) -> Self {
        Self {
            command: RipCommand::Response,
            version: 2,
            reserved: 0,
            entries,
        }
    }

    /// Serialize packet to bytes
    pub fn to_bytes(&self) -> RustRouteResult<Vec<u8>> {
        let mut buffer = Vec::new();

        // Command (1 byte)
        buffer.push(self.command.clone() as u8);

        // Version (1 byte)
        buffer.push(self.version);

        // Reserved (2 bytes)
        buffer.extend_from_slice(&self.reserved.to_be_bytes());

        // Entries
        for entry in &self.entries {
            // Address family (2 bytes)
            buffer.extend_from_slice(&entry.address_family.to_be_bytes());

            // Route tag (2 bytes)
            buffer.extend_from_slice(&entry.route_tag.to_be_bytes());

            // IP address (4 bytes)
            buffer.extend_from_slice(&entry.ip_address.octets());

            // Subnet mask (4 bytes)
            buffer.extend_from_slice(&entry.subnet_mask.octets());

            // Next hop (4 bytes)
            buffer.extend_from_slice(&entry.next_hop.octets());

            // Metric (4 bytes)
            buffer.extend_from_slice(&entry.metric.to_be_bytes());
        }

        Ok(buffer)
    }

    /// Parse packet from bytes
    pub fn from_bytes(data: &[u8]) -> RustRouteResult<Self> {
        if data.len() < 4 {
            return Err(crate::RustRouteError::ProtocolError(
                "Packet too short".to_string(),
            ));
        }

        let command = match data[0] {
            1 => RipCommand::Request,
            2 => RipCommand::Response,
            _ => {
                return Err(crate::RustRouteError::ProtocolError(
                    "Invalid command".to_string(),
                ))
            }
        };

        let version = data[1];
        let reserved = u16::from_be_bytes([data[2], data[3]]);

        let mut entries = Vec::new();
        let mut offset = 4;

        while offset + 20 <= data.len() {
            let address_family = u16::from_be_bytes([data[offset], data[offset + 1]]);
            let route_tag = u16::from_be_bytes([data[offset + 2], data[offset + 3]]);
            let ip_address = Ipv4Addr::from([
                data[offset + 4],
                data[offset + 5],
                data[offset + 6],
                data[offset + 7],
            ]);
            let subnet_mask = Ipv4Addr::from([
                data[offset + 8],
                data[offset + 9],
                data[offset + 10],
                data[offset + 11],
            ]);
            let next_hop = Ipv4Addr::from([
                data[offset + 12],
                data[offset + 13],
                data[offset + 14],
                data[offset + 15],
            ]);
            let metric = u32::from_be_bytes([
                data[offset + 16],
                data[offset + 17],
                data[offset + 18],
                data[offset + 19],
            ]);

            entries.push(RipEntry {
                address_family,
                route_tag,
                ip_address,
                subnet_mask,
                next_hop,
                metric,
            });

            offset += 20;
        }

        Ok(Self {
            command,
            version,
            reserved,
            entries,
        })
    }

    /// Create a new RIP update packet with routes
    pub fn new_update(_router_id: uuid::Uuid, routes: Vec<crate::routing_table::Route>) -> Self {
        let entries = routes
            .into_iter()
            .map(|route| RipEntry {
                address_family: 2, // IP
                route_tag: 0,
                ip_address: route.destination,
                subnet_mask: route.subnet_mask,
                next_hop: route.next_hop,
                metric: route.metric,
            })
            .collect();

        Self {
            command: RipCommand::Response,
            version: 2,
            reserved: 0,
            entries,
        }
    }

    /// Serialize packet to JSON
    pub fn to_json(&self) -> crate::RustRouteResult<String> {
        serde_json::to_string(self).map_err(|e| {
            crate::RustRouteError::ProtocolError(format!("JSON serialization failed: {}", e))
        })
    }

    /// Deserialize packet from JSON
    pub fn from_json(json_str: &str) -> crate::RustRouteResult<Self> {
        serde_json::from_str(json_str).map_err(|e| {
            crate::RustRouteError::ProtocolError(format!("JSON deserialization failed: {}", e))
        })
    }

    /// Validate packet contents
    pub fn validate(&self) -> crate::RustRouteResult<()> {
        if self.version != 2 {
            return Err(crate::RustRouteError::ProtocolError(
                "Invalid RIP version".to_string(),
            ));
        }

        for entry in &self.entries {
            if entry.metric > 16 {
                return Err(crate::RustRouteError::ProtocolError(
                    "Invalid metric value".to_string(),
                ));
            }
        }

        Ok(())
    }
}

impl RipEntry {
    /// Create a new RIP entry
    pub fn new(
        ip_address: Ipv4Addr,
        subnet_mask: Ipv4Addr,
        next_hop: Ipv4Addr,
        metric: u32,
    ) -> Self {
        Self {
            address_family: 2, // IP
            route_tag: 0,
            ip_address,
            subnet_mask,
            next_hop,
            metric,
        }
    }
}
