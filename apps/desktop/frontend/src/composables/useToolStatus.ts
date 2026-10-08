import { ref } from 'vue'

/**
 * Shared availability status for external CLI tools (gitleaks, ocr).
 *
 * The backend `SecretCheck` / `ReviewCheck` spawn the binary to read its
 * version — relatively expensive, so results are cached module-level and
 * reused by both the global footer (always visible) and the Secret / Review
 * views. `refresh()` re-checks (used after settings change a tool path).
 */
export interface ToolStatus {
  available: boolean
  exePath: string
  version: string
  message: string
}

const gitleaks = ref<ToolStatus>({ available: false, exePath: '', version: '', message: '' })
const ocr = ref<ToolStatus>({ available: false, exePath: '', version: '', message: '' })
const reference = ref<ToolStatus>({ available: false, exePath: '', version: '', message: '' })
let loaded = false

export function useToolStatus() {
  const app = window.go?.main?.ByteGitApp

  async function load() {
    const tasks: Promise<void>[] = []
    if (app?.SecretCheck) {
      tasks.push(
        app.SecretCheck().then((r: ToolStatus) => { gitleaks.value = r })
          .catch(() => { gitleaks.value = { available: false, exePath: '', version: '', message: '检测失败' } }),
      )
    }
    if (app?.ReviewCheck) {
      tasks.push(
        app.ReviewCheck().then((r: ToolStatus) => { ocr.value = r })
          .catch(() => { ocr.value = { available: false, exePath: '', version: '', message: '检测失败' } }),
      )
    }
    if (app?.ReferenceCheck) {
      tasks.push(
        app.ReferenceCheck().then((r: ToolStatus) => { reference.value = r })
          .catch(() => { reference.value = { available: false, exePath: '', version: '', message: '检测失败' } }),
      )
    }
    await Promise.all(tasks)
    loaded = true
  }

  /** Load once (idempotent). Call from MainContent onMounted. */
  async function ensureLoaded() {
    if (loaded) return
    await load()
  }

  /** Force re-check (e.g. after changing a tool path in settings). */
  async function refresh() {
    await load()
  }

  return { gitleaks, ocr, reference, ensureLoaded, refresh, isLoaded: () => loaded }
}
