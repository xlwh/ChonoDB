import React from 'react'
import { Empty } from 'antd'

interface RangeQueryProps {
  query: string
}

const RangeQuery: React.FC<RangeQueryProps> = ({ query }) => {
  if (!query) {
    return <Empty description="请输入查询语句" />
  }

  return <div>范围查询结果将在这里显示</div>
}

export default RangeQuery
