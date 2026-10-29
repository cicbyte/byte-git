import { ref, watch } from 'vue'
import { useRepoStore } from '@/stores/repo'

/**
 * Generic data fetcher bound to the global repo path.
 * Re-fetches automatically when the repo path changes.
 *
 * @param fetcher receives the repo arg ('' = current dir), returns a promise
 */
export function useRepoData<T>(fetcher: (repo: string) => Promise<T>) {
  const repo = useRepoStore()
  const data = ref<T | null>(null) as { value: T | null }
  const loading = ref(false)
  const error = ref<string | null>(null)

  async function load() {
    loading.value = true
    error.value = null
    try {
      data.value = await fetcher(repo.repoArg())
    } catch (e: any) {
      error.value = String(e?.message || e)
      data.value = null
    } finally {
      loading.value = false
    }
  }

  // re-fetch when repo path changes
  watch(() => repo.path, load, { immediate: true })

  return { data, loading, error, reload: load }
}
