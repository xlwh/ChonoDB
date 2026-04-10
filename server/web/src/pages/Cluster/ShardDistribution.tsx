import React from 'react'
import { Table } from 'antd'
import { useQuery } from '@tanstack/react-query'
import { adminApi } from '@/api'
import { Loading } from '@/components'
import { formatBytes } from '@/utils/format'

const ShardDistribution: React.FC = () => {
  const { data, isLoading } = useQuery({
    queryKey: ['admin', 'cluster'],
    queryFn: () => adminApi.getCluster(),
    refetchInterval: 5000,
  })

  if (isLoading) {
    return <Loading />
  }

  const shards = data?.data?.shards || []

  const columns = [
    { title: '分片 ID', dataIndex: 'id', key: 'id' },
    { title: '节点 ID', dataIndex: 'nodeId', key: 'nodeId' },
    { title: '时序数', dataIndex: 'seriesCount', key: 'seriesCount' },
    {
      title: '大小',
      dataIndex: 'size',
      key: 'size',
      render: (size: number) => formatBytes(size),
    },
  ]

  return <Table columns={columns} dataSource={shards} rowKey="id" pagination={false} />
}

export default ShardDistribution
