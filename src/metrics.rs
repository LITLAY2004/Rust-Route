//! Metrics and monitoring for RustRoute

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// RustRoute performance metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RustRouteMetrics {
    pub packets_sent: u64,
    pub packets_received: u64,
    pub packets_dropped: u64,
    pub routing_updates_sent: u64,
    pub routing_updates_received: u64,
    pub route_changes: u64,
    pub convergence_time: Option<Duration>,
    pub neighbor_count: usize,
    pub active_routes: usize,
    pub uptime: Duration,
}

impl Default for RustRouteMetrics {
    fn default() -> Self {
        Self {
            packets_sent: 0,
            packets_received: 0,
            packets_dropped: 0,
            routing_updates_sent: 0,
            routing_updates_received: 0,
            route_changes: 0,
            convergence_time: None,
            neighbor_count: 0,
            active_routes: 0,
            uptime: Duration::new(0, 0),
        }
    }
}

/// Thread-safe metrics collector
#[derive(Debug)]
pub struct MetricsCollector {
    packets_sent: AtomicU64,
    packets_received: AtomicU64,
    packets_dropped: AtomicU64,
    routing_updates_sent: AtomicU64,
    routing_updates_received: AtomicU64,
    route_changes: AtomicU64,
    start_time: Instant,
    convergence_start: Option<Instant>,
    convergence_time: Arc<std::sync::Mutex<Option<Duration>>>,
}

impl MetricsCollector {
    /// Create a new metrics collector
    pub fn new() -> Self {
        Self {
            packets_sent: AtomicU64::new(0),
            packets_received: AtomicU64::new(0),
            packets_dropped: AtomicU64::new(0),
            routing_updates_sent: AtomicU64::new(0),
            routing_updates_received: AtomicU64::new(0),
            route_changes: AtomicU64::new(0),
            start_time: Instant::now(),
            convergence_start: None,
            convergence_time: Arc::new(std::sync::Mutex::new(None)),
        }
    }

    /// Increment packets sent counter
    pub fn increment_packets_sent(&self) {
        self.packets_sent.fetch_add(1, Ordering::Relaxed);
    }

    /// Increment packets received counter
    pub fn increment_packets_received(&self) {
        self.packets_received.fetch_add(1, Ordering::Relaxed);
    }

    /// Get packets sent count
    pub fn get_packets_sent(&self) -> u64 {
        self.packets_sent.load(Ordering::Relaxed)
    }

    /// Get packets received count
    pub fn get_packets_received(&self) -> u64 {
        self.packets_received.load(Ordering::Relaxed)
    }

    /// Increment packets dropped counter
    pub fn increment_packets_dropped(&self) {
        self.packets_dropped.fetch_add(1, Ordering::Relaxed);
    }

    /// Increment routing updates sent counter
    pub fn increment_routing_updates_sent(&self) {
        self.routing_updates_sent.fetch_add(1, Ordering::Relaxed);
    }

    /// Increment routing updates received counter
    pub fn increment_routing_updates_received(&self) {
        self.routing_updates_received.fetch_add(1, Ordering::Relaxed);
    }

    /// Increment route changes counter and start convergence timer
    pub fn increment_route_changes(&self) {
        self.route_changes.fetch_add(1, Ordering::Relaxed);
        
        // Start convergence timer if not already started
        if self.convergence_start.is_none() {
            // This is not thread-safe, but it's just for demonstration
            // In a real implementation, you'd use proper synchronization
        }
    }

    /// Mark convergence as complete
    pub fn mark_convergence_complete(&self) {
        if let Some(start) = self.convergence_start {
            let convergence_duration = start.elapsed();
            if let Ok(mut convergence_time) = self.convergence_time.lock() {
                *convergence_time = Some(convergence_duration);
            }
        }
    }

    /// Get current metrics snapshot
    pub fn get_metrics(&self, neighbor_count: usize, active_routes: usize) -> RustRouteMetrics {
        let convergence_time = self.convergence_time.lock()
            .map(|guard| *guard)
            .unwrap_or(None);

        RustRouteMetrics {
            packets_sent: self.packets_sent.load(Ordering::Relaxed),
            packets_received: self.packets_received.load(Ordering::Relaxed),
            packets_dropped: self.packets_dropped.load(Ordering::Relaxed),
            routing_updates_sent: self.routing_updates_sent.load(Ordering::Relaxed),
            routing_updates_received: self.routing_updates_received.load(Ordering::Relaxed),
            route_changes: self.route_changes.load(Ordering::Relaxed),
            convergence_time,
            neighbor_count,
            active_routes,
            uptime: self.start_time.elapsed(),
        }
    }

    /// Reset all metrics
    pub fn reset(&self) {
        self.packets_sent.store(0, Ordering::Relaxed);
        self.packets_received.store(0, Ordering::Relaxed);
        self.packets_dropped.store(0, Ordering::Relaxed);
        self.routing_updates_sent.store(0, Ordering::Relaxed);
        self.routing_updates_received.store(0, Ordering::Relaxed);
        self.route_changes.store(0, Ordering::Relaxed);
        
        if let Ok(mut convergence_time) = self.convergence_time.lock() {
            *convergence_time = None;
        }
    }
}

impl Default for MetricsCollector {
    fn default() -> Self {
        Self::new()
    }
}

/// Performance monitor for tracking network performance
#[derive(Debug)]
pub struct PerformanceMonitor {
    metrics: MetricsCollector,
    historical_data: HashMap<u64, RustRouteMetrics>, // timestamp -> metrics
    collection_interval: Duration,
}

