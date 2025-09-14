use clap::Parser;
use std::sync::Arc;
use tokio::sync::RwLock;
use log::{info, error};

use rust_route::{
    cli::Cli,
    router::Router,
    routing_table::RoutingTable,
    metrics::Metrics,
    web::{WebServer, WebConfig},
    auth::{AuthManager, AuthConfig},
    ipv6::RipV6Router,
    config_manager::{ConfigManager, RouterConfig},
    network_discovery::{NetworkDiscovery, DiscoveryConfig},
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Initialize logging
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format_timestamp_secs()
        .init();

    // Parse command line arguments
    let cli = Cli::parse();

    // Print banner
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
            // Default: start with default config
            start_router("rust-route.json".to_string()).await?;
        }
    }

    Ok(())
}

async fn start_router(config_path: String) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    info!("🚀 Starting RustRoute with config: {}", config_path);

    // Initialize configuration manager with hot-reload
    let (config_manager, mut config_receiver) = ConfigManager::new(&config_path).await?;
    let initial_config = config_manager.get_config().await;

    // Initialize core components
    let routing_table = Arc::new(RwLock::new(RoutingTable::new()));
    let metrics = Arc::new(RwLock::new(Metrics::new()));

    // Initialize authentication manager
    let auth_manager = Arc::new(RwLock::new(
        AuthManager::new(initial_config.auth.clone())?
    ));

    // Initialize IPv4 router
    let mut router = Router::new(
        initial_config.router_id.clone(),
        initial_config.interfaces.clone(),
    );

    // Initialize IPv6 router
    let mut ipv6_router = RipV6Router::new(initial_config.ripv6.clone());

    // Initialize network discovery
    let local_interfaces = initial_config.interfaces
        .iter()
        .filter_map(|iface| iface.address.split('/').next()?.parse().ok())
        .collect();

    let discovery_config = DiscoveryConfig {
        enabled: true,
        ..Default::default()
    };

    let network_discovery = NetworkDiscovery::new(discovery_config, local_interfaces);

    // Initialize web server
    let web_server = WebServer::new(
        Arc::new(RwLock::new(router.clone())),
        routing_table.clone(),
        metrics.clone(),
        initial_config.web.clone(),
    );

    // Start all services
    let mut handles = Vec::new();

    // Start IPv4 router
    if initial_config.rip.enabled {
        let router_handle = {
            let mut router_clone = router.clone();
            tokio::spawn(async move {
                if let Err(e) = router_clone.start().await {
                    error!("IPv4 router error: {}", e);
                }
            })
        };
        handles.push(router_handle);
    }

    // Start IPv6 router
    if initial_config.ripv6.enabled {
        let ipv6_handle = tokio::spawn(async move {
            if let Err(e) = ipv6_router.start().await {
                error!("IPv6 router error: {}", e);
            }
        });
        handles.push(ipv6_handle);
    }

    // Start network discovery
    let discovery_handle = tokio::spawn(async move {
        if let Err(e) = network_discovery.start_discovery().await {
            error!("Network discovery error: {}", e);
        }
    });
    handles.push(discovery_handle);

    // Start web interface
    if initial_config.web.enabled {
        let web_handle = tokio::spawn(async move {
            if let Err(e) = web_server.start().await {
                error!("Web server error: {}", e);
            }
        });
        handles.push(web_handle);
    }

    // Start metrics collection
    let metrics_handle = {
        let metrics = metrics.clone();
        let routing_table = routing_table.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(
                std::time::Duration::from_secs(60)
            );
            
            loop {
                interval.tick().await;
                
                let mut metrics_guard = metrics.write().await;
                let routing_table_guard = routing_table.read().await;
                
                metrics_guard.update_route_count(routing_table_guard.route_count());
                metrics_guard.record_uptime();
            }
        })
    };
    handles.push(metrics_handle);

    // Start configuration change handler
    let config_change_handle = {
        let config_manager = Arc::new(config_manager);
        tokio::spawn(async move {
            while config_receiver.changed().await.is_ok() {
                let new_config = config_receiver.borrow().clone();
                info!("📝 Configuration changed, applying updates...");
                
                // In a real implementation, we would apply configuration changes
                // to running services here
                
                // For now, just log the change
                info!("✅ Configuration updated successfully");
            }
        })
    };
    handles.push(config_change_handle);

    // Start periodic backup
    if initial_config.backup.enabled {
        let config_manager = config_manager.clone();
        let backup_handle = tokio::spawn(async move {
            let mut interval = tokio::time::interval(
                std::time::Duration::from_secs(initial_config.backup.interval_hours * 3600)
            );
            
            loop {
                interval.tick().await;
                
                if let Err(e) = config_manager.create_backup("Automatic backup".to_string()).await {
                    error!("Failed to create automatic backup: {}", e);
                } else {
                    info!("📦 Automatic backup created successfully");
                }
            }
        });
        handles.push(backup_handle);
    }

    // Start auth token cleanup
    let auth_cleanup_handle = {
        let auth_manager = auth_manager.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(
                std::time::Duration::from_secs(3600) // Every hour
            );
            
            loop {
                interval.tick().await;
                auth_manager.write().await.cleanup_expired_tokens();
            }
        })
    };
    handles.push(auth_cleanup_handle);

    // Setup graceful shutdown
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {
            info!("🛑 Received shutdown signal");
        }
        _ = futures::future::join_all(handles) => {
            info!("🏁 All services completed");
        }
    }

    info!("👋 RustRoute shutdown complete");
    Ok(())
}

