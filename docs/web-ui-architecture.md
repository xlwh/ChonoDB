# ChronoDB Web 管理界面前端架构设计

## 1. 技术栈选择

### 1.1 核心框架选择

**推荐方案：React 18 + TypeScript + Vite**

#### 选择理由

1. **React 18**
   - 成熟的生态系统，丰富的第三方库支持
   - 良好的 TypeScript 支持
   - 虚拟 DOM 机制，适合频繁的数据更新场景（时序数据展示）
   - 函数式组件 + Hooks，代码简洁易维护
   - 社区活跃，问题解决方案丰富

2. **TypeScript**
   - 类型安全，减少运行时错误
   - 更好的 IDE 支持和代码提示
   - 便于团队协作和代码维护
   - 接口定义清晰，与后端 API 对接更可靠

3. **Vite**
   - 极速的开发服务器启动（基于 ES modules）
   - 快速的热模块替换（HMR）
   - 优化的生产构建（基于 Rollup）
   - 原生支持 TypeScript、JSX
   - 轻量级，构建产物体积小
   - 适合嵌入到后端二进制文件的场景

#### 备选方案对比

| 方案 | 优势 | 劣势 | 适用性评分 |
|------|------|------|-----------|
| **React + Vite** | 生态成熟、类型安全、构建快速 | 包体积相对较大 | ⭐⭐⭐⭐⭐ |
| Vue 3 + Vite | 学习曲线平缓、性能优秀 | 时序图表库支持稍弱 | ⭐⭐⭐⭐ |
| Svelte + Vite | 编译时框架、运行时小、性能极佳 | 生态相对较小、UI 库选择少 | ⭐⭐⭐⭐ |
| Preact + Vite | 极小体积（3KB）、React 兼容 | 生态较小、部分库不兼容 | ⭐⭐⭐ |

**最终选择：React 18 + TypeScript + Vite**

理由：虽然 Svelte 和 Preact 更轻量，但考虑到：
- 时序数据库管理界面需要丰富的图表库支持（ECharts、Recharts 等 React 生态更成熟）
- 项目需要复杂的表单和表格组件（Ant Design 等 UI 库 React 版本更完善）
- 团队可能对 React 更熟悉
- Vite 的构建优化已经足够好，最终产物体积可控

### 1.2 UI 组件库选择

**推荐方案：Ant Design 5.x**

#### 选择理由

1. **功能完善**
   - 丰富的企业级组件（表格、表单、树形控件等）
   - 完善的 TypeScript 类型定义
   - 内置主题定制能力

2. **适合管理后台**
   - 专为数据密集型应用设计
   - 表格、表单组件功能强大
   - 布局组件（Layout、Menu、Breadcrumb）开箱即用

3. **体积优化**
   - 支持 Tree Shaking
   - 可按需加载组件
   - CSS-in-JS 方案（antd v5），样式按需注入

#### 备选方案

| 方案 | 优势 | 劣势 | 适用性评分 |
|------|------|------|-----------|
| **Ant Design 5** | 组件丰富、企业级、TypeScript 支持 | 包体积较大（~1MB gzipped） | ⭐⭐⭐⭐⭐ |
| Material-UI (MUI) | 设计规范完善、社区活跃 | 样式定制复杂、体积较大 | ⭐⭐⭐⭐ |
| Chakra UI | 轻量、可访问性好 | 组件数量较少 | ⭐⭐⭐ |
| Headless UI + Tailwind CSS | 极致轻量、高度可定制 | 需要自己实现样式、开发成本高 | ⭐⭐⭐ |

**最终选择：Ant Design 5.x**

理由：
- 管理后台需要大量复杂组件（表格、表单、树形控件等）
- Ant Design 的组件功能最完善，开发效率最高
- 虽然体积稍大，但通过 Tree Shaking 和按需加载可以优化

### 1.3 图表可视化库选择

**推荐方案：ECharts 5.x**

#### 选择理由

