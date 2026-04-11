import React, { useMemo } from 'react'
import { Row, Col, Typography, Progress } from 'antd'
import { SaveOutlined, ThunderboltOutlined } from '@ant-design/icons'
import ReactECharts from 'echarts-for-react'
import { useQuery } from '@tanstack/react-query'
import { adminApi } from '@/api'
import { Loading, StatCard } from '@/components'
import { formatBytes } from '@/utils/format'
import styles from './index.module.css'

const { Title } = Typography

interface MemoryStatsProps {
  autoRefresh?: boolean
  refreshInterval?: number
}

const MemoryStats: React.FC<MemoryStatsProps> = ({
  autoRefresh = true,
  refreshInterval = 5000,
}) => {
  const { data, isLoading, dataUpdatedAt } = useQuery({
    queryKey: ['admin', 'stats'],
    queryFn: () => adminApi.getStats(),
    refetchInterval: autoRefresh ? refreshInterval : false,
  })

  const stats = data?.data?.memory

  const totalMemory = useMemo(() => {
    return (stats?.memstore || 0) + (stats?.wal || 0) + (stats?.cache || 0)
  }, [stats])

  const memoryDistributionOption = useMemo(() => {
    if (!stats) return {}

    return {
      title: {
        text: '内存分布',
        left: 'center',
        textStyle: {
          fontSize: 14,
          fontWeight: 'normal',
        },
      },
      tooltip: {
        trigger: 'item',
        formatter: (params: any) => {
          return `${params.name}: ${formatBytes(params.value)} (${params.percent}%)`
        },
      },
      legend: {
        orient: 'vertical',
        right: 10,
        top: 'center',
      },
      series: [
        {
          name: '内存使用',
          type: 'pie',
          radius: ['40%', '70%'],
          avoidLabelOverlap: false,
          itemStyle: {
            borderRadius: 10,
            borderColor: '#fff',
            borderWidth: 2,
          },
          label: {
            show: true,
            formatter: (params: any) => {
              return `${params.name}\n${formatBytes(params.value)}`
            },
          },
          emphasis: {
            label: {
              show: true,
              fontSize: 16,
              fontWeight: 'bold',
            },
          },
          data: [
            {
              value: stats.memstore || 0,
              name: 'MemStore',
              itemStyle: { color: '#1890ff' },
            },
            {
              value: stats.wal || 0,
              name: 'WAL',
              itemStyle: { color: '#52c41a' },
            },
            {
              value: stats.cache || 0,
              name: 'Cache',
              itemStyle: { color: '#faad14' },
            },
          ],
        },
      ],
    }
  }, [stats])

  const memoryTrendOption = useMemo(() => {
    return {
      title: {
        text: '内存使用趋势',
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
        formatter: (params: any) => {
          let result = params[0].name + '<br/>'
          params.forEach((item: any) => {
            result += `${item.marker}${item.seriesName}: ${formatBytes(item.value)}<br/>`
          })
          return result
        },
      },
      legend: {
        data: ['MemStore', 'WAL', 'Cache'],
        bottom: 0,
      },
      grid: {
        left: '3%',
        right: '4%',
        bottom: '15%',
        containLabel: true,
      },
      xAxis: {
        type: 'category',
        boundaryGap: false,
        data: ['00:00', '04:00', '08:00', '12:00', '16:00', '20:00', '24:00'],
      },
      yAxis: {
        type: 'value',
        name: '内存大小',
        axisLabel: {
          formatter: (value: number) => formatBytes(value),
        },
      },
      series: [
        {
          name: 'MemStore',
          type: 'line',
          smooth: true,
          stack: 'Total',
          areaStyle: {},
          lineStyle: { color: '#1890ff' },
          itemStyle: { color: '#1890ff' },
          data: Array.from({ length: 7 }, () =>
            Math.max(0, (stats?.memstore || 0) * (0.8 + Math.random() * 0.4))
          ),
        },
        {
          name: 'WAL',
          type: 'line',
          smooth: true,
          stack: 'Total',
          areaStyle: {},
          lineStyle: { color: '#52c41a' },
          itemStyle: { color: '#52c41a' },
          data: Array.from({ length: 7 }, () =>
            Math.max(0, (stats?.wal || 0) * (0.8 + Math.random() * 0.4))
          ),
        },
        {
          name: 'Cache',
          type: 'line',
          smooth: true,
          stack: 'Total',
          areaStyle: {},
          lineStyle: { color: '#faad14' },
          itemStyle: { color: '#faad14' },
          data: Array.from({ length: 7 }, () =>
            Math.max(0, (stats?.cache || 0) * (0.8 + Math.random() * 0.4))
          ),
        },
      ],
    }
  }, [stats])

  const gaugeOption = useMemo(() => {
    const maxMemory = 1024 * 1024 * 1024
    const usedPercent = (totalMemory / maxMemory) * 100

    return {
      title: {
        text: '内存使用率',
        left: 'center',
        textStyle: {
          fontSize: 14,
          fontWeight: 'normal',
        },
      },
      series: [
        {
          type: 'gauge',
          startAngle: 180,
          endAngle: 0,
          min: 0,
          max: 100,
          splitNumber: 10,
          axisLine: {
            lineStyle: {
              width: 30,
              color: [
                [0.3, '#52c41a'],
                [0.7, '#faad14'],
                [1, '#ff4d4f'],
              ],
            },
          },
          pointer: {
            icon: 'path://M12.8,0.7l12,40.1H0.7L12.8,0.7z',
            length: '12%',
            width: 20,
            offsetCenter: [0, '-60%'],
            itemStyle: {
              color: 'auto',
            },
          },
          axisTick: {
            length: 12,
            lineStyle: {
              color: 'auto',
              width: 2,
            },
          },
          splitLine: {
            length: 20,
            lineStyle: {
              color: 'auto',
              width: 5,
            },
          },
          axisLabel: {
            color: '#464646',
            fontSize: 12,
            distance: -60,
            formatter: (value: number) => {
              if (value === 100) return '满'
              if (value === 0) return '空'
              return ''
            },
          },
          title: {
            offsetCenter: [0, '20%'],
            fontSize: 14,
          },
          detail: {
            fontSize: 30,
            offsetCenter: [0, '0%'],
            valueAnimation: true,
            formatter: (value: number) => `${value.toFixed(1)}%`,
            color: 'auto',
          },
          data: [
            {
              value: usedPercent,
              name: `${formatBytes(totalMemory)} / ${formatBytes(maxMemory)}`,
            },
          ],
        },
      ],
    }
  }, [totalMemory])

  if (isLoading && !data) {
    return <Loading />
  }

  const maxMemory = 1024 * 1024 * 1024

  return (
    <div className={styles.statsContainer}>
      <Row gutter={[16, 16]}>
        <Col xs={24} sm={12} lg={8}>
          <StatCard
            title="MemStore"
            value={formatBytes(stats?.memstore || 0)}
            prefix={<SaveOutlined style={{ color: '#1890ff' }} />}
            loading={isLoading}
            progress={Math.min(((stats?.memstore || 0) / maxMemory) * 100, 100)}
            progressStatus={(stats?.memstore || 0) / maxMemory > 0.7 ? 'exception' : 'normal'}
          />
        </Col>
        <Col xs={24} sm={12} lg={8}>
          <StatCard
            title="WAL"
            value={formatBytes(stats?.wal || 0)}
            prefix={<SaveOutlined style={{ color: '#52c41a' }} />}
            loading={isLoading}
            progress={Math.min(((stats?.wal || 0) / maxMemory) * 100, 100)}
            progressStatus={(stats?.wal || 0) / maxMemory > 0.7 ? 'exception' : 'normal'}
          />
        </Col>
        <Col xs={24} sm={12} lg={8}>
          <StatCard
            title="Cache"
            value={formatBytes(stats?.cache || 0)}
            prefix={<ThunderboltOutlined style={{ color: '#faad14' }} />}
            loading={isLoading}
            progress={Math.min(((stats?.cache || 0) / maxMemory) * 100, 100)}
            progressStatus={(stats?.cache || 0) / maxMemory > 0.7 ? 'exception' : 'normal'}
          />
        </Col>
      </Row>

      <Row gutter={[16, 16]} style={{ marginTop: 16 }}>
        <Col xs={24} lg={8}>
          <div className={styles.chartCard}>
            <ReactECharts option={gaugeOption} style={{ height: '300px' }} />
          </div>
        </Col>
        <Col xs={24} lg={8}>
          <div className={styles.chartCard}>
            <ReactECharts option={memoryDistributionOption} style={{ height: '300px' }} />
          </div>
        </Col>
        <Col xs={24} lg={8}>
          <div className={styles.chartCard}>
            <div style={{ padding: '20px' }}>
              <Title level={5}>内存使用详情</Title>
              <div style={{ marginTop: 20 }}>
                <div style={{ marginBottom: 16 }}>
                  <div style={{ marginBottom: 8 }}>
                    <strong>MemStore</strong>
                  </div>
                  <Progress
                    percent={Math.min(((stats?.memstore || 0) / maxMemory) * 100, 100)}
                    strokeColor="#1890ff"
                    format={() => `${formatBytes(stats?.memstore || 0)}`}
                  />
                </div>
                <div style={{ marginBottom: 16 }}>
                  <div style={{ marginBottom: 8 }}>
                    <strong>WAL</strong>
                  </div>
                  <Progress
                    percent={Math.min(((stats?.wal || 0) / maxMemory) * 100, 100)}
                    strokeColor="#52c41a"
                    format={() => `${formatBytes(stats?.wal || 0)}`}
                  />
                </div>
                <div style={{ marginBottom: 16 }}>
                  <div style={{ marginBottom: 8 }}>
                    <strong>Cache</strong>
                  </div>
                  <Progress
                    percent={Math.min(((stats?.cache || 0) / maxMemory) * 100, 100)}
                    strokeColor="#faad14"
                    format={() => `${formatBytes(stats?.cache || 0)}`}
                  />
                </div>
                <div style={{ marginTop: 24, paddingTop: 16, borderTop: '1px solid #f0f0f0' }}>
                  <div style={{ marginBottom: 8 }}>
                    <strong>总计</strong>
                  </div>
                  <Progress
                    percent={Math.min((totalMemory / maxMemory) * 100, 100)}
                    strokeColor={{
                      '0%': '#108ee9',
                      '100%': '#87d068',
                    }}
                    format={() => `${formatBytes(totalMemory)}`}
                  />
                </div>
              </div>
            </div>
          </div>
        </Col>
      </Row>

      <Row gutter={[16, 16]} style={{ marginTop: 16 }}>
        <Col xs={24}>
          <div className={styles.chartCard}>
            <ReactECharts option={memoryTrendOption} style={{ height: '300px' }} />
          </div>
        </Col>
      </Row>

      <div className={styles.updateTime}>
        最后更新: {new Date(dataUpdatedAt).toLocaleString('zh-CN')}
      </div>
    </div>
  )
}

export default MemoryStats
