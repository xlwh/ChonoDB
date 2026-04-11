import React from 'react'
import { Table, Tag } from 'antd'
import { useQuery } from '@tanstack/react-query'
import { adminApi } from '@/api'
import { Loading } from '@/components'

const NodeList: React.FC = () => {
  const { data, isLoading } = useQuery({
    queryKey: ['admin', 'cluster'],
    queryFn: () => adminApi.getCluster(),
    refetchInterval: 5000,
  })

  if (isLoading) {
    return <Loading />
  }

  const nodes = data?.data?.nodes || []

  const columns = [
    { title: '节点 ID', dataIndex: 'id', key: 'id' },
    { title: '地址', dataIndex: 'address', key: 'address' },
    {
      title: '状态',
      dataIndex: 'status',
      key: 'status',
      render: (status: string) => (
        <Tag color={status === 'online' ? 'green' : 'red'}>
          {status === 'online' ? '在线' : '离线'}
        </Tag>
      ),
    },
    {
      title: 'CPU',
      dataIndex: ['load', 'cpu'],
      key: 'cpu',
      render: (cpu: number) => `${cpu.toFixed(1)}%`,
    },
    {
      title: '内存',
      dataIndex: ['load', 'memory'],
      key: 'memory',
      render: (memory: number) => `${memory.toFixed(1)}%`,
    },
    {
      title: '时序数',
      dataIndex: ['load', 'series'],
      key: 'series',
    },
  ]

  return <Table columns={columns} dataSource={nodes} rowKey="id" pagination={false} />
}

export default NodeList
