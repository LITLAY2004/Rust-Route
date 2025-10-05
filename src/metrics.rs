//! Metrics and monitoring for RustRoute

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Snapshot of router metrics that can be serialized and exposed via the API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsSnapshot {
    pub packets_sent: u64,
    pub packets_received: u64,
    pub packets_dropped: u64,
    pub routing_updates_sent: u64,
    pub routing_updates_received: u64,
    pub route_changes: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub convergence_time_seconds: Option<u64>,
    pub neighbor_count: usize,
    pub active_routes: usize,
    pub uptime_seconds: u64,
    pub route_count: u64,
    pub config_version: u32,
}

impl Default for MetricsSnapshot {
    fn default() -> Self {
        Self {
            packets_sent: 0,
            packets_received: 0,
            packets_dropped: 0,
            routing_updates_sent: 0,
            routing_updates_received: 0,
            route_changes: 0,
            convergence_time_seconds: None,
            neighbor_count: 0,
            active_routes: 0,
            uptime_seconds: 0,
            route_count: 0,
            config_version: 0,
        }
    }
}

#[derive(Debug)]
struct MetricsCollector {
    packets_sent: AtomicU64,
    packets_received: AtomicU64,
    packets_dropped: AtomicU64,
    routing_updates_sent: AtomicU64,
    routing_updates_received: AtomicU64,
    route_changes: AtomicU64,
    convergence_start: Mutex<Option<Instant>>,
    convergence_time: Mutex<Option<Duration>>,
}

impl MetricsCollector {
    fn new() -> Self {
        Self {
            packets_sent: AtomicU64::new(0),
            packets_received: AtomicU64::new(0),
            packets_dropped: AtomicU64::new(0),
            routing_updates_sent: AtomicU64::new(0),
            routing_updates_received: AtomicU64::new(0),
            route_changes: AtomicU64::new(0),
            convergence_start: Mutex::new(None),
            convergence_time: Mutex::new(None),
        }
    }

    fn increment_packets_sent(&self) {
        self.packets_sent.fetch_add(1, Ordering::Relaxed);
    }

    fn increment_packets_received(&self) {
        self.packets_received.fetch_add(1, Ordering::Relaxed);
    }

    fn increment_packets_dropped(&self) {
        self.packets_dropped.fetch_add(1, Ordering::Relaxed);
    }

    fn increment_routing_updates_sent(&self) {
        self.routing_updates_sent.fetch_add(1, Ordering::Relaxed);
    }

    fn increment_routing_updates_received(&self) {
        self.routing_updates_received
            .fetch_add(1, Ordering::Relaxed);
    }

    fn increment_route_changes(&self) {
        self.route_changes.fetch_add(1, Ordering::Relaxed);

        let mut start_guard = self.convergence_start.lock().expect("lock poisoned");
        if start_guard.is_none() {
            *start_guard = Some(Instant::now());
        }
    }

    fn mark_convergence_complete(&self) {
        let mut start_guard = self.convergence_start.lock().expect("lock poisoned");
        if let Some(start) = *start_guard {
            let elapsed = start.elapsed();
            let mut time_guard = self.convergence_time.lock().expect("lock poisoned");
            *time_guard = Some(elapsed);
            *start_guard = None;
        }
    }

    fn reset(&self) {
        self.packets_sent.store(0, Ordering::Relaxed);
        self.packets_received.store(0, Ordering::Relaxed);
        self.packets_dropped.store(0, Ordering::Relaxed);
        self.routing_updates_sent.store(0, Ordering::Relaxed);
        self.routing_updates_received.store(0, Ordering::Relaxed);
        self.route_changes.store(0, Ordering::Relaxed);
        *self.convergence_start.lock().expect("lock poisoned") = None;
        *self.convergence_time.lock().expect("lock poisoned") = None;
    }

