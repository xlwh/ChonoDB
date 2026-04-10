import { useState, useEffect, useCallback } from 'react'

export const useRefresh = (interval: number = 0) => {
  const [refreshKey, setRefreshKey] = useState(0)

  const refresh = useCallback(() => {
    setRefreshKey((prev) => prev + 1)
  }, [])

  useEffect(() => {
    if (interval > 0) {
      const timer = setInterval(refresh, interval)
      return () => clearInterval(timer)
    }
  }, [interval, refresh])

  return { refreshKey, refresh }
}
