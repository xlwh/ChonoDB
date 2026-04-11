import { useQuery } from '@tanstack/react-query'
import { queryApi } from '@/api'

export const useInstantQuery = (query: string, enabled: boolean = false, time?: number) => {
  return useQuery({
    queryKey: ['query', 'instant', query, time],
    queryFn: () => queryApi.instant(query, time),
    enabled: enabled && !!query,
    staleTime: 30000,
    retry: 1,
    refetchOnWindowFocus: false,
  })
}

export const useRangeQuery = (
  query: string,
  start: number,
  end: number,
  step: number,
  enabled: boolean = false
) => {
  return useQuery({
    queryKey: ['query', 'range', query, start, end, step],
    queryFn: () => queryApi.range(query, start, end, step),
    enabled: enabled && !!query && !!start && !!end && !!step,
    staleTime: 30000,
    retry: 1,
    refetchOnWindowFocus: false,
  })
}
