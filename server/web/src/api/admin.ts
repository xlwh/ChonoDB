import client from './client'
import type {
  SystemStats,
  ClusterStatus,
  AlertRules,
  Config,
  SingleWriteRequest,
  SingleWriteResponse,
  BatchWriteRequest,
  BatchWriteResponse,
} from './types'

export const adminApi = {
  getStats: () =>
    client.get<any, { status: string; data: SystemStats }>('/admin/stats'),

  getCluster: () =>
    client.get<any, { status: string; data: ClusterStatus }>('/admin/cluster'),

  getAlertRules: () =>
    client.get<any, { status: string; data: AlertRules }>('/admin/alerts/rules'),

  createAlertRule: (data: { group: string; rule: any }) =>
    client.post<any, { status: string }>('/admin/alerts/rules', data),

  getConfig: () =>
    client.get<any, { status: string; data: Config }>('/admin/config'),

  updateConfig: (data: Partial<Config>) =>
    client.put<any, { status: string }>('/admin/config', data),

  putData: (data: SingleWriteRequest) =>
    client.post<any, SingleWriteResponse>('/admin/data/put', data),

  batchData: (data: BatchWriteRequest) =>
    client.post<any, BatchWriteResponse>('/admin/data/batch', data),
}
