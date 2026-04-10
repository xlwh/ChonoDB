import React from 'react'
import { Table, Tag, Button, Space } from 'antd'
import { useQuery } from '@tanstack/react-query'
import { adminApi } from '@/api'
import { Loading } from '@/components'

const RulesList: React.FC = () => {
  const { data, isLoading } = useQuery({
    queryKey: ['admin', 'alerts', 'rules'],
    queryFn: () => adminApi.getAlertRules(),
  })

  if (isLoading) {
    return <Loading />
  }

  const groups = data?.data?.groups || []
  const rules = groups.flatMap((group) =>
    group.rules.map((rule) => ({ ...rule, group: group.name }))
  )

  const columns = [
    { title: '规则名称', dataIndex: 'name', key: 'name' },
    { title: '分组', dataIndex: 'group', key: 'group' },
    { title: '查询', dataIndex: 'query', key: 'query', ellipsis: true },
    { title: '持续时间', dataIndex: 'duration', key: 'duration' },
    {
      title: '严重程度',
      dataIndex: 'severity',
      key: 'severity',
      render: (severity: string) => {
        const color = severity === 'critical' ? 'red' : severity === 'warning' ? 'orange' : 'blue'
        return <Tag color={color}>{severity}</Tag>
      },
    },
    {
      title: '状态',
      dataIndex: 'state',
      key: 'state',
      render: (state: string) => {
        const color = state === 'firing' ? 'red' : state === 'pending' ? 'orange' : 'green'
        return <Tag color={color}>{state}</Tag>
      },
    },
    {
      title: '操作',
      key: 'action',
      render: () => (
        <Space>
          <Button type="link" size="small">
            编辑
          </Button>
          <Button type="link" size="small" danger>
            删除
          </Button>
        </Space>
      ),
    },
  ]

  return <Table columns={columns} dataSource={rules} rowKey="name" />
}

export default RulesList
