#!/usr/bin/env python3
"""
RustRoute 配置灵活性演示脚本
展示系统的参数化和配置驱动特性
"""

import json
import subprocess
import sys
import time
import os

def run_command(cmd, description):
    """执行命令并显示结果"""
    print(f"\n{'='*60}")
    print(f"🔧 {description}")
    print(f"💻 执行命令: {cmd}")
    print(f"{'='*60}")
    
    try:
        result = subprocess.run(cmd, shell=True, capture_output=True, text=True, timeout=10)
        if result.stdout:
            print(result.stdout)
        if result.stderr and result.returncode != 0:
            print(f"错误: {result.stderr}")
        return result.returncode == 0
    except subprocess.TimeoutExpired:
        print("⏰ 命令超时（正常，某些网络命令可能需要时间）")
        return True
    except Exception as e:
        print(f"❌ 执行失败: {e}")
        return False

def create_custom_config(filename, config_data):
    """创建自定义配置文件"""
    print(f"\n📝 创建配置文件: {filename}")
    with open(filename, 'w', encoding='utf-8') as f:
        json.dump(config_data, f, indent=2)
    print(f"✅ 配置文件创建完成")
    
    # 显示配置内容
    print("\n📋 配置文件内容:")
    print("-" * 40)
    with open(filename, 'r') as f:
        print(f.read())
    print("-" * 40)

def main():
    print("🦀 RustRoute 配置灵活性演示")
    print("="*60)
    print("本演示展示 RustRoute 的完全参数化和配置驱动特性")
    print("="*60)
    
    # 切换到项目目录
    os.chdir("/root/rust-route")
    
    # 1. 展示默认配置
    print("\n🎯 第一部分：默认配置系统")
    run_command("cargo run -- --help", "查看所有可配置参数")
    
    # 2. 创建多个不同的配置文件
    print("\n🎯 第二部分：多环境配置演示")
    
    # 开发环境配置
    dev_config = {
        "router_id": "dev-router-001",
        "port": 5200,
        "rip_version": 2,
        "interfaces": [
            {
                "name": "dev0",
                "ip_address": "10.0.1.100",
                "subnet_mask": "255.255.255.0",
                "enabled": True
            },
            {
                "name": "dev1",
                "ip_address": "10.0.2.100",
                "subnet_mask": "255.255.255.0",
                "enabled": True
            }
        ],
        "update_interval": 15,
        "holddown_timer": 90,
        "garbage_collection_timer": 120,
        "max_hop_count": 10,
        "split_horizon": True,
        "poison_reverse": True
    }
    
    # 生产环境配置
    prod_config = {
        "router_id": "prod-router-001",
        "port": 520,
        "rip_version": 2,
        "interfaces": [
            {
                "name": "eth0",
                "ip_address": "192.168.100.1",
                "subnet_mask": "255.255.255.0",
                "enabled": True
            },
            {
                "name": "eth1",
                "ip_address": "192.168.200.1",
                "subnet_mask": "255.255.255.0",
                "enabled": True
            }
        ],
        "update_interval": 30,
        "holddown_timer": 180,
        "garbage_collection_timer": 240,
        "max_hop_count": 15,
        "split_horizon": True,
        "poison_reverse": False
    }
    
    # 测试环境配置
    test_config = {
        "router_id": "test-router-001",
        "port": 5201,
        "rip_version": 2,
        "interfaces": [
            {
                "name": "test0",
                "ip_address": "172.16.1.1",
                "subnet_mask": "255.255.0.0",
                "enabled": True
            }
        ],
        "update_interval": 5,
        "holddown_timer": 30,
        "garbage_collection_timer": 60,
        "max_hop_count": 5,
        "split_horizon": False,
        "poison_reverse": False
    }
    
    # 创建配置文件
    create_custom_config("dev-config.json", dev_config)
    create_custom_config("prod-config.json", prod_config)
    create_custom_config("test-config.json", test_config)
    
    # 3. 使用不同配置启动
    print("\n🎯 第三部分：配置驱动启动演示")
    
    configs = [
        ("dev-config.json", "开发环境配置"),
        ("prod-config.json", "生产环境配置"),
        ("test-config.json", "测试环境配置")
    ]
    
    for config_file, description in configs:
        print(f"\n📊 {description}")
        run_command(f"timeout 3 cargo run -- start --config {config_file} --verbose", 
                   f"使用 {description} 启动路由器")
    
    # 4. 动态接口配置
    print("\n🎯 第四部分：动态接口配置演示")
    
    interface_configs = [
        ("user-home", "192.168.1.100", "255.255.255.0", "用户家庭网络"),
        ("user-office", "10.0.50.100", "255.255.0.0", "用户办公网络"),
        ("user-cloud", "172.31.100.1", "255.255.240.0", "用户云环境网络")
    ]
    
    for interface, ip, mask, desc in interface_configs:
        run_command(f"cargo run -- configure --interface {interface} --ip-address {ip} --subnet-mask {mask}",
                   f"配置 {desc}")
    
    # 5. 命令行参数覆盖
    print("\n🎯 第五部分：命令行参数覆盖演示")
    
    intervals = [5, 15, 30, 60]
    for interval in intervals:
        run_command(f"timeout 2 cargo run -- start --config test-config.json --interval {interval} --verbose",
                   f"使用 {interval} 秒更新间隔启动")
    
    # 6. 连通性测试
    print("\n🎯 第六部分：灵活的网络测试")
    
    test_targets = [
        ("127.0.0.1", "本地回环"),
        ("8.8.8.8", "Google DNS"),
        ("1.1.1.1", "Cloudflare DNS"),
        ("192.168.1.1", "默认网关（可能）")
    ]
    
    for target, desc in test_targets:
        run_command(f"timeout 5 cargo run -- test {target}",
                   f"测试连接到 {desc}")
    
    # 7. 状态查看
    print("\n🎯 第七部分：状态监控")
    run_command("cargo run -- status", "基本状态查看")
    run_command("cargo run -- status --detailed", "详细状态查看")
    
    # 清理临时文件
    print("\n🧹 清理临时配置文件")
    for config_file, _ in configs:
        try:
            os.remove(config_file)
            print(f"✅ 删除 {config_file}")
        except:
            pass
    
    print("\n" + "="*60)
    print("🎉 配置灵活性演示完成！")
    print("="*60)
    print("✅ RustRoute 是完全参数化和配置驱动的系统")
    print("✅ 用户可以:")
    print("   📝 使用 JSON 配置文件自定义所有参数")
    print("   🔧 通过命令行参数覆盖配置")
    print("   🌐 配置任意 IP 地址和网络接口")
    print("   ⚙️  调整所有 RIP 协议参数")
    print("   🏢 支持多环境部署（开发/测试/生产）")
    print("="*60)

if __name__ == "__main__":
    main()
