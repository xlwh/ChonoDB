import React from 'react'
import { Empty } from 'antd'

interface InstantQueryProps {
  query: string
}

const InstantQuery: React.FC<InstantQueryProps> = ({ query }) => {
  if (!query) {
    return <Empty description="请输入查询语句" />
  }

  return <div>即时查询结果将在这里显示</div>
}

export default InstantQuery