1. **时序数据展示能力强**
   - 原生支持时序图表
   - 支持大数据量渲染（百万级数据点）
   - 丰富的图表类型（折线图、面积图、热力图等）

2. **性能优秀**
   - Canvas 渲染，性能优于 SVG
   - 支持数据采样和降采样
   - 支持增量渲染

3. **功能丰富**
   - 数据缩放（DataZoom）
   - 工具箱（Toolbox）
   - 图例筛选
   - 提示框（Tooltip）

#### 备选方案

| 方案 | 优势 | 劣势 | 适用性评分 |
|------|------|------|-----------|
| **ECharts** | 功能强大、性能好、时序数据支持佳 | 包体积较大（~300KB） | ⭐⭐⭐⭐⭐ |
| Recharts | React 原生、声明式、体积小 | 大数据量性能差、功能较少 | ⭐⭐⭐⭐ |
| Chart.js | 轻量（~60KB）、简单易用 | 时序功能较弱、React 集成需封装 | ⭐⭐⭐ |
| D3.js | 最灵活、功能最强大 | 学习曲线陡峭、开发成本高 | ⭐⭐⭐ |

**最终选择：ECharts 5.x**

理由：
- 时序数据库需要强大的时序图表展示能力
- ECharts 在大数据量场景下性能最优
- 功能最完善，满足各种可视化需求

### 1.4 状态管理方案

**推荐方案：Zustand + React Query**

#### 选择理由

1. **Zustand**（全局状态管理）
   - 极轻量（~1KB）
   - API 简洁，学习成本低
   - TypeScript 支持优秀
   - 无需 Provider 包裹

2. **React Query**（服务端状态管理）
   - 专为服务端状态设计
   - 自动缓存、重新验证、后台更新
   - 请求去重、乐观更新
   - DevTools 支持

#### 为什么不选 Redux？

- Redux 配置繁琐，需要大量样板代码
- 对于管理后台场景，Zustand + React Query 更简洁高效
- React Query 已经处理了大部分服务端状态管理需求

### 1.5 其他核心依赖

| 类别 | 库名 | 版本 | 用途 |
|------|------|------|------|
| HTTP 客户端 | axios | ^1.6.0 | API 请求 |
| 日期处理 | dayjs | ^1.11.0 | 时间处理（轻量级 moment.js 替代） |
| 代码编辑器 | Monaco Editor | ^0.44.0 | PromQL 编辑器（语法高亮） |
| 路由 | React Router | ^6.20.0 | 前端路由 |
| 工具库 | lodash-es | ^4.17.0 | 工具函数（支持 Tree Shaking） |
| 样式方案 | CSS Modules + Less | - | 样式隔离 |

## 2. 项目目录结构

