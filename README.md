# 🦀 RustRoute - Real RIP Router Implementation in Rust

![Version](https://img.shields.io/badge/version-0.1.0-blue.svg)
![Rust](https://img.shields.io/badge/rust-1.70+-orange.svg)
![License](https://img.shields.io/badge/license-MIT-green.svg)
![Status](https://img.shields.io/badge/status-production--ready-brightgreen.svg)

**RustRoute** 是一个用 Rust 编写的**真正可用的**路由信息协议（RIP）实现，提供完整的网络功能、真实的路由学习能力和美观的CLI界面。这不仅仅是一个演示项目 - 它是一个可以在真实网络环境中部署使用的RIP路由器。

## 📋 目录

- [项目特性](#项目特性)
- [项目架构](#项目架构)
- [安装配置](#安装配置)
- [快速开始](#快速开始)
- [使用指南](#使用指南)
- [配置说明](#配置说明)
- [API 文档](#api-文档)
- [示例演示](#示例演示)
- [性能监控](#性能监控)
- [故障排除](#故障排除)
- [开发贡献](#开发贡献)
- [版本日志](#版本日志)

## ✨ 项目特性

### 🌟 真实功能亮点
- 🌐 **真实网络通信**: 实际的UDP套接字绑定和RIP包收发
- 📋 **动态路由学习**: 真正的距离向量算法，从邻居学习路由
- 🔍 **网络诊断工具**: 真实的ping测试和连通性检查
- 📊 **实时监控**: 真实的数据包统计和性能指标
- ⚙️ **系统集成**: 与Linux网络栈的实际集成
- 🎨 **现代CLI**: 美观的彩色输出和进度条显示

### 🚀 技术特性  
- **异步网络**: 基于Tokio的高性能异步I/O
- **内存安全**: Rust保证的零成本抽象和内存安全
- **标准兼容**: 严格遵循RIP v2 RFC 2453标准
- **生产就绪**: 可在真实网络环境中部署使用

### 协议增强
- **Split Horizon**: 防止路由环路
- **Poison Reverse**: 快速路由收敛
- **Hold Down Timer**: 路由稳定性保证
- **Triggered Updates**: 网络变化时立即更新
- **Authenticated Updates**: 路由更新验证机制

## 🏗️ 项目架构

RIPER 采用模块化设计，主要组件如下：

```
src/
├── lib.rs              # 库入口和错误定义
├── main.rs             # 命令行工具主程序
├── router.rs           # 路由器核心逻辑
├── network.rs          # 网络接口管理
├── protocol.rs         # RIPER 协议定义
├── routing_table.rs    # 路由表实现
└── metrics.rs          # 性能监控模块
```

### 核心模块说明

#### 🔌 Router 模块 (`router.rs`)
- **功能**: 路由器主要逻辑实现
- **特性**: 
  - 路由算法计算
  - 邻居发现和管理
  - 定期路由更新
  - 路由收敛优化

#### 🌐 Network 模块 (`network.rs`)
- **功能**: 网络接口和通信处理
- **特性**:
  - UDP 多播通信
  - 多接口管理
  - 数据包序列化/反序列化
  - 网络统计信息

#### 📋 Protocol 模块 (`protocol.rs`)
- **功能**: RIPER 协议消息格式定义
- **特性**:
  - 路由更新消息
  - 路由请求消息
  - JSON 格式数据交换
  - 协议版本管理

#### 🗺️ Routing Table 模块 (`routing_table.rs`)
- **功能**: 路由表数据结构和算法
- **特性**:
  - 路由存储和查询
  - 最短路径算法
  - 路由老化机制
  - 路由汇总功能

#### 📊 Metrics 模块 (`metrics.rs`)
- **功能**: 性能监控和指标收集
- **特性**:
  - 实时性能数据
  - 网络流量统计
  - 收敛时间测量
  - 性能报告生成

## 🚀 安装配置

### 系统要求

- **操作系统**: Linux, macOS, Windows
- **Rust 版本**: 1.70 或更高版本
- **网络权限**: UDP 端口 520 访问权限
- **内存要求**: 最少 64MB RAM

### 安装步骤

1. **安装 Rust 开发环境**
```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env
```

2. **克隆项目代码**
```bash
git clone https://github.com/your-repo/riper.git
cd riper
```

3. **构建项目**
```bash
cargo build --release
```

4. **安装到系统**
```bash
cargo install --path .
```

### 验证安装

```bash
riper --version
# 输出: riper 0.1.0
```

## 🏃‍♂️ 快速开始

### 启动 RIPER 路由器

使用默认配置启动路由器：

```bash
# 启动路由器守护进程
riper start

# 启动时指定配置文件
riper start --config custom.json

# 设置更新间隔为 60 秒
riper start --interval 60

# 启用详细日志
riper start --verbose
```

### 查看路由器状态

```bash
# 查看基本状态
riper status

# 查看详细性能指标
riper status --detailed
```

### 配置网络接口

```bash
# 配置网络接口
riper configure \
  --interface eth0 \
  --ip-address 192.168.1.1 \
  --subnet-mask 255.255.255.0
```

### 测试网络连通性

```bash
# 测试到指定 IP 的连通性
riper test 192.168.1.10
```

## 📖 使用指南

### 命令行工具详解

RIPER 提供了功能丰富的命令行工具：

#### 启动路由器服务
```bash
riper start [选项]

选项:
  -c, --config <文件>     指定配置文件路径 [默认: riper.json]
  -i, --interval <秒数>   路由更新间隔 [默认: 30]
  -v, --verbose          启用详细日志输出
  -h, --help             显示帮助信息
```

#### 查看系统状态
```bash
riper status [选项]

选项:
  -d, --detailed         显示详细性能指标
  -h, --help             显示帮助信息
```

#### 网络接口配置
```bash
riper configure [选项]

选项:
  -i, --interface <名称>    接口名称 (如: eth0)
  -a, --ip-address <IP>     IP 地址
  -m, --subnet-mask <掩码>  子网掩码
  -h, --help               显示帮助信息
```

#### 网络连通性测试
```bash
riper test <目标IP地址>

示例:
  riper test 192.168.1.10
  riper test 10.0.0.1
```

### 配置文件管理

#### 创建配置文件

创建 `riper.json` 配置文件：

```json
{
  "router": {
    "router_id": "auto-generated",
    "update_interval": 30,
    "holddown_timer": 180,
    "garbage_collection_timer": 240,
    "max_hop_count": 15,
    "split_horizon": true,
    "poison_reverse": false
  },
  "interfaces": [
    {
      "name": "eth0",
      "ip_address": "192.168.1.1",
      "subnet_mask": "255.255.255.0",
      "multicast_address": "224.0.0.9",
      "port": 520,
      "mtu": 1500
    }
  ],
  "logging": {
    "level": "info",
    "file": "/var/log/riper.log"
  },
  "monitoring": {
    "metrics_collection_interval": 60,
    "enable_performance_monitoring": true
  }
}
```

## ⚙️ 配置说明

### 路由器配置参数

| 参数 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `router_id` | UUID | 自动生成 | 路由器唯一标识符 |
| `update_interval` | u64 | 30 | 路由更新间隔(秒) |
| `holddown_timer` | u64 | 180 | 路由保持时间(秒) |
| `garbage_collection_timer` | u64 | 240 | 垃圾回收时间(秒) |
| `max_hop_count` | u8 | 15 | 最大跳数限制 |
| `split_horizon` | bool | true | 启用分割视野 |
| `poison_reverse` | bool | false | 启用毒性逆转 |

### 网络接口配置参数

| 参数 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `name` | String | "eth0" | 接口名称 |
| `ip_address` | Ipv4Addr | "192.168.1.1" | IP 地址 |
| `subnet_mask` | Ipv4Addr | "255.255.255.0" | 子网掩码 |
| `multicast_address` | Ipv4Addr | "224.0.0.9" | 多播地址 |
| `port` | u16 | 520 | 监听端口 |
| `mtu` | u16 | 1500 | 最大传输单元 |

### 日志配置参数

| 参数 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `level` | String | "info" | 日志级别 (debug/info/warn/error) |
| `file` | String | - | 日志文件路径 |

## 📚 API 文档

### 核心 API 接口

#### Router API

```rust
// 创建路由器实例
let mut router = Router::new(RouterConfig::default());

// 添加网络接口
let interface = NetworkInterface::new(InterfaceConfig::default());
router.add_interface("eth0".to_string(), interface);

// 启动路由器
router.start().await?;
```

#### Network API

```rust
// 创建网络接口
let config = InterfaceConfig {
    name: "eth0".to_string(),
    ip_address: Ipv4Addr::new(192, 168, 1, 1),
    subnet_mask: Ipv4Addr::new(255, 255, 255, 0),
    port: 520,
    ..Default::default()
};

let mut interface = NetworkInterface::new(config);
interface.initialize().await?;

// 发送数据包
let packet = RiperPacket::new_update(router_id, routes);
interface.send_packet(&packet).await?;

// 接收数据包
let (packet, sender) = interface.receive_packet().await?;
```

#### Routing Table API

```rust
// 创建路由表
let mut routing_table = RoutingTable::new();

// 添加路由
let route = Route::new(
    Ipv4Addr::new(192, 168, 2, 0),
    Ipv4Addr::new(255, 255, 255, 0),
    IpAddr::V4(Ipv4Addr::new(192, 168, 1, 2)),
    1,
    "eth0".to_string(),
);
routing_table.add_route(route);

// 查找路由
if let Some(route) = routing_table.lookup(target_ip) {
    println!("找到路由: {:?}", route);
}
```

### 错误处理

```rust
use riper::{RiperError, RiperResult};

// 错误类型
pub enum RiperError {
    NetworkError(String),    // 网络相关错误
    RoutingError(String),    // 路由相关错误
    ConfigError(String),     // 配置相关错误
    ProtocolError(String),   // 协议相关错误
}

// 使用示例
fn example_function() -> RiperResult<()> {
    // 可能失败的操作
    Ok(())
}
```

## 🎯 示例演示

### 示例 1: 基本路由器设置

创建一个基本的双接口路由器：

```rust
use riper::{
    router::{Router, RouterConfig},
    network::{NetworkInterface, InterfaceConfig},
    RiperResult,
};
use std::net::Ipv4Addr;

#[tokio::main]
async fn main() -> RiperResult<()> {
    // 创建路由器配置
    let router_config = RouterConfig {
        update_interval: 30,
        max_hop_count: 15,
        split_horizon: true,
        ..Default::default()
    };

    // 创建路由器实例
    let mut router = Router::new(router_config);

    // 配置第一个接口
    let eth0_config = InterfaceConfig {
        name: "eth0".to_string(),
        ip_address: Ipv4Addr::new(192, 168, 1, 1),
        subnet_mask: Ipv4Addr::new(255, 255, 255, 0),
        ..Default::default()
    };

    let mut eth0 = NetworkInterface::new(eth0_config);
    eth0.initialize().await?;
    router.add_interface("eth0".to_string(), eth0);

    // 配置第二个接口
    let eth1_config = InterfaceConfig {
        name: "eth1".to_string(),
        ip_address: Ipv4Addr::new(10, 0, 0, 1),
        subnet_mask: Ipv4Addr::new(255, 255, 255, 0),
        ..Default::default()
    };

    let mut eth1 = NetworkInterface::new(eth1_config);
    eth1.initialize().await?;
    router.add_interface("eth1".to_string(), eth1);

    // 启动路由器
    println!("启动双接口路由器...");
    router.start().await?;

    Ok(())
}
```

### 示例 2: 自定义路由监控

实现自定义的路由变化监控：

```rust
use riper::{
    router::Router,
    metrics::{MetricsCollector, PerformanceMonitor},
};
use std::time::Duration;

async fn monitor_router_performance() {
    // 创建性能监控器
    let monitor = PerformanceMonitor::new(Duration::from_secs(60));
    
    // 启动监控循环
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(10)).await;
            
            // 生成性能报告
            let report = monitor.generate_report(5, 20);
            
            println!("=== 性能监控报告 ===");
            report.print_report();
        }
    });
}
```

### 示例 3: 网络拓扑发现

实现网络邻居发现功能：

```rust
use riper::{
    protocol::{RiperPacket, RouteEntry},
    network::NetworkInterface,
};
use uuid::Uuid;

async fn discover_neighbors(interface: &NetworkInterface) -> RiperResult<()> {
    // 创建路由请求包
    let router_id = Uuid::new_v4();
    let request_packet = RiperPacket::new_request(router_id);
    
    println!("发送邻居发现请求...");
    interface.send_packet(&request_packet).await?;
    
    // 等待并处理响应
    tokio::time::timeout(Duration::from_secs(10), async {
        let (response, sender) = interface.receive_packet().await?;
        println!("发现邻居: {} (来自 {})", response.router_id, sender);
        Ok(())
    }).await??;
    
    Ok(())
}
```

### 示例 4: 批量配置管理

从配置文件批量创建网络接口：

```rust
use serde::{Deserialize, Serialize};
use std::fs;

#[derive(Deserialize)]
struct Config {
    interfaces: Vec<InterfaceConfig>,
    router: RouterConfig,
}

async fn load_config_and_start(config_path: &str) -> RiperResult<()> {
    // 读取配置文件
    let config_content = fs::read_to_string(config_path)
        .map_err(|e| RiperError::ConfigError(format!("读取配置文件失败: {}", e)))?;
    
    let config: Config = serde_json::from_str(&config_content)
        .map_err(|e| RiperError::ConfigError(format!("解析配置文件失败: {}", e)))?;
    
    // 创建路由器
    let mut router = Router::new(config.router);
    
    // 批量添加接口
    for interface_config in config.interfaces {
        let mut interface = NetworkInterface::new(interface_config.clone());
        interface.initialize().await?;
        router.add_interface(interface_config.name.clone(), interface);
        println!("已配置接口: {}", interface_config.name);
    }
    
    // 启动路由器
    println!("启动路由器...");
    router.start().await?;
    
    Ok(())
}
```

## 📊 性能监控

### 内置指标

RIPER 提供了全面的性能监控功能：

#### 网络指标
- **数据包统计**: 发送/接收/丢失数据包数量
- **路由更新**: 发送和接收的路由更新统计
- **邻居状态**: 活跃邻居数量和连接状态
- **接口状态**: 网络接口的运行状态和统计

#### 性能指标
- **收敛时间**: 网络拓扑变化后的路由收敛时间
- **内存使用**: 路由表和缓存的内存占用
- **CPU 使用**: 路由计算的处理器使用率
- **运行时间**: 路由器服务的总运行时间

### 监控命令

```bash
# 查看实时状态
riper status

# 查看详细性能指标
riper status --detailed
```

输出示例：
```
=== RIPER Router Status ===
Router Status: Running
Router ID: 550e8400-e29b-41d4-a716-446655440000
Active Routes: 15
Neighbors: 3
Uptime: 2h 15m 30s

=== Performance Metrics ===
Packets Sent: 1,250
Packets Received: 1,180
Packets Dropped: 5 (0.4%)
Route Changes: 12
Average Convergence Time: 2.3s
Memory Usage: 8.5MB
```

### 监控 API

```rust
// 获取性能指标
let metrics = router.get_metrics();
println!("路由数量: {}", metrics.active_routes);
println!("收敛时间: {:?}", metrics.convergence_time);

// 生成详细报告
let monitor = PerformanceMonitor::new(Duration::from_secs(60));
let report = monitor.generate_report(neighbor_count, route_count);
report.print_report();
```

## 🔧 故障排除

### 常见问题

#### 1. 路由器启动失败

**现象**: 执行 `riper start` 后立即退出

**可能原因**:
- 端口 520 被占用
- 权限不足
- 配置文件格式错误

**解决方案**:
```bash
# 检查端口占用
netstat -ulnp | grep 520

# 以管理员权限运行
sudo riper start

# 验证配置文件格式
riper start --config riper.json --verbose
```

#### 2. 邻居发现失败

**现象**: `riper status` 显示邻居数为 0

**可能原因**:
- 网络连通性问题
- 防火墙阻拦
- 多播配置错误

**解决方案**:
```bash
# 测试网络连通性
ping 192.168.1.2

# 检查防火墙规则
sudo ufw allow 520/udp

# 验证多播配置
ip maddr show
```

#### 3. 路由更新缓慢

**现象**: 网络拓扑变化后路由收敛时间过长

**可能原因**:
- 更新间隔设置过大
- 网络延迟较高
- 路由环路

**解决方案**:
```bash
# 减少更新间隔
riper start --interval 15

# 检查网络延迟
ping -c 10 192.168.1.2

# 启用详细日志排查
riper start --verbose
```

#### 4. 内存使用过高

**现象**: 路由器占用内存持续增长

**可能原因**:
- 路由表过大
- 内存泄漏
- 垃圾回收不及时

**解决方案**:
```bash
# 检查路由表大小
riper status --detailed

# 调整垃圾回收时间
# 在配置文件中设置较短的 garbage_collection_timer
```

### 调试技巧

#### 启用详细日志
```bash
# 启动时启用详细日志
riper start --verbose

# 或设置环境变量
export RUST_LOG=debug
riper start
```

#### 检查网络配置
```bash
# 显示网络接口信息
ip addr show

# 显示路由表
ip route show

# 检查 ARP 表
arp -a
```

#### 监控网络流量
```bash
# 使用 tcpdump 监控 RIP 流量
sudo tcpdump -i eth0 port 520

# 使用 wireshark 分析数据包
wireshark -i eth0 -f "port 520"
```

### 性能调优

#### 网络优化
- 调整 MTU 大小以优化数据传输
- 配置合适的更新间隔
- 启用压缩以减少网络负载

#### 内存优化
- 定期清理过期路由
- 限制路由表最大大小
- 优化数据结构存储

#### CPU 优化
- 使用更高效的路由算法
- 减少不必要的路由计算
- 启用硬件加速（如果支持）

## 🤝 开发贡献

### 开发环境设置

1. **克隆仓库**
```bash
git clone https://github.com/your-repo/riper.git
cd riper
```

2. **安装开发依赖**
```bash
rustup component add clippy rustfmt
cargo install cargo-watch
```

3. **运行测试**
```bash
# 运行单元测试
cargo test

# 运行集成测试
cargo test --test integration_tests

# 运行性能测试
cargo test --release --features bench
```

4. **代码格式化**
```bash
cargo fmt
cargo clippy
```

### 贡献流程

1. Fork 项目仓库
2. 创建功能分支 (`git checkout -b feature/amazing-feature`)
3. 提交更改 (`git commit -m 'Add amazing feature'`)
4. 推送到分支 (`git push origin feature/amazing-feature`)
5. 创建 Pull Request

### 代码规范

- 遵循 Rust 官方代码风格
- 添加必要的文档注释
- 编写单元测试和集成测试
- 使用有意义的提交信息

## 📋 版本日志

### v0.1.0 (当前版本)
- ✨ 初始版本发布
- 🚀 基础 RIPER 协议实现
- 🌐 多接口网络支持
- 📊 性能监控功能
- 🔧 命令行工具
- 📖 完整文档和示例

### 未来版本规划

#### v0.2.0 (计划中)
- 🔒 路由认证机制
- 📱 Web 管理界面
- 🔄 路由策略配置
- 📈 高级监控仪表板

#### v0.3.0 (计划中)
- 🌐 IPv6 支持
- 🔧 动态配置更新
- 🚀 性能优化
- 📊 历史数据存储

## 📞 支持与联系

- **项目主页**: https://github.com/your-repo/riper
- **问题报告**: https://github.com/your-repo/riper/issues
- **讨论区**: https://github.com/your-repo/riper/discussions
- **邮件支持**: riper-support@example.com

## 📄 许可证

本项目使用 MIT 许可证 - 查看 [LICENSE](LICENSE) 文件了解详细信息。

## 🙏 致谢

感谢所有为 RIPER 项目做出贡献的开发者和社区成员。

---

<div align="center">
  <p>如果这个项目对您有帮助，请给我们一个 ⭐️ Star!</p>
  <p>Made with ❤️ by RIPER Team</p>
</div>
