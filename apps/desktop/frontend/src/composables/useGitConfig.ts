import { ref } from 'vue'
import { message } from 'ant-design-vue'

/**
 * Git config state + debounced save, scoped to local/global.
 *
 * Unlike useSettings (single YAML), git config has two layers and the effective
 * value is a merge (local > global), so after each save we re-fetch the whole
 * list to reflect the new effective values.
 */
export interface GitConfigEntry {
  key: string
  effectiveValue: string
  localValue: string
  globalValue: string
  hasLocal: boolean
  hasGlobal: boolean
}

// default write scope chosen in the UI per entry
type App = {
  GitConfigList(): Promise<GitConfigEntry[]>
  GitConfigSet(scope: string, key: string, value: string): Promise<void>
  GitConfigUnset(scope: string, key: string): Promise<void>
}
const getApp = (): App | undefined => (window as any).go?.main?.ByteGitApp

const entries = ref<GitConfigEntry[]>([])
const loading = ref(false)

async function load() {
  const app = getApp()
  if (!app) return
  loading.value = true
  try {
    entries.value = await app.GitConfigList()
  } catch (e: any) {
    message.error(`加载 Git 配置失败: ${e?.message || e}`)
  } finally {
    loading.value = false
  }
}

// queue: key -> { scope, value }; flushed after debounce
let pending: Record<string, { scope: string; value: string }> = {}
let timer: ReturnType<typeof setTimeout> | null = null

async function flush() {
  const app = getApp()
  if (!app || Object.keys(pending).length === 0) return
  const batch = { ...pending }
  pending = {}
  try {
    for (const [key, { scope, value }] of Object.entries(batch)) {
      await app.GitConfigSet(scope, key, value)
    }
    message.success('Git 配置已保存', 1.5)
    // re-fetch: effective values may change due to local/global override
    await load()
  } catch (e: any) {
    message.error(`保存失败: ${e?.message || e}`)
  }
}

/** Queue a debounced save for one key at a scope. */
function queueSave(key: string, scope: string, value: string) {
  pending[key] = { scope, value }
  if (timer) clearTimeout(timer)
  timer = setTimeout(flush, 400)
}

export function useGitConfig() {
  return { entries, loading, load, queueSave }
}