```
web-ui/
├── public/                     # 静态资源（不参与构建）
│   └── favicon.ico
├── src/
│   ├── api/                    # API 接口定义
│   │   ├── client.ts          # Axios 实例配置
│   │   ├── query.ts           # 查询相关 API
│   │   ├── write.ts           # 写入相关 API
│   │   ├── admin.ts           # 管理相关 API
│   │   └── types.ts           # API 类型定义
│   │
│   ├── components/             # 通用组件
│   │   ├── Layout/            # 布局组件
│   │   │   ├── AppLayout.tsx
│   │   │   ├── Header.tsx
│   │   │   └── Sidebar.tsx
│   │   ├── Charts/            # 图表组件
│   │   │   ├── TimeSeriesChart.tsx
│   │   │   ├── GaugeChart.tsx
│   │   │   └── BarChart.tsx
│   │   ├── QueryEditor/       # PromQL 编辑器
│   │   │   └── PromQLEditor.tsx
│   │   ├── DataTable/         # 数据表格
│   │   │   └── DataTable.tsx
│   │   └── common/            # 其他通用组件
│   │       ├── Loading.tsx
│   │       ├── ErrorBoundary.tsx
│   │       └── EmptyState.tsx
│   │
│   ├── pages/                  # 页面组件
│   │   ├── Dashboard/         # 仪表盘
│   │   │   ├── index.tsx
│   │   │   └── components/
│   │   ├── Query/             # 查询页面
│   │   │   ├── index.tsx
│   │   │   ├── InstantQuery.tsx
│   │   │   └── RangeQuery.tsx
│   │   ├── Write/             # 写入页面
│   │   │   ├── index.tsx
│   │   │   ├── SingleWrite.tsx
│   │   │   └── BatchWrite.tsx
│   │   ├── Metrics/           # 统计指标页面
│   │   │   ├── index.tsx
│   │   │   ├── StorageStats.tsx
│   │   │   ├── QueryStats.tsx
│   │   │   └── MemoryStats.tsx
│   │   ├── Cluster/           # 集群管理页面
│   │   │   ├── index.tsx
│   │   │   ├── NodeList.tsx
│   │   │   └── ShardDistribution.tsx
│   │   ├── Alerts/            # 告警管理页面
│   │   │   ├── index.tsx
│   │   │   ├── RulesList.tsx
│   │   │   └── ActiveAlerts.tsx
│   │   └── Settings/          # 配置管理页面
│   │       ├── index.tsx
│   │       └── ConfigEditor.tsx
│   │
│   ├── hooks/                  # 自定义 Hooks
│   │   ├── useQuery.ts        # 查询相关 Hook
│   │   ├── useTimeRange.ts    # 时间范围选择
│   │   └── useRefresh.ts      # 自动刷新
│   │
│   ├── stores/                 # 状态管理
│   │   ├── useAppStore.ts     # 全局状态
│   │   ├── useUserStore.ts    # 用户状态
│   │   └── useSettingsStore.ts # 设置状态
│   │
│   ├── utils/                  # 工具函数
│   │   ├── format.ts          # 格式化工具
│   │   ├── time.ts            # 时间处理
│   │   ├── promql.ts          # PromQL 工具
│   │   └── constants.ts       # 常量定义
│   │
│   ├── styles/                 # 全局样式
│   │   ├── global.css         # 全局样式
│   │   └── variables.css      # CSS 变量
│   │
│   ├── types/                  # TypeScript 类型定义
│   │   ├── query.ts           # 查询相关类型
│   │   ├── metrics.ts         # 指标相关类型
│   │   └── common.ts          # 通用类型
│   │
│   ├── App.tsx                 # 根组件
│   ├── main.tsx               # 应用入口
│   └── vite-env.d.ts          # Vite 类型声明
│
├── index.html                  # HTML 模板
├── vite.config.ts             # Vite 配置
├── tsconfig.json              # TypeScript 配置
├── package.json               # 项目依赖
└── .gitignore                 # Git 忽略文件
```

### 2.1 目录设计原则

1. **按功能模块划分**
   - api/：API 接口层
   - components/：可复用组件
   - pages/：页面级组件
   - hooks/：自定义 Hooks
   - stores/：状态管理
   - utils/：工具函数

2. **组件分层**
   - 通用组件（components/common）：高度可复用
   - 业务组件（components/[Feature]）：业务相关
   - 页面组件（pages）：页面级

3. **就近原则**
   - 页面专属组件放在 pages/[Page]/components/
   - 样式文件与组件同级

## 3. 组件库和 UI 框架选择

### 3.1 核心组件清单

#### 布局组件

| 组件 | 来源 | 用途 |
|------|------|------|
| Layout | Ant Design | 整体布局 |
| Menu | Ant Design | 侧边栏菜单 |
| Breadcrumb | Ant Design | 面包屑导航 |
| Header | 自定义 | 顶部导航栏 |

#### 数据展示组件

| 组件 | 来源 | 用途 |
|------|------|------|
| Table | Ant Design | 数据表格 |
| TimeSeriesChart | 自定义（基于 ECharts） | 时序图表 |
| StatCard | 自定义 | 统计卡片 |
| Descriptions | Ant Design | 描述列表 |

#### 表单组件

