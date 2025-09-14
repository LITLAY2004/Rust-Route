use axum::{
    extract::{Path, Query, State},
    http::{header, StatusCode},
    response::{Html, Json},
    routing::{get, post, put, delete},
    Router,
};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, sync::Arc};
use tokio::sync::RwLock;
use tower_http::{cors::CorsLayer, services::ServeDir};

use crate::{
    router::Router as RipRouter,
    routing_table::{Route, RoutingTable},
    metrics::Metrics,
};

/// Web interface configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebConfig {
    pub enabled: bool,
    pub bind_address: String,
    pub port: u16,
    pub auth_enabled: bool,
    pub admin_username: String,
    pub admin_password_hash: String,
}

impl Default for WebConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            bind_address: "127.0.0.1".to_string(),
            port: 8080,
            auth_enabled: false,
            admin_username: "admin".to_string(),
            admin_password_hash: "$2b$12$dummy.hash.for.default.config".to_string(),
        }
    }
}

/// Shared application state
#[derive(Clone)]
pub struct AppState {
    pub router: Arc<RwLock<RipRouter>>,
    pub routing_table: Arc<RwLock<RoutingTable>>,
    pub metrics: Arc<RwLock<Metrics>>,
    pub config: Arc<RwLock<WebConfig>>,
}

/// API response wrapper
#[derive(Serialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub message: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

impl<T> ApiResponse<T> {
    pub fn success(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            message: "Success".to_string(),
            timestamp: chrono::Utc::now(),
        }
    }

    pub fn error(message: String) -> Self {
        Self {
            success: false,
            data: None,
            message,
            timestamp: chrono::Utc::now(),
        }
    }
}

/// Route information for API
#[derive(Debug, Serialize, Deserialize)]
pub struct RouteInfo {
    pub destination: String,
    pub next_hop: String,
    pub metric: u32,
    pub interface: String,
    pub age: u64,
    pub learned_from: String,
}

/// System status information
#[derive(Debug, Serialize, Deserialize)]
pub struct SystemStatus {
    pub uptime: u64,
    pub version: String,
    pub router_id: String,
    pub interfaces: Vec<InterfaceInfo>,
    pub route_count: usize,
    pub memory_usage: u64,
    pub cpu_usage: f32,
}

/// Network interface information
#[derive(Debug, Serialize, Deserialize)]
pub struct InterfaceInfo {
    pub name: String,
    pub address: String,
    pub status: String,
    pub packets_sent: u64,
    pub packets_received: u64,
    pub bytes_sent: u64,
    pub bytes_received: u64,
}

/// Configuration update request
#[derive(Debug, Deserialize)]
pub struct ConfigUpdateRequest {
    pub field: String,
    pub value: serde_json::Value,
}

/// Web server implementation
pub struct WebServer {
    state: AppState,
    config: WebConfig,
}

impl WebServer {
    pub fn new(
        router: Arc<RwLock<RipRouter>>,
        routing_table: Arc<RwLock<RoutingTable>>,
        metrics: Arc<RwLock<Metrics>>,
        config: WebConfig,
    ) -> Self {
        let state = AppState {
            router,
            routing_table,
            metrics,
            config: Arc::new(RwLock::new(config.clone())),
        };

        Self { state, config }
    }

    pub async fn start(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let app = self.create_app().await;
        
        let bind_addr = format!("{}:{}", self.config.bind_address, self.config.port);
        log::info!("🌐 Starting web interface on http://{}", bind_addr);
        
        let listener = tokio::net::TcpListener::bind(&bind_addr).await?;
        axum::serve(listener, app).await?;
        
        Ok(())
    }

    async fn create_app(&self) -> Router {
        Router::new()
            // Static files
            .nest_service("/static", ServeDir::new("web/static"))
            
            // Web interface routes
            .route("/", get(dashboard_handler))
            .route("/dashboard", get(dashboard_handler))
            .route("/routes", get(routes_page_handler))
            .route("/config", get(config_page_handler))
            .route("/metrics", get(metrics_page_handler))
            
            // API routes
            .route("/api/status", get(get_system_status))
            .route("/api/routes", get(get_routes))
            .route("/api/routes/:destination", delete(delete_route))
            .route("/api/interfaces", get(get_interfaces))
            .route("/api/metrics", get(get_metrics))
            .route("/api/config", get(get_config))
            .route("/api/config", put(update_config))
            .route("/api/router/restart", post(restart_router))
            
            // WebSocket for real-time updates
            .route("/ws", get(websocket_handler))
            
            .layer(CorsLayer::permissive())
            .with_state(self.state.clone())
    }
}

// Handler functions

async fn dashboard_handler() -> Html<&'static str> {
    Html(include_str!("../web/templates/dashboard.html"))
}

