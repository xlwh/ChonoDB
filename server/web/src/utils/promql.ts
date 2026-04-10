export const formatPromQL = (query: string): string => {
  return query.trim()
}

export const validatePromQL = (query: string): boolean => {
  return query.trim().length > 0
}

export const extractMetricName = (query: string): string | null => {
  const match = query.match(/^([a-zA-Z_:][a-zA-Z0-9_:]*)/)
  return match ? match[1] : null
}

export const extractLabels = (query: string): Record<string, string> => {
  const labels: Record<string, string> = {}
  const regex = /([a-zA-Z_][a-zA-Z0-9_]*)\s*=\s*"([^"]*)"/g
  let match

  while ((match = regex.exec(query)) !== null) {
    labels[match[1]] = match[2]
  }

  return labels
}
