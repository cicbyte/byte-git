import { ref, reactive } from 'vue'
import { message } from 'ant-design-vue'

/**
 * Load/save OpenCodeReview's native config.json (~/.opencodereview/config.json).
 *
 * This is intentionally separate from useSettings: the LLM endpoint config
 * (provider / api key / model) lives in OCR's own file, not byte-git's
 * config.yaml. Writing here stays fully interoperable with `ocr config` CLI.
 *
 * API keys are masked by the backend (****<last4>); an empty or masked value
 * sent back is ignored by the backend so the real secret is never wiped.
 */
export interface OcrProviderEntry {
  api_key?: string
  url?: string
  protocol?: string
  model?: string
  models?: string[]
  auth_header?: string
}
export interface OcrConfig {
  provider?: string
  model?: string
  providers?: Record<string, OcrProviderEntry>
  custom_providers?: Record<string, OcrProviderEntry>
  llm?: { url?: string; auth_token?: string; model?: string }
  language?: string
}
export interface OcrProviderPreset {
  name: string
  displayName: string
  protocol: string
  baseUrl: string
  authHeader?: string
  models: string[]
}
export interface OcrConfigResult {
  config: OcrConfig
  path: string
  exists: boolean
  presets: OcrProviderPreset[]
}

type App = {
  OcrConfigGet(): Promise<OcrConfigResult>
  OcrConfigSave(patch: Record<string, string>): Promise<void>
  OcrConfigGetRaw(): Promise<{ path: string; exists: boolean; content: string }>
  OcrConfigSaveRaw(content: string): Promise<string>
}

const getApp = (): App | undefined => (window as any).go?.main?.ByteGitApp

// shared reactive state
const cfg = reactive<OcrConfig>({})
const path = ref('')
const exists = ref(false)
const presets = ref<OcrProviderPreset[]>([])
const loading = ref(false)
const saving = ref(false)

// raw JSON editor state (advanced mode) — holds the full unmasked file text
const rawContent = ref('')
const rawDirty = ref(false)
const rawLoading = ref(false)

// debounced patch accumulator
let pending: Record<string, string> = {}
let timer: ReturnType<typeof setTimeout> | null = null

async function flush() {
  const app = getApp()
  if (!app || Object.keys(pending).length === 0) return
  const patch = { ...pending }
  pending = {}
  saving.value = true
  try {
    await app.OcrConfigSave(patch)
    message.success('已保存', 1.5)
  } catch (e: any) {
    message.error(`保存 OCR 配置失败: ${e?.message || e}`)
  } finally {
    saving.value = false
  }
}

/** Queue a debounced save for one dot-path key (e.g. "providers.deepseek.api_key"). */
function save(key: string, value: string) {
  pending[key] = value
  if (timer) clearTimeout(timer)
  timer = setTimeout(flush, 300)
}

async function load() {
  const app = getApp()
  if (!app) return
  loading.value = true
  try {
    const r = await app.OcrConfigGet()
    Object.keys(cfg).forEach((k) => delete (cfg as any)[k])
    Object.assign(cfg, r.config)
    path.value = r.path
    exists.value = r.exists
    presets.value = r.presets || []
  } catch (e: any) {
    message.error(`读取 OCR 配置失败: ${e?.message || e}`)
  } finally {
    loading.value = false
  }
}

// ---- raw JSON editor ----
async function loadRaw() {
  const app = getApp()
  if (!app) return
  rawLoading.value = true
  try {
    const r = await app.OcrConfigGetRaw()
    path.value = r.path
    exists.value = r.exists
    // empty file → seed a minimal template so the editor isn't blank
    rawContent.value = r.content || '{\n  \n}\n'
    rawDirty.value = false
  } catch (e: any) {
    message.error(`读取 OCR 配置失败: ${e?.message || e}`)
  } finally {
    rawLoading.value = false
  }
}

async function saveRaw() {
  const app = getApp()
  if (!app) return
  saving.value = true
  try {
    await app.OcrConfigSaveRaw(rawContent.value)
    rawDirty.value = false
    message.success('JSON 已保存', 1.5)
  } catch (e: any) {
    message.error(`保存失败: ${e?.message || e}`)
  } finally {
    saving.value = false
  }
}

export function useOcrConfig() {
  return {
    // structured form
    cfg, path, exists, presets, loading, saving, load, save, flush,
    // raw JSON editor
    rawContent, rawDirty, rawLoading, loadRaw, saveRaw,
  }
}