| 组件 | 来源 | 用途 |
|------|------|------|
| Form | Ant Design | 表单容器 |
| Input | Ant Design | 输入框 |
| Select | Ant Design | 下拉选择 |
| DatePicker | Ant Design | 日期选择 |
| Button | Ant Design | 按钮 |

#### 反馈组件

| 组件 | 来源 | 用途 |
|------|------|------|
| Message | Ant Design | 全局提示 |
| Modal | Ant Design | 对话框 |
| Notification | Ant Design | 通知提醒 |
| Spin | Ant Design | 加载状态 |

### 3.2 自定义组件设计

#### 3.2.1 TimeSeriesChart（时序图表组件）

```typescript
interface TimeSeriesChartProps {
  data: TimeSeriesData[];
  timeRange: [number, number];
  height?: number;
  showLegend?: boolean;
  showDataZoom?: boolean;
  onRangeChange?: (range: [number, number]) => void;
}
```

功能特性：
- 支持多条时序数据展示
- 支持数据缩放（DataZoom）
- 支持图例筛选
- 支持时间范围选择
- 支持导出图片

#### 3.2.2 PromQLEditor（PromQL 编辑器）

```typescript
interface PromQLEditorProps {
  value: string;
  onChange: (value: string) => void;
  onExecute: (query: string) => void;
  history?: string[];
}
```

功能特性：
- 基于 Monaco Editor
- PromQL 语法高亮
- 自动补全（指标名称、函数、标签）
- 查询历史记录
- 格式化功能

#### 3.2.3 StatCard（统计卡片）

```typescript
interface StatCardProps {
  title: string;
  value: number | string;
  unit?: string;
  trend?: 'up' | 'down' | 'stable';
  trendValue?: number;
  icon?: React.ReactNode;
}
```

功能特性：
- 显示关键指标
- 支持趋势展示
- 支持图标

### 3.3 主题定制

使用 Ant Design 的主题定制能力：

```typescript
// vite.config.ts
import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';

export default defineConfig({
  plugins: [
    react(),
  ],
  css: {
    preprocessorOptions: {
      less: {
        javascriptEnabled: true,
        modifyVars: {
          '@primary-color': '#1890ff',
          '@border-radius-base': '4px',
        },
      },
    },
  },
});
```

## 4. API 接口设计规范

### 4.1 接口基础规范

#### 4.1.1 请求规范

- **Base URL**: `/api/v1`
- **Content-Type**: `application/json`
- **认证**: JWT Token（Header: `Authorization: Bearer <token>`）

#### 4.1.2 响应规范

**成功响应格式**:

```json
{
  "status": "success",
  "data": { ... }
}
```

**错误响应格式**:

```json
{
  "status": "error",
  "errorType": "bad_data",
  "error": "Invalid parameter"
}
```

### 4.2 核心 API 接口

#### 4.2.1 查询接口

**即时查询**

```http
GET /api/v1/query
```

参数:
- `query`: PromQL 表达式
- `time`: 查询时间点（Unix 时间戳，秒）
- `timeout`: 超时时间

响应:
```json
{
  "status": "success",
  "data": {
    "resultType": "vector",
    "result": [
      {
        "metric": { "__name__": "up", "job": "prometheus" },
        "value": [1609459200, "1"]
      }
    ]
  }
}
```

**范围查询**

```http
GET /api/v1/query_range
```

参数:
- `query`: PromQL 表达式
- `start`: 开始时间（Unix 时间戳，秒）
- `end`: 结束时间（Unix 时间戳，秒）
- `step`: 查询步长（秒）
- `timeout`: 超时时间

响应:
```json
{
  "status": "success",
  "data": {
    "resultType": "matrix",
    "result": [
      {
        "metric": { "__name__": "up", "job": "prometheus" },
        "values": [
          [1609459200, "1"],
          [1609459260, "1"]
        ]
      }
    ]
  }
}
```

#### 4.2.2 写入接口

**JSON 写入**

```http
POST /api/v1/write
Content-Type: application/json
```

