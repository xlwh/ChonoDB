import React, { useState, useEffect } from 'react'
import { useNavigate, useLocation } from 'react-router-dom'
import { Layout, Menu } from 'antd'
import {
  DashboardOutlined,
  SearchOutlined,
  EditOutlined,
  BarChartOutlined,
  ClusterOutlined,
  AlertOutlined,
  SettingOutlined,
} from '@ant-design/icons'
import { useAppStore } from '../../stores/useAppStore'
import styles from './Sidebar.module.css'

const { Sider } = Layout

const menuItems = [
  {
    key: '/ui/dashboard',
    icon: <DashboardOutlined />,
    label: '仪表盘',
  },
  {
    key: '/ui/query',
    icon: <SearchOutlined />,
    label: '查询',
  },
  {
    key: '/ui/write',
    icon: <EditOutlined />,
    label: '写入',
  },
  {
    key: '/ui/metrics',
    icon: <BarChartOutlined />,
    label: '统计指标',
  },
  {
    key: '/ui/cluster',
    icon: <ClusterOutlined />,
    label: '集群管理',
  },
  {
    key: '/ui/alerts',
    icon: <AlertOutlined />,
    label: '告警管理',
  },
  {
    key: '/ui/settings',
    icon: <SettingOutlined />,
    label: '配置管理',
  },
]

const Sidebar: React.FC = () => {
  const navigate = useNavigate()
  const location = useLocation()
  const { sidebarCollapsed, toggleSidebar } = useAppStore()
  const [isMobile, setIsMobile] = useState(false)

  useEffect(() => {
    const checkMobile = () => {
      setIsMobile(window.innerWidth < 768)
      if (window.innerWidth < 768 && !sidebarCollapsed) {
        toggleSidebar()
      }
    }

    checkMobile()
    window.addEventListener('resize', checkMobile)
    return () => window.removeEventListener('resize', checkMobile)
  }, [sidebarCollapsed, toggleSidebar])

  const handleMenuClick = ({ key }: { key: string }) => {
    navigate(key)
    if (isMobile && !sidebarCollapsed) {
      toggleSidebar()
    }
  }

  const getSelectedKey = () => {
    const path = location.pathname
    const matchingItem = menuItems.find(item => path.startsWith(item.key))
    return matchingItem ? matchingItem.key : '/ui/dashboard'
  }

  return (
    <Sider
      trigger={null}
      collapsible
      collapsed={sidebarCollapsed}
      breakpoint="md"
      collapsedWidth={isMobile ? 0 : 80}
      onBreakpoint={(broken) => {
        if (broken && !sidebarCollapsed) {
          toggleSidebar()
        }
      }}
      className={`${styles.sider} ${isMobile && sidebarCollapsed ? styles.siderHidden : ''}`}
      width={200}
    >
      <div className={styles.logo}>
        {!sidebarCollapsed && <span className={styles.logoText}>ChronoDB</span>}
      </div>
      <Menu
        mode="inline"
        selectedKeys={[getSelectedKey()]}
        items={menuItems}
        onClick={handleMenuClick}
        className={styles.menu}
      />
    </Sider>
  )
}

export default Sidebar
