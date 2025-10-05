use clap::Parser;
use log::{error, info, warn};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Mutex, RwLock};

use rust_route::protocol::RipCommand;
use rust_route::{
    auth::AuthManager,
    cli::{Cli, ConfigAction},
    config_manager::{ConfigManager, RouterConfig},
    events::{ActivityLevel, EventBus, MetricsEvent, RouteEvent, WebEvent},
    metrics::Metrics,
    protocol::RipPacket,
    router::{handle_rip_response, Router},
    routing_table::{Route, RoutingTable},
    web::WebServer,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format_timestamp_secs()
        .init();

    let cli = Cli::parse();
    print_banner();

    match cli.command {
        Some(rust_route::cli::Commands::Start { config }) => {
            start_router(config).await?;
        }
        Some(rust_route::cli::Commands::Config { action }) => {
            handle_config_command(action).await?;
        }
        Some(rust_route::cli::Commands::Test { .. }) => {
            run_tests().await?;
        }
        Some(rust_route::cli::Commands::Benchmark) => {
            run_benchmarks().await?;
        }
        None => {
            start_router("rust-route.json".to_string()).await?;
        }
    }

    Ok(())
}

async fn start_router(config_path: String) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    info!("🚀 Starting RustRoute with config: {}", config_path);

    let (manager, mut config_receiver) = ConfigManager::new(&config_path).await?;
    let manager = Arc::new(manager);
    let initial_config = manager.get_config().await;
    let config_version = manager.get_config_version().await;

    let routing_table = Arc::new(RwLock::new(RoutingTable::new()));
    let metrics = Metrics::new();
    metrics.set_config_version(config_version);

    let event_bus = EventBus::new(256);

    let auth_state: Arc<Mutex<Option<AuthManager>>> = Arc::new(Mutex::new(None));
    let auth_active = initial_config.auth.enabled && initial_config.web.auth_enabled;
    if initial_config.auth.enabled != initial_config.web.auth_enabled {
        warn!(
            "Authentication configuration mismatch: auth.enabled={}, web.auth_enabled={}. Authentication will remain disabled.",
            initial_config.auth.enabled, initial_config.web.auth_enabled
        );
        event_bus.publish_activity(
            ActivityLevel::Warn,
            "Authentication mismatch detected; enable both auth.enabled and web.auth_enabled to require login",
        );
    }

    {
        let mut guard = auth_state.lock().await;
        if auth_active {
            match AuthManager::new(initial_config.auth.clone()) {
                Ok(manager) => {
                    *guard = Some(manager);
                    event_bus.publish_activity(ActivityLevel::Info, "Authentication enabled");
                }
                Err(err) => {
                    event_bus.publish_activity(
                        ActivityLevel::Error,
                        format!("Failed to initialize authentication: {}", err),
                    );
                }
            }
        }
    }

    let router = Router::new(
        initial_config.clone(),
        Arc::clone(&routing_table),
        metrics.clone(),
    )
    .await?;
    let router = Arc::new(RwLock::new(router));

    let initial_route_count = routing_table.read().await.route_count();
    metrics.update_route_count(initial_route_count);

    // Watch for configuration changes
    let router_for_config = Arc::clone(&router);
    let routing_table_for_config = Arc::clone(&routing_table);
    let metrics_for_config = metrics.clone();
    let manager_for_config = Arc::clone(&manager);

    let event_bus_for_config = event_bus.clone();
    let auth_state_for_config = Arc::clone(&auth_state);
    tokio::spawn(async move {
        while config_receiver.changed().await.is_ok() {
            let new_config = config_receiver.borrow().clone();
            match router_for_config
                .write()
                .await
                .apply_config(new_config.clone())
                .await
            {
                Ok(_) => {
                    let version = manager_for_config.get_config_version().await;
                    metrics_for_config.set_config_version(version);
                    let route_count = routing_table_for_config.read().await.route_count();
                    metrics_for_config.update_route_count(route_count);
                    info!("✅ Configuration change applied successfully");
                    event_bus_for_config
                        .publish_activity(ActivityLevel::Info, "Configuration reloaded from disk");

                    {
                        let mut auth_guard = auth_state_for_config.lock().await;
                        let auth_enabled = new_config.auth.enabled;
                        let web_auth_enabled = new_config.web.auth_enabled;
                        let auth_active = auth_enabled && web_auth_enabled;

                        if auth_active {
                            match AuthManager::new(new_config.auth.clone()) {
                                Ok(manager) => {
                                    *auth_guard = Some(manager);
                                    event_bus_for_config.publish_activity(
                                        ActivityLevel::Info,
                                        "Authentication settings updated",
                                    );
                                }
                                Err(err) => {
                                    *auth_guard = None;
                                    event_bus_for_config.publish_activity(
                                        ActivityLevel::Error,
                                        format!(
                                            "Failed to update authentication settings: {}",
                                            err
                                        ),
                                    );
                                }
                            }
                        } else {
                            if auth_guard.is_some() {
                                *auth_guard = None;
                                event_bus_for_config.publish_activity(
                                    ActivityLevel::Warn,
                                    "Authentication disabled via configuration",
                                );
                            }

                            if auth_enabled != web_auth_enabled {
                                warn!(
                                    "Authentication configuration mismatch: auth.enabled={}, web.auth_enabled={}. Authentication will remain disabled.",
                                    auth_enabled, web_auth_enabled
                                );
                                event_bus_for_config.publish_activity(
                                    ActivityLevel::Warn,
                                    "Authentication mismatch detected; enable both auth.enabled and web.auth_enabled to require login",
                                );
                            }
                        }
                    }
                }
                Err(err) => {
                    error!("Failed to apply new configuration: {}", err);
                    event_bus_for_config.publish_activity(
                        ActivityLevel::Error,
                        format!("Failed to apply configuration: {}", err),
                    );
                }
            }
        }
    });

    // Periodically recompute route counts and clean neighbors
    let routing_table_for_metrics = Arc::clone(&routing_table);
    let metrics_updater = metrics.clone();
    let router_for_metrics_events = Arc::clone(&router);
    let events_for_metrics = event_bus.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(30));
        loop {
            interval.tick().await;
            let count = routing_table_for_metrics.read().await.route_count();
            metrics_updater.update_route_count(count);

            let neighbor_count = {
                let neighbors_arc = {
                    let router_guard = router_for_metrics_events.read().await;
                    router_guard.neighbors()
                };
                let count = neighbors_arc.read().await.len();
                count
            };

            let snapshot = metrics_updater.snapshot(neighbor_count, count);
            events_for_metrics.publish(WebEvent::Metrics(MetricsEvent { snapshot }));
        }
    });

    let rip_settings_snapshot = {
        let guard = router.read().await;
        (
            guard.rip_enabled(),
            guard.rip_config().clone(),
            guard.router_uuid(),
            guard.network_interfaces(),
            guard.neighbors(),
        )
    };

    let (rip_enabled, rip_config, router_uuid, interfaces, neighbors_arc) = rip_settings_snapshot;

    if rip_enabled {
        let rip_config = Arc::new(rip_config);

        // Periodic neighbor cleanup based on RIP timers
        let router_for_cleanup = Arc::clone(&router);
        let rip_config_for_cleanup = rip_config.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(60));
            loop {
                interval.tick().await;
                router_for_cleanup
                    .read()
                    .await
                    .cleanup_neighbors(Duration::from_secs(
                        rip_config_for_cleanup.garbage_collection_timeout.max(60),
                    ))
                    .await;
            }
        });

        // Periodic routing updates
        let routing_table_for_updates = Arc::clone(&routing_table);
        let metrics_for_updates = metrics.clone();
        let interfaces_for_updates = interfaces.clone();
        let rip_config_for_updates = rip_config.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(
                rip_config_for_updates.update_interval.max(5),
            ));
            loop {
                interval.tick().await;

                for iface in &interfaces_for_updates {
                    let routes: Vec<Route> = {
                        let table = routing_table_for_updates.read().await;
                        table
                            .get_routes_for_advertising(&iface.config.name)
                            .into_iter()
                            .cloned()
                            .collect()
                    };

                    if routes.is_empty() {
                        continue;
                    }

                    let packet = RipPacket::new_update(router_uuid, routes);
                    if let Err(err) = iface.send_packet(&packet).await {
                        warn!(
                            "Failed to broadcast routes on {}: {}",
                            iface.config.name, err
                        );
                        continue;
                    }

                    metrics_for_updates.increment_packets_sent();
                    metrics_for_updates.increment_routing_updates_sent();
                }
            }
        });

        // Routing table maintenance (timeouts & garbage collection)
        let routing_table_for_timers = Arc::clone(&routing_table);
        let metrics_for_timers = metrics.clone();
        let rip_config_for_timers = rip_config.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(
                (rip_config_for_timers.update_interval.max(5)) * 2,
            ));
            loop {
                interval.tick().await;

                let mut table = routing_table_for_timers.write().await;
                table.process_timeouts();
                table.garbage_collect();
                metrics_for_timers.update_route_count(table.route_count());
            }
        });

        // Packet receive loops per interface
        for iface in interfaces {
            let iface_clone = Arc::clone(&iface);
            let routing_table_for_iface = Arc::clone(&routing_table);
            let metrics_for_iface = metrics.clone();
            let neighbors_for_iface = Arc::clone(&neighbors_arc);
            let rip_config_for_iface = rip_config.clone();
            let iface_name = iface.config.name.clone();
            let events_for_iface = event_bus.clone();

            tokio::spawn(async move {
                loop {
                    match iface_clone.receive_packet().await {
                        Ok((packet, sender)) => {
                            metrics_for_iface.increment_packets_received();
                            match packet.command {
                                RipCommand::Request => {
                                    let routes: Vec<Route> = {
                                        let table = routing_table_for_iface.read().await;
                                        table
                                            .get_routes_for_advertising(&iface_name)
                                            .into_iter()
                                            .cloned()
                                            .collect()
                                    };

                                    let response = RipPacket::new_update(router_uuid, routes);
                                    if let Err(err) =
                                        iface_clone.send_packet_to(&response, sender).await
                                    {
                                        warn!(
                                            "Failed to reply RIP request on {}: {}",
                                            iface_name, err
                                        );
                                    } else {
                                        metrics_for_iface.increment_packets_sent();
                                        metrics_for_iface.increment_routing_updates_sent();
                                    }
                                }
                                RipCommand::Response => {
                                    match handle_rip_response(
                                        Arc::clone(&routing_table_for_iface),
                                        Arc::clone(&neighbors_for_iface),
                                        metrics_for_iface.clone(),
                                        rip_config_for_iface.clone(),
                                        iface_name.clone(),
                                        packet,
                                        sender,
                                    )
                                    .await
                                    {
                                        Ok(routes) => {
                                            for route in routes {
                                                events_for_iface.publish(WebEvent::Route(
                                                    RouteEvent::from_parts(
                                                        route.destination,
                                                        route.subnet_mask,
                                                        route.next_hop,
                                                        route.metric,
                                                        route.interface.clone(),
                                                        route.source,
                                                    ),
                                                ));
                                            }
                                        }
                                        Err(err) => {
                                            warn!(
                                                "Failed to process RIP response on {}: {}",
                                                iface_name, err
                                            );
                                            events_for_iface.publish_activity(
                                                ActivityLevel::Warn,
                                                format!(
                                                    "Failed to process RIP response on {}: {}",
                                                    iface_name, err
                                                ),
                                            );
                                        }
                                    }
                                }
                            }
                        }
                        Err(err) => {
                            warn!("Error receiving packet on {}: {}", iface_name, err);
                            tokio::time::sleep(Duration::from_secs(1)).await;
                        }
                    }
                }
            });
        }
    } else {
        info!("RIP networking disabled or no active interfaces; skipping UDP tasks");
    }

    // Launch web interface
    let web_server = WebServer::new(
        Arc::clone(&router),
        Arc::clone(&routing_table),
        metrics.clone(),
        Arc::clone(&manager),
        initial_config.web.clone(),
        event_bus.clone(),
        Arc::clone(&auth_state),
    );

    let web_handle = tokio::spawn(async move {
        if let Err(err) = web_server.start().await {
            error!("Web server error: {}", err);
        }
    });

    tokio::select! {
        _ = tokio::signal::ctrl_c() => {
            info!("🛑 Received shutdown signal");
        }
        result = web_handle => {
            if let Err(err) = result {
                error!("Web server stopped unexpectedly: {}", err);
            }
        }
    }

    Ok(())
}