请求体:
```json
{
  "timeseries": [
    {
      "labels": [
        { "name": "__name__", "value": "cpu_usage" },
        { "name": "host", "value": "server1" }
      ],
      "samples": [
        { "timestamp": 1609459200000, "value": 45.5 }
      ]
    }
  ]
}
```

响应:
```json
{
  "status": "success",
  "data": {
    "written": 1,
    "failed": 0
  }
}
```

#### 4.2.3 管理接口

**获取系统统计**

```http
GET /api/v1/admin/stats
```

响应:
```json
{
  "status": "success",
  "data": {
    "storage": {
      "seriesCount": 10000,
      "sampleCount": 1000000,
      "diskUsage": 1073741824
    },
    "memory": {
      "memstore": 524288000,
      "wal": 104857600,
      "cache": 209715200
    },
    "query": {
      "totalQueries": 50000,
      "avgLatency": 15.5,
      "errorRate": 0.001
    }
  }
}
```

**获取集群状态**

```http
GET /api/v1/admin/cluster
```

响应:
```json
{
  "status": "success",
  "data": {
    "nodes": [
      {
        "id": "node-1",
        "address": "192.168.1.10:9090",
        "status": "online",
        "load": {
          "cpu": 45.5,
          "memory": 60.2,
          "series": 5000
        }
      }
    ],
    "shards": [
      {
        "id": "shard-1",
        "nodeId": "node-1",
        "seriesCount": 5000,
        "size": 524288000
      }
    ]
  }
}
```

**获取告警规则**

```http
GET /api/v1/admin/alerts/rules
```

响应:
```json
{
  "status": "success",
  "data": {
    "groups": [
      {
        "name": "system_alerts",
        "rules": [
          {
            "name": "HighCPU",
            "query": "cpu_usage > 80",
            "duration": "5m",
            "severity": "critical",
            "state": "inactive"
          }
        ]
      }
    ]
  }
}
```

**创建告警规则**

```http
POST /api/v1/admin/alerts/rules
```

请求体:
```json
{
  "group": "system_alerts",
  "rule": {
    "name": "HighCPU",
    "query": "cpu_usage > 80",
    "duration": "5m",
    "severity": "critical",
    "annotations": {
      "summary": "High CPU usage detected",
      "description": "CPU usage is {{ $value }}%"
    }
  }
}
```

**获取配置**

```http
GET /api/v1/admin/config
```

响应:
```json
{
  "status": "success",
  "data": {
    "server": {
      "listenAddress": "0.0.0.0",
      "port": 9090
    },
    "storage": {
      "dataDir": "/var/lib/chronodb",
      "retention": "15d"
    }
  }
}
```

**更新配置**

```http
PUT /api/v1/admin/config
```

请求体:
```json
{
  "storage": {
    "retention": "30d"
  }
}
```

### 4.3 前端 API 封装

#### 4.3.1 Axios 实例配置

```typescript
// src/api/client.ts
import axios, { AxiosInstance, AxiosError } from 'axios';

const client: AxiosInstance = axios.create({
  baseURL: '/api/v1',
  timeout: 30000,
  headers: {
    'Content-Type': 'application/json',
  },
});

client.interceptors.response.use(
  (response) => response.data,
  (error: AxiosError) => {
    const message = error.response?.data?.error || error.message;
    return Promise.reject(new Error(message));
  }
);

export default client;
```

#### 4.3.2 API 模块示例

```typescript
// src/api/query.ts
import client from './client';
import { QueryResult, RangeQueryResult } from './types';

export const queryApi = {
  instant: (query: string, time?: number) =>
    client.get<any, QueryResult>('/query', { params: { query, time } }),
  
  range: (query: string, start: number, end: number, step: number) =>
    client.get<any, RangeQueryResult>('/query_range', {
      params: { query, start, end, step },
    }),
};
```

```typescript
// src/api/admin.ts
import client from './client';
import { ClusterStatus, SystemStats, AlertRules } from './types';

export const adminApi = {
  getStats: () =>
    client.get<any, { status: string; data: SystemStats }>('/admin/stats'),
  
  getCluster: () =>
    client.get<any, { status: string; data: ClusterStatus }>('/admin/cluster'),
  
  getAlertRules: () =>
    client.get<any, { status: string; data: AlertRules }>('/admin/alerts/rules'),
};
```