    fn snapshot(&self, neighbor_count: usize, active_routes: usize) -> MetricsSnapshot {
        let convergence_seconds = self
            .convergence_time
            .lock()
            .expect("lock poisoned")
            .map(|d| d.as_secs());

        MetricsSnapshot {
            packets_sent: self.packets_sent.load(Ordering::Relaxed),
            packets_received: self.packets_received.load(Ordering::Relaxed),
            packets_dropped: self.packets_dropped.load(Ordering::Relaxed),
            routing_updates_sent: self.routing_updates_sent.load(Ordering::Relaxed),
            routing_updates_received: self.routing_updates_received.load(Ordering::Relaxed),
            route_changes: self.route_changes.load(Ordering::Relaxed),
            convergence_time_seconds: convergence_seconds,
            neighbor_count,
            active_routes,
            ..MetricsSnapshot::default()
        }
    }
}

#[derive(Debug)]
struct MetricsInner {
    collector: MetricsCollector,
    route_count: AtomicU64,
    config_version: AtomicU32,
    start_time: Mutex<Instant>,
}

impl MetricsInner {
    fn uptime_seconds(&self) -> u64 {
        self.start_time
            .lock()
            .expect("lock poisoned")
            .elapsed()
            .as_secs()
    }

    fn reset(&self) {
        self.collector.reset();
        self.route_count.store(0, Ordering::Relaxed);
        *self.start_time.lock().expect("lock poisoned") = Instant::now();
    }
}

/// Thread-safe metrics facade used across the application
#[derive(Clone, Debug)]
pub struct Metrics {
    inner: Arc<MetricsInner>,
}

impl Metrics {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(MetricsInner {
                collector: MetricsCollector::new(),
                route_count: AtomicU64::new(0),
                config_version: AtomicU32::new(1),
                start_time: Mutex::new(Instant::now()),
            }),
        }
    }

    pub fn increment_packets_sent(&self) {
        self.inner.collector.increment_packets_sent();
    }

    pub fn increment_packets_received(&self) {
        self.inner.collector.increment_packets_received();
    }

    pub fn increment_packets_dropped(&self) {
        self.inner.collector.increment_packets_dropped();
    }

    pub fn increment_routing_updates_sent(&self) {
        self.inner.collector.increment_routing_updates_sent();
    }

    pub fn increment_routing_updates_received(&self) {
        self.inner.collector.increment_routing_updates_received();
    }

    pub fn increment_route_changes(&self) {
        self.inner.collector.increment_route_changes();
    }

    pub fn mark_convergence_complete(&self) {
        self.inner.collector.mark_convergence_complete();
    }

    pub fn update_route_count(&self, route_count: usize) {
        self.inner
            .route_count
            .store(route_count as u64, Ordering::Relaxed);
    }

    pub fn set_config_version(&self, version: u32) {
        self.inner.config_version.store(version, Ordering::Relaxed);
    }

    pub fn uptime_seconds(&self) -> u64 {
        self.inner.uptime_seconds()
    }

    pub fn reset(&self) {
        self.inner.reset();
    }

    pub fn snapshot(&self, neighbor_count: usize, active_routes: usize) -> MetricsSnapshot {
        let mut snapshot = self.inner.collector.snapshot(neighbor_count, active_routes);

        snapshot.route_count = self.inner.route_count.load(Ordering::Relaxed);
        snapshot.config_version = self.inner.config_version.load(Ordering::Relaxed);
        snapshot.uptime_seconds = self.uptime_seconds();
        snapshot
    }
}

/// Performance monitor for recording historical metrics
#[derive(Debug)]
pub struct PerformanceMonitor {
    metrics: Metrics,
    historical_data: Mutex<HashMap<u64, MetricsSnapshot>>, // timestamp -> metrics
    collection_interval: Duration,
}

impl PerformanceMonitor {
    pub fn new(collection_interval: Duration) -> Self {
        Self {
            metrics: Metrics::new(),
            historical_data: Mutex::new(HashMap::new()),
            collection_interval,
        }
    }

    pub fn metrics(&self) -> Metrics {
        self.metrics.clone()
    }

    pub async fn start_collection(&self, neighbor_count: usize, active_routes: usize) {
        let mut interval = tokio::time::interval(self.collection_interval);

        loop {
            interval.tick().await;

            let snapshot = self.metrics.snapshot(neighbor_count, active_routes);
            let timestamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();

            let mut history = self.historical_data.lock().expect("lock poisoned");
            history.insert(timestamp, snapshot);

            // Keep last 100 entries
            if history.len() > 100 {
                if let Some(oldest_key) = history.keys().min().cloned() {
                    history.remove(&oldest_key);
                }
            }
        }
    }

