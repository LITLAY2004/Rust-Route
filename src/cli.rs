//! CLI formatting and user interface utilities

use indicatif::{ProgressBar, ProgressStyle};
use std::time::Duration;

/// CLI formatter for consistent output
pub struct CliFormatter;

impl CliFormatter {
    /// Print the application banner
    pub fn print_banner() {
        println!();
        println!("🦀 ████████████████████████████████████████████████████████████");
        println!("🦀 ██                                                        ██");
        println!("🦀 ██  ____            _   ____             _               ██");
        println!("🦀 ██ |  _ \\ _   _ ___| |_|  _ \\ ___  _   _| |_ ___        ██");
        println!("🦀 ██ | |_) | | | / __| __| |_) / _ \\| | | | __/ _ \\       ██");
        println!("🦀 ██ |  _ <| |_| \\__ \\ |_|  _ < (_) | |_| | ||  __/       ██");
        println!("🦀 ██ |_| \\_\\\\__,_|___/\\__|_| \\_\\___/ \\__,_|\\__\\___|       ██");
        println!("🦀 ██                                                        ██");
        println!("🦀 ██               RIP Router Implementation               ██");
        println!("🦀 ██                     v0.1.0                           ██");
        println!("🦀 ██                                                        ██");
        println!("🦀 ████████████████████████████████████████████████████████████");
        println!();
    }

    /// Print success message in green
    pub fn print_success(message: &str) {
        println!("✅ {}", message);
    }

    /// Print info message in blue
    pub fn print_info(message: &str) {
        println!("ℹ️  {}", message);
    }

    /// Print warning message in yellow
    pub fn print_warning(message: &str) {
        println!("⚠️  {}", message);
    }

    /// Print error message in red
    pub fn print_error(message: &str) {
        println!("❌ {}", message);
    }

    /// Show a progress bar
    pub fn show_progress(message: &str, duration: Duration) -> ProgressBar {
        let pb = ProgressBar::new(100);
        pb.set_style(
            ProgressStyle::default_bar()
                .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos:>7}/{len:7} {msg}")
                .unwrap()
                .progress_chars("#>-")
        );
        pb.set_message(message.to_string());
        pb
    }

    /// Print router statistics
    pub fn print_statistics(stats: &RouterStatistics) {
        println!("┌─────────────────────────────────────┐");
        println!("│            路由器统计信息             │");
        println!("├─────────────────────────────────────┤");
        println!("│ 运行时间    : {:20} │", stats.uptime);
        println!("│ 发送包数    : {:20} │", stats.packets_sent);
        println!("│ 接收包数    : {:20} │", stats.packets_received);
        println!("│ 路由表条目  : {:20} │", stats.route_count);
        println!("│ 邻居数量    : {:20} │", stats.neighbor_count);
        println!("│ 内存使用    : {:20} │", stats.memory_usage);
        println!("└─────────────────────────────────────┘");
    }
}

/// Router statistics for CLI display
#[derive(Debug)]
pub struct RouterStatistics {
    pub uptime: String,
    pub packets_sent: u64,
    pub packets_received: u64,
    pub route_count: usize,
    pub neighbor_count: usize,
    pub memory_usage: u64,
}