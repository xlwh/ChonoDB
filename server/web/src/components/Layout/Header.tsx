import React, { useState, useEffect } from 'react'
import { Layout, Typography, Badge, Dropdown, Avatar, Button, Tooltip } from 'antd'
import { 
  MenuFoldOutlined, 
  MenuUnfoldOutlined, 
  UserOutlined, 
  LogoutOutlined,
  SettingOutlined,
  BellOutlined,
  CheckCircleOutlined,
  ExclamationCircleOutlined,
} from '@ant-design/icons'
import type { MenuProps } from 'antd'
import { useAppStore } from '../../stores/useAppStore'
import { useUserStore } from '../../stores/useUserStore'
import styles from './Header.module.css'

const { Header: AntHeader } = Layout
const { Title, Text } = Typography

interface SystemStatus {
  status: 'healthy' | 'warning' | 'error'
  message: string
}

const Header: React.FC = () => {
  const { sidebarCollapsed, toggleSidebar } = useAppStore()
  const { username, logout } = useUserStore()
  const [systemStatus, setSystemStatus] = useState<SystemStatus>({
    status: 'healthy',
    message: '系统运行正常',
  })

  useEffect(() => {
    const checkSystemStatus = async () => {
      try {
        const response = await fetch('/api/v1/health')
        if (response.ok) {
          setSystemStatus({ status: 'healthy', message: '系统运行正常' })
        } else {
          setSystemStatus({ status: 'warning', message: '系统状态异常' })
        }
      } catch {
        setSystemStatus({ status: 'error', message: '无法连接到服务器' })
      }
    }

    checkSystemStatus()
    const interval = setInterval(checkSystemStatus, 30000)
    return () => clearInterval(interval)
  }, [])

  const userMenuItems: MenuProps['items'] = [
    {
      key: 'profile',
      icon: <UserOutlined />,
      label: '个人资料',
    },
    {
      key: 'settings',
      icon: <SettingOutlined />,
      label: '设置',
    },
    {
      type: 'divider',
    },
    {
      key: 'logout',
      icon: <LogoutOutlined />,
      label: '退出登录',
      onClick: logout,
    },
  ]

  const getStatusIcon = () => {
    switch (systemStatus.status) {
      case 'healthy':
        return <CheckCircleOutlined style={{ color: '#52c41a' }} />
      case 'warning':
        return <ExclamationCircleOutlined style={{ color: '#faad14' }} />
      case 'error':
        return <ExclamationCircleOutlined style={{ color: '#ff4d4f' }} />
    }
  }

  return (
    <AntHeader className={styles.header}>
      <div className={styles.left}>
        <Button
          type="text"
          icon={sidebarCollapsed ? <MenuUnfoldOutlined /> : <MenuFoldOutlined />}
          onClick={toggleSidebar}
          className={styles.trigger}
        />
        <Title level={4} className={styles.title}>
          ChronoDB
        </Title>
      </div>

      <div className={styles.right}>
        <Tooltip title={systemStatus.message}>
          <div className={styles.statusIndicator}>
            <Badge 
              status={systemStatus.status === 'healthy' ? 'success' : systemStatus.status === 'warning' ? 'warning' : 'error'} 
            />
            <Text className={styles.statusText}>
              {systemStatus.message}
            </Text>
            {getStatusIcon()}
          </div>
        </Tooltip>

        <Tooltip title="通知">
          <Badge count={0} showZero={false}>
            <Button 
              type="text" 
              icon={<BellOutlined />} 
              className={styles.iconButton}
            />
          </Badge>
        </Tooltip>

        <Dropdown menu={{ items: userMenuItems }} placement="bottomRight">
          <div className={styles.user}>
            <Avatar 
              size="small" 
              icon={<UserOutlined />} 
              className={styles.avatar}
            />
            <Text className={styles.username}>
              {username || '管理员'}
            </Text>
          </div>
        </Dropdown>
      </div>
    </AntHeader>
  )
}

export default Header