async fn handle_config_command(
    action: ConfigAction,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    match action {
        ConfigAction::Validate { file } => {
            let config_content = tokio::fs::read_to_string(&file).await?;
            let config: RouterConfig = serde_json::from_str(&config_content)?;
            let validation = ConfigManager::validate_config(&config);

            if validation.is_valid() {
                println!("✅ Configuration is valid");
                if !validation.warnings.is_empty() {
                    println!("⚠️  Warnings:");
                    for warning in validation.warnings {
                        println!("  - {}", warning);
                    }
                }
            } else {
                println!("❌ Configuration is invalid");
                for error in validation.errors {
                    println!("  - {}", error);
                }
                std::process::exit(1);
            }
        }
        ConfigAction::Generate { output } => {
            let default_config = RouterConfig::default();
            let json = serde_json::to_string_pretty(&default_config)?;
            tokio::fs::write(&output, json).await?;
            println!("✅ Default configuration generated: {}", output);
        }
        ConfigAction::Backup { config, output } => {
            let (manager, _) = ConfigManager::new(&config).await?;
            let backup_path = manager.create_backup("Manual backup".to_string()).await?;
            if let Some(path) = output {
                tokio::fs::copy(&backup_path, &path).await?;
                println!("✅ Backup created: {}", path);
            } else {
                println!("✅ Backup created: {}", backup_path.display());
            }
        }
        ConfigAction::Restore { backup, config } => {
            let (manager, _) = ConfigManager::new(&config).await?;
            manager.restore_backup(&backup).await?;
            println!("✅ Configuration restored from backup: {}", backup);
        }
    }
    Ok(())
}