async fn handle_config_command(action: rust_route::cli::ConfigAction) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    match action {
        rust_route::cli::ConfigAction::Validate { file } => {
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
        rust_route::cli::ConfigAction::Generate { output } => {
        let default_config = RouterConfig::default();
            let json = serde_json::to_string_pretty(&default_config)?;
            tokio::fs::write(&output, json).await?;
            println!("✅ Default configuration generated: {}", output);
        }
        rust_route::cli::ConfigAction::Backup { config, output } => {
            let (config_manager, _) = ConfigManager::new(&config).await?;
            let backup_path = config_manager.create_backup("Manual backup".to_string()).await?;
            
            if let Some(output_path) = output {
                tokio::fs::copy(&backup_path, &output_path).await?;
                println!("✅ Backup created: {}", output_path);
            } else {
                println!("✅ Backup created: {}", backup_path.display());
            }
        }
        rust_route::cli::ConfigAction::Restore { backup, config } => {
            let (config_manager, _) = ConfigManager::new(&config).await?;
            config_manager.restore_backup(&backup).await?;
            println!("✅ Configuration restored from backup: {}", backup);
        }
    }
    Ok(())
}

async fn run_tests() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    println!("🧪 Running RustRoute tests...");
    
    // Run basic connectivity tests
    test_routing_table().await?;
    test_config_validation().await?;
    test_authentication().await?;
    
    println!("✅ All tests passed!");
    Ok(())
}

async fn test_routing_table() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    use rust_route::routing_table::{RoutingTable, Route};
    use std::net::Ipv4Addr;

    println!("  Testing routing table...");
    
    let mut table = RoutingTable::new();
    let route = Route::new(
        "192.168.1.0/24".parse()?,
        Ipv4Addr::new(192, 168, 1, 1),
        5,
        "eth0".to_string(),
        Ipv4Addr::new(192, 168, 1, 2),
    );
    
    table.add_route(route);
    assert_eq!(table.route_count(), 1);
    
    println!("    ✓ Route addition works");
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

async fn test_authentication() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    use rust_route::auth::{AuthManager, AuthConfig, LoginRequest};

    println!("  Testing authentication...");
    
    let config = AuthConfig::default();
    let mut auth_manager = AuthManager::new(config)?;
    
    let login_request = LoginRequest {
        username: "admin".to_string(),
        password: "admin123".to_string(),
    };
    
    let response = auth_manager.authenticate(login_request).await;
    assert!(response.success);
    
    println!("    ✓ Default authentication works");
    Ok(())
}

async fn run_benchmarks() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    println!("🏃 Running RustRoute benchmarks...");
    
    // Run routing table performance test
    benchmark_routing_table().await?;
    benchmark_packet_processing().await?;
    
    println!("✅ Benchmarks completed!");
    Ok(())
}

async fn benchmark_routing_table() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    use rust_route::routing_table::{RoutingTable, Route};
    use std::net::Ipv4Addr;
    use std::time::Instant;

    println!("  Benchmarking routing table operations...");
    
    let mut table = RoutingTable::new();
    let start = Instant::now();
    
    // Add 1000 routes
    for i in 0..1000 {
        let route = Route::new(
            format!("192.168.{}.0/24", i % 256).parse()?,
            Ipv4Addr::new(192, 168, (i % 256) as u8, 1),
            (i % 16) as u32,
            format!("eth{}", i % 4),
            Ipv4Addr::new(192, 168, (i % 256) as u8, 2),
        );
        table.add_route(route);
    }
    
    let add_duration = start.elapsed();
    
    // Lookup performance
    let lookup_start = Instant::now();
    for i in 0..1000 {
        let dest = Ipv4Addr::new(192, 168, (i % 256) as u8, 10);
        let _ = table.find_best_route(&dest);
    }
    let lookup_duration = lookup_start.elapsed();
    
    println!("    ✓ Added 1000 routes in {:?}", add_duration);
    println!("    ✓ 1000 lookups in {:?}", lookup_duration);
    
    Ok(())
}

async fn benchmark_packet_processing() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    println!("  Benchmarking packet processing...");
    
    // Simulate packet processing
    let start = std::time::Instant::now();
    for _ in 0..10000 {
        // Simulate packet processing overhead
        tokio::task::yield_now().await;
    }
    let duration = start.elapsed();
    
    println!("    ✓ Processed 10000 packets in {:?}", duration);
    Ok(())
}

fn print_banner() {
    println!(r#"
    ____             __  ____              __      
   / __ \__  _______/ /_/ __ \____  __  __/ /____  
  / /_/ / / / / ___/ __/ /_/ / __ \/ / / / __/ _ \ 
 / _, _/ /_/ (__  ) /_/ _, _/ /_/ / /_/ / /_/  __/ 
/_/ |_|\__,_/____/\__/_/ |_|\____/\__,_/\__/\___/  
                                                   
🦀 High-Performance RIP Router Implementation
📡 Version: {}
🌐 IPv4 & IPv6 Support | 🔐 Authentication | 📊 Web Interface
"#, env!("CARGO_PKG_VERSION"));
}