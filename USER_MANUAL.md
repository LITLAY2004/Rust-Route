# RustRoute 用户使用手册

## 目录
1. [简介](#简介)
2. [系统要求](#系统要求)
3. [安装指南](#安装指南)
4. [快速开始](#快速开始)
5. [配置详解](#配置详解)
6. [命令详解](#命令详解)
7. [配置文件详解](#配置文件详解)
8. [网络部署](#网络部署)
9. [故障排除](#故障排除)
10. [高级用法](#高级用法)
11. [API参考](#api参考)

---

## 简介

RustRoute 是一个基于 Rust 语言开发的 RIP（Routing Information Protocol）路由器实现。它支持 RIPv1 和 RIPv2 协议，提供完全参数化的配置系统，可以在真实网络环境中部署和使用。

### 主要特性

- **完整的 RIP 协议支持**：支持 RIPv1 和 RIPv2
- **参数化配置系统**：支持 JSON 配置文件和命令行参数覆盖
- **动态接口配置**：可以运行时配置网络接口
- **网络连通性测试**：内置网络测试工具
- **多环境支持**：支持开发、测试、生产环境
- **实时状态监控**：详细的路由器状态和指标监控
- **用户友好界面**：彩色输出和进度指示器

---

## 系统要求

### 硬件要求
- CPU：x86_64 或 ARM64 架构
- 内存：最少 256MB RAM
- 存储：至少 50MB 可用空间
- 网络：支持以太网接口

### 软件要求
- 操作系统：Linux (Ubuntu 18.04+, CentOS 7+, Debian 9+)
- Rust：1.70.0 或更高版本
- 权限：需要 root 权限进行网络配置

---

## 安装指南

### 方法一：从源码编译

```bash
# 1. 克隆仓库
git clone https://github.com/your-org/rust-route.git
cd rust-route

# 2. 编译项目
cargo build --release

# 3. 安装到系统路径（可选）
sudo cp target/release/rust-route /usr/local/bin/
```

### 方法二：使用预编译二进制

```bash
# 下载最新版本
wget https://github.com/your-org/rust-route/releases/latest/download/rust-route-linux-x64.tar.gz

# 解压并安装
tar -xzf rust-route-linux-x64.tar.gz
sudo mv rust-route /usr/local/bin/
```

### 验证安装

```bash
rust-route --version
rust-route --help
```

---

## 快速开始

### 1. 检查系统状态

```bash
rust-route status
```

### 2. 基本配置

创建配置文件 `config.json`：

```json
{
  "router_id": "192.168.1.1",
  "interfaces": [
    {
      "name": "eth0",
      "ip_address": "192.168.1.1",
      "subnet_mask": "255.255.255.0",
      "enabled": true
    }
  ],
  "rip": {
    "version": 2,
    "update_interval": 30,
    "timeout": 180,
    "garbage_collection": 120
  }
}
```

### 3. 启动路由器

```bash
# 使用默认配置
rust-route start

# 使用指定配置文件
rust-route start --config config.json

# 开发模式启动
rust-route start --environment development
```

### 4. 测试连通性

```bash
# 测试邻居连通性
rust-route test --target 192.168.1.2

# 测试所有配置的邻居
rust-route test --all
```

---

## 配置详解

### 配置优先级

RustRoute 使用以下配置优先级（从高到低）：

1. 命令行参数
2. 环境变量
3. 配置文件
4. 默认值

### 环境变量

```bash
export RUST_ROUTE_ROUTER_ID="192.168.1.1"
export RUST_ROUTE_LOG_LEVEL="info"
export RUST_ROUTE_CONFIG_FILE="/etc/rust-route/config.json"
```

### 命令行参数覆盖

```bash
# 覆盖路由器ID
rust-route start --router-id 10.0.0.1

# 覆盖日志级别
rust-route start --log-level debug

# 覆盖接口配置
rust-route start --interface eth0:192.168.1.1/24
```

---

## 命令详解

### start - 启动路由器

```bash
rust-route start [OPTIONS]
```

**选项：**
- `--config <FILE>` : 指定配置文件路径
- `--router-id <IP>` : 设置路由器ID
- `--environment <ENV>` : 设置运行环境 (development|test|production)
- `--log-level <LEVEL>` : 设置日志级别 (error|warn|info|debug|trace)
- `--interface <SPEC>` : 配置接口 (格式: name:ip/mask)
- `--daemon` : 以守护进程模式运行

**示例：**

```bash
# 基本启动
rust-route start

# 生产环境启动
rust-route start --environment production --daemon

# 自定义配置启动
rust-route start --config /etc/rust-route/prod.json --log-level info
```

### status - 查看状态

```bash
rust-route status [OPTIONS]
```

**选项：**
- `--json` : 以JSON格式输出
- `--watch` : 持续监控模式
- `--interval <SECONDS>` : 更新间隔（配合--watch使用）

**示例：**

```bash
# 查看基本状态
rust-route status

# JSON格式输出
rust-route status --json

# 持续监控
rust-route status --watch --interval 5
```

### configure - 配置管理

```bash
rust-route configure [SUBCOMMAND]
```

**子命令：**

#### interfaces - 配置接口

```bash
rust-route configure interfaces [OPTIONS]
```

**选项：**
- `--add <SPEC>` : 添加接口 (格式: name:ip/mask)
- `--remove <NAME>` : 删除接口
- `--enable <NAME>` : 启用接口
- `--disable <NAME>` : 禁用接口
- `--list` : 列出所有接口

**示例：**

```bash
# 添加接口
rust-route configure interfaces --add eth1:192.168.2.1/24

# 禁用接口
rust-route configure interfaces --disable eth0

# 列出接口
rust-route configure interfaces --list
```

### test - 连通性测试

```bash
rust-route test [OPTIONS]
```

**选项：**
- `--target <IP>` : 测试指定目标
- `--all` : 测试所有邻居
- `--timeout <SECONDS>` : 设置超时时间
- `--count <NUM>` : 设置测试次数

**示例：**

```bash
# 测试单个目标
rust-route test --target 192.168.1.2 --timeout 10

# 测试所有邻居
rust-route test --all --count 3
```

---

## 配置文件详解

### 完整配置文件示例

```json
{
  "router_id": "192.168.1.1",
  "environment": "production",
  "logging": {
    "level": "info",
    "file": "/var/log/rust-route/router.log",
    "console": true,
    "max_file_size": "10MB",
    "max_files": 5
  },
  "interfaces": [
    {
      "name": "eth0",
      "ip_address": "192.168.1.1",
      "subnet_mask": "255.255.255.0",
      "enabled": true,
      "description": "主网络接口"
    },
    {
      "name": "eth1",
      "ip_address": "10.0.1.1",
      "subnet_mask": "255.255.255.0",
      "enabled": true,
      "description": "备用网络接口"
    }
  ],
  "rip": {
    "version": 2,
    "update_interval": 30,
    "timeout": 180,
    "garbage_collection": 120,
    "split_horizon": true,
    "poison_reverse": true,
    "authentication": {
      "enabled": false,
      "type": "simple",
      "password": ""
    }
  },
  "static_routes": [
    {
      "destination": "0.0.0.0",
      "mask": "0.0.0.0",
      "gateway": "192.168.1.254",
      "metric": 1,
      "description": "默认路由"
    }
  ],
  "neighbors": [
    {
      "ip": "192.168.1.2",
      "description": "路由器B"
    },
    {
      "ip": "192.168.1.3",
      "description": "路由器C"
    }
  ],
  "metrics": {
    "enabled": true,
    "collection_interval": 60,
    "retention_period": "7d"
  },
  "security": {
    "max_packet_size": 1500,
    "rate_limit": {
      "enabled": true,
      "packets_per_second": 100
    }
  }
}
```

### 配置字段说明

#### 基本设置

- `router_id`: 路由器唯一标识符（IP地址格式）
- `environment`: 运行环境（development/test/production）

#### 日志配置

- `logging.level`: 日志级别
- `logging.file`: 日志文件路径
- `logging.console`: 是否在控制台输出
- `logging.max_file_size`: 单个日志文件最大大小
- `logging.max_files`: 保留的日志文件数量

#### 接口配置

- `interfaces[].name`: 接口名称
- `interfaces[].ip_address`: IP地址
- `interfaces[].subnet_mask`: 子网掩码
- `interfaces[].enabled`: 是否启用
- `interfaces[].description`: 接口描述

#### RIP协议配置

- `rip.version`: RIP版本（1或2）
- `rip.update_interval`: 更新间隔（秒）
- `rip.timeout`: 路由超时时间（秒）
- `rip.garbage_collection`: 垃圾回收时间（秒）
- `rip.split_horizon`: 是否启用水平分割
- `rip.poison_reverse`: 是否启用毒性逆转

---

## 网络部署

### 单路由器部署

适用于小型网络或测试环境：

```bash
# 1. 配置网络接口
sudo ip addr add 192.168.1.1/24 dev eth0
sudo ip link set eth0 up

# 2. 启动路由器
rust-route start --router-id 192.168.1.1 --interface eth0:192.168.1.1/24
```

### 多路由器网络

在多路由器环境中部署：

#### 路由器A配置

```json
{
  "router_id": "192.168.1.1",
  "interfaces": [
    {
      "name": "eth0",
      "ip_address": "192.168.1.1",
      "subnet_mask": "255.255.255.0",
      "enabled": true
    },
    {
      "name": "eth1",
      "ip_address": "10.0.1.1",
      "subnet_mask": "255.255.255.0",
      "enabled": true
    }
  ]
}
```

#### 路由器B配置

```json
{
  "router_id": "192.168.1.2",
  "interfaces": [
    {
      "name": "eth0",
      "ip_address": "192.168.1.2",
      "subnet_mask": "255.255.255.0",
      "enabled": true
    },
    {
      "name": "eth1",
      "ip_address": "10.0.2.1",
      "subnet_mask": "255.255.255.0",
      "enabled": true
    }
  ]
}
```

### 部署脚本

创建自动化部署脚本 `deploy.sh`：

```bash
#!/bin/bash

# 设置网络接口
setup_interface() {
    local interface=$1
    local ip=$2
    local mask=$3
    
    echo "配置接口 $interface: $ip/$mask"
    sudo ip addr add $ip/$mask dev $interface
    sudo ip link set $interface up
}

# 启动路由器
start_router() {
    local config_file=$1
    
    echo "启动RustRoute..."
    sudo ./rust-route start --config $config_file --daemon
}

# 主程序
main() {
    setup_interface eth0 192.168.1.1 24
    setup_interface eth1 10.0.1.1 24
    start_router router_config.json
    echo "路由器部署完成"
}

main "$@"
```

---

## 故障排除

### 常见问题

#### 1. 路由器启动失败

**症状：** 运行 `rust-route start` 时出错

**可能原因：**
- 权限不足
- 配置文件错误
- 网络接口不存在

**解决方案：**

```bash
# 检查权限
sudo rust-route start

# 验证配置文件
rust-route start --config config.json --dry-run

# 检查网络接口
ip link show
```

#### 2. 无法收到RIP更新

**症状：** 路由表没有更新

**可能原因：**
- 防火墙阻止
- 网络配置错误
- RIP版本不匹配

**解决方案：**

```bash
# 检查防火墙
sudo iptables -L | grep 520
sudo ufw status

# 开放RIP端口
sudo ufw allow 520/udp

# 检查网络连通性
rust-route test --target 192.168.1.2
```

#### 3. 路由收敛慢

**症状：** 网络变化后路由更新缓慢

**解决方案：**

调整RIP定时器：

```json
{
  "rip": {
    "update_interval": 15,
    "timeout": 90,
    "garbage_collection": 60
  }
}
```

### 调试技巧

#### 启用详细日志

```bash
# 启用调试日志
rust-route start --log-level debug

# 查看日志文件
tail -f /var/log/rust-route/router.log
```

#### 网络抓包

```bash
# 抓取RIP包
sudo tcpdump -i eth0 port 520 -v

# 保存到文件
sudo tcpdump -i eth0 port 520 -w rip_packets.pcap
```

#### 检查路由表

```bash
# 查看系统路由表
ip route show

# 查看RustRoute路由表
rust-route status --json | jq '.routing_table'
```

---

## 高级用法

### 配置模板

使用配置模板简化部署：

```bash
# 生成配置模板
rust-route configure template --type basic > basic_config.json
rust-route configure template --type advanced > advanced_config.json
```

### 批量配置

使用脚本批量配置多个路由器：

```python
#!/usr/bin/env python3
import json
import subprocess

def generate_config(router_id, interfaces):
    config = {
        "router_id": router_id,
        "interfaces": interfaces,
        "rip": {
            "version": 2,
            "update_interval": 30
        }
    }
    return config

def deploy_router(config_file):
    subprocess.run(['rust-route', 'start', '--config', config_file])

# 批量部署
routers = [
    ("192.168.1.1", [{"name": "eth0", "ip_address": "192.168.1.1", "subnet_mask": "255.255.255.0", "enabled": True}]),
    ("192.168.1.2", [{"name": "eth0", "ip_address": "192.168.1.2", "subnet_mask": "255.255.255.0", "enabled": True}])
]

for router_id, interfaces in routers:
    config = generate_config(router_id, interfaces)
    config_file = f"router_{router_id.replace('.', '_')}.json"
    
    with open(config_file, 'w') as f:
        json.dump(config, f, indent=2)
    
    deploy_router(config_file)
```

### 监控和告警

#### 监控脚本

```bash
#!/bin/bash

# 监控路由器状态
monitor_router() {
    while true; do
        status=$(rust-route status --json)
        
        # 检查路由器是否运行
        if ! echo "$status" | jq -e '.running' > /dev/null; then
            echo "警告：路由器未运行" | mail -s "RustRoute Alert" admin@example.com
        fi
        
        # 检查邻居数量
        neighbor_count=$(echo "$status" | jq '.neighbors | length')
        if [ "$neighbor_count" -lt 2 ]; then
            echo "警告：邻居数量过少 ($neighbor_count)" | mail -s "RustRoute Alert" admin@example.com
        fi
        
        sleep 300  # 每5分钟检查一次
    done
}

monitor_router &
```

### 性能优化

#### 内存优化

```json
{
  "performance": {
    "max_routes": 10000,
    "hash_table_size": 1024,
    "garbage_collection_interval": 300
  }
}
```

#### CPU优化

```json
{
  "threading": {
    "worker_threads": 4,
    "io_threads": 2
  }
}
```

---

## API参考

### HTTP API

RustRoute 提供 REST API 进行远程管理（可选功能）：

#### 启用API服务

```json
{
  "api": {
    "enabled": true,
    "bind_address": "0.0.0.0:8080",
    "auth_token": "your-secret-token"
  }
}
```

#### API端点

##### 获取状态

```bash
curl -H "Authorization: Bearer your-secret-token" \
     http://localhost:8080/api/v1/status
```

##### 获取路由表

```bash
curl -H "Authorization: Bearer your-secret-token" \
     http://localhost:8080/api/v1/routes
```

##### 添加静态路由

```bash
curl -X POST \
     -H "Authorization: Bearer your-secret-token" \
     -H "Content-Type: application/json" \
     -d '{"destination": "10.0.0.0/8", "gateway": "192.168.1.1", "metric": 1}' \
     http://localhost:8080/api/v1/routes
```

### 配置验证API

```bash
# 验证配置文件
curl -X POST \
     -H "Content-Type: application/json" \
     -d @config.json \
     http://localhost:8080/api/v1/config/validate
```

---

## 附录

### A. 默认配置值

```json
{
  "router_id": "127.0.0.1",
  "environment": "development",
  "logging": {
    "level": "info",
    "console": true
  },
  "rip": {
    "version": 2,
    "update_interval": 30,
    "timeout": 180,
    "garbage_collection": 120,
    "split_horizon": true,
    "poison_reverse": false
  }
}
```

### B. 环境变量列表

| 变量名 | 说明 | 默认值 |
|--------|------|--------|
| `RUST_ROUTE_CONFIG_FILE` | 配置文件路径 | `rust-route.json` |
| `RUST_ROUTE_ROUTER_ID` | 路由器ID | `127.0.0.1` |
| `RUST_ROUTE_LOG_LEVEL` | 日志级别 | `info` |
| `RUST_ROUTE_ENVIRONMENT` | 运行环境 | `development` |

### C. 错误代码

| 代码 | 说明 |
|------|------|
| 0 | 成功 |
| 1 | 通用错误 |
| 2 | 配置错误 |
| 3 | 网络错误 |
| 4 | 权限错误 |
| 5 | 资源不足 |

### D. 性能基准

| 指标 | 值 |
|------|-----|
| 最大路由数 | 10,000 |
| 内存使用 | ~50MB |
| CPU使用 | ~5% (单核) |
| 网络延迟 | <1ms |
| 收敛时间 | <30s |

---

## 支持和社区

- **GitHub**: https://github.com/your-org/rust-route
- **文档**: https://rust-route.readthedocs.io
- **问题反馈**: https://github.com/your-org/rust-route/issues
- **社区讨论**: https://github.com/your-org/rust-route/discussions

---

*最后更新: 2024年12月*
*版本: v0.2.0*
