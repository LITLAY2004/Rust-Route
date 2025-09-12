use clap::{Parser, Subcommand};
use rust_route::{
    router::{Router, RouterConfig, InterfaceConfig, RouterStatistics},
    network::NetworkInterface,
    metrics::PerformanceMonitor,
    cli::CliFormatter,
    testing,
    RustRouteResult,
};
use std::net::Ipv4Addr;
use std::time::Duration;
use tokio;

#[derive(Parser)]
#[command(name = "rust-route")]
#[command(about = "🦀 RustRoute: Real RIP Router Implementation in Rust")]
#[command(version = "0.1.0")]
#[command(author = "RustRoute Team")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the RustRoute router daemon
    Start {
        /// Configuration file path
        #[arg(short, long, default_value = "rust-route.json")]
        config: String,
        /// Router update interval in seconds
        #[arg(short, long, default_value = "30")]
        interval: u64,
        /// Enable verbose logging
        #[arg(short, long)]
        verbose: bool,
    },
    /// Show router status and metrics
    Status {
        /// Show detailed metrics
        #[arg(short, long)]
        detailed: bool,
    },
    /// Configure network interfaces
    Configure {
        /// Interface name
        #[arg(short, long)]
        interface: String,
        /// IP address
        #[arg(short = 'a', long)]
        ip_address: String,
        /// Subnet mask
        #[arg(short = 'm', long)]
        subnet_mask: String,
    },
    /// Test connectivity to neighbors
    Test {
        /// Target IP address
        target: String,
    },
}

#[tokio::main]
async fn main() -> RustRouteResult<()> {
    // Print banner
    CliFormatter::print_banner();
    
    // Initialize logging
    env_logger::Builder::from_default_env()
        .filter_level(log::LevelFilter::Info)
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Start { config, interval, verbose } => {
            if verbose {
                log::set_max_level(log::LevelFilter::Debug);
            }
            
            start_router(config, interval).await
        }
        Commands::Status { detailed } => {
            show_status(detailed).await
        }
        Commands::Configure { interface, ip_address, subnet_mask } => {
            configure_interface(interface, ip_address, subnet_mask).await
        }
        Commands::Test { target } => {
            test_connectivity(target).await
        }
    }
}

/// Start the RustRoute router daemon
async fn start_router(config_path: String, interval: u64) -> RustRouteResult<()> {
    log::info!("Starting RustRoute router daemon...");
    
    // Load or create configuration
    let config = load_or_create_config(&config_path).await?;
    
    CliFormatter::print_info(&format!("配置已加载: {}", config_path));
    
    // Create router instance with real network capabilities
    let mut router = Router::new(config.clone()).await?;
    
    CliFormatter::print_success("路由器实例创建成功");
    
    // Initialize real network interfaces
    for interface_config in &config.interfaces {
        CliFormatter::print_info(&format!("初始化网络接口: {}", interface_config.name));
        
        // Create real network interface
        let network_interface_config = rust_route::network::InterfaceConfig {
            name: interface_config.name.clone(),
            ip_address: interface_config.ip_address,
            subnet_mask: interface_config.subnet_mask,
            port: config.port,
            multicast_address: Ipv4Addr::new(224, 0, 0, 9),
            mtu: 1500,
            enabled: interface_config.enabled,
        };
        
        let interface = NetworkInterface::new(network_interface_config);
        router.add_interface(interface).await?;
        
        log::info!("Interface {} configured with IP {} on real socket", 
                   interface_config.name, interface_config.ip_address);
    }
    
    CliFormatter::print_success("所有网络接口初始化完成");
    
    // Show startup information
    CliFormatter::print_info(&format!("路由器ID: {}", config.router_id));
    CliFormatter::print_info(&format!("更新间隔: {}秒", interval));
    CliFormatter::print_info(&format!("RIP版本: {}", config.rip_version));
    CliFormatter::print_info(&format!("监听端口: {}", config.port));
    
    // Start router with real network communication
    CliFormatter::print_success("🚀 RustRoute 路由器启动成功!");
    CliFormatter::print_info("按 Ctrl+C 停止路由器");
    
    // Set up graceful shutdown
    let shutdown = tokio::signal::ctrl_c();
    
    // Run router with real RIP protocol
    let router_task = router.run_with_real_networking(Duration::from_secs(interval));
    
    tokio::select! {
        result = router_task => {
            match result {
                Ok(_) => CliFormatter::print_success("路由器正常退出"),
                Err(e) => CliFormatter::print_error(&format!("路由器错误: {}", e)),
            }
        }
        _ = shutdown => {
            CliFormatter::print_info("收到停止信号，正在关闭路由器...");
            router.shutdown().await?;
        }
    }
    
    log::info!("RustRoute router stopped");
    Ok(())
}

/// Load configuration from file or create default
async fn load_or_create_config(config_path: &str) -> RustRouteResult<RouterConfig> {
    if std::path::Path::new(config_path).exists() {
        let config_content = std::fs::read_to_string(config_path)
            .map_err(|e| rust_route::RustRouteError::ConfigError(format!("Failed to read config file {}: {}", config_path, e)))?;
        
        serde_json::from_str(&config_content)
            .map_err(|e| rust_route::RustRouteError::ConfigError(format!("Invalid config format: {}", e)))
    } else {
        // Create default configuration
        let default_config = RouterConfig::default();
        let config_json = serde_json::to_string_pretty(&default_config)
            .map_err(|e| rust_route::RustRouteError::ConfigError(format!("Failed to serialize default config: {}", e)))?;
        
        std::fs::write(config_path, config_json)
            .map_err(|e| rust_route::RustRouteError::ConfigError(format!("Failed to write default config: {}", e)))?;
        
        CliFormatter::print_info(&format!("创建默认配置文件: {}", config_path));
        Ok(default_config)
    }
}

