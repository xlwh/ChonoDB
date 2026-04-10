import React from 'react'
import { Typography, Tabs, Card } from 'antd'
import RulesList from './RulesList'
import ActiveAlerts from './ActiveAlerts'
import styles from './index.module.css'

const { Title } = Typography
const { TabPane } = Tabs

const Alerts: React.FC = () => {
  return (
    <div className={styles.container}>
      <Title level={3}>告警管理</Title>
      <Card>
        <Tabs defaultActiveKey="rules">
          <TabPane tab="告警规则" key="rules">
            <RulesList />
          </TabPane>
          <TabPane tab="活动告警" key="active">
            <ActiveAlerts />
          </TabPane>
        </Tabs>
      </Card>
    </div>
  )
}

export default Alerts
