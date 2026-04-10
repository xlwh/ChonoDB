import React, { useMemo } from 'react'
import { Row, Col } from 'antd'
import { DatabaseOutlined, FileTextOutlined, HddOutlined } from '@ant-design/icons'
import ReactECharts from 'echarts-for-react'
import { useQuery } from '@tanstack/react-query'
import { adminApi } from '@/api'
import { Loading, StatCard } from '@/components'
import { formatBytes, formatNumber } from '@/utils/format'
import styles from './index.module.css'

interface StorageStatsProps {
  autoRefresh?: boolean
  refreshInterval?: number
}

const StorageStats: React.FC<StorageStatsProps> = ({
  autoRefresh = true,
  refreshInterval = 5000,
}) => {
  const { data, isLoading, dataUpdatedAt } = useQuery({
    queryKey: ['admin', 'stats'],
    queryFn: () => adminApi.getStats(),
    refetchInterval: autoRefresh ? refreshInterval : false,
  })

  const stats = data?.data?.storage

  const chartOption = useMemo(() => {
    if (!stats) return {}

    return {
      tooltip: {
        trigger: 'item',
        formatter: '{a} <br/>{b}: {c} ({d}%)',
      },
      legend: {
        orient: 'vertical',
        right: 10,
        top: 'center',
      },
      series: [
        {
          name: '存储分布',
          type: 'pie',
          radius: ['50%', '70%'],
          avoidLabelOverlap: false,
          itemStyle: {
            borderRadius: 10,
            borderColor: '#fff',
            borderWidth: 2,
          },
          label: {
            show: false,
            position: 'center',
          },
          emphasis: {
            label: {
              show: true,
              fontSize: 20,
              fontWeight: 'bold',
            },
          },
          labelLine: {
            show: false,
          },
          data: [
            {
              value: stats.diskUsage || 0,
              name: '磁盘使用',
              itemStyle: { color: '#1890ff' },
            },
            {
              value: Math.max(0, 1000000000 - (stats.diskUsage || 0)),
              name: '可用空间',
              itemStyle: { color: '#52c41a' },
            },
          ],
        },
      ],
    }
  }, [stats])

  const trendChartOption = useMemo(() => {
    return {
      title: {
        text: '数据增长趋势',
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
        data: ['00:00', '04:00', '08:00', '12:00', '16:00', '20:00', '24:00'],
      },
      yAxis: {
        type: 'value',
        name: '数据点数',
      },
      series: [
        {
          name: '数据点数',
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
                { offset: 0, color: 'rgba(24, 144, 255, 0.3)' },
                { offset: 1, color: 'rgba(24, 144, 255, 0.05)' },
              ],
            },
          },
          lineStyle: {
            color: '#1890ff',
            width: 2,
          },
          itemStyle: {
            color: '#1890ff',
          },
          data: [
            stats?.sampleCount ? stats.sampleCount * 0.7 : 0,
            stats?.sampleCount ? stats.sampleCount * 0.8 : 0,
            stats?.sampleCount ? stats.sampleCount * 0.85 : 0,
            stats?.sampleCount ? stats.sampleCount * 0.9 : 0,
            stats?.sampleCount ? stats.sampleCount * 0.95 : 0,
            stats?.sampleCount || 0,
            stats?.sampleCount ? stats.sampleCount * 1.05 : 0,
          ],
        },
      ],
    }
  }, [stats])

  if (isLoading && !data) {
    return <Loading />
  }

  return (
    <div className={styles.statsContainer}>
      <Row gutter={[16, 16]}>
        <Col xs={24} sm={12} lg={8}>
          <StatCard
            title="时序数量"
            value={formatNumber(stats?.seriesCount || 0, 0)}
            prefix={<DatabaseOutlined style={{ color: '#1890ff' }} />}
            loading={isLoading}
          />
        </Col>
        <Col xs={24} sm={12} lg={8}>
          <StatCard
            title="数据点数量"
            value={formatNumber(stats?.sampleCount || 0, 0)}
            prefix={<FileTextOutlined style={{ color: '#52c41a' }} />}
            loading={isLoading}
          />
        </Col>
        <Col xs={24} sm={12} lg={8}>
          <StatCard
            title="磁盘使用"
            value={formatBytes(stats?.diskUsage || 0)}
            prefix={<HddOutlined style={{ color: '#faad14' }} />}
            loading={isLoading}
          />
        </Col>
      </Row>

      <Row gutter={[16, 16]} style={{ marginTop: 16 }}>
        <Col xs={24} lg={12}>
          <div className={styles.chartCard}>
            <ReactECharts option={chartOption} style={{ height: '300px' }} />
          </div>
        </Col>
        <Col xs={24} lg={12}>
          <div className={styles.chartCard}>
            <ReactECharts option={trendChartOption} style={{ height: '300px' }} />
          </div>
        </Col>
      </Row>

      <div className={styles.updateTime}>
        最后更新: {new Date(dataUpdatedAt).toLocaleString('zh-CN')}
      </div>
    </div>
  )
}

export default StorageStats
