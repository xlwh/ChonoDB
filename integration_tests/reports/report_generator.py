#!/usr/bin/env python3
"""
测试报告生成器
生成HTML和JSON格式的测试报告
"""

import json
import os
from typing import Dict, List, Any, Optional
from datetime import datetime
from pathlib import Path

import sys
sys.path.insert(0, str(Path(__file__).parent.parent))
from core.logger import get_logger
from core.base_test import TestSuite
from comparators.result_comparator import ComparisonReport


class ReportGenerator:
    """报告生成器"""
    
    def __init__(self, output_dir: str = "./integration_test_reports"):
        self.output_dir = Path(output_dir)
        self.output_dir.mkdir(parents=True, exist_ok=True)
        self.logger = get_logger()
    
    def generate_json_report(self, test_suites: List[TestSuite],
                            comparison_report: Optional[ComparisonReport] = None,
                            metadata: Optional[Dict[str, Any]] = None) -> str:
        """
        生成JSON格式报告
        
        Args:
            test_suites: 测试套件列表
            comparison_report: 对比报告
            metadata: 元数据
        
        Returns:
            报告文件路径
        """
        report = {
            "report_type": "integration_test",
            "generated_at": datetime.now().isoformat(),
            "metadata": metadata or {},
            "summary": {
                "total_suites": len(test_suites),
                "total_tests": sum(s.total_count for s in test_suites),
                "total_passed": sum(s.passed_count for s in test_suites),
                "total_failed": sum(s.failed_count for s in test_suites),
                "overall_pass_rate": (
                    sum(s.passed_count for s in test_suites) / 
                    sum(s.total_count for s in test_suites) * 100
                    if sum(s.total_count for s in test_suites) > 0 else 0
                )
            },
            "test_suites": [s.to_dict() for s in test_suites]
        }
        
        if comparison_report:
            report["comparison"] = comparison_report.to_dict()
        
        # 保存JSON报告
        timestamp = datetime.now().strftime("%Y%m%d_%H%M%S")
        json_path = self.output_dir / f"integration_test_report_{timestamp}.json"
        
        with open(json_path, 'w', encoding='utf-8') as f:
            json.dump(report, f, indent=2, ensure_ascii=False)
        
        self.logger.info(f"JSON报告已生成: {json_path}")
        return str(json_path)
    
    def generate_html_report(self, test_suites: List[TestSuite],
                            comparison_report: Optional[ComparisonReport] = None,
                            metadata: Optional[Dict[str, Any]] = None) -> str:
        """
        生成HTML格式报告
        
        Args:
            test_suites: 测试套件列表
            comparison_report: 对比报告
            metadata: 元数据
        
        Returns:
            报告文件路径
        """
        timestamp = datetime.now().strftime("%Y%m%d_%H%M%S")
        html_path = self.output_dir / f"integration_test_report_{timestamp}.html"
        
        html_content = self._build_html_content(test_suites, comparison_report, metadata)
        
        with open(html_path, 'w', encoding='utf-8') as f:
            f.write(html_content)
        
        self.logger.info(f"HTML报告已生成: {html_path}")
        return str(html_path)
    
    def _build_html_content(self, test_suites: List[TestSuite],
                           comparison_report: Optional[ComparisonReport],
                           metadata: Optional[Dict[str, Any]]) -> str:
        """构建HTML内容"""
        
        # 计算汇总数据
        total_tests = sum(s.total_count for s in test_suites)
        total_passed = sum(s.passed_count for s in test_suites)
        total_failed = sum(s.failed_count for s in test_suites)
        pass_rate = (total_passed / total_tests * 100) if total_tests > 0 else 0
        
        # 构建测试套件HTML
        suites_html = ""
        for suite in test_suites:
            suite_pass_rate = suite.pass_rate
            suite_status_class = "success" if suite_pass_rate == 100 else "warning" if suite_pass_rate >= 80 else "danger"
            
            results_html = ""
            for result in suite.results:
                status_class = "success" if result.passed else "danger"
                status_icon = "✓" if result.passed else "✗"
                details_html = ""
                if result.details:
                    details_html = f'<div class="details"><pre>{json.dumps(result.details, indent=2)}</pre></div>'
                
                results_html += f"""
                <tr class="{status_class}">
                    <td>{status_icon}</td>
                    <td>{result.name}</td>
                    <td>{result.duration_ms:.1f}ms</td>
                    <td>{result.message}</td>
                </tr>
                {details_html}
                """
            
            suites_html += f"""
            <div class="suite">
                <h3>{suite.name} <span class="badge {suite_status_class}">{suite.passed_count}/{suite.total_count}</span></h3>
                <table>
                    <thead>
                        <tr>
                            <th>状态</th>
                            <th>测试名称</th>
                            <th>耗时</th>
                            <th>消息</th>
                        </tr>
                    </thead>
                    <tbody>
                        {results_html}
                    </tbody>
                </table>
            </div>
            """
        
        # 构建对比报告HTML
        comparison_html = ""
        if comparison_report:
            match_rate = comparison_report.match_rate
            perf_ratio = comparison_report.performance_ratio
            perf_text = f"快 {1/perf_ratio:.2f}x" if perf_ratio < 1 else f"慢 {perf_ratio:.2f}x"
            
            comparison_html = f"""
            <div class="section">
                <h2>Prometheus vs ChronoDB 对比结果</h2>
                <div class="stats-grid">
                    <div class="stat">
                        <span class="stat-value">{comparison_report.total_queries}</span>
                        <span class="stat-label">总查询数</span>
                    </div>
                    <div class="stat">
                        <span class="stat-value">{match_rate:.1f}%</span>
                        <span class="stat-label">匹配率</span>
                    </div>
                    <div class="stat">
                        <span class="stat-value">{perf_text}</span>
                        <span class="stat-label">性能对比</span>
                    </div>
                </div>
            </div>
            """
        
        # 构建完整HTML
        html = f"""<!DOCTYPE html>
<html lang="zh-CN">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>ChronoDB 集成测试报告</title>
    <style>
        * {{
            margin: 0;
            padding: 0;
            box-sizing: border-box;
        }}
        
        body {{
            font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, "Helvetica Neue", Arial, sans-serif;
            line-height: 1.6;
            color: #333;
            background: #f5f5f5;
            padding: 20px;
        }}
        
        .container {{
            max-width: 1200px;
            margin: 0 auto;
            background: white;
            border-radius: 8px;
            box-shadow: 0 2px 8px rgba(0,0,0,0.1);
            overflow: hidden;
        }}
        
        .header {{
            background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
            color: white;
            padding: 30px;
        }}
        
        .header h1 {{
            font-size: 28px;
            margin-bottom: 10px;
        }}
        
        .header .meta {{
            opacity: 0.9;
            font-size: 14px;
        }}
        
        .summary {{
            display: grid;
            grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
            gap: 20px;
            padding: 30px;
            background: #f8f9fa;
        }}
        
        .stat-card {{
            background: white;
            padding: 20px;
            border-radius: 8px;
            text-align: center;
            box-shadow: 0 2px 4px rgba(0,0,0,0.05);
        }}
        
        .stat-value {{
            font-size: 36px;
            font-weight: bold;
            color: #667eea;
        }}
        
        .stat-label {{
            color: #666;
            font-size: 14px;
            margin-top: 5px;
        }}
        
        .content {{
            padding: 30px;
        }}
        
        .section {{
            margin-bottom: 30px;
        }}
        
        .section h2 {{
            font-size: 20px;
            margin-bottom: 15px;
            padding-bottom: 10px;
            border-bottom: 2px solid #eee;
        }}
        
        .suite {{
            margin-bottom: 20px;
            border: 1px solid #eee;
            border-radius: 8px;
            overflow: hidden;
        }}
        
        .suite h3 {{
            background: #f8f9fa;
            padding: 15px;
            font-size: 16px;
            display: flex;
            justify-content: space-between;
            align-items: center;
        }}
        
        .badge {{
            padding: 4px 12px;
            border-radius: 12px;
            font-size: 12px;
            font-weight: bold;
        }}
        
        .badge.success {{
            background: #d4edda;
            color: #155724;
        }}
        
        .badge.warning {{
            background: #fff3cd;
            color: #856404;
        }}
        
        .badge.danger {{
            background: #f8d7da;
            color: #721c24;
        }}
        
        table {{
            width: 100%;
            border-collapse: collapse;
        }}
        
        th, td {{
            padding: 12px;
            text-align: left;
            border-bottom: 1px solid #eee;
        }}
        
        th {{
            background: #f8f9fa;
            font-weight: 600;
            font-size: 14px;
        }}
        
        tr.success td {{
            background: #f1f8f4;
        }}
        
        tr.danger td {{
            background: #fdf2f2;
        }}
        
        .details {{
            padding: 15px;
            background: #f8f9fa;
            border-top: 1px solid #eee;
        }}
        
        .details pre {{
            background: white;
            padding: 10px;
            border-radius: 4px;
            overflow-x: auto;
            font-size: 12px;
        }}
        
        .stats-grid {{
            display: grid;
            grid-template-columns: repeat(auto-fit, minmax(150px, 1fr));
            gap: 15px;
        }}
        
        .stat {{
            text-align: center;
            padding: 15px;
            background: #f8f9fa;
            border-radius: 8px;
        }}
        
        .footer {{
            padding: 20px;
            text-align: center;
            color: #666;
            font-size: 12px;
            border-top: 1px solid #eee;
        }}
    </style>
</head>
<body>
    <div class="container">
        <div class="header">
            <h1>ChronoDB 集成测试报告</h1>
            <div class="meta">
                生成时间: {datetime.now().strftime("%Y-%m-%d %H:%M:%S")}<br>
                测试环境: {metadata.get("environment", "Unknown") if metadata else "Unknown"}
            </div>
        </div>
        
        <div class="summary">
            <div class="stat-card">
                <div class="stat-value">{total_tests}</div>
                <div class="stat-label">总测试数</div>
            </div>
            <div class="stat-card">
                <div class="stat-value" style="color: #28a745;">{total_passed}</div>
                <div class="stat-label">通过</div>
            </div>
            <div class="stat-card">
                <div class="stat-value" style="color: #dc3545;">{total_failed}</div>
                <div class="stat-label">失败</div>
            </div>
            <div class="stat-card">
                <div class="stat-value" style="color: {'#28a745' if pass_rate >= 90 else '#ffc107' if pass_rate >= 70 else '#dc3545'};">{pass_rate:.1f}%</div>
                <div class="stat-label">通过率</div>
            </div>
        </div>
        
        <div class="content">
            {comparison_html}
            
            <div class="section">
                <h2>测试套件详情</h2>
                {suites_html}
            </div>
        </div>
        
        <div class="footer">
            Generated by ChronoDB Integration Test Framework
        </div>
    </div>
</body>
</html>"""
        
        return html
    
    def generate_markdown_report(self, test_suites: List[TestSuite],
                                comparison_report: Optional[ComparisonReport] = None,
                                metadata: Optional[Dict[str, Any]] = None) -> str:
        """
        生成Markdown格式报告
        
        Args:
            test_suites: 测试套件列表
            comparison_report: 对比报告
            metadata: 元数据
        
        Returns:
            报告文件路径
        """
        timestamp = datetime.now().strftime("%Y%m%d_%H%M%S")
        md_path = self.output_dir / f"integration_test_report_{timestamp}.md"
        
        # 计算汇总数据
        total_tests = sum(s.total_count for s in test_suites)
        total_passed = sum(s.passed_count for s in test_suites)
        total_failed = sum(s.failed_count for s in test_suites)
        pass_rate = (total_passed / total_tests * 100) if total_tests > 0 else 0
        
        md_content = f"""# ChronoDB 集成测试报告

**生成时间**: {datetime.now().strftime("%Y-%m-%d %H:%M:%S")}

## 测试汇总

| 指标 | 数值 |
|------|------|
| 总测试数 | {total_tests} |
| 通过 | {total_passed} |
| 失败 | {total_failed} |
| 通过率 | {pass_rate:.1f}% |

"""
        
        # 添加对比报告
        if comparison_report:
            md_content += f"""## Prometheus vs ChronoDB 对比结果

| 指标 | 数值 |
|------|------|
| 总查询数 | {comparison_report.total_queries} |
| 匹配数 | {comparison_report.matched_queries} |
| 不匹配数 | {comparison_report.mismatched_queries} |
| 匹配率 | {comparison_report.match_rate:.1f}% |
| Prometheus平均耗时 | {comparison_report.avg_prometheus_duration_ms:.2f}ms |
| ChronoDB平均耗时 | {comparison_report.avg_chronodb_duration_ms:.2f}ms |

"""
        
        # 添加测试套件详情
        md_content += "## 测试套件详情\n\n"
        
        for suite in test_suites:
            md_content += f"### {suite.name}\n\n"
            md_content += f"**结果**: {suite.passed_count}/{suite.total_count} 通过 ({suite.pass_rate:.1f}%)\n\n"
            md_content += "| 状态 | 测试名称 | 耗时 | 消息 |\n"
            md_content += "|------|----------|------|------|\n"
            
            for result in suite.results:
                status = "✓" if result.passed else "✗"
                md_content += f"| {status} | {result.name} | {result.duration_ms:.1f}ms | {result.message} |\n"
            
            md_content += "\n"
        
        with open(md_path, 'w', encoding='utf-8') as f:
            f.write(md_content)
        
        self.logger.info(f"Markdown报告已生成: {md_path}")
        return str(md_path)
    
    def generate_all_reports(self, test_suites: List[TestSuite],
                            comparison_report: Optional[ComparisonReport] = None,
                            metadata: Optional[Dict[str, Any]] = None) -> Dict[str, str]:
        """
        生成所有格式的报告
        
        Returns:
            报告文件路径字典
        """
        reports = {}
        
        reports['json'] = self.generate_json_report(test_suites, comparison_report, metadata)
        reports['html'] = self.generate_html_report(test_suites, comparison_report, metadata)
        reports['markdown'] = self.generate_markdown_report(test_suites, comparison_report, metadata)
        
        self.logger.section("报告生成完成")
        self.logger.info(f"报告目录: {self.output_dir}")
        
        return reports


# 便捷函数
def create_report_generator(output_dir: str = "./integration_test_reports") -> ReportGenerator:
    """创建报告生成器"""
    return ReportGenerator(output_dir)
