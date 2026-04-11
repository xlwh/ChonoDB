import React from 'react'
import { Typography, Card } from 'antd'
import ConfigEditor from './ConfigEditor'
import styles from './index.module.css'

const { Title } = Typography

const Settings: React.FC = () => {
  return (
    <div className={styles.container}>
      <Title level={3}>配置管理</Title>
      <Card>
        <ConfigEditor />
      </Card>
    </div>
  )
}

export default Settings
