import React, { useState, useEffect } from 'react'
import { Select, DatePicker, Space } from 'antd'
import dayjs, { Dayjs } from 'dayjs'
import styles from './index.module.css'

const { RangePicker } = DatePicker

interface TimeRangePickerProps {
  value: [number, number]
  onChange: (range: [number, number]) => void
  step?: number
  onStepChange?: (step: number) => void
}

const RELATIVE_TIME_OPTIONS = [
  { label: '最近 5 分钟', value: '5m' },
  { label: '最近 15 分钟', value: '15m' },
  { label: '最近 30 分钟', value: '30m' },
  { label: '最近 1 小时', value: '1h' },
  { label: '最近 3 小时', value: '3h' },
  { label: '最近 6 小时', value: '6h' },
  { label: '最近 12 小时', value: '12h' },
  { label: '最近 1 天', value: '1d' },
  { label: '最近 3 天', value: '3d' },
  { label: '最近 7 天', value: '7d' },
  { label: '最近 30 天', value: '30d' },
]

const STEP_OPTIONS = [
  { label: '10s', value: 10 },
  { label: '30s', value: 30 },
  { label: '1m', value: 60 },
  { label: '5m', value: 300 },
  { label: '15m', value: 900 },
  { label: '30m', value: 1800 },
  { label: '1h', value: 3600 },
  { label: '6h', value: 21600 },
  { label: '1d', value: 86400 },
]

const parseRelativeTime = (value: string): [number, number] => {
  const now = dayjs()
  let start: Dayjs

  switch (value) {
    case '5m':
      start = now.subtract(5, 'minute')
      break
    case '15m':
      start = now.subtract(15, 'minute')
      break
    case '30m':
      start = now.subtract(30, 'minute')
      break
    case '1h':
      start = now.subtract(1, 'hour')
      break
    case '3h':
      start = now.subtract(3, 'hour')
      break
    case '6h':
      start = now.subtract(6, 'hour')
      break
    case '12h':
      start = now.subtract(12, 'hour')
      break
    case '1d':
      start = now.subtract(1, 'day')
      break
    case '3d':
      start = now.subtract(3, 'day')
      break
    case '7d':
      start = now.subtract(7, 'day')
      break
    case '30d':
      start = now.subtract(30, 'day')
      break
    default:
      start = now.subtract(1, 'hour')
  }

  return [start.unix(), now.unix()]
}

const TimeRangePicker: React.FC<TimeRangePickerProps> = ({
  value,
  onChange,
  step = 60,
  onStepChange,
}) => {
  const [mode, setMode] = useState<'relative' | 'absolute'>('relative')
  const [relativeValue, setRelativeValue] = useState<string>('1h')

  useEffect(() => {
    if (mode === 'relative') {
      const range = parseRelativeTime(relativeValue)
      onChange(range)
    }
  }, [mode, relativeValue, onChange])

  const handleRelativeChange = (val: string) => {
    setRelativeValue(val)
  }

  const handleAbsoluteChange = (dates: [Dayjs | null, Dayjs | null] | null) => {
    if (dates && dates[0] && dates[1]) {
      onChange([dates[0].unix(), dates[1].unix()])
    }
  }

  const handleStepChange = (val: number | null) => {
    if (val && onStepChange) {
      onStepChange(val)
    }
  }

  const suggestStep = (start: number, end: number): number => {
    const duration = end - start
    if (duration <= 300) return 10
    if (duration <= 900) return 30
    if (duration <= 3600) return 60
    if (duration <= 10800) return 300
    if (duration <= 21600) return 900
    if (duration <= 86400) return 1800
    return 3600
  }

  const currentStep = step || suggestStep(value[0], value[1])

  return (
    <div className={styles.timeRangePicker}>
      <Space size="middle" wrap>
        <Select
          value={mode}
          onChange={setMode}
          style={{ width: 100 }}
          options={[
            { label: '相对时间', value: 'relative' },
            { label: '绝对时间', value: 'absolute' },
          ]}
        />

        {mode === 'relative' ? (
          <Select
            value={relativeValue}
            onChange={handleRelativeChange}
            style={{ width: 150 }}
            options={RELATIVE_TIME_OPTIONS}
          />
        ) : (
          <RangePicker
            showTime
            value={[dayjs.unix(value[0]), dayjs.unix(value[1])]}
            onChange={(dates) => handleAbsoluteChange(dates as [Dayjs | null, Dayjs | null] | null)}
            style={{ width: 400 }}
            format="YYYY-MM-DD HH:mm:ss"
          />
        )}

        <div className={styles.stepSelector}>
          <span className={styles.stepLabel}>步长:</span>
          <Select
            value={currentStep}
            onChange={handleStepChange}
            style={{ width: 100 }}
            options={STEP_OPTIONS}
          />
        </div>
      </Space>
    </div>
  )
}

export default TimeRangePicker
