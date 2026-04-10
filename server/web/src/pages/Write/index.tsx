import React from 'react'
import { Typography, Tabs, Card } from 'antd'
import SingleWrite from './SingleWrite'
import BatchWrite from './BatchWrite'
import styles from './index.module.css'

const { Title } = Typography
const { TabPane } = Tabs

const Write: React.FC = () => {
  return (
    <div className={styles.container}>
      <Title level={3}>数据写入</Title>
      <Card>
        <Tabs defaultActiveKey="single">
          <TabPane tab="单条写入" key="single">
            <SingleWrite />
          </TabPane>
          <TabPane tab="批量写入" key="batch">
            <BatchWrite />
          </TabPane>
        </Tabs>
      </Card>
    </div>
  )
}

export default Write
