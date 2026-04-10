import client from './client'
import type { QueryResult, RangeQueryResult } from './types'

export const queryApi = {
  instant: (query: string, time?: number) =>
    client.get<any, QueryResult>('/query', { params: { query, time } }),

  range: (query: string, start: number, end: number, step: number) =>
    client.get<any, RangeQueryResult>('/query_range', {
      params: { query, start, end, step },
    }),
}
