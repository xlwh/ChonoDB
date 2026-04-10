import { useState, useCallback } from 'react'
import dayjs, { Dayjs } from 'dayjs'

export const useTimeRange = (defaultRange: [Dayjs, Dayjs] = [dayjs().subtract(1, 'hour'), dayjs()]) => {
  const [timeRange, setTimeRange] = useState<[Dayjs, Dayjs]>(defaultRange)

  const handleTimeRangeChange = useCallback((range: [Dayjs, Dayjs]) => {
    setTimeRange(range)
  }, [])

  const toUnixRange = useCallback(() => {
    return [timeRange[0].unix(), timeRange[1].unix()] as [number, number]
  }, [timeRange])

  return {
    timeRange,
    setTimeRange: handleTimeRangeChange,
    toUnixRange,
  }
}
