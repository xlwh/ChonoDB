import React, { useState, useCallback, useEffect } from 'react'
import { Typography, Card, Button, Tabs, message, Space, Tag, Alert, Segmented, Row, Col } from 'antd'
import { PlayCircleOutlined, ClearOutlined, TableOutlined, LineChartOutlined } from '@ant-design/icons'
import QueryEditor from './QueryEditor'
import TimeRangePicker from './TimeRangePicker'
import QueryResultTable from './QueryResultTable'
import QueryResultChart from './QueryResultChart'
import QueryHistory, { addQueryToHistory, QueryHistoryItem } from './QueryHistory'
import { useInstantQuery, useRangeQuery } from '@/hooks/useQuery'
import styles from './index.module.css'

const { Title, Text } = Typography

const EXAMPLE_QUERIES = [
  'up',
  'http_requests_total',
  'http_requests_total{method="GET"}',
  'rate(http_requests_total[5m])',
  'sum by (job) (up)',
]

const Query: React.FC = () => {
  const [query, setQuery] = useState('')
  const [queryType, setQueryType] = useState<'instant' | 'range'>('instant')
  const [timeRange, setTimeRange] = useState<[number, number]>(() => {
    const now = Math.floor(Date.now() / 1000)
    return [now - 3600, now]
  })
  const [step, setStep] = useState(60)
  const [viewMode, setViewMode] = useState<'chart' | 'table'>('chart')
  const [executeTrigger, setExecuteTrigger] = useState(0)

  const instantQuery = useInstantQuery(query, executeTrigger > 0 ? undefined : undefined)
  const rangeQuery = useRangeQuery(
    query,
    timeRange[0],
    timeRange[1],
    step,
    executeTrigger > 0
  )

  const currentQuery = queryType === 'instant' ? instantQuery : rangeQuery
  const isLoading = currentQuery.isLoading || currentQuery.isFetching
  const queryResult = currentQuery.data

  const handleExecute = useCallback(() => {
    if (!query.trim()) {
      message.warning('请输入查询语句')
      return
    }

    setExecuteTrigger(prev => prev + 1)

    addQueryToHistory({
      query: query.trim(),
      type: queryType,
      timeRange: queryType === 'range' ? timeRange : undefined,
      step: queryType === 'range' ? step : undefined,
    })
  }, [query, queryType, timeRange, step])

  const handleClear = useCallback(() => {
    setQuery('')
    setExecuteTrigger(0)
  }, [])

  const handleHistorySelect = useCallback((item: QueryHistoryItem) => {
    setQuery(item.query)
    setQueryType(item.type)
    if (item.timeRange) {
      setTimeRange(item.timeRange)
    }
    if (item.step) {
      setStep(item.step)
    }
    setExecuteTrigger(prev => prev + 1)

    addQueryToHistory({
      query: item.query,
      type: item.type,
      timeRange: item.timeRange,
      step: item.step,
    })
  }, [])

  const handleExampleClick = useCallback((example: string) => {
    setQuery(example)
  }, [])

  useEffect(() => {
    if (currentQuery.isError) {
      const error = currentQuery.error as any
      message.error(error?.message || '查询失败')
    }
  }, [currentQuery.isError, currentQuery.error])

  const renderResult = () => {
    if (currentQuery.isError) {
      const error = currentQuery.error as any
      return (
        <Alert
          message="查询错误"
          description={error?.response?.data?.error || error?.message || '未知错误'}
          type="error"
          showIcon
          className={styles.errorAlert}
        />
      )
    }

    if (!queryResult) {
      return null
    }

    const resultType = queryResult.data?.resultType || 'vector'
    const resultData = queryResult.data?.result || []

    return (
      <div>
        <div className={styles.resultHeader}>
          <div className={styles.resultStats}>
            <Tag color="blue">结果类型: {resultType}</Tag>
            <Tag color="green">数据条数: {Array.isArray(resultData) ? resultData.length : 0}</Tag>
          </div>
          <Segmented
            value={viewMode}
            onChange={(value) => setViewMode(value as 'chart' | 'table')}
            options={[
              { label: '图表', value: 'chart', icon: <LineChartOutlined /> },
              { label: '表格', value: 'table', icon: <TableOutlined /> },
            ]}
          />
        </div>

        {viewMode === 'chart' ? (
          <div className={styles.chartContainer}>
            <QueryResultChart
              data={resultData}
              resultType={resultType}
              loading={isLoading}
            />
          </div>
        ) : null}

        <div className={styles.tableContainer}>
          <QueryResultTable
            data={resultData}
            resultType={resultType}
            loading={isLoading}
          />
        </div>
      </div>
    )
  }

  return (
    <div className={styles.container}>
      <Title level={3}>PromQL 查询</Title>

      <Card className={styles.editor}>
        <QueryEditor
          value={query}
          onChange={setQuery}
          onSubmit={handleExecute}
        />
        <div className={styles.actions}>
          <div className={styles.exampleQueries}>
            <Text type="secondary" style={{ marginRight: 8 }}>示例:</Text>
            {EXAMPLE_QUERIES.map((example) => (
              <span
                key={example}
                className={styles.exampleQuery}
                onClick={() => handleExampleClick(example)}
              >
                {example}
              </span>
            ))}
          </div>
          <Space>
            <Button icon={<ClearOutlined />} onClick={handleClear}>
              清空
            </Button>
            <Button
              type="primary"
              icon={<PlayCircleOutlined />}
              onClick={handleExecute}
              loading={isLoading}
            >
              执行查询
            </Button>
          </Space>
        </div>
      </Card>

      <Tabs
        activeKey={queryType}
        onChange={(key) => setQueryType(key as 'instant' | 'range')}
        className={styles.queryTypeTabs}
        items={[
          {
            key: 'instant',
            label: '即时查询',
            children: null,
          },
          {
            key: 'range',
            label: '范围查询',
            children: (
              <TimeRangePicker
                value={timeRange}
                onChange={setTimeRange}
                step={step}
                onStepChange={setStep}
              />
            ),
          },
        ]}
      />

      {renderResult()}

      <Row gutter={16}>
        <Col span={24}>
          <QueryHistory onSelect={handleHistorySelect} />
        </Col>
      </Row>
    </div>
  )
}

export default Query
