#!/usr/bin/env python3
"""
RustRoute 测试演示脚本
创建多个路由器实例并演示路由收敛过程
"""

import subprocess
import time
import json
import threading
import signal
import sys
from pathlib import Path

class RouterInstance:
    def __init__(self, name, ip, port, config_file):
        self.name = name
        self.ip = ip
        self.port = port
        self.config_file = config_file
        self.process = None
        
    def create_config(self):
        """创建路由器配置文件"""
        config = {
            "router": {
                "router_id": self.name,
                "update_interval": 10,
                "holddown_timer": 60,
                "garbage_collection_timer": 120,
                "max_hop_count": 15,
                "split_horizon": True,
                "poison_reverse": False
            },
            "interfaces": [
                {
                    "name": f"test-{self.name}",
                    "ip_address": self.ip,
                    "subnet_mask": "255.255.255.0",
                    "multicast_address": "224.0.0.9",
                    "port": self.port,
                    "mtu": 1500
                }
            ],
            "logging": {
                "level": "info",
                "file": f"/tmp/{self.name}.log"
            },
            "monitoring": {
                "metrics_collection_interval": 30,
                "enable_performance_monitoring": True
            }
        }
        
        with open(self.config_file, 'w') as f:
            json.dump(config, f, indent=2)
            
    def start(self):
        """启动路由器实例"""
        print(f"🚀 启动路由器 {self.name} (IP: {self.ip}, Port: {self.port})")
        self.create_config()
        
        cmd = [
            "cargo", "run", "--", 
            "start", 
            "-c", self.config_file,
            "-v"
        ]
        
        self.process = subprocess.Popen(
            cmd,
            cwd="/root/rust-route",
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True
        )
        
    def stop(self):
        """停止路由器实例"""
        if self.process:
            print(f"🛑 停止路由器 {self.name}")
            self.process.terminate()
            self.process.wait()
            
    def get_status(self):
        """获取路由器状态"""
        try:
            result = subprocess.run([
                "cargo", "run", "--", 
                "status", 
                "-c", self.config_file
            ], 
            cwd="/root/rust-route",
            capture_output=True, 
            text=True, 
            timeout=10
            )
            return result.stdout
        except subprocess.TimeoutExpired:
            return f"❌ {self.name} 状态查询超时"
        except Exception as e:
            return f"❌ {self.name} 状态查询失败: {e}"

def print_banner():
    """打印测试横幅"""
    print("=" * 60)
    print("🦀 RustRoute 路由器测试演示")
    print("=" * 60)
    print()

def create_test_topology():
    """创建测试拓扑"""
    print("📋 创建测试拓扑...")
    
    routers = [
        RouterInstance("router-A", "192.168.1.10", 5200, "/tmp/router-A.json"),
        RouterInstance("router-B", "192.168.1.20", 5201, "/tmp/router-B.json"),
        RouterInstance("router-C", "192.168.1.30", 5202, "/tmp/router-C.json"),
    ]
    
    return routers

def test_basic_functionality():
    """测试基本功能"""
    print("\n🔧 测试基本功能...")
    
    # 测试配置命令
    print("  • 测试接口配置...")
    result = subprocess.run([
        "cargo", "run", "--",
        "configure",
        "-i", "test0",
        "-a", "192.168.100.1",
        "-m", "255.255.255.0"
    ], cwd="/root/rust-route", capture_output=True, text=True)
    
    if result.returncode == 0:
        print("    ✅ 接口配置成功")
    else:
        print("    ❌ 接口配置失败")
        print(f"    错误: {result.stderr}")
    
    # 测试连接测试
    print("  • 测试连接功能...")
    result = subprocess.run([
        "cargo", "run", "--",
        "test", "127.0.0.1"
    ], cwd="/root/rust-route", capture_output=True, text=True)
    
    if result.returncode == 0:
        print("    ✅ 连接测试成功")
    else:
        print("    ❌ 连接测试失败")

def run_convergence_test(routers):
    """运行路由收敛测试"""
    print("\n🌐 开始路由收敛测试...")
    
    # 启动所有路由器
    for router in routers:
        router.start()
        time.sleep(2)  # 错开启动时间
    
    print("⏳ 等待路由器启动和初始化...")
    time.sleep(10)
    
    # 检查路由器状态
    print("\n📊 路由器状态报告:")
    for router in routers:
        status = router.get_status()
        print(f"\n{router.name}:")
        print(status)
    
    print("\n⏳ 等待路由收敛...")
    time.sleep(20)
    
    # 再次检查状态
    print("\n📊 收敛后状态报告:")
    for router in routers:
        status = router.get_status()
        print(f"\n{router.name}:")
        print(status)

def cleanup_test_files():
    """清理测试文件"""
    print("\n🧹 清理测试文件...")
    test_files = [
        "/tmp/router-A.json",
        "/tmp/router-B.json", 
        "/tmp/router-C.json",
        "/tmp/router-A.log",
        "/tmp/router-B.log",
        "/tmp/router-C.log"
    ]
    
    for file_path in test_files:
        try:
            Path(file_path).unlink(missing_ok=True)
        except Exception as e:
            print(f"清理文件 {file_path} 失败: {e}")

def signal_handler(sig, frame):
    """信号处理器"""
    print("\n\n🛑 收到中断信号，正在清理...")
    cleanup_test_files()
    sys.exit(0)

def main():
    """主函数"""
    # 注册信号处理器
    signal.signal(signal.SIGINT, signal_handler)
    signal.signal(signal.SIGTERM, signal_handler)
    
    try:
        print_banner()
        
        # 测试基本功能
        test_basic_functionality()
        
        # 创建测试拓扑
        routers = create_test_topology()
        
        # 运行收敛测试
        run_convergence_test(routers)
        
        print("\n🎉 测试完成！")
        
        # 停止所有路由器
        for router in routers:
            router.stop()
            
    except Exception as e:
        print(f"\n❌ 测试过程中发生错误: {e}")
    finally:
        cleanup_test_files()

if __name__ == "__main__":
    main()
