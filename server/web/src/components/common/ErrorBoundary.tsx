import { Component, ErrorInfo, ReactNode } from 'react'
import { Result, Button } from 'antd'
import styles from './ErrorBoundary.module.css'

interface Props {
  children: ReactNode
}

interface State {
  hasError: boolean
  error?: Error
}

class ErrorBoundary extends Component<Props, State> {
  public state: State = {
    hasError: false,
  }

  public static getDerivedStateFromError(error: Error): State {
    return { hasError: true, error }
  }

  public componentDidCatch(error: Error, errorInfo: ErrorInfo) {
    console.error('Error caught by boundary:', error, errorInfo)
  }

  private handleReset = () => {
    this.setState({ hasError: false, error: undefined })
    window.location.href = '/'
  }

  public render() {
    if (this.state.hasError) {
      return (
        <div className={styles.container}>
          <Result
            status="error"
            title="页面出错了"
            subTitle={this.state.error?.message || '请刷新页面重试'}
            extra={
              <Button type="primary" onClick={this.handleReset}>
                返回首页
              </Button>
            }
          />
        </div>
      )
    }

    return this.props.children
  }
}

export default ErrorBoundary
