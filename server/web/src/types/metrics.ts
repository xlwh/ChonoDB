export interface Metric {
  name: string
  type: 'counter' | 'gauge' | 'histogram' | 'summary' | 'untyped'
  help: string
  labels: string[]
}

export interface MetricMetadata {
  metrics: Metric[]
}