async fn routes_page_handler() -> Html<&'static str> {
    Html(include_str!("../web/templates/routes.html"))
}

async fn config_page_handler() -> Html<&'static str> {
    Html(include_str!("../web/templates/config.html"))
}

async fn metrics_page_handler() -> Html<&'static str> {
    Html(include_str!("../web/templates/metrics.html"))
}

async fn get_system_status(
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<SystemStatus>>, StatusCode> {
    let router = state.router.read().await;
    let metrics = state.metrics.read().await;
    
    let status = SystemStatus {
        uptime: metrics.uptime_seconds(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        router_id: router.router_id().to_string(),
        interfaces: get_interface_info(&router).await,
        route_count: router.routing_table().route_count(),
        memory_usage: get_memory_usage(),
        cpu_usage: get_cpu_usage(),
    };
    
    Ok(Json(ApiResponse::success(status)))
}

async fn get_routes(
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<Vec<RouteInfo>>>, StatusCode> {
    let routing_table = state.routing_table.read().await;
    let routes = routing_table.get_all_routes()
        .into_iter()
        .map(|route| RouteInfo {
            destination: route.destination.to_string(),
            next_hop: route.next_hop.to_string(),
            metric: route.metric,
            interface: route.interface.clone(),
            age: route.age_seconds(),
            learned_from: route.learned_from.to_string(),
        })
        .collect();
    
    Ok(Json(ApiResponse::success(routes)))
}

async fn delete_route(
    Path(destination): Path<String>,
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<()>>, StatusCode> {
    let mut routing_table = state.routing_table.write().await;
    
    match destination.parse() {
        Ok(dest_ip) => {
            routing_table.remove_route(&dest_ip);
            Ok(Json(ApiResponse::success(())))
        }
        Err(_) => Ok(Json(ApiResponse::error("Invalid destination IP".to_string()))),
    }
}

async fn get_interfaces(
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<Vec<InterfaceInfo>>>, StatusCode> {
    let router = state.router.read().await;
    let interfaces = get_interface_info(&router).await;
    Ok(Json(ApiResponse::success(interfaces)))
}

async fn get_metrics(
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<serde_json::Value>>, StatusCode> {
    let metrics = state.metrics.read().await;
    let metrics_json = serde_json::to_value(&*metrics)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(ApiResponse::success(metrics_json)))
}

async fn get_config(
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<serde_json::Value>>, StatusCode> {
    let router = state.router.read().await;
    let config_json = serde_json::to_value(router.config())
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(ApiResponse::success(config_json)))
}

async fn update_config(
    State(state): State<AppState>,
    Json(request): Json<ConfigUpdateRequest>,
) -> Result<Json<ApiResponse<()>>, StatusCode> {
    let mut router = state.router.write().await;
    
    match router.update_config_field(&request.field, request.value) {
        Ok(_) => Ok(Json(ApiResponse::success(()))),
        Err(e) => Ok(Json(ApiResponse::error(format!("Config update failed: {}", e)))),
    }
}

async fn restart_router(
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<()>>, StatusCode> {
    let mut router = state.router.write().await;
    
    match router.restart().await {
        Ok(_) => Ok(Json(ApiResponse::success(()))),
        Err(e) => Ok(Json(ApiResponse::error(format!("Restart failed: {}", e)))),
    }
}

async fn websocket_handler() -> &'static str {
    "WebSocket endpoint - implementation needed"
}

// Helper functions

async fn get_interface_info(_router: &RipRouter) -> Vec<InterfaceInfo> {
    // Implementation would get real interface information
    vec![
        InterfaceInfo {
            name: "eth0".to_string(),
            address: "192.168.1.1".to_string(),
            status: "UP".to_string(),
            packets_sent: 1234,
            packets_received: 5678,
            bytes_sent: 1234567,
            bytes_received: 7654321,
        }
    ]
}

fn get_memory_usage() -> u64 {
    // Implementation would get real memory usage
    1024 * 1024 * 64 // 64MB placeholder
}

fn get_cpu_usage() -> f32 {
    // Implementation would get real CPU usage
    15.5 // 15.5% placeholder
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use tokio::sync::RwLock;

    #[tokio::test]
    async fn test_web_config_default() {
        let config = WebConfig::default();
        assert_eq!(config.port, 8080);
        assert_eq!(config.bind_address, "127.0.0.1");
        assert!(config.enabled);
    }

    #[tokio::test]
    async fn test_api_response_success() {
        let response = ApiResponse::success("test data");
        assert!(response.success);
        assert_eq!(response.data.unwrap(), "test data");
    }

    #[tokio::test]
    async fn test_api_response_error() {
        let response: ApiResponse<()> = ApiResponse::error("test error".to_string());
        assert!(!response.success);
        assert_eq!(response.message, "test error");
    }
}
