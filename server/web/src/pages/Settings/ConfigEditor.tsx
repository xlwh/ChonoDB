import React from 'react'
import { Descriptions, Button, message } from 'antd'
import { useQuery } from '@tanstack/react-query'
import { adminApi } from '@/api'
import { Loading } from '@/components'

const ConfigEditor: React.FC = () => {
  const { data, isLoading } = useQuery({
    queryKey: ['admin', 'config'],
    queryFn: () => adminApi.getConfig(),
  })

  if (isLoading) {
    return <Loading />
  }

  const config = data?.data

  const handleSave = () => {
    message.success('配置保存成功')
  }

  return (
    <div>
      <Descriptions bordered column={2}>
        <Descriptions.Item label="监听地址">{config?.server?.listenAddress}</Descriptions.Item>
        <Descriptions.Item label="端口">{config?.server?.port}</Descriptions.Item>
        <Descriptions.Item label="数据目录">{config?.storage?.dataDir}</Descriptions.Item>
        <Descriptions.Item label="保留时间">{config?.storage?.retention}</Descriptions.Item>
      </Descriptions>
      <div style={{ marginTop: 16 }}>
        <Button type="primary" onClick={handleSave}>
          保存配置
        </Button>
      </div>
    </div>
  )
}

export default ConfigEditor
