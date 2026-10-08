/**
 * GitHub 状态 + PR 列表缓存（模块级单例）。
 *
 * 认证检测（GhStatus）和 PR 列表（GhPRList）都涉及 GitHub API 调用，
 * 不应每次进入页面都重新执行。首次加载后缓存，
 * 再次进入直接秒显示旧数据，后台静默刷新。
 */
import { ref } from 'vue'

const status = ref({ tokenSet: false, authed: false, user: '', remoteIsGh: false, ownerRepo: '' })
const prs = ref([])
const statusLoaded = ref(false)
const prsLoaded = ref(false)

export function useGitHubCache() {
  const app = window.go?.main?.ByteGitApp

  async function ensureStatusLoaded() {
    if (statusLoaded.value) return status.value
    await refreshStatus()
    return status.value
  }

  async function refreshStatus() {
    if (!app?.GhStatus) return
    try {
      status.value = await app.GhStatus()
    } catch {
      status.value = { tokenSet: false, authed: false, user: '', remoteIsGh: false, ownerRepo: '' }
    }
    statusLoaded.value = true
  }

  async function ensurePRsLoaded() {
    if (prsLoaded.value) return prs.value
    await refreshPRs()
    return prs.value
  }

  async function refreshPRs() {
    if (!app?.GhPRList) { prs.value = []; return }
    try {
      prs.value = (await app.GhPRList()) || []
    } catch {
      prs.value = []
    }
    prsLoaded.value = true
  }

  function invalidateStatus() { statusLoaded.value = false }
  function invalidatePRs() { prsLoaded.value = false }
  function invalidateAll() { statusLoaded.value = false; prsLoaded.value = false }

  return {
    status, prs,
    ensureStatusLoaded, refreshStatus, invalidateStatus,
    ensurePRsLoaded, refreshPRs, invalidatePRs,
    invalidateAll,
  }
}
