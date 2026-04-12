#!/usr/bin/env python3
"""
ChronoDB 运维 CLI 工具
提供数据备份、恢复、健康检查、性能分析等功能
"""

import argparse
import json
import os
import subprocess
import sys
import time
from datetime import datetime
from pathlib import Path
from typing import Optional

import requests

# 默认配置
DEFAULT_DATA_DIR = "/var/lib/chronodb"
DEFAULT_BACKUP_DIR = "/var/backups/chronodb"
DEFAULT_API_URL = "http://localhost:9090"


class ChronoDBCli:
    def __init__(self, api_url: str = DEFAULT_API_URL, data_dir: str = DEFAULT_DATA_DIR):
        self.api_url = api_url
        self.data_dir = Path(data_dir)
        self.backup_dir = Path(DEFAULT_BACKUP_DIR)
        
    def health_check(self) -> bool:
        """检查 ChronoDB 健康状态"""
        print("=" * 60)
        print("ChronoDB Health Check")
        print("=" * 60)
        
        # 检查 HTTP API
        try:
            response = requests.get(f"{self.api_url}/-/healthy", timeout=5)
            if response.status_code == 200:
                print("✅ HTTP API: Healthy")
            else:
                print(f"❌ HTTP API: Unhealthy (status {response.status_code})")
                return False
        except Exception as e:
            print(f"❌ HTTP API: Error - {e}")
            return False
        
        # 检查就绪状态
        try:
            response = requests.get(f"{self.api_url}/-/ready", timeout=5)
            if response.status_code == 200:
                print("✅ Readiness: Ready")
            else:
                print(f"⚠️  Readiness: Not ready (status {response.status_code})")
        except Exception as e:
            print(f"⚠️  Readiness: Error - {e}")
        
        # 检查数据目录
        if self.data_dir.exists():
            print(f"✅ Data directory: {self.data_dir}")
            # 检查磁盘空间
            stat = os.statvfs(self.data_dir)
            free_gb = (stat.f_bavail * stat.f_frsize) / (1024**3)
            total_gb = (stat.f_blocks * stat.f_frsize) / (1024**3)
            used_gb = total_gb - free_gb
            usage_percent = (used_gb / total_gb) * 100
            
            print(f"   Disk usage: {used_gb:.1f}GB / {total_gb:.1f}GB ({usage_percent:.1f}%)")
            if usage_percent > 90:
                print("   ⚠️  WARNING: Disk usage is above 90%!")
            elif usage_percent > 80:
                print("   ⚠️  WARNING: Disk usage is above 80%!")
        else:
            print(f"❌ Data directory not found: {self.data_dir}")
            return False
        
        # 检查指标
        try:
            response = requests.get(f"{self.api_url}/api/v1/query?query=chronodb_series_total", timeout=10)
            if response.status_code == 200:
                data = response.json()
                if data.get("data", {}).get("result"):
                    series_count = data["data"]["result"][0]["value"][1]
                    print(f"✅ Metrics: {series_count} series")
                else:
                    print("⚠️  Metrics: No data available")
            else:
                print(f"⚠️  Metrics: API returned {response.status_code}")
        except Exception as e:
            print(f"⚠️  Metrics: Error - {e}")
        
        print("\n✅ Health check completed")
        return True
    
    def backup(self, backup_name: Optional[str] = None) -> bool:
        """备份 ChronoDB 数据"""
        print("=" * 60)
        print("ChronoDB Backup")
        print("=" * 60)
        
        # 生成备份名称
        if not backup_name:
            backup_name = f"chronodb_backup_{datetime.now().strftime('%Y%m%d_%H%M%S')}"
        
        backup_path = self.backup_dir / backup_name
        
        # 创建备份目录
        backup_path.mkdir(parents=True, exist_ok=True)
        
        print(f"Backup destination: {backup_path}")
        print(f"Data directory: {self.data_dir}")
        
        # 检查数据目录
        if not self.data_dir.exists():
            print(f"❌ Data directory not found: {self.data_dir}")
            return False
        
        # 创建元数据文件
        metadata = {
            "backup_name": backup_name,
            "backup_time": datetime.now().isoformat(),
            "data_dir": str(self.data_dir),
            "chronodb_version": self._get_version(),
        }
        
        metadata_path = backup_path / "metadata.json"
        with open(metadata_path, "w") as f:
            json.dump(metadata, f, indent=2)
        
        print(f"✅ Created metadata: {metadata_path}")
        
        # 使用 rsync 进行备份
        try:
            cmd = [
                "rsync",
                "-av",
                "--delete",
                "--exclude=*.lock",
                str(self.data_dir) + "/",
                str(backup_path / "data") + "/"
            ]
            
            print(f"\nRunning: {' '.join(cmd)}")
            result = subprocess.run(cmd, capture_output=True, text=True)
            
            if result.returncode == 0:
                print(f"✅ Backup completed successfully")
                print(f"   Backup location: {backup_path}")
                
                # 计算备份大小
                backup_size = self._get_dir_size(backup_path)
                print(f"   Backup size: {backup_size / (1024**2):.1f} MB")
                
                return True
            else:
                print(f"❌ Backup failed: {result.stderr}")
                return False
                
        except Exception as e:
            print(f"❌ Backup error: {e}")
            return False
    
    def restore(self, backup_name: str, force: bool = False) -> bool:
        """从备份恢复 ChronoDB 数据"""
        print("=" * 60)
        print("ChronoDB Restore")
        print("=" * 60)
        
        backup_path = self.backup_dir / backup_name
        
        if not backup_path.exists():
            print(f"❌ Backup not found: {backup_path}")
            return False
        
        print(f"Backup source: {backup_path}")
        print(f"Data directory: {self.data_dir}")
        
        # 检查元数据
        metadata_path = backup_path / "metadata.json"
        if metadata_path.exists():
            with open(metadata_path) as f:
                metadata = json.load(f)
            print(f"\nBackup info:")
            print(f"  Created: {metadata.get('backup_time', 'unknown')}")
            print(f"  Version: {metadata.get('chronodb_version', 'unknown')}")
        
        # 确认恢复
        if not force:
            response = input(f"\n⚠️  This will overwrite data in {self.data_dir}. Continue? [y/N]: ")
            if response.lower() != 'y':
                print("Restore cancelled")
                return False
        
        # 停止服务（如果运行中）
        print("\nStopping ChronoDB service...")
        self._stop_service()
        
        # 备份当前数据
        if self.data_dir.exists():
            current_backup = self.backup_dir / f"pre_restore_{datetime.now().strftime('%Y%m%d_%H%M%S')}"
            print(f"Backing up current data to: {current_backup}")
            try:
                cmd = ["rsync", "-av", str(self.data_dir) + "/", str(current_backup) + "/"]
                subprocess.run(cmd, capture_output=True)
            except Exception as e:
                print(f"⚠️  Failed to backup current data: {e}")
        
        # 执行恢复
        try:
            # 清空当前数据目录
            if self.data_dir.exists():
                import shutil
                shutil.rmtree(self.data_dir)
            
            # 恢复数据
            data_backup_path = backup_path / "data"
            if data_backup_path.exists():
                cmd = ["rsync", "-av", str(data_backup_path) + "/", str(self.data_dir) + "/"]
                result = subprocess.run(cmd, capture_output=True, text=True)
                
                if result.returncode == 0:
                    print(f"✅ Restore completed successfully")
                    print(f"\nStarting ChronoDB service...")
                    self._start_service()
                    return True
                else:
                    print(f"❌ Restore failed: {result.stderr}")
                    return False
            else:
                print(f"❌ Data directory not found in backup: {data_backup_path}")
                return False
                
        except Exception as e:
            print(f"❌ Restore error: {e}")
            return False
    
    def list_backups(self):
        """列出所有备份"""
        print("=" * 60)
        print("ChronoDB Backups")
        print("=" * 60)
        
        if not self.backup_dir.exists():
            print(f"No backup directory found: {self.backup_dir}")
            return
        
        backups = []
        for item in self.backup_dir.iterdir():
            if item.is_dir():
                metadata_path = item / "metadata.json"
                metadata = {}
                if metadata_path.exists():
                    try:
                        with open(metadata_path) as f:
                            metadata = json.load(f)
                    except:
                        pass
                
                size = self._get_dir_size(item)
                backups.append({
                    "name": item.name,
                    "created": metadata.get("backup_time", "unknown"),
                    "size": size,
                    "version": metadata.get("chronodb_version", "unknown")
                })
        
        if not backups:
            print("No backups found")
            return
        
        # 按创建时间排序
        backups.sort(key=lambda x: x["created"], reverse=True)
        
        print(f"{'Name':<40} {'Created':<25} {'Size':<15} {'Version'}")
        print("-" * 100)
        for backup in backups:
            size_str = f"{backup['size'] / (1024**2):.1f} MB"
            print(f"{backup['name']:<40} {backup['created']:<25} {size_str:<15} {backup['version']}")
    
    def clean_old_backups(self, keep_days: int = 7):
        """清理旧备份"""
        print("=" * 60)
        print("Clean Old Backups")
        print("=" * 60)
        
        if not self.backup_dir.exists():
            print(f"No backup directory found: {self.backup_dir}")
            return
        
        cutoff_time = time.time() - (keep_days * 24 * 60 * 60)
        removed_count = 0
        
        for item in self.backup_dir.iterdir():
            if item.is_dir():
                item_time = item.stat().st_mtime
                if item_time < cutoff_time:
                    print(f"Removing old backup: {item.name}")
                    import shutil
                    shutil.rmtree(item)
                    removed_count += 1
        
        print(f"\nRemoved {removed_count} old backups")
        print(f"Kept backups from last {keep_days} days")
    
    def performance_report(self):
        """生成性能报告"""
        print("=" * 60)
        print("ChronoDB Performance Report")
        print("=" * 60)
        
        # 查询性能指标
        metrics_queries = [
            ("Series Count", "chronodb_series_total"),
            ("Sample Count", "chronodb_samples_total"),
            ("Query Latency", "chronodb_query_latency_ms"),
            ("Write Latency", "chronodb_write_latency_ms"),
            ("Storage Size", "chronodb_storage_bytes"),
        ]
        
        print("\n📊 Current Metrics:")
        print("-" * 60)
        
        for name, query in metrics_queries:
            try:
                response = requests.get(
                    f"{self.api_url}/api/v1/query",
                    params={"query": query},
                    timeout=10
                )
                if response.status_code == 200:
                    data = response.json()
                    if data.get("data", {}).get("result"):
                        value = data["data"]["result"][0]["value"][1]
                        print(f"  {name}: {value}")
                    else:
                        print(f"  {name}: N/A")
                else:
                    print(f"  {name}: Error ({response.status_code})")
            except Exception as e:
                print(f"  {name}: Error - {e}")
        
        # 系统资源使用
        print("\n💻 System Resources:")
        print("-" * 60)
        
        try:
            # 获取 CPU 使用率
            response = requests.get(
                f"{self.api_url}/api/v1/query",
                params={"query": "chronodb_cpu_usage_percent"},
                timeout=10
            )
            if response.status_code == 200:
                data = response.json()
                if data.get("data", {}).get("result"):
                    value = data["data"]["result"][0]["value"][1]
                    print(f"  CPU Usage: {float(value):.1f}%")
        except:
            pass
        
        try:
            # 获取内存使用
            response = requests.get(
                f"{self.api_url}/api/v1/query",
                params={"query": "chronodb_memory_usage_bytes"},
                timeout=10
            )
            if response.status_code == 200:
                data = response.json()
                if data.get("data", {}).get("result"):
                    value = int(data["data"]["result"][0]["value"][1])
                    print(f"  Memory Usage: {value / (1024**2):.1f} MB")
        except:
            pass
        
        print("\n✅ Performance report generated")
    
    def _get_version(self) -> str:
        """获取 ChronoDB 版本"""
        try:
            response = requests.get(f"{self.api_url}/api/v1/status/buildinfo", timeout=5)
            if response.status_code == 200:
                data = response.json()
                return data.get("data", {}).get("version", "unknown")
        except:
            pass
        return "unknown"
    
    def _get_dir_size(self, path: Path) -> int:
        """获取目录大小"""
        total = 0
        for entry in path.rglob("*"):
            if entry.is_file():
                total += entry.stat().st_size
        return total
    
    def _stop_service(self):
        """停止 ChronoDB 服务"""
        try:
            subprocess.run(["systemctl", "stop", "chronodb"], capture_output=True)
            print("  Service stopped")
        except:
            print("  Could not stop service (may not be running)")
    
    def _start_service(self):
        """启动 ChronoDB 服务"""
        try:
            subprocess.run(["systemctl", "start", "chronodb"], capture_output=True)
            print("  Service started")
        except:
            print("  Could not start service (may not be installed)")


