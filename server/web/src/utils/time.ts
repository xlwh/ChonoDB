import dayjs from 'dayjs'
import relativeTime from 'dayjs/plugin/relativeTime'

dayjs.extend(relativeTime)

export const formatTime = (timestamp: number, format: string = 'YYYY-MM-DD HH:mm:ss'): string => {
  return dayjs(timestamp * 1000).format(format)
}

export const formatTimeFromNow = (timestamp: number): string => {
  return dayjs(timestamp * 1000).fromNow()
}

export const parseTimeRange = (range: string): [number, number] => {
  const now = dayjs()
  let start: dayjs.Dayjs

  switch (range) {
    case '5m':
      start = now.subtract(5, 'minute')
      break
    case '15m':
      start = now.subtract(15, 'minute')
      break
    case '30m':
      start = now.subtract(30, 'minute')
      break
    case '1h':
      start = now.subtract(1, 'hour')
      break
    case '3h':
      start = now.subtract(3, 'hour')
      break
    case '6h':
      start = now.subtract(6, 'hour')
      break
    case '12h':
      start = now.subtract(12, 'hour')
      break
    case '1d':
      start = now.subtract(1, 'day')
      break
    case '7d':
      start = now.subtract(7, 'day')
      break
    case '30d':
      start = now.subtract(30, 'day')
      break
    default:
      start = now.subtract(1, 'hour')
  }

  return [start.unix(), now.unix()]
}
