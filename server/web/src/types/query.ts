export interface TimeSeriesData {
  metric: Record<string, string>
  values: [number, string][]
}

export interface MetricValue {
  metric: Record<string, string>
  value: [number, string]
}

export interface QueryResponse {
  status: string
  data: {
    resultType: 'vector' | 'matrix' | 'scalar' | 'string'
    result: TimeSeriesData[] | MetricValue[]
  }
}

export interface ApiResponse<T> {
  status: 'success' | 'error'
  data?: T
  errorType?: string
  error?: string
}