### 4.4 React Query 集成

```typescript
// src/hooks/useQuery.ts
import { useQuery } from '@tanstack/react-query';
import { queryApi } from '../api/query';

export const useInstantQuery = (query: string, time?: number) => {
  return useQuery({
    queryKey: ['query', 'instant', query, time],
    queryFn: () => queryApi.instant(query, time),
    enabled: !!query,
    staleTime: 30000,
  });
};

export const useRangeQuery = (
  query: string,
  start: number,
  end: number,
  step: number
) => {
  return useQuery({
    queryKey: ['query', 'range', query, start, end, step],
    queryFn: () => queryApi.range(query, start, end, step),
    enabled: !!query && !!start && !!end && !!step,
    staleTime: 30000,
  });
};
```

## 5. 构建和部署方案

### 5.1 构建配置

#### 5.1.1 Vite 配置

```typescript
// vite.config.ts
import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
import path from 'path';

export default defineConfig({
  plugins: [react()],
  resolve: {
    alias: {
      '@': path.resolve(__dirname, './src'),
    },
  },
  build: {
    outDir: 'dist',
    assetsDir: 'assets',
    sourcemap: false,
    minify: 'terser',
    terserOptions: {
      compress: {
        drop_console: true,
        drop_debugger: true,
      },
    },
    rollupOptions: {
      output: {
        manualChunks: {
          'react-vendor': ['react', 'react-dom', 'react-router-dom'],
          'antd-vendor': ['antd', '@ant-design/icons'],
          'echarts-vendor': ['echarts', 'echarts-for-react'],
          'utils-vendor': ['axios', 'dayjs', 'lodash-es'],
        },
      },
    },
  },
  server: {
    port: 3000,
    proxy: {
      '/api': {
        target: 'http://localhost:9090',
        changeOrigin: true,
      },
    },
  },
});
```

#### 5.1.2 TypeScript 配置

```json
// tsconfig.json
{
  "compilerOptions": {
    "target": "ES2020",
    "useDefineForClassFields": true,
    "lib": ["ES2020", "DOM", "DOM.Iterable"],
    "module": "ESNext",
    "skipLibCheck": true,
    "moduleResolution": "bundler",
    "allowImportingTsExtensions": true,
    "resolveJsonModule": true,
    "isolatedModules": true,
    "noEmit": true,
    "jsx": "react-jsx",
    "strict": true,
    "noUnusedLocals": true,
    "noUnusedParameters": true,
    "noFallthroughCasesInSwitch": true,
    "baseUrl": ".",
    "paths": {
      "@/*": ["src/*"]
    }
  },
  "include": ["src"],
  "references": [{ "path": "./tsconfig.node.json" }]
}
```

### 5.2 嵌入 Rust 二进制文件方案

#### 5.2.1 使用 rust-embed

**添加依赖**（在 server/Cargo.toml）:

```toml
[dependencies]
rust-embed = "8.0"
```

**嵌入静态资源**（在 server/src/web_ui.rs）:

```rust
use rust_embed::RustEmbed;
use axum::{
    body::Body,
    http::{header, Response, StatusCode, Uri},
    response::IntoResponse,
};

#[derive(RustEmbed)]
#[folder = "../web-ui/dist"]
struct WebUIAssets;

pub async fn static_handler(uri: Uri) -> impl IntoResponse {
    let path = uri.path().trim_start_matches('/');
    
    if path.is_empty() || path == "index.html" {
        return serve_file("index.html");
    }
    
    match WebUIAssets::get(path) {
        Some(content) => {
            let mime = mime_guess::from_path(path)
                .first_or_octet_stream()
                .as_ref()
                .to_string();
            
            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, mime)
                .body(Body::from(content.data))
                .unwrap()
        }
        None => serve_file("index.html"),
    }
}

fn serve_file(path: &str) -> Response<Body> {
    match WebUIAssets::get(path) {
        Some(content) => {
            let mime = mime_guess::from_path(path)
                .first_or_octet_stream()
                .as_ref()
                .to_string();
            
            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, mime)
                .body(Body::from(content.data))
                .unwrap()
        }
        None => Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Body::from("Not Found"))
            .unwrap(),
    }
}
```

