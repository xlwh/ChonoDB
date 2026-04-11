import client from './client'
import type { WriteRequest, WriteResponse } from './types'

export const writeApi = {
  write: (data: WriteRequest) =>
    client.post<any, WriteResponse>('/write', data),
}
