import { Routes, Route, Navigate } from 'react-router-dom'
import { Layout } from 'antd'
import AppLayout from './components/Layout/AppLayout'
import Dashboard from './pages/Dashboard'
import Query from './pages/Query'
import Write from './pages/Write'
import Metrics from './pages/Metrics'
import Cluster from './pages/Cluster'
import Alerts from './pages/Alerts'
import Settings from './pages/Settings'

function App() {
  return (
    <Layout style={{ minHeight: '100vh' }}>
      <Routes>
        <Route path="/" element={<AppLayout />}>
          <Route index element={<Navigate to="/ui/dashboard" replace />} />
          <Route path="ui/dashboard" element={<Dashboard />} />
          <Route path="ui/query" element={<Query />} />
          <Route path="ui/write" element={<Write />} />
          <Route path="ui/metrics" element={<Metrics />} />
          <Route path="ui/cluster" element={<Cluster />} />
          <Route path="ui/alerts" element={<Alerts />} />
          <Route path="ui/settings" element={<Settings />} />
          <Route path="*" element={<Navigate to="/ui/dashboard" replace />} />
        </Route>
      </Routes>
    </Layout>
  )
}

export default App
