import axios, { AxiosInstance, AxiosError } from 'axios'

const client: AxiosInstance = axios.create({
  baseURL: '/api/v1',
  timeout: 30000,
  headers: {
    'Content-Type': 'application/json',
  },
})

client.interceptors.response.use(
  (response) => response.data,
  (error: AxiosError<{ error?: string }>) => {
    const message = error.response?.data?.error || error.message
    return Promise.reject(new Error(message))
  }
)

export default client
