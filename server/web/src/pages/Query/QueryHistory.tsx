import React, { useState, useEffect, useCallback } from 'react'
import { List, Button, Empty, Typography, Tag, Popconfirm, Space } from 'antd'
import { PlayCircleOutlined, DeleteOutlined, ClockCircleOutlined, HistoryOutlined } from '@ant-design/icons'
import dayjs from 'dayjs'
import styles from './index.module.css'

const { Text, Paragraph } = Typography

export interface QueryHistoryItem {
  id: string
  query: string
  type: 'instant' | 'range'
  timestamp: number
  timeRange?: [number, number]
  step?: number
}

interface QueryHistoryProps {
  onSelect: (item: QueryHistoryItem) => void
  maxItems?: number
}

const STORAGE_KEY = 'chronodb_query_history'

const QueryHistory: React.FC<QueryHistoryProps> = ({
  onSelect,
  maxItems = 20,
}) => {
  const [history, setHistory] = useState<QueryHistoryItem[]>([])

  useEffect(() => {
    loadHistory()
  }, [])

  const loadHistory = () => {
    try {
      const stored = localStorage.getItem(STORAGE_KEY)
      if (stored) {
        setHistory(JSON.parse(stored))
      }
    } catch (e) {
      console.error('Failed to load query history:', e)
    }
  }

  const saveHistory = useCallback((items: QueryHistoryItem[]) => {
    try {
      localStorage.setItem(STORAGE_KEY, JSON.stringify(items))
      setHistory(items)
    } catch (e) {
      console.error('Failed to save query history:', e)
    }
  }, [])

  const addToHistory = useCallback((item: Omit<QueryHistoryItem, 'id' | 'timestamp'>) => {
    const newItem: QueryHistoryItem = {
      ...item,
      id: Date.now().toString() + Math.random().toString(36).substr(2, 9),
      timestamp: Date.now(),
    }

    setHistory(prev => {
      const filtered = prev.filter(h => h.query !== item.query)
      const updated = [newItem, ...filtered].slice(0, maxItems)
      saveHistory(updated)
      return updated
    })
  }, [maxItems, saveHistory])

  const removeFromHistory = useCallback((id: string) => {
    setHistory(prev => {
      const updated = prev.filter(h => h.id !== id)
      saveHistory(updated)
      return updated
    })
  }, [saveHistory])

  const clearHistory = useCallback(() => {
    localStorage.removeItem(STORAGE_KEY)
    setHistory([])
  }, [])

  const formatTimeRange = (range?: [number, number]): string => {
    if (!range) return ''
    const duration = range[1] - range[0]
    if (duration < 60) return `${duration}s`
    if (duration < 3600) return `${Math.floor(duration / 60)}m`
    if (duration < 86400) return `${Math.floor(duration / 3600)}h`
    return `${Math.floor(duration / 86400)}d`
  }

  useEffect(() => {
    const handleAddHistory = (e: CustomEvent) => {
      addToHistory(e.detail)
    }

    window.addEventListener('addQueryHistory', handleAddHistory as EventListener)
    return () => {
      window.removeEventListener('addQueryHistory', handleAddHistory as EventListener)
    }
  }, [addToHistory])

  if (history.length === 0) {
    return (
      <div className={styles.historyContainer}>
        <div className={styles.historyHeader}>
          <HistoryOutlined /> <Text strong>查询历史</Text>
        </div>
        <Empty description="暂无查询历史" image={Empty.PRESENTED_IMAGE_SIMPLE} />
      </div>
    )
  }

  return (
    <div className={styles.historyContainer}>
      <div className={styles.historyHeader}>
        <Space>
          <HistoryOutlined />
          <Text strong>查询历史</Text>
          <Tag color="blue">{history.length}</Tag>
        </Space>
        <Popconfirm
          title="确定清空所有历史记录？"
          onConfirm={clearHistory}
          okText="确定"
          cancelText="取消"
        >
          <Button type="link" size="small" danger>
            清空
          </Button>
        </Popconfirm>
      </div>
      <List
        className={styles.historyList}
        dataSource={history}
        renderItem={(item) => (
          <List.Item className={styles.historyItem}>
            <div className={styles.historyItemContent}>
              <div className={styles.historyItemHeader}>
                <Space size="small">
                  <Tag color={item.type === 'instant' ? 'green' : 'blue'}>
                    {item.type === 'instant' ? '即时' : '范围'}
                  </Tag>
                  {item.timeRange && (
                    <Tag>{formatTimeRange(item.timeRange)}</Tag>
                  )}
                  <Text type="secondary" className={styles.historyTime}>
                    <ClockCircleOutlined /> {dayjs(item.timestamp).format('MM-DD HH:mm')}
                  </Text>
                </Space>
              </div>
              <Paragraph
                className={styles.historyQuery}
                ellipsis={{ rows: 2, expandable: true, symbol: '展开' }}
                style={{ marginBottom: 8, fontFamily: 'monospace' }}
              >
                {item.query}
              </Paragraph>
              <div className={styles.historyActions}>
                <Button
                  type="primary"
                  size="small"
                  icon={<PlayCircleOutlined />}
                  onClick={() => onSelect(item)}
                >
                  执行
                </Button>
                <Button
                  type="text"
                  size="small"
                  icon={<DeleteOutlined />}
                  danger
                  onClick={() => removeFromHistory(item.id)}
                />
              </div>
            </div>
          </List.Item>
        )}
      />
    </div>
  )
}

export const addQueryToHistory = (item: Omit<QueryHistoryItem, 'id' | 'timestamp'>) => {
  const event = new CustomEvent('addQueryHistory', { detail: item })
  window.dispatchEvent(event)
}

export default QueryHistory
