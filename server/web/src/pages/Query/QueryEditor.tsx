import React, { useRef, useEffect, useCallback } from 'react'
import styles from './index.module.css'

interface QueryEditorProps {
  value: string
  onChange: (value: string) => void
  onSubmit: () => void
  placeholder?: string
}

const PROMQL_TOKENS = [
  { pattern: /(sum|avg|min|max|count|rate|irate|increase|histogram_quantile|topk|bottomk|group|stddev|stdvar|abs|ceil|floor|round|clamp|clamp_max|clamp_min|changes|deriv|predict_linear|resets|day_of_month|day_of_week|days_in_month|hour|minute|month|year|vector|scalar|time)\b/g, className: styles.tokenFunction },
  { pattern: /(by|without|on|ignoring|group_left|group_right|offset|bool)\b/g, className: styles.tokenKeyword },
  { pattern: /([a-zA-Z_:][a-zA-Z0-9_:]*)/g, className: styles.tokenMetric },
  { pattern: /"([^"\\]|\\.)*"/g, className: styles.tokenString },
  { pattern: /'([^'\\]|\\.)*'/g, className: styles.tokenString },
  { pattern: /(\d+\.?\d*)([smhdwy]?)/g, className: styles.tokenNumber },
  { pattern: /([+\-*/^%])/g, className: styles.tokenOperator },
  { pattern: /(==|!=|>=|<=|>|<|=~|!~)/g, className: styles.tokenOperator },
  { pattern: /([\{\}\[\]\(\)])/g, className: styles.tokenPunctuation },
  { pattern: /(,)/g, className: styles.tokenPunctuation },
  { pattern: /(#.*$)/gm, className: styles.tokenComment },
]

const QueryEditor: React.FC<QueryEditorProps> = ({
  value,
  onChange,
  onSubmit,
  placeholder = '输入 PromQL 查询语句，例如：http_requests_total{method="GET"}',
}) => {
  const textareaRef = useRef<HTMLTextAreaElement>(null)
  const highlightRef = useRef<HTMLDivElement>(null)

  const highlightCode = useCallback((code: string): string => {
    let highlighted = code
      .replace(/&/g, '&amp;')
      .replace(/</g, '&lt;')
      .replace(/>/g, '&gt;')

    PROMQL_TOKENS.forEach(({ pattern, className }) => {
      highlighted = highlighted.replace(pattern, (match) => {
        return `<span class="${className.slice(1)}">${match}</span>`
      })
    })

    return highlighted
  }, [])

  useEffect(() => {
    if (highlightRef.current) {
      highlightRef.current.innerHTML = highlightCode(value) + '\n'
    }
  }, [value, highlightCode])

  const handleScroll = () => {
    if (textareaRef.current && highlightRef.current) {
      highlightRef.current.scrollTop = textareaRef.current.scrollTop
      highlightRef.current.scrollLeft = textareaRef.current.scrollLeft
    }
  }

  const handleKeyDown = (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.key === 'Enter' && (e.ctrlKey || e.metaKey)) {
      e.preventDefault()
      onSubmit()
    }
  }

  const handleChange = (e: React.ChangeEvent<HTMLTextAreaElement>) => {
    onChange(e.target.value)
  }

  return (
    <div className={styles.editorContainer}>
      <div className={styles.editorWrapper}>
        <div
          ref={highlightRef}
          className={styles.highlightLayer}
          aria-hidden="true"
        >
          {value}
        </div>
        <textarea
          ref={textareaRef}
          value={value}
          onChange={handleChange}
          onScroll={handleScroll}
          onKeyDown={handleKeyDown}
          placeholder={placeholder}
          className={styles.textareaLayer}
          spellCheck={false}
          autoComplete="off"
          autoCorrect="off"
          autoCapitalize="off"
        />
      </div>
      <div className={styles.editorHint}>
        按 Ctrl/Cmd + Enter 执行查询
      </div>
    </div>
  )
}

export default QueryEditor