    pub fn historical_data(&self) -> HashMap<u64, MetricsSnapshot> {
        self.historical_data.lock().expect("lock poisoned").clone()
    }

    pub fn packet_loss_rate(&self) -> f64 {
        let snapshot = self.metrics.snapshot(0, 0);
        if snapshot.packets_sent == 0 {
            0.0
        } else {
            snapshot.packets_dropped as f64 / snapshot.packets_sent as f64
        }
    }

    pub fn average_convergence_time(&self) -> Option<u64> {
        let history = self.historical_data.lock().expect("lock poisoned");
        let times: Vec<u64> = history
            .values()
            .filter_map(|entry| entry.convergence_time_seconds)
            .collect();

        if times.is_empty() {
            None
        } else {
            Some(times.iter().sum::<u64>() / times.len() as u64)
        }
    }

    pub fn generate_report(
        &self,
        neighbor_count: usize,
        active_routes: usize,
    ) -> PerformanceReport {
        let current_metrics = self.metrics.snapshot(neighbor_count, active_routes);

        PerformanceReport {
            current_metrics: current_metrics.clone(),
            packet_loss_rate: self.packet_loss_rate(),
            average_convergence_time_seconds: self.average_convergence_time(),
            total_data_points: self.historical_data.lock().expect("lock poisoned").len(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceReport {
    pub current_metrics: MetricsSnapshot,
    pub packet_loss_rate: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub average_convergence_time_seconds: Option<u64>,
    pub total_data_points: usize,
}

impl PerformanceReport {
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    pub fn print_report(&self) {
        println!("=== RustRoute Performance Report ===");
        println!("Uptime: {}s", self.current_metrics.uptime_seconds);
        println!("Packets Sent: {}", self.current_metrics.packets_sent);
        println!(
            "Packets Received: {}",
            self.current_metrics.packets_received
        );
        println!("Packets Dropped: {}", self.current_metrics.packets_dropped);
        println!("Packet Loss Rate: {:.2}%", self.packet_loss_rate * 100.0);
        println!(
            "Routing Updates Sent: {}",
            self.current_metrics.routing_updates_sent
        );
        println!(
            "Routing Updates Received: {}",
            self.current_metrics.routing_updates_received
        );
        println!("Route Changes: {}", self.current_metrics.route_changes);
        println!("Active Routes: {}", self.current_metrics.active_routes);
        println!("Neighbor Count: {}", self.current_metrics.neighbor_count);

        if let Some(conv) = self.current_metrics.convergence_time_seconds {
            println!("Last Convergence Time: {}s", conv);
        }

        if let Some(avg) = self.average_convergence_time_seconds {
            println!("Average Convergence Time: {}s", avg);
        }

        println!("Historical Data Points: {}", self.total_data_points);
        println!("================================");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metrics_counters_increment() {
        let metrics = Metrics::new();
        metrics.increment_packets_sent();
        metrics.increment_packets_received();
        metrics.increment_route_changes();
        metrics.update_route_count(3);
        metrics.set_config_version(7);

        let snapshot = metrics.snapshot(2, 5);
        assert_eq!(snapshot.packets_sent, 1);
        assert_eq!(snapshot.packets_received, 1);
        assert_eq!(snapshot.route_changes, 1);
        assert_eq!(snapshot.neighbor_count, 2);
        assert_eq!(snapshot.active_routes, 5);
        assert_eq!(snapshot.route_count, 3);
        assert_eq!(snapshot.config_version, 7);
    }

    #[test]
    fn packet_loss_rate_calculation() {
        let monitor = PerformanceMonitor::new(Duration::from_secs(1));
        let metrics = monitor.metrics();

        for _ in 0..10 {
            metrics.increment_packets_sent();
        }
        for _ in 0..2 {
            metrics.increment_packets_dropped();
        }

        let loss = monitor.packet_loss_rate();
        assert!((loss - 0.2).abs() < f64::EPSILON);
    }
}
