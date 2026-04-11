import React from 'react'
import { Spin } from 'antd'
import styles from './Loading.module.css'

interface LoadingProps {
  tip?: string
}

const Loading: React.FC<LoadingProps> = ({ tip = '加载中...' }) => {
  return (
    <div className={styles.container}>
      <Spin size="large" tip={tip} />
    </div>
  )
}

export default Loading
