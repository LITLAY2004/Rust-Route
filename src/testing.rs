//! Network connectivity testing module

use crate::{RustRouteError, RustRouteResult};
use std::net::Ipv4Addr;
use std::time::{Duration, Instant};
use tokio::net::TcpStream;
use tokio::time::timeout;

/// Ping test results
#[derive(Debug)]
pub struct PingTestResults {
    pub packets_sent: u32,
    pub packets_received: u32,
    pub packet_loss_percent: f64,
    pub avg_rtt_ms: f64,
    pub min_rtt_ms: f64,
    pub max_rtt_ms: f64,
}

/// Perform real connectivity test
pub async fn perform_connectivity_test(target_ip: Ipv4Addr) -> RustRouteResult<PingTestResults> {
    // Try system ping first
    if let Ok(results) = system_ping_test(target_ip).await {
        return Ok(results);
    }
    
    // Fallback to TCP connectivity test
    tcp_connectivity_test(target_ip).await
}

/// Use system ping command
async fn system_ping_test(target_ip: Ipv4Addr) -> RustRouteResult<PingTestResults> {
    use std::process::Command;
    
    let output = Command::new("ping")
        .arg("-c")
        .arg("4") // 4 packets
        .arg("-W")
        .arg("2") // 2 second timeout
        .arg(target_ip.to_string())
        .output()
        .map_err(|e| RustRouteError::NetworkError(format!("Failed to execute ping: {}", e)))?;
    
    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_ping_output(&stdout)
}

/// Parse ping output
fn parse_ping_output(output: &str) -> RustRouteResult<PingTestResults> {
    let mut packets_sent = 4; // Default
    let mut packets_received = 0;
    let mut rtts = Vec::new();
    
    for line in output.lines() {
        // Parse RTT from ping responses
        if line.contains("time=") {
            if let Some(time_start) = line.find("time=") {
                let time_part = &line[time_start + 5..];
                if let Some(space_pos) = time_part.find(' ') {
                    if let Ok(rtt) = time_part[..space_pos].parse::<f64>() {
                        rtts.push(rtt);
                        packets_received += 1;
                    }
                }
            }
        }
        
        // Parse summary line
        if line.contains("packets transmitted") && line.contains("received") {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 4 {
                packets_sent = parts[0].parse().unwrap_or(4);
                packets_received = parts[3].parse().unwrap_or(0);
            }
        }
    }
    
    let packet_loss_percent = if packets_sent > 0 {
        ((packets_sent - packets_received) as f64 / packets_sent as f64) * 100.0
    } else {
        100.0
    };
    
    let (avg_rtt_ms, min_rtt_ms, max_rtt_ms) = if !rtts.is_empty() {
        let avg = rtts.iter().sum::<f64>() / rtts.len() as f64;
        let min = rtts.iter().fold(f64::INFINITY, |a, &b| a.min(b));
        let max = rtts.iter().fold(0.0f64, |a, &b| a.max(b));
        (avg, min, max)
    } else {
        (0.0, 0.0, 0.0)
    };
    
    Ok(PingTestResults {
        packets_sent,
        packets_received,
        packet_loss_percent,
        avg_rtt_ms,
        min_rtt_ms,
        max_rtt_ms,
    })
}

/// TCP connectivity test as fallback
async fn tcp_connectivity_test(target_ip: Ipv4Addr) -> RustRouteResult<PingTestResults> {
    let mut successful_connections = 0;
    let mut rtts = Vec::new();
    let test_ports = [80, 443, 22, 53]; // Common ports
    
    for &port in &test_ports {
        let addr = std::net::SocketAddr::new(std::net::IpAddr::V4(target_ip), port);
        
        let start_time = Instant::now();
        let connect_result = timeout(Duration::from_secs(2), TcpStream::connect(addr)).await;
        
        if connect_result.is_ok() && connect_result.unwrap().is_ok() {
            let rtt = start_time.elapsed().as_secs_f64() * 1000.0;
            successful_connections += 1;
            rtts.push(rtt);
        }
    }
    
    let packets_sent = test_ports.len() as u32;
    let packets_received = successful_connections;
    let packet_loss_percent = ((packets_sent - packets_received) as f64 / packets_sent as f64) * 100.0;
    
    let (avg_rtt_ms, min_rtt_ms, max_rtt_ms) = if !rtts.is_empty() {
        let avg = rtts.iter().sum::<f64>() / rtts.len() as f64;
        let min = rtts.iter().fold(f64::INFINITY, |a, &b| a.min(b));
        let max = rtts.iter().fold(0.0f64, |a, &b| a.max(b));
        (avg, min, max)
    } else {
        (0.0, 0.0, 0.0)
    };
    
    Ok(PingTestResults {
        packets_sent,
        packets_received,
        packet_loss_percent,
        avg_rtt_ms,
        min_rtt_ms,
        max_rtt_ms,
    })
}

/// Network diagnosis helper
pub fn is_private_ip(ip: Ipv4Addr) -> bool {
    let octets = ip.octets();
    
    // 10.0.0.0/8, 172.16.0.0/12, 192.168.0.0/16, 127.0.0.0/8
    octets[0] == 10 ||
    (octets[0] == 172 && octets[1] >= 16 && octets[1] <= 31) ||
    (octets[0] == 192 && octets[1] == 168) ||
    octets[0] == 127
}

/// Provide network diagnosis suggestions
pub fn get_diagnosis_suggestions(target_ip: Ipv4Addr) -> Vec<String> {
    let mut suggestions = Vec::new();
    
    if is_private_ip(target_ip) {
        suggestions.push("目标是私有IP，检查本地网络配置".to_string());
        suggestions.push("确认网络接口配置正确".to_string());
        suggestions.push("检查交换机/路由器设置".to_string());
    } else {
        suggestions.push("目标是公网IP，检查网关配置".to_string());
        suggestions.push("确认DNS服务器设置".to_string());
        suggestions.push("检查防火墙规则".to_string());
    }
    
    suggestions.push("使用 ip route show 检查路由表".to_string());
    suggestions.push("使用 ip addr show 检查接口状态".to_string());
    
    suggestions
}