async fn run_tests() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    println!("🧪 Running RustRoute tests...");
    test_routing_table().await?;
    test_config_validation().await?;
    test_metrics_flow().await?;
    println!("✅ All tests passed!");
    Ok(())
}

async fn test_routing_table() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    use std::net::Ipv4Addr;

    println!("  Testing routing table...");
    let mut table = RoutingTable::new();
    table.install_direct_route(
        Ipv4Addr::new(192, 168, 1, 0),
        Ipv4Addr::new(255, 255, 255, 0),
        "eth0".to_string(),
    );
    assert_eq!(table.route_count(), 1);
    println!("    ✓ Direct route installation works");
    Ok(())
}

async fn test_config_validation() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    println!("  Testing configuration validation...");
    let config = RouterConfig::default();
    let validation = ConfigManager::validate_config(&config);
    assert!(validation.is_valid());
    println!("    ✓ Default config is valid");
    Ok(())
}

async fn test_metrics_flow() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    println!("  Testing metrics flow...");
    let metrics = Metrics::new();
    metrics.increment_packets_sent();
    metrics.increment_packets_received();
    metrics.update_route_count(4);
    let snapshot = metrics.snapshot(1, 4);
    assert_eq!(snapshot.packets_sent, 1);
    assert_eq!(snapshot.route_count, 4);
    println!("    ✓ Metrics snapshot looks good");
    Ok(())
}

