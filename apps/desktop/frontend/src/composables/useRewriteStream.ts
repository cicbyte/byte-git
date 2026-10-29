import { ref } from 'vue'
import { EventsOn, EventsOff } from '@bridge/runtime/runtime'

/**
 * Streaming support for AI rewrite.
 *
 * Backend emits two events:
 *   "rewrite:token"    { hash, token }  — one token of the streaming suggestion
 *   "rewrite:progress" { hash, ok, done, total } — a commit finished in batch mode
 */
export function useRewriteStream() {
  const TOKEN_EVENT = 'rewrite:token'
  const PROGRESS_EVENT = 'rewrite:progress'

  /** token accumulator per hash (for live typing display) */
  const streaming = ref<Record<string, string>>({})
  /** batch progress */
  const batchDone = ref(0)
  const batchTotal = ref(0)
  const generating = ref(false)

  function subscribeToken(onToken: (hash: string, token: string) => void) {
    EventsOn(TOKEN_EVENT, (data: { hash: string; token: string }) => {
      streaming.value[data.hash] = (streaming.value[data.hash] || '') + data.token
      onToken(data.hash, data.token)
    })
  }

  function subscribeProgress(onProgress?: (p: { hash: string; ok: boolean; done: number; total: number }) => void) {
    EventsOn(PROGRESS_EVENT, (p: { hash: string; ok: boolean; done: number; total: number }) => {
      batchDone.value = p.done
      batchTotal.value = p.total
      onProgress?.(p)
    })
  }

  /**
   * Run a single-commit generation. `onToken` fires per token;
   * resolves with the final full message from the backend.
   */
  async function runSingle(
    hash: string,
    fetcher: () => Promise<string>
  ): Promise<string> {
    generating.value = true
    streaming.value[hash] = ''
    subscribeToken(() => {})
    try {
      const full = await fetcher()
      return full
    } finally {
      generating.value = false
      EventsOff(TOKEN_EVENT)
    }
  }

  /**
   * Run batch generation. Subscribes to token + progress events,
   * resolves with the per-commit results array.
   */
  async function runBatch<T>(
    fetcher: () => Promise<T[]>,
    onProgress?: (p: { hash: string; ok: boolean; done: number; total: number }) => void
  ): Promise<T[]> {
    generating.value = true
    batchDone.value = 0
    batchTotal.value = 0
    subscribeToken(() => {})
    subscribeProgress(onProgress)
    try {
      return await fetcher()
    } finally {
      generating.value = false
      EventsOff(TOKEN_EVENT)
      EventsOff(PROGRESS_EVENT)
    }
  }

  function reset() {
    streaming.value = {}
    batchDone.value = 0
    batchTotal.value = 0
  }

  return { streaming, batchDone, batchTotal, generating, runSingle, runBatch, reset }
}
