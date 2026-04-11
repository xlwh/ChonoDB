import React from 'react'
import { Card, Statistic, Progress } from 'antd'
import { ArrowUpOutlined, ArrowDownOutlined } from '@ant-design/icons'
import styles from './StatCard.module.css'

interface StatCardProps {
  title: string
  value: number | string
  suffix?: string
  prefix?: React.ReactNode
  precision?: number
  trend?: 'up' | 'down' | 'neutral'
  trendValue?: number
  loading?: boolean
  progress?: number
  progressStatus?: 'success' | 'normal' | 'exception' | 'active'
  extra?: React.ReactNode
  onClick?: () => void
}

const StatCard: React.FC<StatCardProps> = ({
  title,
  value,
  suffix,
  prefix,
  precision = 2,
  trend,
  trendValue,
  loading = false,
  progress,
  progressStatus = 'normal',
  extra,
  onClick,
}) => {
  const renderTrend = () => {
    if (!trend || trendValue === undefined) return null

    const trendColor = trend === 'up' ? '#52c41a' : trend === 'down' ? '#ff4d4f' : '#8c8c8c'
    const TrendIcon = trend === 'up' ? ArrowUpOutlined : ArrowDownOutlined

    return (
      <div className={styles.trend} style={{ color: trendColor }}>
        <TrendIcon />
        <span>{Math.abs(trendValue).toFixed(2)}%</span>
      </div>
    )
  }

  return (
    <Card
      className={`${styles.statCard} ${onClick ? styles.clickable : ''}`}
      loading={loading}
      onClick={onClick}
      hoverable={!!onClick}
    >
      <div className={styles.header}>
        <div className={styles.title}>{title}</div>
        {extra}
      </div>
      <div className={styles.content}>
        <Statistic
          value={value}
          suffix={suffix}
          prefix={prefix}
          precision={precision}
          valueStyle={{ fontSize: '28px', fontWeight: 600 }}
        />
        {renderTrend()}
      </div>
      {progress !== undefined && (
        <div className={styles.progress}>
          <Progress
            percent={progress}
            status={progressStatus}
            strokeColor={{
              '0%': '#108ee9',
              '100%': '#87d068',
            }}
          />
        </div>
      )}
    </Card>
  )
}

export default StatCard
