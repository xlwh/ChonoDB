export interface PaginatedResponse<T> {
  data: T[]
  total: number
  page: number
  pageSize: number
}

export interface ErrorResponse {
  status: 'error'
  errorType: string
  error: string
}

export type TimeRange = '5m' | '15m' | '30m' | '1h' | '3h' | '6h' | '12h' | '1d' | '7d' | '30d'

export type Severity = 'critical' | 'warning' | 'info'

export type AlertState = 'inactive' | 'pending' | 'firing'
