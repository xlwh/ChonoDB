import React from 'react'
import { Outlet } from 'react-router-dom'
import { Layout } from 'antd'
import Header from './Header'
import Sidebar from './Sidebar'
import { useAppStore } from '../../stores/useAppStore'
import styles from './AppLayout.module.css'

const { Content } = Layout

const AppLayout: React.FC = () => {
  const { sidebarCollapsed } = useAppStore()

  return (
    <Layout className={styles.layout}>
      <Sidebar />
      <Layout className={sidebarCollapsed ? styles.collapsedLayout : ''}>
        <Header />
        <Content className={styles.content}>
          <Outlet />
        </Content>
      </Layout>
    </Layout>
  )
}

export default AppLayout