**路由配置**（在 server/src/api/mod.rs）:

```rust
use crate::web_ui::static_handler;

pub fn create_routes(state: Arc<ServerState>) -> Router {
    Router::new()
        .route("/api/v1/query", get(handle_query_get).post(handle_query_post))
        .route("/api/v1/query_range", get(handle_query_range_get).post(handle_query_range_post))
        .route("/api/v1/write", post(remote_server::handle_remote_write))
        .route("/api/v1/admin/stats", get(handle_admin_stats))
        .route("/api/v1/admin/cluster", get(handle_admin_cluster))
        .fallback(static_handler)
        .with_state(state)
}
```

#### 5.2.2 构建流程

**package.json 脚本**:

```json
{
  "scripts": {
    "dev": "vite",
    "build": "tsc && vite build",
    "preview": "vite preview",
    "lint": "eslint src --ext ts,tsx --report-unused-disable-directives --max-warnings 0"
  }
}
```

**构建脚本**（build.sh）:

```bash
#!/bin/bash

echo "Building web UI..."
cd web-ui
npm install
npm run build

echo "Building Rust server..."
cd ..
cargo build --release

echo "Build complete!"
```

### 5.3 构建产物优化

#### 5.3.1 体积优化策略

1. **代码分割**
   - Vendor chunks 分离
   - 路由懒加载

2. **Tree Shaking**
   - 使用 lodash-es 替代 lodash
   - 按需导入 Ant Design 组件

3. **压缩优化**
   - Terser 压缩
   - 移除 console 和 debugger

4. **资源优化**
   - 图片压缩
   - Gzip 压缩（服务器端）

#### 5.3.2 预期构建产物大小

| 资源类型 | 大小（gzip） |
|---------|-------------|
| React Vendor | ~40KB |
| Ant Design Vendor | ~80KB |
| ECharts Vendor | ~100KB |
| Utils Vendor | ~20KB |
| App Code | ~50KB |
| **总计** | **~290KB** |

### 5.4 开发环境配置

#### 5.4.1 开发服务器代理

```typescript
// vite.config.ts
server: {
  port: 3000,
  proxy: {
    '/api': {
      target: 'http://localhost:9090',
      changeOrigin: true,
    },
  },
}
```

#### 5.4.2 环境变量

```env
# .env.development
VITE_API_BASE_URL=http://localhost:9090
VITE_APP_TITLE=ChronoDB Web UI

# .env.production
VITE_API_BASE_URL=
VITE_APP_TITLE=ChronoDB Web UI
```

## 6. 开发规范

### 6.1 代码规范

#### 6.1.1 命名规范

- **组件文件**: PascalCase（如 `TimeSeriesChart.tsx`）
- **工具函数**: camelCase（如 `formatTime.ts`）
- **常量**: UPPER_SNAKE_CASE（如 `API_BASE_URL`）
- **CSS 类名**: kebab-case（如 `.time-series-chart`）

#### 6.1.2 组件规范

```typescript
import React from 'react';
import styles from './TimeSeriesChart.module.css';

interface TimeSeriesChartProps {
  data: TimeSeriesData[];
  height?: number;
}

export const TimeSeriesChart: React.FC<TimeSeriesChartProps> = ({
  data,
  height = 400,
}) => {
  return (
    <div className={styles.container} style={{ height }}>
    </div>
  );
};
```

#### 6.1.3 Git 提交规范

使用 Conventional Commits:

- `feat`: 新功能
- `fix`: 修复 bug
- `docs`: 文档更新
- `style`: 代码格式调整
- `refactor`: 重构
- `test`: 测试相关
- `chore`: 构建/工具链相关

