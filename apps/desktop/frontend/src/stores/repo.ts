import { defineStore } from 'pinia'
import { ref } from 'vue'

export interface RecentRepo {
  path: string
  name: string
  lastOpened: string
  /** byte-git 扩展：置顶项稳定显示在项目栏顶部 */
  pinned?: boolean
}

export interface RepoInfo {
  path: string
  name: string
  branch: string
}

/**
 * Global repository path state.
 *
 * `path` is the absolute toplevel of the current git repo (resolved by the
 * backend). Views read `repoArg()` to get the value passed to backend methods.
 *
 * Repo switching is backed by the Go side: `SwitchRepo` validates the path,
 * stores it as the backend's current repo, and records it in the recent list
 * (persisted to repos.json). `repoEpoch` increments on every successful switch
 * so views that load data on mount can watch it to refresh.
 */
export const useRepoStore = defineStore('repo', () => {
  const app = window.go?.main?.ByteGitApp

  const path = ref('')
  const name = ref('')
  const branch = ref('')
  const recent = ref<RecentRepo[]>([])
  const switching = ref(false)
  /** Incremented after each successful switch — views watch it to reload. */
  const repoEpoch = ref(0)

  /** What to actually pass to backend methods ('' for current dir). */
  const repoArg = () => path.value.trim()

  /** Load display info (path/name/branch) from the backend's current repo. */
  async function detectCurrent() {
    if (!app?.CurrentRepoInfo) return
    try {
      const info: RepoInfo = await app.CurrentRepoInfo()
      path.value = info?.path || ''
      name.value = info?.name || ''
      branch.value = info?.branch || ''
    } catch {
      // not running in Wails
    }
  }

  /** Load the recent-repos list from the backend. */
  async function loadRecent() {
    if (!app?.ListRecentRepos) return
    try {
      recent.value = (await app.ListRecentRepos()) || []
    } catch {
      recent.value = []
    }
  }

  /** Switch the current repo to `dir`; refreshes state + recent list. */
  async function switchTo(dir: string) {
    if (!app?.SwitchRepo || !dir) return false
    switching.value = true
    try {
      const res = await app.SwitchRepo(dir)
      path.value = res.path
      name.value = res.name
      branch.value = res.branch
      recent.value = res.recent || []
      repoEpoch.value++
      return true
    } finally {
      switching.value = false
    }
  }

  /** Open a native directory picker; returns the chosen path or ''. */
  async function pickFolder(): Promise<string> {
    if (!app?.PickRepoFolder) return ''
    try {
      return (await app.PickRepoFolder()) || ''
    } catch {
      return ''
    }
  }

  /** Remove a repo from the recent list (persists on backend). */
  async function removeRecent(p: string) {
    if (!app?.RemoveRecentRepo) return
    try {
      await app.RemoveRecentRepo(p)
      recent.value = recent.value.filter((r) => r.path !== p)
    } catch {
      // ignore
    }
  }

  /** Pin/unpin a repo (persists on backend; pinned items float to the top). */
  async function setPinned(p: string, pinned: boolean) {
    if (!app?.SetRecentPinned) return
    try {
      recent.value = (await app.SetRecentPinned(p, pinned)) || recent.value
    } catch {
      // ignore
    }
  }

  /** Persist drag-reordered lists (both as full ordered path arrays). */
  async function setOrder(pinnedPaths: string[], recentPaths: string[]) {
    if (!app?.SetRecentOrder) return
    try {
      recent.value = (await app.SetRecentOrder(recentPaths, pinnedPaths)) || recent.value
    } catch {
      // ignore — order stays local until next successful persist
    }
  }

  return {
    path, name, branch, recent, switching, repoEpoch,
    repoArg, detectCurrent, loadRecent, switchTo, pickFolder, removeRecent, setPinned, setOrder,
  }
})
