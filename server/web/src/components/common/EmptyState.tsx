import React from 'react'
import { Empty } from 'antd'
import styles from './EmptyState.module.css'

interface EmptyStateProps {
  description?: string
}

const EmptyState: React.FC<EmptyStateProps> = ({ description = '暂无数据' }) => {
  return (
    <div className={styles.container}>
      <Empty description={description} />
    </div>
  )
}

export default EmptyState