def main():
    parser = argparse.ArgumentParser(
        description="ChronoDB 运维 CLI 工具",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
示例:
  %(prog)s health                    # 健康检查
  %(prog)s backup                    # 创建备份
  %(prog)s restore <backup_name>     # 恢复备份
  %(prog)s list-backups              # 列出备份
  %(prog)s clean-backups --days 7    # 清理7天前的备份
  %(prog)s performance               # 性能报告
        """
    )
    
    parser.add_argument(
        "--api-url",
        default=DEFAULT_API_URL,
        help=f"ChronoDB API URL (default: {DEFAULT_API_URL})"
    )
    parser.add_argument(
        "--data-dir",
        default=DEFAULT_DATA_DIR,
        help=f"ChronoDB data directory (default: {DEFAULT_DATA_DIR})"
    )
    
    subparsers = parser.add_subparsers(dest="command", help="可用命令")
    
    # health 命令
    subparsers.add_parser("health", help="执行健康检查")
    
    # backup 命令
    backup_parser = subparsers.add_parser("backup", help="创建数据备份")
    backup_parser.add_argument("--name", help="备份名称（可选）")
    
    # restore 命令
    restore_parser = subparsers.add_parser("restore", help="从备份恢复数据")
    restore_parser.add_argument("backup_name", help="备份名称")
    restore_parser.add_argument("--force", action="store_true", help="强制恢复，不提示确认")
    
    # list-backups 命令
    subparsers.add_parser("list-backups", help="列出所有备份")
    
    # clean-backups 命令
    clean_parser = subparsers.add_parser("clean-backups", help="清理旧备份")
    clean_parser.add_argument("--days", type=int, default=7, help="保留最近几天的备份（默认：7）")
    
    # performance 命令
    subparsers.add_parser("performance", help="生成性能报告")
    
    args = parser.parse_args()
    
    if not args.command:
        parser.print_help()
        sys.exit(1)
    
    cli = ChronoDBCli(api_url=args.api_url, data_dir=args.data_dir)
    
    if args.command == "health":
        success = cli.health_check()
        sys.exit(0 if success else 1)
    
    elif args.command == "backup":
        success = cli.backup(args.name)
        sys.exit(0 if success else 1)
    
    elif args.command == "restore":
        success = cli.restore(args.backup_name, args.force)
        sys.exit(0 if success else 1)
    
    elif args.command == "list-backups":
        cli.list_backups()
    
    elif args.command == "clean-backups":
        cli.clean_old_backups(args.days)
    
    elif args.command == "performance":
        cli.performance_report()
    
    else:
        parser.print_help()
        sys.exit(1)


if __name__ == "__main__":
    main()
