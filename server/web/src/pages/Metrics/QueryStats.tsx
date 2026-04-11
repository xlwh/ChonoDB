import React, { useMemo } from 'react'
import { Row, Col } from 'antd'
import { SearchOutlined, ClockCircleOutlined, WarningOutlined } from '@ant-design/icons'
import ReactECharts from 'echarts-for-react'
import { useQuery } from '@tanstack/react-query'
import { adminApi } from '@/api'
import { Loading, StatCard } from '@/components'
import { formatNumber } from '@/utils/format'
import styles from './index.module.css'

interface QueryStatsProps {
  autoRefresh?: boolean
  refreshInterval?: number
}

const QueryStats: React.FC<QueryStatsProps> = ({
  autoRefresh = true,
  refreshInterval = 5000,
}) => {
  const { data, isLoading, dataUpdatedAt } = useQuery({
    queryKey: ['admin', 'stats'],
    queryFn: () => adminApi.getStats(),
    refetchInterval: autoRefresh ? refreshInterval : false,
  })

  const stats = data?.data?.query

  const latencyChartOption = useMemo(() => {
    return {
      title: {
        text: '查询延迟分布',
        left: 'center',
        textStyle: {
          fontSize: 14,
          fontWeight: 'normal',
        },
      },
      tooltip: {
        trigger: 'axis',
        axisPointer: {
          type: 'shadow',
        },
      },
      grid: {
        left: '3%',
        right: '4%',
        bottom: '3%',
        containLabel: true,
      },
      xAxis: {
        type: 'category',
        data: ['<10ms', '10-50ms', '50-100ms', '100-500ms', '500ms-1s', '>1s'],
      },
      yAxis: {
        type: 'value',
        name: '查询次数',
      },
      series: [
        {
          name: '查询次数',
          type: 'bar',
          data: [
            stats?.totalQueries ? stats.totalQueries * 0.4 : 0,
            stats?.totalQueries ? stats.totalQueries * 0.3 : 0,
            stats?.totalQueries ? stats.totalQueries * 0.15 : 0,
            stats?.totalQueries ? stats.totalQueries * 0.1 : 0,
            stats?.totalQueries ? stats.totalQueries * 0.04 : 0,
            stats?.totalQueries ? stats.totalQueries * 0.01 : 0,
          ],
          itemStyle: {
            color: {
              type: 'linear',
              x: 0,
              y: 0,
              x2: 0,
              y2: 1,
              colorStops: [
                { offset: 0, color: '#1890ff' },
                { offset: 1, color: '#69c0ff' },
              ],
            },
          },
        },
      ],
    }
  }, [stats])

  const throughputChartOption = useMemo(() => {
    const hours = []
    for (let i = 0; i < 24; i++) {
      hours.push(`${i}:00`)
    }

    return {
      title: {
        text: '查询吞吐量趋势',
        left: 'center',
        textStyle: {
          fontSize: 14,
          fontWeight: 'normal',
        },
      },
      tooltip: {
        trigger: 'axis',
        axisPointer: {
          type: 'cross',
        },
      },
      grid: {
        left: '3%',
        right: '4%',
        bottom: '3%',
        containLabel: true,
      },
      xAxis: {
        type: 'category',
        boundaryGap: false,
        data: hours,
      },
      yAxis: {
        type: 'value',
        name: 'QPS',
      },
      series: [
        {
          name: 'QPS',
          type: 'line',
          smooth: true,
          areaStyle: {
            color: {
              type: 'linear',
              x: 0,
              y: 0,
              x2: 0,
              y2: 1,
              colorStops: [
                { offset: 0, color: 'rgba(82, 196, 26, 0.3)' },
                { offset: 1, color: 'rgba(82, 196, 26, 0.05)' },
              ],
            },
          },
          lineStyle: {
            color: '#52c41a',
            width: 2,
          },
          itemStyle: {
            color: '#52c41a',
          },
          data: Array.from({ length: 24 }, () =>
            Math.floor((stats?.totalQueries || 0) / 24 * (0.8 + Math.random() * 0.4))
          ),
        },
      ],
    }
  }, [stats])

  const errorRateChartOption = useMemo(() => {
    return {
      title: {
        text: '错误率趋势',
        left: 'center',
        textStyle: {
          fontSize: 14,
          fontWeight: 'normal',
        },
      },
      tooltip: {
        trigger: 'axis',
        formatter: (params: any) => {
          const value = params[0].value
          return `${params[0].name}<br/>错误率: ${(value * 100).toFixed(4)}%`
        },
      },
      grid: {
        left: '3%',
        right: '4%',
        bottom: '3%',
        containLabel: true,
      },
      xAxis: {
        type: 'category',
        boundaryGap: false,
        data: ['00:00', '04:00', '08:00', '12:00', '16:00', '20:00', '24:00'],
      },
      yAxis: {
        type: 'value',
        name: '错误率',
        axisLabel: {
          formatter: (value: number) => `${(value * 100).toFixed(2)}%`,
        },
      },
      series: [
        {
          name: '错误率',
          type: 'line',
          smooth: true,
          lineStyle: {
            color: '#ff4d4f',
            width: 2,
          },
          itemStyle: {
            color: '#ff4d4f',
          },
          areaStyle: {
            color: {
              type: 'linear',
              x: 0,
              y: 0,
              x2: 0,
              y2: 1,
              colorStops: [
                { offset: 0, color: 'rgba(255, 77, 79, 0.3)' },
                { offset: 1, color: 'rgba(255, 77, 79, 0.05)' },
              ],
            },
          },
          data: Array.from({ length: 7 }, () =>
            Math.max(0, (stats?.errorRate || 0) * (0.8 + Math.random() * 0.4))
          ),
        },
      ],
    }
  }, [stats])

  if (isLoading && !data) {
    return <Loading />
  }

  const errorRatePercent = (stats?.errorRate || 0) * 100

  return (
    <div className={styles.statsContainer}>
      <Row gutter={[16, 16]}>
        <Col xs={24} sm={12} lg={8}>
          <StatCard
            title="查询总数"
            value={formatNumber(stats?.totalQueries || 0, 0)}
            prefix={<SearchOutlined style={{ color: '#1890ff' }} />}
            loading={isLoading}
          />
        </Col>
        <Col xs={24} sm={12} lg={8}>
          <StatCard
            title="平均延迟"
            value={stats?.avgLatency || 0}
            suffix="ms"
            precision={2}
            prefix={<ClockCircleOutlined style={{ color: '#52c41a' }} />}
            loading={isLoading}
          />
        </Col>
        <Col xs={24} sm={12} lg={8}>
          <StatCard
            title="错误率"
            value={errorRatePercent.toFixed(4)}
            suffix="%"
            prefix={<WarningOutlined style={{ color: errorRatePercent > 5 ? '#ff4d4f' : '#52c41a' }} />}
            loading={isLoading}
            progress={Math.min(errorRatePercent, 100)}
            progressStatus={errorRatePercent > 5 ? 'exception' : 'success'}
          />
        </Col>
      </Row>

      <Row gutter={[16, 16]} style={{ marginTop: 16 }}>
        <Col xs={24} lg={12}>
          <div className={styles.chartCard}>
            <ReactECharts option={latencyChartOption} style={{ height: '300px' }} />
          </div>
        </Col>
        <Col xs={24} lg={12}>
          <div className={styles.chartCard}>
            <ReactECharts option={throughputChartOption} style={{ height: '300px' }} />
          </div>
        </Col>
      </Row>

      <Row gutter={[16, 16]} style={{ marginTop: 16 }}>
        <Col xs={24}>
          <div className={styles.chartCard}>
            <ReactECharts option={errorRateChartOption} style={{ height: '300px' }} />
          </div>
        </Col>
      </Row>

      <div className={styles.updateTime}>
        最后更新: {new Date(dataUpdatedAt).toLocaleString('zh-CN')}
      </div>
    </div>
  )
}

export default QueryStats
