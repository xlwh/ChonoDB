import React, { useMemo } from 'react'
import ReactECharts from 'echarts-for-react'
import { Empty, Spin, Typography } from 'antd'
import dayjs from 'dayjs'
import type { TimeSeries } from '@/api/types'

const { Text } = Typography

interface QueryResultChartProps {
  data: TimeSeries[] | undefined
  resultType: 'vector' | 'matrix' | 'scalar' | 'string'
  loading?: boolean
}

const COLORS = [
  '#5470c6', '#91cc75', '#fac858', '#ee6666', '#73c0de',
  '#3ba272', '#fc8452', '#9a60b4', '#ea7ccc', '#48b8d0',
]

const QueryResultChart: React.FC<QueryResultChartProps> = ({
  data,
  resultType,
  loading = false,
}) => {
  const chartData = useMemo(() => {
    if (!data || data.length === 0) return null

    if (resultType === 'matrix') {
      const series = data.slice(0, 10).map((item, index) => {
        const metricName = item.metric?.__name__ || 'unknown'
        const labels = Object.entries(item.metric || {})
          .filter(([key]) => key !== '__name__')
          .map(([key, value]) => `${key}="${value}"`)
          .join(', ')
        const name = labels ? `${metricName}{${labels}}` : metricName

        const values = (item.values || []).map((sample) => [
          sample.timestamp * 1000,
          parseFloat(sample.value) || 0,
        ])

        return {
          name,
          type: 'line' as const,
          smooth: true,
          symbol: 'none',
          lineStyle: {
            width: 2,
          },
          areaStyle: {
            opacity: 0.1,
          },
          data: values,
          itemStyle: {
            color: COLORS[index % COLORS.length],
          },
        }
      })

      return {
        series,
        hasData: series.some(s => s.data && s.data.length > 0),
      }
    }

    if (resultType === 'vector') {
      const barData = data.slice(0, 20).map((item) => {
        const metricName = item.metric?.__name__ || 'unknown'
        const labels = Object.entries(item.metric || {})
          .filter(([key]) => key !== '__name__')
          .map(([key, value]) => `${key}="${value}"`)
          .join(', ')
        const name = labels ? `${metricName}{${labels}}` : metricName
        const value = parseFloat(item.value?.value || '0') || 0

        return { name, value }
      })

      return {
        series: [{
          name: 'Value',
          type: 'bar' as const,
          data: barData.map(d => d.value),
          itemStyle: {
            color: '#5470c6',
          },
        }],
        xAxisData: barData.map(d => d.name),
        isBar: true,
        hasData: barData.length > 0,
      }
    }

    return null
  }, [data, resultType])

  const option = useMemo(() => {
    if (!chartData) return {}

    if (chartData.isBar) {
      return {
        tooltip: {
          trigger: 'axis' as const,
          axisPointer: {
            type: 'shadow' as const,
          },
        },
        grid: {
          left: '3%',
          right: '4%',
          bottom: '15%',
          containLabel: true,
        },
        xAxis: {
          type: 'category' as const,
          data: chartData.xAxisData,
          axisLabel: {
            rotate: 45,
            interval: 0,
            fontSize: 10,
            formatter: (value: string) => {
              if (value.length > 30) {
                return value.substring(0, 30) + '...'
              }
              return value
            },
          },
        },
        yAxis: {
          type: 'value' as const,
          axisLabel: {
            formatter: (value: number) => {
              if (Math.abs(value) >= 1e9) return (value / 1e9).toFixed(1) + 'G'
              if (Math.abs(value) >= 1e6) return (value / 1e6).toFixed(1) + 'M'
              if (Math.abs(value) >= 1e3) return (value / 1e3).toFixed(1) + 'K'
              return value.toString()
            },
          },
        },
        series: chartData.series,
        dataZoom: chartData.xAxisData && chartData.xAxisData.length > 10 ? [
          {
            type: 'slider' as const,
            xAxisIndex: 0,
            start: 0,
            end: 50,
          },
        ] : undefined,
      }
    }

    return {
      tooltip: {
        trigger: 'axis' as const,
        formatter: (params: any) => {
          if (!params || params.length === 0) return ''
          const time = dayjs(params[0].data[0]).format('YYYY-MM-DD HH:mm:ss')
          let html = `<div style="font-weight: bold; margin-bottom: 4px;">${time}</div>`
          params.forEach((param: any) => {
            const value = param.data[1]
            const formattedValue = typeof value === 'number'
              ? (Math.abs(value) >= 1e6 ? (value / 1e6).toFixed(2) + 'M'
                : Math.abs(value) >= 1e3 ? (value / 1e3).toFixed(2) + 'K'
                : value.toFixed(4))
              : value
            html += `<div><span style="display:inline-block;width:10px;height:10px;border-radius:50%;background:${param.color};margin-right:8px;"></span>${param.seriesName}: <b>${formattedValue}</b></div>`
          })
          return html
        },
      },
      legend: {
        type: 'scroll' as const,
        bottom: 0,
        data: chartData.series?.map(s => s.name) || [],
        textStyle: {
          fontSize: 11,
        },
      },
      grid: {
        left: '3%',
        right: '4%',
        bottom: '15%',
        top: '10%',
        containLabel: true,
      },
      xAxis: {
        type: 'time' as const,
        axisLabel: {
          formatter: (value: number) => {
            return dayjs(value).format('HH:mm:ss')
          },
        },
      },
      yAxis: {
        type: 'value' as const,
        axisLabel: {
          formatter: (value: number) => {
            if (Math.abs(value) >= 1e9) return (value / 1e9).toFixed(1) + 'G'
            if (Math.abs(value) >= 1e6) return (value / 1e6).toFixed(1) + 'M'
            if (Math.abs(value) >= 1e3) return (value / 1e3).toFixed(1) + 'K'
            return value.toString()
          },
        },
      },
      dataZoom: [
        {
          type: 'inside' as const,
          start: 0,
          end: 100,
        },
        {
          type: 'slider' as const,
          start: 0,
          end: 100,
          height: 20,
        },
      ],
      series: chartData.series,
    }
  }, [chartData])

  if (loading) {
    return (
      <div style={{ textAlign: 'center', padding: '50px' }}>
        <Spin size="large" />
      </div>
    )
  }

  if (!data || data.length === 0) {
    return <Empty description="暂无图表数据" />
  }

  if (!chartData || !chartData.hasData) {
    return <Empty description="数据格式不支持图表展示" />
  }

  if (resultType === 'scalar' || resultType === 'string') {
    return (
      <div style={{ textAlign: 'center', padding: '20px' }}>
        <Text type="secondary">标量/字符串结果不支持图表展示</Text>
      </div>
    )
  }

  return (
    <ReactECharts
      option={option}
      style={{ height: 400 }}
      notMerge={true}
      lazyUpdate={true}
      opts={{ renderer: 'canvas' }}
    />
  )
}

export default QueryResultChart
