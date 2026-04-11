import React, { useState } from 'react'
import { Typography, Tabs, Card, Switch, Select, Space, Button, message } from 'antd'
import { ReloadOutlined } from '@ant-design/icons'
import { useQueryClient } from '@tanstack/react-query'
import StorageStats from './StorageStats'
import QueryStats from './QueryStats'
import MemoryStats from './MemoryStats'
import styles from './index.module.css'

const { Title } = Typography
const { TabPane } = Tabs

const Metrics: React.FC = () => {
  const [autoRefresh, setAutoRefresh] = useState(true)
  const [refreshInterval, setRefreshInterval] = useState(5000)
  const queryClient = useQueryClient()

  const handleRefresh = () => {
    queryClient.invalidateQueries({ queryKey: ['admin', 'stats'] })
    message.success('数据已刷新')
  }

  return (
    <div className={styles.container}>
      <div className={styles.header}>
        <Title level={3} style={{ margin: 0 }}>
          统计指标
        </Title>
        <Space size="middle">
          <div className={styles.refreshControl}>
            <span className={styles.label}>自动刷新</span>
            <Switch checked={autoRefresh} onChange={setAutoRefresh} />
          </div>
          {autoRefresh && (
            <div className={styles.intervalControl}>
              <span className={styles.label}>刷新间隔</span>
              <Select
                value={refreshInterval}
                onChange={setRefreshInterval}
                style={{ width: 120 }}
                options={[
                  { label: '3 秒', value: 3000 },
                  { label: '5 秒', value: 5000 },
                  { label: '10 秒', value: 10000 },
                  { label: '30 秒', value: 30000 },
                  { label: '1 分钟', value: 60000 },
                ]}
              />
            </div>
          )}
          <Button
            type="primary"
            icon={<ReloadOutlined />}
            onClick={handleRefresh}
          >
            刷新数据
          </Button>
        </Space>
      </div>

      <Card className={styles.mainCard}>
        <Tabs defaultActiveKey="storage" size="large">
          <TabPane tab="存储统计" key="storage">
            <StorageStats
              autoRefresh={autoRefresh}
              refreshInterval={refreshInterval}
            />
          </TabPane>
          <TabPane tab="查询统计" key="query">
            <QueryStats
              autoRefresh={autoRefresh}
              refreshInterval={refreshInterval}
            />
          </TabPane>
          <TabPane tab="内存统计" key="memory">
            <MemoryStats
              autoRefresh={autoRefresh}
              refreshInterval={refreshInterval}
            />
          </TabPane>
        </Tabs>
      </Card>
    </div>
  )
}

export default Metrics