async fn run_benchmarks() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    use std::net::Ipv4Addr;
    use std::time::Instant;

    println!("🏃 Running RustRoute benchmarks...");

    println!("  Benchmarking routing table operations...");
    let mut table = RoutingTable::new();
    let start = Instant::now();
    for i in 0..1000 {
        let network = Ipv4Addr::new(10, (i / 256) as u8, (i % 256) as u8, 0);
        table.add_static_route(
            network,
            Ipv4Addr::new(255, 255, 255, 0),
            Ipv4Addr::new(10, 0, 0, 1),
            2,
            format!("eth{}", i % 4),
        );
    }
    let duration = start.elapsed();
    println!("    ✓ Added 1000 routes in {:?}", duration);

    let lookup_start = Instant::now();
    for i in 0..1000 {
        let dest = Ipv4Addr::new(10, (i / 256) as u8, (i % 256) as u8, 10);
        let _ = table.find_best_route(&dest);
    }
    let lookup_duration = lookup_start.elapsed();
    println!("    ✓ 1000 lookups in {:?}", lookup_duration);

    println!("✅ Benchmarks completed!");
    Ok(())
}

fn print_banner() {
    println!(
        r#"
    ____             __  ____              __      
   / __ \__  _______/ /_/ __ \____  __  __/ /____  
  / /_/ / / / / ___/ __/ /_/ / __ \/ / / / __/ _ \ 
 / _, _/ /_/ (__  ) /_/ _, _/ /_/ / /_/ / /_/  __/ 
/_/ |_|\__,_/____/\__/_/ |_|\____/\__,_/\__/\___/  
                                                   
🦀 Configurable RIP Router Runtime
📡 Version: {}
"#,
        env!("CARGO_PKG_VERSION")
    );
}
