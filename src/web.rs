use async_stream::stream;
use axum::response::sse::{self, KeepAlive};
use axum::{
    extract::{Path, Query, State},
    http::{header, HeaderMap, StatusCode},
    response::{Html, Json, Sse},
    routing::{delete, get, post, put},
    Router as AxumRouter,
};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, convert::Infallible, net::Ipv4Addr, sync::Arc, time::Duration};
use tokio::sync::{broadcast::error::RecvError, Mutex, RwLock};
use tower_http::{cors::CorsLayer, services::ServeDir};

use crate::{
    auth::{require_permission, AuthError, AuthManager, LoginRequest, LoginResponse, UserRole},
    config_manager::{
        ConfigDiff, ConfigHistoryEntry, ConfigManager, InterfaceConfig, RouterConfig,
    },
    events::{ActivityLevel, EventBus},
    metrics::{Metrics, MetricsSnapshot},
    router::{Router, RouterStatistics},
    routing_table::{RouteSource, RoutingTable},
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
    #[serde(default = "default_static_dir")]
    pub static_dir: String,
}

fn default_static_dir() -> String {
    "web/static".to_string()
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
            static_dir: default_static_dir(),
        }
    }
}

/// Shared application state
#[derive(Clone)]
pub struct AppState {
    pub router: Arc<RwLock<Router>>,
    pub routing_table: Arc<RwLock<RoutingTable>>,
    pub metrics: Metrics,
    pub config_manager: Arc<ConfigManager>,
    pub events: EventBus,
    pub auth: Arc<Mutex<Option<AuthManager>>>,
}

#[derive(Debug, Serialize)]
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

#[derive(Debug, Serialize, Deserialize)]
pub struct RouteInfo {
    pub destination: String,
    pub subnet_mask: String,
    pub next_hop: String,
    pub metric: u32,
    pub interface: String,
    pub age_seconds: u64,
    pub source: RouteSource,
    pub learned_from: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SystemStatus {
    pub uptime_seconds: u64,
    pub version: String,
    pub router_id: String,
    pub interfaces: Vec<InterfaceInfo>,
    pub route_count: usize,
    pub metrics: MetricsSnapshot,
    pub router_stats: RouterStatistics,
    pub cpu_usage: f32,
    pub memory_usage: u64,
    pub auth_required: bool,
}

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

#[derive(Debug, Deserialize)]
struct EventStreamParams {
    token: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateRouteRequest {
    pub destination: String,
    pub mask: String,
    pub next_hop: String,
    pub metric: Option<u32>,
    pub interface: String,
}

#[derive(Debug, Deserialize)]
pub struct DeleteRouteParams {
    pub destination: String,
    pub mask: String,
}

#[derive(Debug, Deserialize)]
struct ConfigVersionPath {
    version: u32,
}

pub struct WebServer {
    state: AppState,
    config: WebConfig,
}

impl WebServer {
    pub fn new(
        router: Arc<RwLock<Router>>,
        routing_table: Arc<RwLock<RoutingTable>>,
        metrics: Metrics,
        config_manager: Arc<ConfigManager>,
        config: WebConfig,
        events: EventBus,
        auth: Arc<Mutex<Option<AuthManager>>>,
    ) -> Self {
        let state = AppState {
            router,
            routing_table,
            metrics,
            config_manager,
            events,
            auth,
        };

        Self { state, config }
    }

    pub async fn start(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if !self.config.enabled {
            log::warn!("Web interface disabled in configuration");
            return Ok(());
        }

        let app = self.create_app();
        let bind_addr = format!("{}:{}", self.config.bind_address, self.config.port);
        log::info!("🌐 Starting web interface on http://{}", bind_addr);

        let listener = tokio::net::TcpListener::bind(&bind_addr).await?;
        axum::serve(listener, app).await?;
        Ok(())
    }

