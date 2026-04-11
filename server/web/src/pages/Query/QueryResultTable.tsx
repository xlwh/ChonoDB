import React from 'react'
import { Table, Tag, Empty, Typography, Tooltip } from 'antd'
import type { ColumnsType } from 'antd/es/table'
import dayjs from 'dayjs'
import type { TimeSeries } from '@/api/types'
import styles from './index.module.css'

const { Text } = Typography

interface QueryResultTableProps {
  data: TimeSeries[] | undefined
  resultType: 'vector' | 'matrix' | 'scalar' | 'string'
  loading?: boolean
}

const QueryResultTable: React.FC<QueryResultTableProps> = ({
  data,
  resultType,
  loading = false,
}) => {
  if (!data || data.length === 0) {
    return <Empty description="暂无查询结果" />
  }

  const formatValue = (value: string): string => {
    const num = parseFloat(value)
    if (isNaN(num)) return value
    if (Math.abs(num) >= 1e9) return (num / 1e9).toFixed(2) + 'G'
    if (Math.abs(num) >= 1e6) return (num / 1e6).toFixed(2) + 'M'
    if (Math.abs(num) >= 1e3) return (num / 1e3).toFixed(2) + 'K'
    return num.toFixed(6)
  }

  const formatTimestamp = (timestamp: number): string => {
    return dayjs(timestamp * 1000).format('YYYY-MM-DD HH:mm:ss.SSS')
  }

  const formatLabels = (metric: Record<string, string>): React.ReactNode => {
    if (!metric || Object.keys(metric).length === 0) {
      return <Text type="secondary">-</Text>
    }

    return (
      <div className={styles.labelsContainer}>
        {Object.entries(metric).map(([key, value]) => (
          <Tag key={key} className={styles.labelTag}>
            <Text strong>{key}</Text>=<Text type="success">"{value}"</Text>
          </Tag>
        ))}
      </div>
    )
  }

  if (resultType === 'scalar' || resultType === 'string') {
    const scalarData = data[0]
    const value = scalarData?.value
    return (
      <div className={styles.scalarResult}>
        <Text strong>值: </Text>
        <Text code>{value?.value ?? '-'}</Text>
        {value?.timestamp && (
          <>
            <Text strong style={{ marginLeft: 16 }}>时间: </Text>
            <Text>{formatTimestamp(value.timestamp)}</Text>
          </>
        )}
      </div>
    )
  }

  if (resultType === 'vector') {
    const columns: ColumnsType<TimeSeries> = [
      {
        title: '指标',
        dataIndex: 'metric',
        key: 'metric',
        width: '60%',
        render: (metric: Record<string, string>) => formatLabels(metric),
      },
      {
        title: '值',
        dataIndex: 'value',
        key: 'value',
        width: '20%',
        render: (value: { timestamp: number; value: string }) => (
          <Tooltip title={`原始值: ${value.value}`}>
            <Text code className={styles.valueText}>
              {formatValue(value.value)}
            </Text>
          </Tooltip>
        ),
      },
      {
        title: '时间',
        dataIndex: 'value',
        key: 'timestamp',
        width: '20%',
        render: (value: { timestamp: number; value: string }) => (
          <Text type="secondary">
            {formatTimestamp(value.timestamp)}
          </Text>
        ),
      },
    ]

    return (
      <Table
        columns={columns}
        dataSource={data.map((item, index) => ({ ...item, key: index }))}
        loading={loading}
        pagination={{
          pageSize: 20,
          showSizeChanger: true,
          showTotal: (total) => `共 ${total} 条结果`,
        }}
        size="small"
        scroll={{ x: 'max-content' }}
      />
    )
  }

  if (resultType === 'matrix') {
    const columns: ColumnsType<TimeSeries> = [
      {
        title: '指标',
        dataIndex: 'metric',
        key: 'metric',
        width: '40%',
        render: (metric: Record<string, string>) => formatLabels(metric),
      },
      {
        title: '数据点数量',
        dataIndex: 'values',
        key: 'count',
        width: '15%',
        render: (values: { timestamp: number; value: string }[]) => (
          <Tag color="blue">{values?.length || 0} 个点</Tag>
        ),
      },
      {
        title: '起始时间',
        dataIndex: 'values',
        key: 'startTime',
        width: '22%',
        render: (values: { timestamp: number; value: string }[]) => {
          if (!values || values.length === 0) return '-'
          return (
            <Text type="secondary">
              {formatTimestamp(values[0].timestamp)}
            </Text>
          )
        },
      },
      {
        title: '结束时间',
        dataIndex: 'values',
        key: 'endTime',
        width: '23%',
        render: (values: { timestamp: number; value: string }[]) => {
          if (!values || values.length === 0) return '-'
          return (
            <Text type="secondary">
              {formatTimestamp(values[values.length - 1].timestamp)}
            </Text>
          )
        },
      },
    ]

    return (
      <Table
        columns={columns}
        dataSource={data.map((item, index) => ({ ...item, key: index }))}
        loading={loading}
        pagination={{
          pageSize: 20,
          showSizeChanger: true,
          showTotal: (total) => `共 ${total} 条时序`,
        }}
        size="small"
        scroll={{ x: 'max-content' }}
        expandable={{
          expandedRowRender: (record) => {
            if (!record.values || record.values.length === 0) {
              return <Empty description="无数据点" />
            }

            const detailColumns: ColumnsType<{ timestamp: number; value: string }> = [
              {
                title: '时间',
                dataIndex: 'timestamp',
                key: 'timestamp',
                render: (ts: number) => formatTimestamp(ts),
              },
              {
                title: '值',
                dataIndex: 'value',
                key: 'value',
                render: (val: string) => (
                  <Tooltip title={`原始值: ${val}`}>
                    <Text code>{formatValue(val)}</Text>
                  </Tooltip>
                ),
              },
            ]

            return (
              <Table
                columns={detailColumns}
                dataSource={record.values.map((v, i) => ({ ...v, key: i }))}
                pagination={{
                  pageSize: 10,
                  showTotal: (total) => `共 ${total} 个数据点`,
                }}
                size="small"
              />
            )
          },
        }}
      />
    )
  }

  return <Empty description="未知的结果类型" />
}

export default QueryResultTable
