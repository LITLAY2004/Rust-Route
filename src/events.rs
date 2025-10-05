use chrono::{DateTime, Utc};
use serde::Serialize;
use std::net::Ipv4Addr;
use tokio::sync::broadcast;

use crate::metrics::MetricsSnapshot;
use crate::routing_table::RouteSource;

#[derive(Clone)]
pub struct EventBus {
    sender: broadcast::Sender<WebEvent>,
}

impl EventBus {
    pub fn new(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity);
        Self { sender }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<WebEvent> {
        self.sender.subscribe()
    }

    pub fn publish(&self, event: WebEvent) {
        let _ = self.sender.send(event);
    }

    pub fn publish_activity<S: Into<String>>(&self, level: ActivityLevel, message: S) {
        let event = WebEvent::Activity(ActivityEvent {
            level,
            message: message.into(),
            timestamp: Utc::now(),
        });
        self.publish(event);
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", content = "data")]
pub enum WebEvent {
    Metrics(MetricsEvent),
    Route(RouteEvent),
    Activity(ActivityEvent),
}

#[derive(Debug, Clone, Serialize)]
pub struct MetricsEvent {
    pub snapshot: MetricsSnapshot,
}

#[derive(Debug, Clone, Serialize)]
pub struct RouteEvent {
    pub destination: String,
    pub subnet_mask: String,
    pub metric: u32,
    pub interface: String,
    pub source: RouteSource,
    pub next_hop: String,
}

impl RouteEvent {
    pub fn from_parts(
        destination: Ipv4Addr,
        subnet_mask: Ipv4Addr,
        next_hop: Ipv4Addr,
        metric: u32,
        interface: String,
        source: RouteSource,
    ) -> Self {
        Self {
            destination: destination.to_string(),
            subnet_mask: subnet_mask.to_string(),
            next_hop: next_hop.to_string(),
            metric,
            interface,
            source,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ActivityEvent {
    pub level: ActivityLevel,
    pub message: String,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
pub enum ActivityLevel {
    Info,
    Warn,
    Error,
}