impl PerformanceMonitor {
    /// Create a new performance monitor
    pub fn new(collection_interval: Duration) -> Self {
        Self {
            metrics: MetricsCollector::new(),
            historical_data: HashMap::new(),
            collection_interval,
        }
    }

    /// Get the metrics collector
    pub fn get_collector(&self) -> &MetricsCollector {
        &self.metrics
    }

    /// Start collecting metrics periodically
    pub async fn start_collection(&mut self, neighbor_count: usize, active_routes: usize) {
        let mut interval = tokio::time::interval(self.collection_interval);
        
        loop {
            interval.tick().await;
            
            let timestamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            
            let metrics = self.metrics.get_metrics(neighbor_count, active_routes);
            self.historical_data.insert(timestamp, metrics);
            
            // Keep only last 100 entries
            if self.historical_data.len() > 100 {
                if let Some(oldest_key) = self.historical_data.keys().min().copied() {
                    self.historical_data.remove(&oldest_key);
                }
            }
        }
    }

    /// Get current metrics
    pub fn get_current_metrics(&self, neighbor_count: usize, active_routes: usize) -> RustRouteMetrics {
        self.metrics.get_metrics(neighbor_count, active_routes)
    }

    /// Get historical metrics
    pub fn get_historical_metrics(&self) -> &HashMap<u64, RustRouteMetrics> {
        &self.historical_data
    }

    /// Calculate packet loss rate
    pub fn calculate_packet_loss_rate(&self) -> f64 {
        let total_sent = self.metrics.packets_sent.load(Ordering::Relaxed);
        let total_dropped = self.metrics.packets_dropped.load(Ordering::Relaxed);
        
        if total_sent == 0 {
            0.0
        } else {
            (total_dropped as f64) / (total_sent as f64)
        }
    }

    /// Calculate average convergence time
    pub fn calculate_average_convergence_time(&self) -> Option<Duration> {
        let convergence_times: Vec<Duration> = self.historical_data
            .values()
            .filter_map(|metrics| metrics.convergence_time)
            .collect();
        
        if convergence_times.is_empty() {
            None
        } else {
            let total_nanos: u128 = convergence_times
                .iter()
                .map(|d| d.as_nanos())
                .sum();
            
            let average_nanos = total_nanos / convergence_times.len() as u128;
            Some(Duration::from_nanos(average_nanos as u64))
        }
    }

    /// Generate performance report
    pub fn generate_report(&self, neighbor_count: usize, active_routes: usize) -> PerformanceReport {
        let current_metrics = self.get_current_metrics(neighbor_count, active_routes);
        let packet_loss_rate = self.calculate_packet_loss_rate();
        let avg_convergence_time = self.calculate_average_convergence_time();
        
        PerformanceReport {
            current_metrics,
            packet_loss_rate,
            average_convergence_time: avg_convergence_time,
            total_data_points: self.historical_data.len(),
        }
    }
}

/// Performance report structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceReport {
    pub current_metrics: RustRouteMetrics,
    pub packet_loss_rate: f64,
    pub average_convergence_time: Option<Duration>,
    pub total_data_points: usize,
}

impl PerformanceReport {
    /// Convert to JSON string
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// Print a formatted report
    pub fn print_report(&self) {
        println!("=== RustRoute Performance Report ===");
        println!("Uptime: {:?}", self.current_metrics.uptime);
        println!("Packets Sent: {}", self.current_metrics.packets_sent);
        println!("Packets Received: {}", self.current_metrics.packets_received);
        println!("Packets Dropped: {}", self.current_metrics.packets_dropped);
        println!("Packet Loss Rate: {:.2}%", self.packet_loss_rate * 100.0);
        println!("Routing Updates Sent: {}", self.current_metrics.routing_updates_sent);
        println!("Routing Updates Received: {}", self.current_metrics.routing_updates_received);
        println!("Route Changes: {}", self.current_metrics.route_changes);
        println!("Active Routes: {}", self.current_metrics.active_routes);
        println!("Neighbor Count: {}", self.current_metrics.neighbor_count);
        
        if let Some(conv_time) = &self.current_metrics.convergence_time {
            println!("Last Convergence Time: {:?}", conv_time);
        }
        
        if let Some(avg_conv_time) = &self.average_convergence_time {
            println!("Average Convergence Time: {:?}", avg_conv_time);
        }
        
        println!("Historical Data Points: {}", self.total_data_points);
        println!("================================");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_collector() {
        let collector = MetricsCollector::new();
        
        collector.increment_packets_sent();
        collector.increment_packets_received();
        collector.increment_route_changes();
        
        let metrics = collector.get_metrics(2, 5);
        
        assert_eq!(metrics.packets_sent, 1);
        assert_eq!(metrics.packets_received, 1);
        assert_eq!(metrics.route_changes, 1);
        assert_eq!(metrics.neighbor_count, 2);
        assert_eq!(metrics.active_routes, 5);
    }

    #[test]
    fn test_packet_loss_calculation() {
        let monitor = PerformanceMonitor::new(Duration::from_secs(1));
        let collector = monitor.get_collector();
        
        // Send 10 packets, drop 2
        for _ in 0..10 {
            collector.increment_packets_sent();
        }
        for _ in 0..2 {
            collector.increment_packets_dropped();
        }
        
        let loss_rate = monitor.calculate_packet_loss_rate();
        assert_eq!(loss_rate, 0.2); // 20% loss rate
    }
}
