import React from 'react'
import { Typography, Row, Col, Card, Statistic } from 'antd'
import {
  DatabaseOutlined,
  ClockCircleOutlined,
  CheckCircleOutlined,
  WarningOutlined,
} from '@ant-design/icons'
import { useQuery } from '@tanstack/react-query'
import { adminApi } from '@/api'
import { Loading } from '@/components'
import styles from './index.module.css'

const { Title } = Typography

const Dashboard: React.FC = () => {
  const { data, isLoading } = useQuery({
    queryKey: ['admin', 'stats'],
    queryFn: () => adminApi.getStats(),
    refetchInterval: 5000,
  })

  if (isLoading) {
    return <Loading />
  }

  const stats = data?.data

  return (
    <div className={styles.container}>
      <Title level={3}>仪表盘</Title>
      <Row gutter={[16, 16]}>
        <Col xs={24} sm={12} lg={6}>
          <Card>
            <Statistic
              title="时序数量"
              value={stats?.storage.seriesCount || 0}
              prefix={<DatabaseOutlined />}
            />
          </Card>
        </Col>
        <Col xs={24} sm={12} lg={6}>
          <Card>
            <Statistic
              title="数据点数量"
              value={stats?.storage.sampleCount || 0}
              prefix={<ClockCircleOutlined />}
            />
          </Card>
        </Col>
        <Col xs={24} sm={12} lg={6}>
          <Card>
            <Statistic
              title="查询总数"
              value={stats?.query.totalQueries || 0}
              prefix={<CheckCircleOutlined />}
            />
          </Card>
        </Col>
        <Col xs={24} sm={12} lg={6}>
          <Card>
            <Statistic
              title="错误率"
              value={stats?.query.errorRate || 0}
              precision={4}
              suffix="%"
              prefix={<WarningOutlined />}
              valueStyle={{ color: (stats?.query.errorRate || 0) > 0.01 ? '#cf1322' : '#3f8600' }}
            />
          </Card>
        </Col>
      </Row>
    </div>
  )
}

export default Dashboard