/// Show router status and metrics
async fn show_status(detailed: bool) -> RustRouteResult<()> {
    CliFormatter::print_info("正在获取路由器状态...");
    
    let pb = CliFormatter::show_progress("收集系统信息中...", Duration::from_secs(1));
    tokio::time::sleep(Duration::from_millis(500)).await;
    pb.finish_with_message("✅ 状态信息收集完成");
    
    if detailed {
        // Show detailed status
        CliFormatter::print_info("=== 路由器详细状态 ===");
        
        // Show example statistics (in real deployment, this would be from running router)
        let stats = RouterStatistics {
            uptime: "路由器未运行".to_string(),
            packets_sent: 0,
            packets_received: 0,
            route_count: 0,
            neighbor_count: 0,
            memory_usage: 0,
        };
        
        // Convert to CLI format
        let cli_stats = rust_route::cli::RouterStatistics {
            uptime: stats.uptime,
            packets_sent: stats.packets_sent,
            packets_received: stats.packets_received,
            route_count: stats.route_count,
            neighbor_count: stats.neighbor_count,
            memory_usage: stats.memory_usage,
        };
        
        CliFormatter::print_statistics(&cli_stats);
        
        // Show performance monitor
        let monitor = PerformanceMonitor::new(Duration::from_secs(60));
        let report = monitor.generate_report(3, 10);
        report.print_report();
        
        CliFormatter::print_info("💡 启动路由器以查看实时数据: rust-route start");
        
    } else {
        // Show basic status
        CliFormatter::print_warning("路由器运行状态: 未运行");
        CliFormatter::print_info("版本: RustRoute 0.1.0");
        CliFormatter::print_info("启动命令: rust-route start");
    }
    
    Ok(())
}

/// Configure network interface
async fn configure_interface(interface_name: String, ip_address: String, subnet_mask: String) -> RustRouteResult<()> {
    CliFormatter::print_info("配置网络接口...");
    
    // Validate IP address
    let ip: Ipv4Addr = ip_address.parse()
        .map_err(|_| rust_route::RustRouteError::InvalidInput(format!("Invalid IP address: {}", ip_address)))?;
    
    // Validate subnet mask
    let mask: Ipv4Addr = subnet_mask.parse()
        .map_err(|_| rust_route::RustRouteError::InvalidInput(format!("Invalid subnet mask: {}", subnet_mask)))?;
    
    CliFormatter::print_success("接口配置验证通过");
    
    // Create interface config
    let config = InterfaceConfig {
        name: interface_name.clone(),
        ip_address: ip,
        subnet_mask: mask,
        enabled: true,
    };
    
    // Display configuration
    CliFormatter::print_info("=== 接口配置 ===");
    CliFormatter::print_info(&format!("接口名称: {}", config.name));
    CliFormatter::print_info(&format!("IP地址: {}", config.ip_address));
    CliFormatter::print_info(&format!("子网掩码: {}", config.subnet_mask));
    CliFormatter::print_info(&format!("状态: {}", if config.enabled { "启用" } else { "禁用" }));
    
    CliFormatter::print_success("✅ 接口配置完成");
    CliFormatter::print_info("💡 使用 'rust-route start' 启动路由器以应用配置");
    
    Ok(())
}

/// Test real connectivity to a target
async fn test_connectivity(target: String) -> RustRouteResult<()> {
    CliFormatter::print_info(&format!("开始测试连接到: {}", target));
    
    let target_ip: Ipv4Addr = target.parse()
        .map_err(|_| rust_route::RustRouteError::InvalidInput(format!("Invalid IP address: {}", target)))?;
    
    let pb = CliFormatter::show_progress(&format!("正在测试到 {} 的连接...", target_ip), Duration::from_secs(3));
    
    // Perform real connectivity test
    let test_results = testing::perform_connectivity_test(target_ip).await?;
    
    pb.finish_with_message("✅ 连接测试完成");
    
    // Display results
    CliFormatter::print_info("=== 连通性测试结果 ===");
    CliFormatter::print_info(&format!("目标地址: {}", target_ip));
    CliFormatter::print_info(&format!("发送包数: {}", test_results.packets_sent));
    CliFormatter::print_info(&format!("接收包数: {}", test_results.packets_received));
    CliFormatter::print_info(&format!("丢包率: {:.1}%", test_results.packet_loss_percent));
    
    if test_results.packets_received > 0 {
        CliFormatter::print_info(&format!("平均延迟: {:.1}ms", test_results.avg_rtt_ms));
        CliFormatter::print_info(&format!("最小延迟: {:.1}ms", test_results.min_rtt_ms));
        CliFormatter::print_info(&format!("最大延迟: {:.1}ms", test_results.max_rtt_ms));
        
        if test_results.packet_loss_percent < 10.0 {
            CliFormatter::print_success("✅ 网络连通性良好");
        } else {
            CliFormatter::print_warning("⚠️  网络连通性不稳定");
        }
    } else {
        CliFormatter::print_error("❌ 目标不可达");
        
        // Provide diagnosis
        CliFormatter::print_info("=== 网络诊断建议 ===");
        let suggestions = testing::get_diagnosis_suggestions(target_ip);
        for suggestion in suggestions {
            CliFormatter::print_info(&format!("• {}", suggestion));
        }
    }
    
    Ok(())
}