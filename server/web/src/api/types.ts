export interface Metric {
  [key: string]: string
}

export interface Sample {
  timestamp: number
  value: string
}

export interface TimeSeries {
  metric: Metric
  values?: Sample[]
  value?: Sample
}

export interface QueryResult {
  status: string
  data: {
    resultType: 'vector' | 'scalar' | 'string'
    result: TimeSeries[]
  }
}

export interface RangeQueryResult {
  status: string
  data: {
    resultType: 'matrix'
    result: TimeSeries[]
  }
}

export interface WriteRequest {
  timeseries: {
    labels: { name: string; value: string }[]
    samples: { timestamp: number; value: number }[]
  }[]
}

export interface WriteResponse {
  status: string
  data: {
    written: number
    failed: number
  }
}

export interface StorageStats {
  seriesCount: number
  sampleCount: number
  diskUsage: number
}

export interface MemoryStats {
  memstore: number
  wal: number
  cache: number
}

export interface QueryStats {
  totalQueries: number
  avgLatency: number
  errorRate: number
}

export interface SystemStats {
  storage: StorageStats
  memory: MemoryStats
  query: QueryStats
}

export interface NodeLoad {
  cpu: number
  memory: number
  series: number
}

export interface Node {
  id: string
  address: string
  status: 'online' | 'offline'
  load: NodeLoad
}

export interface Shard {
  id: string
  nodeId: string
  seriesCount: number
  size: number
}

export interface ClusterStatus {
  nodes: Node[]
  shards: Shard[]
}

export interface AlertRule {
  name: string
  query: string
  duration: string
  severity: 'critical' | 'warning' | 'info'
  state: 'inactive' | 'pending' | 'firing'
  annotations?: {
    summary?: string
    description?: string
  }
}

export interface AlertGroup {
  name: string
  rules: AlertRule[]
}

export interface AlertRules {
  groups: AlertGroup[]
}

export interface ServerConfig {
  listenAddress: string
  port: number
}

export interface StorageConfig {
  dataDir: string
  retention: string
}

export interface Config {
  server: ServerConfig
  storage: StorageConfig
}

export interface SingleWriteRequest {
  metric: string
  labels: Record<string, string>
  value: number
  timestamp?: number
}

export interface SingleWriteResponse {
  status: string
  data: {
    success: boolean
    message: string
  }
}

export interface BatchWriteRequest {
  timeseries: {
    labels: { name: string; value: string }[]
    samples: { timestamp: number; value: number }[]
  }[]
}

export interface BatchWriteResponse {
  status: string
  data: {
    total: number
    success: number
    failed: number
    errors?: string[]
  }
}

export interface WriteHistoryItem {
  id: string
  type: 'single' | 'batch'
  metric?: string
  timestamp: number
  status: 'success' | 'failed'
  message: string
  count?: number
}
