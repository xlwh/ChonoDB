import React from 'react'
import { Typography, Tabs, Card } from 'antd'
import NodeList from './NodeList'
import ShardDistribution from './ShardDistribution'
import styles from './index.module.css'

const { Title } = Typography
const { TabPane } = Tabs

const Cluster: React.FC = () => {
  return (
    <div className={styles.container}>
      <Title level={3}>集群管理</Title>
      <Card>
        <Tabs defaultActiveKey="nodes">
          <TabPane tab="节点列表" key="nodes">
            <NodeList />
          </TabPane>
          <TabPane tab="分片分布" key="shards">
            <ShardDistribution />
          </TabPane>
        </Tabs>
      </Card>
    </div>
  )
}

export default Cluster
