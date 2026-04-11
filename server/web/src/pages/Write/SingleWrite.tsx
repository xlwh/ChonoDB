import React, { useState } from 'react'
import {
  Form,
  Input,
  InputNumber,
  Button,
  Space,
  message,
  Card,
  Table,
  Tag,
  Divider,
  DatePicker,
  Switch,
} from 'antd'
import { PlusOutlined, MinusCircleOutlined, EyeOutlined, SendOutlined } from '@ant-design/icons'
import dayjs from 'dayjs'
import { adminApi } from '../../api'
import type { SingleWriteRequest, WriteHistoryItem } from '../../api/types'

interface LabelPair {
  key: string
  value: string
}

interface FormValues {
  metric: string
  labels: LabelPair[]
  value: number
  useCustomTimestamp: boolean
  timestamp?: number
}

const SingleWrite: React.FC = () => {
  const [form] = Form.useForm<FormValues>()
  const [loading, setLoading] = useState(false)
  const [previewData, setPreviewData] = useState<SingleWriteRequest | null>(null)
  const [history, setHistory] = useState<WriteHistoryItem[]>([])

  const parseLabels = (labels: LabelPair[]): Record<string, string> => {
    const result: Record<string, string> = {}
    labels?.forEach((item) => {
      if (item.key && item.value) {
        result[item.key] = item.value
      }
    })
    return result
  }

  const buildRequestData = (values: FormValues): SingleWriteRequest => {
    const data: SingleWriteRequest = {
      metric: values.metric,
      labels: parseLabels(values.labels || []),
      value: values.value,
    }

    if (values.useCustomTimestamp && values.timestamp) {
      data.timestamp = values.timestamp
    }

    return data
  }

  const handlePreview = () => {
    form.validateFields().then((values) => {
      const data = buildRequestData(values)
      setPreviewData(data)
    })
  }

  const handleSubmit = async () => {
    try {
      const values = await form.validateFields()
      const data = buildRequestData(values)
      setLoading(true)

      const response = await adminApi.putData(data)

      if (response.status === 'success') {
        message.success('数据写入成功')
        form.resetFields()
        setPreviewData(null)

        const historyItem: WriteHistoryItem = {
          id: Date.now().toString(),
          type: 'single',
          metric: data.metric,
          timestamp: Date.now(),
          status: 'success',
          message: `成功写入指标 ${data.metric}`,
        }
        setHistory((prev) => [historyItem, ...prev].slice(0, 10))
      }
    } catch (error: any) {
      message.error(error.message || '写入失败')
      const historyItem: WriteHistoryItem = {
        id: Date.now().toString(),
        type: 'single',
        metric: form.getFieldValue('metric'),
        timestamp: Date.now(),
        status: 'failed',
        message: error.message || '写入失败',
      }
      setHistory((prev) => [historyItem, ...prev].slice(0, 10))
    } finally {
      setLoading(false)
    }
  }

  const previewColumns = [
    {
      title: '字段',
      dataIndex: 'field',
      key: 'field',
      width: 120,
    },
    {
      title: '值',
      dataIndex: 'value',
      key: 'value',
      render: (value: any) => {
        if (typeof value === 'object') {
          return JSON.stringify(value)
        }
        return String(value)
      },
    },
  ]

  const getPreviewDataSource = () => {
    if (!previewData) return []
    return [
      { field: '指标名称', value: previewData.metric },
      { field: '标签', value: previewData.labels },
      { field: '值', value: previewData.value },
      {
        field: '时间戳',
        value: previewData.timestamp
          ? `${previewData.timestamp} (${dayjs(previewData.timestamp).format('YYYY-MM-DD HH:mm:ss')})`
          : '当前时间',
      },
    ]
  }

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
      render: () => <Tag color="blue">单条</Tag>,
    },
    {
      title: '指标',
      dataIndex: 'metric',
      key: 'metric',
      ellipsis: true,
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

  return (
    <div>
      <Form
        form={form}
        layout="vertical"
        initialValues={{
          labels: [{ key: '', value: '' }],
          useCustomTimestamp: false,
        }}
      >
        <Form.Item
          name="metric"
          label="指标名称"
          rules={[
            { required: true, message: '请输入指标名称' },
            { pattern: /^[a-zA-Z_][a-zA-Z0-9_]*$/, message: '指标名称只能包含字母、数字和下划线，且必须以字母或下划线开头' },
          ]}
        >
          <Input placeholder="例如: cpu_usage" />
        </Form.Item>

        <Form.List name="labels">
          {(fields, { add, remove }) => (
            <div>
              <div style={{ marginBottom: 8, fontWeight: 500 }}>标签</div>
              {fields.map((field) => (
                <Space key={field.key} style={{ display: 'flex', marginBottom: 8 }} align="baseline">
                  <Form.Item
                    {...field}
                    name={[field.name, 'key']}
                    rules={[
                      { required: false },
                      { pattern: /^[a-zA-Z_][a-zA-Z0-9_]*$/, message: '键名格式不正确' },
                    ]}
                  >
                    <Input placeholder="标签键" style={{ width: 150 }} />
                  </Form.Item>
                  <Form.Item {...field} name={[field.name, 'value']} rules={[{ required: false }]}>
                    <Input placeholder="标签值" style={{ width: 200 }} />
                  </Form.Item>
                  {fields.length > 1 && (
                    <MinusCircleOutlined onClick={() => remove(field.name)} />
                  )}
                </Space>
              ))}
              <Button type="dashed" onClick={() => add()} icon={<PlusOutlined />}>
                添加标签
              </Button>
            </div>
          )}
        </Form.List>

        <Form.Item
          name="value"
          label="值"
          rules={[{ required: true, message: '请输入值' }]}
        >
          <InputNumber
            style={{ width: '100%' }}
            placeholder="例如: 45.5"
            precision={6}
          />
        </Form.Item>

        <Form.Item name="useCustomTimestamp" label="自定义时间戳" valuePropName="checked">
          <Switch />
        </Form.Item>

        <Form.Item
          noStyle
          shouldUpdate={(prevValues, currentValues) => prevValues.useCustomTimestamp !== currentValues.useCustomTimestamp}
        >
          {({ getFieldValue }) =>
            getFieldValue('useCustomTimestamp') ? (
              <Form.Item
                name="timestamp"
                label="时间戳"
                rules={[{ required: true, message: '请选择时间' }]}
              >
                <DatePicker
                  showTime
                  style={{ width: '100%' }}
                  placeholder="选择时间"
                  onChange={(date) => {
                    if (date) {
                      form.setFieldValue('timestamp', date.valueOf())
                    }
                  }}
                />
              </Form.Item>
            ) : null
          }
        </Form.Item>

        <Form.Item>
          <Space>
            <Button icon={<EyeOutlined />} onClick={handlePreview}>
              预览数据
            </Button>
            <Button
              type="primary"
              icon={<SendOutlined />}
              loading={loading}
              onClick={handleSubmit}
            >
              写入数据
            </Button>
          </Space>
        </Form.Item>
      </Form>

      {previewData && (
        <Card title="数据预览" style={{ marginTop: 16 }} size="small">
          <Table
            columns={previewColumns}
            dataSource={getPreviewDataSource()}
            pagination={false}
            size="small"
            rowKey="field"
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

export default SingleWrite