    fn create_app(&self) -> AxumRouter {
        AxumRouter::new()
            .nest_service("/static", ServeDir::new(&self.config.static_dir))
            .route("/", get(dashboard_handler))
            .route("/dashboard", get(dashboard_handler))
            .route("/routes", get(routes_page_handler))
            .route("/config", get(config_page_handler))
            .route("/metrics", get(metrics_page_handler))
            .route("/api/status", get(get_system_status))
            .route("/api/auth/login", post(login))
            .route("/api/auth/logout", post(logout))
            .route("/api/events", get(events_stream))
            .route("/api/routes", get(get_routes))
            .route("/api/routes", post(create_route))
            .route("/api/routes/:destination/:mask", delete(delete_route))
            .route("/api/interfaces", get(get_interfaces))
            .route("/api/metrics", get(get_metrics))
            .route("/api/config", get(get_config))
            .route("/api/config", put(update_config))
            .route("/api/config/history", get(get_config_history))
            .route("/api/config/history/:version/diff", get(get_config_diff))
            .route(
                "/api/config/history/:version/rollback",
                post(rollback_config),
            )
            .route("/api/router/restart", post(restart_router))
            .layer(CorsLayer::permissive())
            .with_state(self.state.clone())
    }
}

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

async fn events_stream(
    State(state): State<AppState>,
    Query(params): Query<EventStreamParams>,
) -> Result<Sse<impl futures_core::Stream<Item = Result<sse::Event, Infallible>>>, StatusCode> {
    ensure_permission(&state, None, params.token.clone(), Some(UserRole::ReadOnly)).await?;
    let mut receiver = state.events.subscribe();
    let stream = stream! {
        loop {
            match receiver.recv().await {
                Ok(event) => match serde_json::to_string(&event) {
                    Ok(payload) => yield Ok(sse::Event::default().data(payload)),
                    Err(err) => {
                        log::error!("Failed to serialize event: {}", err);
                    }
                },
                Err(RecvError::Lagged(skipped)) => {
                    log::warn!("Event stream lagged; skipped {} messages", skipped);
                }
                Err(RecvError::Closed) => {
                    break;
                }
            }
        }
    };

    Ok(Sse::new(stream).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("keepalive"),
    ))
}

async fn get_system_status(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<ApiResponse<SystemStatus>>, StatusCode> {
    ensure_permission(&state, Some(&headers), None, Some(UserRole::ReadOnly)).await?;
    let router_stats = {
        let router_guard = state.router.read().await;
        router_guard.statistics().await
    };

    let metrics_snapshot = state
        .metrics
        .snapshot(router_stats.neighbor_count, router_stats.route_count);

    let config = {
        let router_guard = state.router.read().await;
        router_guard.config_snapshot()
    };

    let interfaces = collect_interface_info(&config.interfaces).await;
    let cpu_usage = cpu_usage_percent().await;

    let auth_required = config.auth.enabled && config.web.auth_enabled;
    let memory_usage = router_stats.memory_usage;

    let status = SystemStatus {
        uptime_seconds: metrics_snapshot.uptime_seconds,
        version: env!("CARGO_PKG_VERSION").to_string(),
        router_id: config.router_id.clone(),
        interfaces,
        route_count: router_stats.route_count,
        metrics: metrics_snapshot,
        router_stats,
        cpu_usage,
        memory_usage,
        auth_required,
    };

    Ok(Json(ApiResponse::success(status)))
}

async fn get_routes(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<ApiResponse<Vec<RouteInfo>>>, StatusCode> {
    ensure_permission(&state, Some(&headers), None, Some(UserRole::ReadOnly)).await?;
    let routing_table = state.routing_table.read().await;
    let routes = routing_table
        .snapshot()
        .into_iter()
        .map(|entry| RouteInfo {
            destination: entry.destination,
            subnet_mask: entry.subnet_mask,
            next_hop: entry.next_hop,
            metric: entry.metric,
            interface: entry.interface,
            age_seconds: entry.age_seconds,
            source: entry.source,
            learned_from: entry.learned_from,
        })
        .collect();

    Ok(Json(ApiResponse::success(routes)))
}

async fn create_route(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<CreateRouteRequest>,
) -> Result<Json<ApiResponse<()>>, StatusCode> {
    ensure_permission(&state, Some(&headers), None, Some(UserRole::Operator)).await?;
    let destination: Ipv4Addr = request
        .destination
        .parse()
        .map_err(|_| StatusCode::BAD_REQUEST)?;
    let mask: Ipv4Addr = request.mask.parse().map_err(|_| StatusCode::BAD_REQUEST)?;
    let next_hop: Ipv4Addr = request
        .next_hop
        .parse()
        .map_err(|_| StatusCode::BAD_REQUEST)?;
    let metric = request.metric.unwrap_or(1).max(1);

    let mut table = state.routing_table.write().await;
    table.add_static_route(destination, mask, next_hop, metric, request.interface);
    drop(table);

    let count = state.routing_table.read().await.route_count();
    state.metrics.update_route_count(count);

    Ok(Json(ApiResponse::success(())))
}

async fn delete_route(
    Path(params): Path<DeleteRouteParams>,
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<ApiResponse<()>>, StatusCode> {
    ensure_permission(&state, Some(&headers), None, Some(UserRole::Operator)).await?;
    let destination: Ipv4Addr = params
        .destination
        .parse()
        .map_err(|_| StatusCode::BAD_REQUEST)?;
    let mask: Ipv4Addr = params.mask.parse().map_err(|_| StatusCode::BAD_REQUEST)?;

    let mut table = state.routing_table.write().await;
    if !table.remove_route(destination, mask) {
        return Ok(Json(ApiResponse::error("Route not found".to_string())));
    }
    drop(table);

    let count = state.routing_table.read().await.route_count();
    state.metrics.update_route_count(count);

    Ok(Json(ApiResponse::success(())))
}

async fn get_interfaces(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<ApiResponse<Vec<InterfaceInfo>>>, StatusCode> {
    ensure_permission(&state, Some(&headers), None, Some(UserRole::ReadOnly)).await?;
    let config = {
        let router = state.router.read().await;
        router.config_snapshot()
    };

    let interfaces = collect_interface_info(&config.interfaces).await;
    Ok(Json(ApiResponse::success(interfaces)))
}

async fn get_metrics(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<ApiResponse<MetricsSnapshot>>, StatusCode> {
    ensure_permission(&state, Some(&headers), None, Some(UserRole::ReadOnly)).await?;
    let table_count = state.routing_table.read().await.route_count();
    let metric_snapshot = state.metrics.snapshot(0, table_count);
    Ok(Json(ApiResponse::success(metric_snapshot)))
}

async fn get_config(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<ApiResponse<RouterConfig>>, StatusCode> {
    ensure_permission(&state, Some(&headers), None, Some(UserRole::Operator)).await?;
    let config = state.config_manager.get_config().await;
    Ok(Json(ApiResponse::success(config)))
}

async fn get_config_history(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<ApiResponse<Vec<ConfigHistoryEntry>>>, StatusCode> {
    ensure_permission(&state, Some(&headers), None, Some(UserRole::Operator)).await?;
    let history = state.config_manager.list_history().await;
    Ok(Json(ApiResponse::success(history)))
}

async fn get_config_diff(
    Path(path): Path<ConfigVersionPath>,
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<ApiResponse<ConfigDiff>>, StatusCode> {
    ensure_permission(&state, Some(&headers), None, Some(UserRole::Operator)).await?;
    match state.config_manager.diff(path.version).await {
        Ok(diff) => Ok(Json(ApiResponse::success(diff))),
        Err(err) => {
            log::error!(
                "Failed to fetch config diff for version {}: {}",
                path.version,
                err
            );
            let status = if err.to_string().contains("Snapshot not found") {
                StatusCode::NOT_FOUND
            } else {
                StatusCode::INTERNAL_SERVER_ERROR
            };
            Err(status)
        }
    }
}

async fn rollback_config(
    Path(path): Path<ConfigVersionPath>,
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<ApiResponse<()>>, StatusCode> {
    ensure_permission(&state, Some(&headers), None, Some(UserRole::Admin)).await?;
    match state.config_manager.rollback_to(path.version).await {
        Ok(_) => {
            state.events.publish_activity(
                ActivityLevel::Warn,
                format!("Configuration rolled back to version {}", path.version),
            );
            Ok(Json(ApiResponse::success(())))
        }
        Err(err) => {
            log::error!(
                "Failed to rollback configuration to version {}: {}",
                path.version,
                err
            );
            let status = if err.to_string().contains("Snapshot not found") {
                StatusCode::NOT_FOUND
            } else {
                StatusCode::INTERNAL_SERVER_ERROR
            };
            Err(status)
        }
    }
}

async fn update_config(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<RouterConfig>,
) -> Result<Json<ApiResponse<()>>, StatusCode> {
    ensure_permission(&state, Some(&headers), None, Some(UserRole::Admin)).await?;
    state
        .config_manager
        .update_config(request)
        .await
        .map_err(|e| {
            log::error!("Failed to update config via API: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    state
        .events
        .publish_activity(ActivityLevel::Info, "Configuration updated via API");

    Ok(Json(ApiResponse::success(())))
}

async fn restart_router(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<ApiResponse<()>>, StatusCode> {
    ensure_permission(&state, Some(&headers), None, Some(UserRole::Admin)).await?;
    let mut router = state.router.write().await;
    router.restart().await.map_err(|e| {
        log::error!("Failed to restart router: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(Json(ApiResponse::success(())))
}

async fn login(
    State(state): State<AppState>,
    Json(request): Json<LoginRequest>,
) -> Result<Json<ApiResponse<LoginResponse>>, StatusCode> {
    let mut guard = state.auth.lock().await;
    let manager = match guard.as_mut() {
        Some(manager) => manager,
        None => {
            return Ok(Json(ApiResponse::<LoginResponse>::error(
                "Authentication disabled".to_string(),
            )))
        }
    };

    let response = manager.authenticate(request).await;
    if response.success {
        if let Some(user) = response.user.as_ref() {
            state.events.publish_activity(
                ActivityLevel::Info,
                format!("User {} logged in", user.username),
            );
        }
        Ok(Json(ApiResponse::success(response)))
    } else {
        Ok(Json(ApiResponse::<LoginResponse>::error(response.message)))
    }
}

async fn logout(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<ApiResponse<()>>, StatusCode> {
    let token = extract_token(&headers).ok_or(StatusCode::UNAUTHORIZED)?;

    let mut guard = state.auth.lock().await;
    let manager = guard.as_mut().ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    let claims = manager
        .validate_token(&token)
        .map_err(|_| StatusCode::UNAUTHORIZED)?;

    manager
        .logout(&token)
        .map_err(|_| StatusCode::BAD_REQUEST)?;

    state.events.publish_activity(
        ActivityLevel::Info,
        format!("User {} logged out", claims.sub),
    );

    Ok(Json(ApiResponse::success(())))
}

// Utility helpers

fn extract_token(headers: &HeaderMap) -> Option<String> {
    if let Some(value) = headers.get(header::AUTHORIZATION) {
        if let Ok(text) = value.to_str() {
            if let Some(stripped) = text.trim().strip_prefix("Bearer ") {
                let token = stripped.trim();
                if !token.is_empty() {
                    return Some(token.to_string());
                }
            }
        }
    }

    headers
        .get("x-auth-token")
        .and_then(|value| value.to_str().ok())
        .map(|token| token.trim().to_string())
}

async fn ensure_permission(
    state: &AppState,
    headers: Option<&HeaderMap>,
    token_override: Option<String>,
    required_role: Option<UserRole>,
) -> Result<(), StatusCode> {
    let mut guard = state.auth.lock().await;
    let manager = match guard.as_mut() {
        Some(manager) => manager,
        None => return Ok(()),
    };

    let token = if let Some(token) = token_override {
        token
    } else if let Some(headers) = headers {
        extract_token(headers).ok_or(StatusCode::UNAUTHORIZED)?
    } else {
        return Err(StatusCode::UNAUTHORIZED);
    };

    let claims = manager
        .validate_token(&token)
        .map_err(|_| StatusCode::UNAUTHORIZED)?;

    if let Some(role) = required_role {
        let checker = require_permission(role);
        checker(&claims).map_err(|err| match err {
            AuthError::InsufficientPermissions => StatusCode::FORBIDDEN,
            _ => StatusCode::UNAUTHORIZED,
        })?;
    }

    Ok(())
}

async fn collect_interface_info(interfaces: &[InterfaceConfig]) -> Vec<InterfaceInfo> {
    let stats = read_network_stats();

    interfaces
        .iter()
        .map(|iface| {
            let status = if iface.enabled { "up" } else { "down" };
            let metrics = stats.get(&iface.name);

            InterfaceInfo {
                name: iface.name.clone(),
                address: iface.address.clone(),
                status: status.to_string(),
                packets_sent: metrics.map(|m| m.tx_packets).unwrap_or(0),
                packets_received: metrics.map(|m| m.rx_packets).unwrap_or(0),
                bytes_sent: metrics.map(|m| m.tx_bytes).unwrap_or(0),
                bytes_received: metrics.map(|m| m.rx_bytes).unwrap_or(0),
            }
        })
        .collect()
}

async fn cpu_usage_percent() -> f32 {
    let first = read_cpu_times();
    tokio::time::sleep(Duration::from_millis(150)).await;
    let second = read_cpu_times();

    match (first, second) {
        (Some((idle1, total1)), Some((idle2, total2))) if total2 > total1 && idle2 >= idle1 => {
            let total_delta = total2 - total1;
            let idle_delta = idle2 - idle1;
            if total_delta == 0 {
                0.0
            } else {
                let usage = (total_delta - idle_delta) as f32 / total_delta as f32;
                (usage * 100.0).clamp(0.0, 100.0)
            }
        }
        _ => 0.0,
    }
}

#[derive(Default)]
struct NetStats {
    rx_bytes: u64,
    tx_bytes: u64,
    rx_packets: u64,
    tx_packets: u64,
}

fn read_network_stats() -> HashMap<String, NetStats> {
    let mut stats = HashMap::new();

    if let Ok(content) = std::fs::read_to_string("/proc/net/dev") {
        for line in content.lines().skip(2) {
            if let Some((iface, data)) = line.split_once(':') {
                let parts: Vec<&str> = data.split_whitespace().collect();
                if parts.len() >= 16 {
                    let rx_bytes = parts[0].parse().unwrap_or(0);
                    let rx_packets = parts[1].parse().unwrap_or(0);
                    let tx_bytes = parts[8].parse().unwrap_or(0);
                    let tx_packets = parts[9].parse().unwrap_or(0);
                    stats.insert(
                        iface.trim().to_string(),
                        NetStats {
                            rx_bytes,
                            tx_bytes,
                            rx_packets,
                            tx_packets,
                        },
                    );
                }
            }
        }
    }

    stats
}

fn read_cpu_times() -> Option<(u64, u64)> {
    let content = std::fs::read_to_string("/proc/stat").ok()?;
    let mut fields = content.lines().next()?.split_whitespace();
    let _cpu = fields.next()?; // skip label

    let mut values = Vec::new();
    for field in fields.take(10) {
        if let Ok(value) = field.parse::<u64>() {
            values.push(value);
        }
    }

    if values.len() < 4 {
        return None;
    }

    let user = values.get(0).copied().unwrap_or(0);
    let nice = values.get(1).copied().unwrap_or(0);
    let system = values.get(2).copied().unwrap_or(0);
    let idle = values.get(3).copied().unwrap_or(0);
    let iowait = values.get(4).copied().unwrap_or(0);
    let irq = values.get(5).copied().unwrap_or(0);
    let softirq = values.get(6).copied().unwrap_or(0);
    let steal = values.get(7).copied().unwrap_or(0);

    let idle_all = idle + iowait;
    let non_idle = user + nice + system + irq + softirq + steal;
    let total = idle_all + non_idle;

    Some((idle_all, total))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config_manager::InterfaceConfig;

    #[tokio::test]
    async fn api_response_success_wraps_data() {
        let response = ApiResponse::success("value");
        assert!(response.success);
        assert_eq!(response.data.unwrap(), "value");
    }

    #[tokio::test]
    async fn interface_info_collection_handles_missing() {
        let interfaces = vec![InterfaceConfig {
            name: "lo".to_string(),
            address: "127.0.0.1/8".to_string(),
            enabled: true,
            cost: 1,
        }];

        let results = collect_interface_info(&interfaces).await;
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "lo");
    }
}
