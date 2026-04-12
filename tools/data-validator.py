#!/usr/bin/env python3
"""
ChronoDB 数据校验工具
用于验证数据完整性、检查损坏的数据块等
"""

import argparse
import json
import os
import struct
import sys
from pathlib import Path
from typing import List, Tuple, Optional
import hashlib


class DataValidator:
    """数据校验器"""
    
    def __init__(self, data_dir: str):
        self.data_dir = Path(data_dir)
        self.errors = []
        self.warnings = []
        
    def validate_all(self) -> bool:
        """执行所有校验"""
        print("=" * 60)
        print("ChronoDB Data Validator")
        print("=" * 60)
        print(f"Data directory: {self.data_dir}")
        print()
        
        if not self.data_dir.exists():
            print(f"❌ Data directory not found: {self.data_dir}")
            return False
        
        # 检查目录结构
        self._check_directory_structure()
        
        # 检查 WAL 文件
        self._check_wal_files()
        
        # 检查数据块文件
        self._check_block_files()
        
        # 检查索引文件
        self._check_index_files()
        
        # 检查元数据文件
        self._check_metadata_files()
        
        # 输出结果
        print("\n" + "=" * 60)
        print("Validation Summary")
        print("=" * 60)
        
        if self.errors:
            print(f"❌ Found {len(self.errors)} errors:")
            for error in self.errors:
                print(f"  - {error}")
        
        if self.warnings:
            print(f"⚠️  Found {len(self.warnings)} warnings:")
            for warning in self.warnings:
                print(f"  - {warning}")
        
        if not self.errors and not self.warnings:
            print("✅ All checks passed!")
            return True
        elif not self.errors:
            print("✅ No critical errors found (warnings only)")
            return True
        else:
            print(f"\n❌ Validation failed with {len(self.errors)} errors")
            return False
    
    def _check_directory_structure(self):
        """检查目录结构"""
        print("Checking directory structure...")
        
        expected_dirs = ["wal", "blocks", "index", "metadata"]
        
        for dir_name in expected_dirs:
            dir_path = self.data_dir / dir_name
            if dir_path.exists():
                print(f"  ✅ {dir_name}/")
            else:
                self.warnings.append(f"Directory not found: {dir_name}/")
                print(f"  ⚠️  {dir_name}/ (not found)")
    
    def _check_wal_files(self):
        """检查 WAL 文件"""
        print("\nChecking WAL files...")
        
        wal_dir = self.data_dir / "wal"
        if not wal_dir.exists():
            self.warnings.append("WAL directory not found")
            return
        
        wal_files = list(wal_dir.glob("*.wal"))
        
        if not wal_files:
            print("  ℹ️  No WAL files found")
            return
        
        print(f"  Found {len(wal_files)} WAL file(s)")
        
        for wal_file in wal_files:
            try:
                file_size = wal_file.stat().st_size
                print(f"    📄 {wal_file.name} ({self._format_size(file_size)})")
                
                # 检查文件是否可读
                with open(wal_file, 'rb') as f:
                    # 读取文件头
                    header = f.read(8)
                    if len(header) < 8:
                        self.errors.append(f"WAL file too small: {wal_file.name}")
                        continue
                    
                    # 简单的魔数检查
                    magic = struct.unpack('>I', header[:4])[0]
                    if magic != 0x57414C00:  # WAL\0
                        self.warnings.append(f"WAL file has unexpected magic number: {wal_file.name}")
                        
            except Exception as e:
                self.errors.append(f"Failed to read WAL file {wal_file.name}: {e}")
    
    def _check_block_files(self):
        """检查数据块文件"""
        print("\nChecking block files...")
        
        blocks_dir = self.data_dir / "blocks"
        if not blocks_dir.exists():
            self.warnings.append("Blocks directory not found")
            return
        
        block_files = list(blocks_dir.glob("*.blk"))
        
        if not block_files:
            print("  ℹ️  No block files found")
            return
        
        print(f"  Found {len(block_files)} block file(s)")
        
        total_size = 0
        corrupted_blocks = []
        
        for block_file in block_files:
            try:
                file_size = block_file.stat().st_size
                total_size += file_size
                
                # 检查文件大小是否合理
                if file_size < 16:  # 最小块大小
                    self.errors.append(f"Block file too small: {block_file.name}")
                    corrupted_blocks.append(block_file)
                    continue
                
                with open(block_file, 'rb') as f:
                    # 读取文件头
                    header = f.read(16)
                    if len(header) < 16:
                        self.errors.append(f"Block file header incomplete: {block_file.name}")
                        corrupted_blocks.append(block_file)
                        continue
                    
                    # 解析头部
                    magic, version, compressed_size, uncompressed_size = struct.unpack('>IIQQ', header)
                    
                    # 检查魔数
                    if magic != 0x43484E42:  # CHNB
                        self.warnings.append(f"Block file has unexpected magic number: {block_file.name}")
                    
                    # 检查大小是否合理
                    if compressed_size > file_size - 16:
                        self.errors.append(f"Block file corrupted (size mismatch): {block_file.name}")
                        corrupted_blocks.append(block_file)
                        continue
                    
                    # 检查校验和（如果存在）
                    f.seek(file_size - 4)
                    checksum_data = f.read(4)
                    if len(checksum_data) == 4:
                        stored_checksum = struct.unpack('>I', checksum_data)[0]
                        # 这里可以添加实际的校验和计算
                        
            except Exception as e:
                self.errors.append(f"Failed to read block file {block_file.name}: {e}")
                corrupted_blocks.append(block_file)
        
        print(f"  Total size: {self._format_size(total_size)}")
        
        if corrupted_blocks:
            print(f"  ❌ Found {len(corrupted_blocks)} corrupted block(s)")
    
    def _check_index_files(self):
        """检查索引文件"""
        print("\nChecking index files...")
        
        index_dir = self.data_dir / "index"
        if not index_dir.exists():
            self.warnings.append("Index directory not found")
            return
        
        index_files = list(index_dir.glob("*.idx"))
        
        if not index_files:
            print("  ℹ️  No index files found")
            return
        
        print(f"  Found {len(index_files)} index file(s)")
        
        for index_file in index_files:
            try:
                file_size = index_file.stat().st_size
                print(f"    📄 {index_file.name} ({self._format_size(file_size)})")
                
                # 简单的 JSON 格式检查
                with open(index_file, 'r') as f:
                    content = f.read()
                    try:
                        json.loads(content)
                    except json.JSONDecodeError as e:
                        self.errors.append(f"Index file JSON parse error {index_file.name}: {e}")
                        
            except Exception as e:
                self.errors.append(f"Failed to read index file {index_file.name}: {e}")
    
    def _check_metadata_files(self):
        """检查元数据文件"""
        print("\nChecking metadata files...")
        
        metadata_dir = self.data_dir / "metadata"
        if not metadata_dir.exists():
            self.warnings.append("Metadata directory not found")
            return
        
        metadata_files = list(metadata_dir.glob("*.json"))
        
        if not metadata_files:
            print("  ℹ️  No metadata files found")
            return
        
        print(f"  Found {len(metadata_files)} metadata file(s)")
        
        for metadata_file in metadata_files:
            try:
                with open(metadata_file, 'r') as f:
                    content = f.read()
                    try:
                        data = json.loads(content)
                        # 检查必需的字段
                        if "version" not in data:
                            self.warnings.append(f"Metadata file missing version: {metadata_file.name}")
                    except json.JSONDecodeError as e:
                        self.errors.append(f"Metadata file JSON parse error {metadata_file.name}: {e}")
                        
            except Exception as e:
                self.errors.append(f"Failed to read metadata file {metadata_file.name}: {e}")
    
    def repair(self) -> bool:
        """尝试修复发现的问题"""
        print("=" * 60)
        print("ChronoDB Data Repair")
        print("=" * 60)
        
        if not self.errors:
            print("No errors to repair")
            return True
        
        repaired = 0
        failed = 0
        
        # 修复损坏的块文件
        blocks_dir = self.data_dir / "blocks"
        if blocks_dir.exists():
            for block_file in blocks_dir.glob("*.blk"):
                try:
                    with open(block_file, 'rb') as f:
                        header = f.read(16)
                        if len(header) < 16:
                            print(f"Repairing {block_file.name}...")
                            # 备份损坏的文件
                            backup_path = block_file.with_suffix('.blk.bak')
                            block_file.rename(backup_path)
                            print(f"  Moved to {backup_path.name}")
                            repaired += 1
                            
                except Exception as e:
                    print(f"Failed to repair {block_file.name}: {e}")
                    failed += 1
        
        print(f"\nRepair completed: {repaired} repaired, {failed} failed")
        return failed == 0
    
    def generate_report(self, output_file: str):
        """生成校验报告"""
        report = {
            "timestamp": str(datetime.now()),
            "data_directory": str(self.data_dir),
            "errors": self.errors,
            "warnings": self.warnings,
            "summary": {
                "total_errors": len(self.errors),
                "total_warnings": len(self.warnings),
                "status": "healthy" if not self.errors else "unhealthy"
            }
        }
        
        with open(output_file, 'w') as f:
            json.dump(report, f, indent=2)
        
        print(f"\nReport saved to: {output_file}")
    
    @staticmethod
    def _format_size(size_bytes: int) -> str:
        """格式化文件大小"""
        for unit in ['B', 'KB', 'MB', 'GB', 'TB']:
            if size_bytes < 1024.0:
                return f"{size_bytes:.1f} {unit}"
            size_bytes /= 1024.0
        return f"{size_bytes:.1f} PB"


def main():
    parser = argparse.ArgumentParser(
        description="ChronoDB 数据校验工具",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
示例:
  %(prog)s /var/lib/chronodb              # 校验数据
  %(prog)s /var/lib/chronodb --repair     # 校验并修复
  %(prog)s /var/lib/chronodb --report report.json  # 生成报告
        """
    )
    
    parser.add_argument("data_dir", help="ChronoDB 数据目录路径")
    parser.add_argument("--repair", action="store_true", help="尝试修复发现的问题")
    parser.add_argument("--report", metavar="FILE", help="生成 JSON 格式的校验报告")
    
    args = parser.parse_args()
    
    validator = DataValidator(args.data_dir)
    
    # 执行校验
    valid = validator.validate_all()
    
    # 尝试修复
    if args.repair and validator.errors:
        validator.repair()
    
    # 生成报告
    if args.report:
        validator.generate_report(args.report)
    
    sys.exit(0 if valid else 1)


if __name__ == "__main__":
    from datetime import datetime
    main()