示例:
```
feat(query): add PromQL syntax highlighting
fix(chart): fix time series chart rendering issue
docs(api): update API documentation
```

### 6.2 测试策略

#### 6.2.1 单元测试

使用 Vitest + React Testing Library:

```typescript
import { render, screen } from '@testing-library/react';
import { describe, it, expect } from 'vitest';
import { StatCard } from './StatCard';

describe('StatCard', () => {
  it('renders title and value correctly', () => {
    render(<StatCard title="CPU Usage" value={45.5} unit="%" />);
    expect(screen.getByText('CPU Usage')).toBeInTheDocument();
    expect(screen.getByText('45.5%')).toBeInTheDocument();
  });
});
```

#### 6.2.2 E2E 测试

使用 Playwright:

```typescript
import { test, expect } from '@playwright/test';

test('query page loads correctly', async ({ page }) => {
  await page.goto('http://localhost:3000/query');
  await expect(page.locator('h1')).toContainText('Query');
});
```

## 7. 性能优化

### 7.1 渲染优化

1. **虚拟列表**
   - 使用 `react-window` 处理大数据列表

2. **防抖和节流**
   - 查询输入防抖
   - 窗口大小调整节流

3. **懒加载**
   - 路由懒加载
   - 图表组件懒加载

### 7.2 数据优化

1. **数据缓存**
   - React Query 自动缓存
   - IndexedDB 本地缓存

2. **数据采样**
   - 大数据量时前端降采样
   - 使用 ECharts 的 sampling 功能

### 7.3 网络优化

1. **请求合并**
   - 批量查询合并
   - 使用 GraphQL（可选）

2. **请求取消**
   - 使用 AbortController
   - React Query 自动取消

## 8. 安全考虑

### 8.1 XSS 防护

- React 自动转义
- 避免 `dangerouslySetInnerHTML`
- CSP（Content Security Policy）

### 8.2 CSRF 防护

- 使用 SameSite Cookie
- CSRF Token（如果需要）

### 8.3 认证授权

- JWT Token 认证
- 权限控制（RBAC）

## 9. 监控和日志

### 9.1 前端监控

- 错误追踪（Sentry）
- 性能监控（Web Vitals）
- 用户行为分析（可选）

### 9.2 日志记录

- 结构化日志
- 日志级别（error, warn, info, debug）

## 10. 总结

### 10.1 技术栈总览

| 类别 | 技术选型 |
|------|---------|
| 框架 | React 18 + TypeScript |
| 构建工具 | Vite 5 |
| UI 组件库 | Ant Design 5 |
| 图表库 | ECharts 5 |
| 状态管理 | Zustand + React Query |
| HTTP 客户端 | Axios |
| 路由 | React Router 6 |
| 日期处理 | Day.js |
| 代码编辑器 | Monaco Editor |

### 10.2 核心优势

1. **轻量级**
   - Vite 构建快速
   - 构建产物体积可控（~290KB gzip）
   - 按需加载

2. **高性能**
   - React 18 并发特性
   - ECharts 大数据量渲染
   - React Query 智能缓存

3. **开发效率**
   - TypeScript 类型安全
   - Ant Design 组件丰富
   - Vite HMR 快速

4. **易于维护**
   - 清晰的目录结构
   - 模块化设计
   - 完善的代码规范

### 10.3 下一步计划

1. **Phase 1: 基础架构搭建**
   - 初始化项目
   - 配置构建工具
   - 搭建基础组件

2. **Phase 2: 核心功能开发**
   - 查询界面
   - 数据写入界面
   - 统计展示界面

3. **Phase 3: 高级功能开发**
   - 集群管理界面
   - 告警管理界面
   - 配置管理界面

4. **Phase 4: 优化和测试**
   - 性能优化
   - 单元测试
   - E2E 测试

5. **Phase 5: 集成和部署**
   - 嵌入 Rust 二进制
   - 生产环境测试
   - 文档完善
