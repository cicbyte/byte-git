import { defineStore } from 'pinia'
import { ref } from 'vue'

/**
 * Global error log — captures window.onerror + unhandledrejection so the user
 * can see JS errors in Settings → About without needing DevTools.
 */
export interface ErrorEntry {
  time: number
  type: 'error' | 'promise'
  message: string
  source?: string
  line?: number
  col?: number
  stack?: string
}

export const useErrorLogStore = defineStore('errorLog', () => {
  const errors = ref<ErrorEntry[]>([])
  const MAX = 50

  function log(entry: Omit<ErrorEntry, 'time'>) {
    errors.value.unshift({ ...entry, time: Date.now() })
    if (errors.value.length > MAX) errors.value = errors.value.slice(0, MAX)
    console.error('[errorLog]', entry.message, entry.stack || '')
  }

  function clear() { errors.value = [] }

  // register global listeners once
  if (typeof window !== 'undefined') {
    window.addEventListener('error', (e) => {
      log({
        type: 'error',
        message: String(e.error?.message || e.message || 'Unknown error'),
        source: e.filename,
        line: e.lineno,
        col: e.colno,
        stack: e.error?.stack,
      })
    })
    window.addEventListener('unhandledrejection', (e) => {
      log({
        type: 'promise',
        message: 'Unhandled rejection: ' + String(e.reason?.message || e.reason || ''),
        stack: e.reason?.stack,
      })
    })
  }

  return { errors, log, clear }
})
