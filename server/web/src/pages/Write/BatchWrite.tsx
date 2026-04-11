import React, { useState } from 'react'
import {
  Input,
  Button,
  message,
  Card,
  Table,
  Tag,
  Divider,
  Upload,
  Space,
  Alert,
  Statistic,
  Row,
  Col,
} from 'antd'
import {
  UploadOutlined,
  EyeOutlined,
  SendOutlined,
  FileTextOutlined,
  CheckCircleOutlined,
  CloseCircleOutlined,
} from '@ant-design/icons'
import type { UploadFile } from 'antd/es/upload/interface'
import dayjs from 'dayjs'
import { adminApi } from '../../api'
import type { BatchWriteRequest, WriteHistoryItem, BatchWriteResponse } from '../../api/types'

const { TextArea } = Input

const BatchWrite: React.FC = () => {
  const [jsonText, setJsonText] = useState('')
  const [loading, setLoading] = useState(false)
  const [previewData, setPreviewData] = useState<BatchWriteRequest | null>(null)
  const [history, setHistory] = useState<WriteHistoryItem[]>([])
  const [writeResult, setWriteResult] = useState<BatchWriteResponse | null>(null)
  const [fileList, setFileList] = useState<UploadFile[]>([])

  const validateJson = (text: string): { valid: boolean; error?: string } => {
    if (!text.trim()) {
      return { valid: false, error: '请输入 JSON 数据' }
    }

    try {
      const data = JSON.parse(text)
      if (!data.timeseries || !Array.isArray(data.timeseries)) {
        return { valid: false, error: 'JSON 格式错误：必须包含 timeseries 数组' }
      }

      for (let i = 0; i < data.timeseries.length; i++) {
        const ts = data.timeseries[i]
        if (!ts.labels || !Array.isArray(ts.labels)) {
          return { valid: false, error: `第 ${i + 1} 个时间序列缺少 labels 字段` }
        }
        if (!ts.samples || !Array.isArray(ts.samples)) {
          return { valid: false, error: `第 ${i + 1} 个时间序列缺少 samples 字段` }
        }

        for (const label of ts.labels) {
          if (!label.name || !label.value) {
            return { valid: false, error: `第 ${i + 1} 个时间序列的标签格式不正确` }
          }
        }

        for (const sample of ts.samples) {
          if (typeof sample.timestamp !== 'number' || typeof sample.value !== 'number') {
            return { valid: false, error: `第 ${i + 1} 个时间序列的样本数据格式不正确` }
          }
        }
      }

      return { valid: true }
    } catch (e: any) {
      return { valid: false, error: `JSON 解析错误: ${e.message}` }
    }
  }

  const handlePreview = () => {
    const validation = validateJson(jsonText)
    if (!validation.valid) {
      message.error(validation.error)
      return
    }

    try {
      const data = JSON.parse(jsonText) as BatchWriteRequest
      setPreviewData(data)
      setWriteResult(null)
      message.success(`数据验证通过，共 ${data.timeseries.length} 个时间序列`)
    } catch (e: any) {
      message.error(`解析错误: ${e.message}`)
    }
  }

  const handleSubmit = async () => {
    const validation = validateJson(jsonText)
    if (!validation.valid) {
      message.error(validation.error)
      return
    }

    try {
      const data = JSON.parse(jsonText) as BatchWriteRequest
      setLoading(true)
      setWriteResult(null)

      const response = await adminApi.batchData(data)

      if (response.status === 'success') {
        setWriteResult(response)

        if (response.data.failed === 0) {
          message.success(`批量写入成功，共写入 ${response.data.success} 条数据`)
        } else {
          message.warning(`写入完成：成功 ${response.data.success} 条，失败 ${response.data.failed} 条`)
        }

        const historyItem: WriteHistoryItem = {
          id: Date.now().toString(),
          type: 'batch',
          timestamp: Date.now(),
          status: response.data.failed === 0 ? 'success' : 'failed',
          message: `批量写入：成功 ${response.data.success}，失败 ${response.data.failed}`,
          count: response.data.total,
        }
        setHistory((prev) => [historyItem, ...prev].slice(0, 10))
      }
    } catch (error: any) {
      message.error(error.message || '批量写入失败')
      const historyItem: WriteHistoryItem = {
        id: Date.now().toString(),
        type: 'batch',
        timestamp: Date.now(),
        status: 'failed',
        message: error.message || '批量写入失败',
      }
      setHistory((prev) => [historyItem, ...prev].slice(0, 10))
    } finally {
      setLoading(false)
    }
  }

  const handleFileUpload = (file: File) => {
    const reader = new FileReader()
    reader.onload = (e) => {
      const text = e.target?.result as string
      setJsonText(text)
      setPreviewData(null)
      setWriteResult(null)
      message.success('文件加载成功')
    }
    reader.onerror = () => {
      message.error('文件读取失败')
    }
    reader.readAsText(file)
    return false
  }

  const handleClear = () => {
    setJsonText('')
    setPreviewData(null)
    setWriteResult(null)
    setFileList([])
  }

  const previewColumns = [
    {
      title: '序号',
      dataIndex: 'index',
      key: 'index',
      width: 60,
      render: (_: any, __: any, index: number) => index + 1,
    },
    {
      title: '标签',
      dataIndex: 'labels',
      key: 'labels',
      render: (labels: { name: string; value: string }[]) => {
        const metricLabel = labels.find((l) => l.name === '__name__')
        const otherLabels = labels.filter((l) => l.name !== '__name__')
        return (
          <div>
            {metricLabel && (
              <Tag color="blue" style={{ marginBottom: 4 }}>
                {metricLabel.value}
              </Tag>
            )}
            {otherLabels.map((label, idx) => (
              <Tag key={idx} style={{ marginBottom: 4 }}>
                {label.name}={label.value}
              </Tag>
            ))}
          </div>
        )
      },
    },
    {
      title: '样本数',
      dataIndex: 'samples',
      key: 'samples',
      width: 100,
      render: (samples: any[]) => samples.length,
    },
    {
      title: '时间范围',
      key: 'timeRange',
      width: 200,
      render: (_: any, record: any) => {
        if (!record.samples || record.samples.length === 0) return '-'
        const timestamps = record.samples.map((s: any) => s.timestamp)
        const min = Math.min(...timestamps)
        const max = Math.max(...timestamps)
        return (
          <div style={{ fontSize: 12 }}>
            <div>{dayjs(min).format('YYYY-MM-DD HH:mm:ss')}</div>
            <div>{dayjs(max).format('YYYY-MM-DD HH:mm:ss')}</div>
          </div>
        )
      },
    },
  ]

  const historyColumns = [
    {
      title: '时间',
      dataIndex: 'timestamp',
      key: 'timestamp',
      width: 180,
      render: (timestamp: number) => dayjs(timestamp).format('HH:mm:ss'),
    },
    {
      title: '类型',
      dataIndex: 'type',
      key: 'type',
      width: 80,
      render: () => <Tag color="purple">批量</Tag>,
    },
    {
      title: '数据量',
      dataIndex: 'count',
      key: 'count',
      width: 100,
      render: (count: number) => (count ? `${count} 条` : '-'),
    },
    {
      title: '状态',
      dataIndex: 'status',
      key: 'status',
      width: 80,
      render: (status: string) => (
        <Tag color={status === 'success' ? 'success' : 'error'}>
          {status === 'success' ? '成功' : '失败'}
        </Tag>
      ),
    },
    {
      title: '消息',
      dataIndex: 'message',
      key: 'message',
      ellipsis: true,
    },
  ]

  const exampleJson = `{
  "timeseries": [
    {
      "labels": [
        {"name": "__name__", "value": "cpu_usage"},
        {"name": "host", "value": "server1"},
        {"name": "region", "value": "us-west"}
      ],
      "samples": [
        {"timestamp": ${Date.now()}, "value": 45.5},
        {"timestamp": ${Date.now() + 60000}, "value": 52.3}
      ]
    },
    {
      "labels": [
        {"name": "__name__", "value": "memory_usage"},
        {"name": "host", "value": "server1"}
      ],
      "samples": [
        {"timestamp": ${Date.now()}, "value": 1024.5}
      ]
    }
  ]
}`

  return (
    <div>
      <Alert
        message="数据格式说明"
        description={
          <div>
            <p>批量写入需要提供 JSON 格式的数据，包含 timeseries 数组。每个时间序列包含 labels 和 samples。</p>
            <p>labels 数组中必须包含 __name__ 标签来指定指标名称。</p>
          </div>
        }
        type="info"
        showIcon
        style={{ marginBottom: 16 }}
      />

      <Space style={{ marginBottom: 16 }}>
        <Upload
          accept=".json"
          beforeUpload={handleFileUpload}
          fileList={fileList}
          onChange={({ fileList }) => setFileList(fileList)}
          showUploadList={false}
        >
          <Button icon={<UploadOutlined />}>上传 JSON 文件</Button>
        </Upload>
        <Button onClick={() => setJsonText(exampleJson)}>加载示例</Button>
        <Button onClick={handleClear}>清空</Button>
      </Space>

      <TextArea
        rows={12}
        value={jsonText}
        onChange={(e) => {
          setJsonText(e.target.value)
          setPreviewData(null)
          setWriteResult(null)
        }}
        placeholder="请输入 JSON 格式的数据，或上传 JSON 文件..."
        style={{ fontFamily: 'monospace', marginBottom: 16 }}
      />

      <Space style={{ marginBottom: 16 }}>
        <Button icon={<EyeOutlined />} onClick={handlePreview}>
          预览数据
        </Button>
        <Button
          type="primary"
          icon={<SendOutlined />}
          loading={loading}
          onClick={handleSubmit}
        >
          批量写入
        </Button>
      </Space>

      {writeResult && (
        <Card style={{ marginBottom: 16 }}>
          <Row gutter={16}>
            <Col span={6}>
              <Statistic
                title="总数"
                value={writeResult.data.total}
                prefix={<FileTextOutlined />}
              />
            </Col>
            <Col span={6}>
              <Statistic
                title="成功"
                value={writeResult.data.success}
                valueStyle={{ color: '#3f8600' }}
                prefix={<CheckCircleOutlined />}
              />
            </Col>
            <Col span={6}>
              <Statistic
                title="失败"
                value={writeResult.data.failed}
                valueStyle={{ color: writeResult.data.failed > 0 ? '#cf1322' : '#3f8600' }}
                prefix={<CloseCircleOutlined />}
              />
            </Col>
            <Col span={6}>
              <Statistic
                title="成功率"
                value={
                  writeResult.data.total > 0
                    ? ((writeResult.data.success / writeResult.data.total) * 100).toFixed(1)
                    : 0
                }
                suffix="%"
              />
            </Col>
          </Row>
          {writeResult.data.errors && writeResult.data.errors.length > 0 && (
            <div style={{ marginTop: 16 }}>
              <div style={{ fontWeight: 500, marginBottom: 8 }}>错误详情：</div>
              {writeResult.data.errors.map((error, idx) => (
                <Alert key={idx} message={error} type="error" style={{ marginBottom: 4 }} />
              ))}
            </div>
          )}
        </Card>
      )}

      {previewData && (
        <Card title={`数据预览 (${previewData.timeseries.length} 个时间序列)`} style={{ marginBottom: 16 }} size="small">
          <Table
            columns={previewColumns}
            dataSource={previewData.timeseries}
            pagination={{ pageSize: 10 }}
            size="small"
            rowKey={(_, index) => `ts-${index}`}
          />
        </Card>
      )}

      {history.length > 0 && (
        <>
          <Divider>写入历史</Divider>
          <Table
            columns={historyColumns}
            dataSource={history}
            pagination={false}
            size="small"
            rowKey="id"
          />
        </>
      )}
    </div>
  )
}

export default BatchWrite
