import { ref } from 'vue'
import { EventsOn, EventsOff } from '../../wailsjs/runtime/runtime'

/**
 * Stream AI-generated commit tokens into a reactive buffer.
 *
 * The backend CommitGenerate emits "commit:token" per token via
 * runtime.EventsEmit; this composable subscribes, accumulates the text,
 * and exposes a generating flag for the UI.
 */
export function useCommitStream() {
  const message = ref('')
  const generating = ref(false)
  const error = ref<string | null>(null)
  const TOKEN_EVENT = 'commit:token'

  function subscribe() {
    EventsOn(TOKEN_EVENT, (token: string) => {
      message.value += token
    })
  }

  function unsubscribe() {
    EventsOff(TOKEN_EVENT)
  }

  /**
   * Run a stream generation. The fetcher should call CommitGenerate on the
   * backend (which both emits tokens AND resolves with the full message).
   */
  async function run(fetcher: () => Promise<string>) {
    message.value = ''
    error.value = null
    generating.value = true
    subscribe()
    try {
      const full = await fetcher()
      // backend already cleaned the message; prefer it as the source of truth
      if (full) message.value = full
    } catch (e: any) {
      error.value = String(e?.message || e)
    } finally {
      generating.value = false
      unsubscribe()
    }
  }

  function reset() {
    message.value = ''
    error.value = null
  }

  function setMessage(v: string) {
    message.value = v
  }

  return { message, generating, error, run, reset, setMessage }
}
