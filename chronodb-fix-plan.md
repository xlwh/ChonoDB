# ChronoDB 与 Prometheus 接口兼容性问题修复计划

## 概述

本计划基于之前的测试报告，针对发现的接口兼容性问题制定修复方案。目标是确保 ChronoDB 与 Prometheus 的接口完全兼容，为用户提供一个功能对等的时间序列数据库选择。

## 问题列表

1. **HTTP 方法不一致**：ChronoDB 的查询 API 使用 POST 方法，而 Prometheus 使用 GET 方法。
2. **Remote Write 格式问题**：无法向两个服务器成功写入数据，可能是格式或压缩问题。
3. **ChronoDB 时间范围限制**：系列 API 测试中，ChronoDB 报告时间范围太大的错误。
4. **告警规则解析**：ChronoDB 成功加载了规则文件，但没有返回规则详情。

## 修复计划

### 1. HTTP 方法不一致

#### 问题描述
ChronoDB 的查询 API (`/api/v1/query` 和 `/api/v1/query_range`) 使用 POST 方法，而 Prometheus 使用 GET 方法。这导致客户端需要根据不同的服务器使用不同的 HTTP 方法，降低了兼容性。

#### 修复方案
修改 ChronoDB 的 API 路由配置，使查询 API 同时支持 GET 和 POST 方法。

#### 实现步骤
1. 打开 `server/src/api/mod.rs` 文件
2. 修改 `/api/v1/query` 和 `/api/v1/query_range` 的路由配置，使用 `route` 方法并指定支持的 HTTP 方法
3. 确保处理函数能够正确解析来自 GET 查询字符串和 POST 请求体的参数

#### 验证方法
使用测试脚本 `test_http_api.py` 测试两种 HTTP 方法是否都能正常工作。

### 2. Remote Write 格式问题

#### 问题描述
无法向 ChronoDB 成功写入数据，可能是格式或压缩问题。测试中尝试使用 Snappy 压缩的文本格式数据，但 ChronoDB 报告了解压缩错误和 Protobuf 解码错误。

#### 修复方案
1. 检查 ChronoDB 的 Remote Write 处理代码，确保它能够正确处理 Prometheus 的 Remote Write 格式
2. 支持多种数据格式，包括 Snappy 压缩的 Protobuf 格式和文本格式

#### 实现步骤
1. 打开 `server/src/remote_server.rs` 文件
2. 检查 `handle_remote_write` 函数的实现
3. 确保它能够正确处理 Snappy 压缩的数据
4. 确保它能够正确解析 Protobuf 格式的数据
5. 添加对文本格式数据的支持

#### 验证方法
使用测试脚本 `test_remote_write_read.py` 测试数据写入功能是否正常工作。

### 3. ChronoDB 时间范围限制

#### 问题描述
在系列 API 测试中，ChronoDB 报告时间范围太大的错误。这可能是因为 ChronoDB 对查询时间范围设置了过于严格的限制。

#### 修复方案
调整 ChronoDB 的时间范围限制，使其与 Prometheus 的行为一致。

#### 实现步骤
1. 打开 `server/src/api/handlers.rs` 文件
2. 找到 `handle_series` 函数中关于时间范围限制的代码
3. 调整时间范围限制，使其与 Prometheus 的行为一致
4. 也检查其他查询相关的函数，确保时间范围限制设置合理

#### 验证方法
使用测试脚本 `test_data_model.py` 测试系列 API 是否能够正常工作。

### 4. 告警规则解析

#### 问题描述
ChronoDB 成功加载了告警规则文件，但没有返回规则详情。这可能是因为告警规则的解析或存储存在问题。

#### 修复方案
完善 ChronoDB 的告警规则解析和存储功能，确保能够正确解析和返回告警规则详情。

#### 实现步骤
1. 打开 `server/src/rules/mod.rs` 文件
2. 检查规则加载和解析的代码
3. 确保规则被正确存储和管理
4. 打开 `server/src/api/handlers.rs` 文件
5. 检查 `handle_rules` 函数的实现，确保它能够正确返回规则详情

#### 验证方法
使用测试脚本 `test_alert_rules.py` 测试告警规则 API 是否能够正确返回规则详情。

## 修复顺序

1. **HTTP 方法不一致**：优先级高，因为这影响到所有查询操作的兼容性
2. **ChronoDB 时间范围限制**：优先级高，因为这影响到数据模型 API 的可用性
3. **告警规则解析**：优先级中，因为告警规则是 Prometheus 的重要功能
4. **Remote Write 格式问题**：优先级中，因为数据写入是核心功能

## 验证流程

1. 对每个修复的问题，运行相应的测试脚本验证修复效果
2. 运行完整的测试套件 `run_all_tests.sh` 验证整体兼容性
3. 生成新的测试报告，对比修复前后的结果

## 预期结果

修复后，ChronoDB 应该能够：
1. 同时支持 GET 和 POST 方法进行查询
2. 正确处理 Prometheus 的 Remote Write 格式
3. 支持与 Prometheus 一致的时间范围限制
4. 正确解析和返回告警规则详情

通过这些修复，ChronoDB 将实现与 Prometheus 的完全接口兼容，为用户提供一个功能对等的时间序列数据库选择。
