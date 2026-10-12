import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import { useRepoStore } from './repo'
import { useCommitStream } from '../composables/useCommitStream'
import { message as AMessage } from 'ant-design-vue'
import { EventsOn, EventsOff } from '../../bridge/runtime/runtime'

/**
 * Centralized Source Control Management state.
 *
 * Backs the global SCM drawer (Navbar button → a-drawer): file change list,
 * stage/unstage/discard actions, commit message + AI generation. Kept separate
 * from LiveView's own state so the two coexist without coupling; both call the
 * same backend methods.
 *
 * Refresh strategy:
 *  - explicit `refresh()` after each mutating action
 *  - the drawer calls `refresh()` on open
 *  - the store itself owns a global filesystem watcher (`LiveStartWatch`) and
 *    subscribes to the "live:changed" event so the Navbar badge auto-updates
 *    whenever the working tree changes — independent of whether LiveView is
 *    currently mounted. LiveView's own watcher subscription is harmless here
 *    because LiveStartWatch is idempotent (same path → no-op) and the event
 *    broadcast reaches all subscribers.
 */

export interface ScmFileEntry {
  path: string
  status: string // modified/added/deleted/renamed/untracked
}

export interface ScmStatus {
  staged: ScmFileEntry[]
  unstaged: ScmFileEntry[]
  untracked: ScmFileEntry[]
}

// conventional commit types (mirrors LiveView/LogView)
export const COMMIT_TYPES = [
  { type: 'feat',     label: 'feat',     desc: '新功能',    color: '#50fa7b' },
  { type: 'fix',      label: 'fix',      desc: '修复 bug', color: '#ffb86c' },
  { type: 'docs',     label: 'docs',     desc: '文档',     color: '#8be9fd' },
  { type: 'style',    label: 'style',    desc: '格式',     color: '#bd93f9' },
  { type: 'refactor', label: 'refactor', desc: '重构',     color: '#bd93f9' },
  { type: 'perf',     label: 'perf',     desc: '性能',     color: '#ff79c6' },
  { type: 'test',     label: 'test',     desc: '测试',     color: '#ffb86c' },
  { type: 'build',    label: 'build',    desc: '构建',     color: '#6272a4' },
  { type: 'ci',       label: 'ci',       desc: 'CI',      color: '#6272a4' },
  { type: 'chore',    label: 'chore',    desc: '杂务',     color: '#6272a4' },
]

// status → display color class
export const STATUS_COLOR: Record<string, string> = {
  modified: 'M',
  added: 'A',
  deleted: 'D',
  renamed: 'R',
  copied: 'C',
  untracked: 'U',
}

const STORAGE_KEY = 'byte-git:commit-templates'

// safely extract a message from an unknown catch value (TS: catch e is unknown)
function errMsg(e: unknown): string {
  if (e && typeof e === 'object' && 'message' in e) return String((e as any).message || e)
  return String(e)
}

export const useScmStore = defineStore('scm', () => {
  const repo = useRepoStore()
  const app = window.go?.main?.ByteGitApp

  // ---- status ----
  const status = ref<ScmStatus>({ staged: [], unstaged: [], untracked: [] })
  const loading = ref(false)

  const stagedCount = computed(() => status.value.staged.length)
  const unstagedCount = computed(() => status.value.unstaged.length)
  const untrackedCount = computed(() => status.value.untracked.length)
  /** total uncommitted changes — drives the Navbar badge */
  const totalChanges = computed(() => stagedCount.value + unstagedCount.value + untrackedCount.value)

  // ---- global filesystem watcher → auto-refresh badge ----
  // Backend "live:changed" is debounced + diff-gated (only fires when the git
  // status signature actually changes), so subscribing here is cheap. The
  // handler is debounced again with a tiny guard so a rapid burst during
  // operations like `git reset` collapses into one refresh.
  const LIVE_EVENT = 'live:changed'
  let liveTimer: ReturnType<typeof setTimeout> | null = null
  let liveBound = false
  function onLiveChanged() {
    if (liveTimer) clearTimeout(liveTimer)
    liveTimer = setTimeout(() => { refresh() }, 200)
  }
  /** Start the global watcher so the Navbar badge tracks the working tree
   *  even when LiveView is not mounted. Idempotent + repo-aware. */
  function startGlobalWatch() {
    if (!app?.LiveStartWatch) return
    if (!liveBound) {
      EventsOn(LIVE_EVENT, onLiveChanged)
      liveBound = true
    }
    // LiveStartWatch is idempotent (same path → no-op); safe to call on every
    // repo switch and on repeated invocations.
    app.LiveStartWatch?.().catch(() => { /* silent */ })
  }

  // ---- commit message templates (persisted to localStorage) ----
  const customTemplates = ref<{ name: string; body: string }[]>([])
  try {
    const saved = localStorage.getItem(STORAGE_KEY)
    if (saved) customTemplates.value = JSON.parse(saved)
  } catch {}

  function persistTemplates() {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(customTemplates.value))
  }

  // ---- commit message + AI streaming ----
  const tplScope = ref('')
  const noVerify = ref(false)
  const committing = ref(false)
  const { message: commitMsg, generating, error: genError, run: runGenerate, reset } = useCommitStream()

  function applyType(t: string) {
    const scope = tplScope.value.trim()
    const prefix = scope ? `${t}(${scope}): ` : `${t}: `
    const cur = commitMsg.value
    const stripped = cur.replace(/^(feat|fix|docs|style|refactor|perf|test|build|ci|chore)(\([^)]+\))?:\s*/i, '')
    commitMsg.value = prefix + stripped
  }

  // ---- data loading ----
  async function refresh() {
    if (!app?.CommitStatus) return
    loading.value = true
    try {
      const r = await app.CommitStatus(repo.repoArg())
      status.value = {
        staged: r.staged || [],
        unstaged: r.unstaged || [],
        untracked: r.untracked || [],
      }
    } catch (e) {
      // silent — drawer may be opened with no repo
      status.value = { staged: [], unstaged: [], untracked: [] }
    } finally {
      loading.value = false
    }
    // also refresh the ahead count so the button mode stays accurate
    loadAhead()
  }

  // ---- stage / unstage ----
  async function stage(paths: string[]) {
    if (!paths.length) return
    try {
      await app.CommitStage(repo.repoArg(), paths)
      await refresh()
    } catch (e) {
      AMessage.error('暂存失败: ' + errMsg(e))
    }
  }

  async function unstage(paths: string[]) {
    if (!paths.length) return
    try {
      await app.CommitUnstage(repo.repoArg(), paths)
      await refresh()
    } catch (e) {
      AMessage.error('取消暂存失败: ' + errMsg(e))
    }
  }

  async function stageAll() {
    try {
      await app.CommitStageAll(repo.repoArg())
      await refresh()
    } catch (e) {
      AMessage.error('暂存全部失败: ' + errMsg(e))
    }
  }

  async function unstageAll() {
    try {
      await app.CommitUnstageAll(repo.repoArg())
      await refresh()
    } catch (e) {
      AMessage.error('取消暂存全部失败: ' + errMsg(e))
    }
  }

  // ---- discard (irreversible; caller confirms in UI) ----
  async function discardTracked(paths: string[]) {
    if (!paths.length) return
    try {
      await app.CommitDiscardTracked(repo.repoArg(), paths)
      AMessage.success(`已放弃 ${paths.length} 个文件的改动`)
      await refresh()
    } catch (e) {
      AMessage.error('放弃更改失败: ' + errMsg(e))
    }
  }

  async function discardUntracked(paths: string[]) {
    if (!paths.length) return
    try {
      await app.CommitDiscardUntracked(repo.repoArg(), paths)
      AMessage.success(`已删除 ${paths.length} 个未跟踪文件`)
      await refresh()
    } catch (e) {
      AMessage.error('删除未跟踪文件失败: ' + errMsg(e))
    }
  }

  // ---- AI generate ----
  async function aiGenerate() {
    if (!stagedCount.value) {
      AMessage.warning('暂存区为空，请先暂存文件')
      return
    }
    await runGenerate(() => app.CommitGenerate(repo.repoArg()))
    if (genError.value) AMessage.error('生成失败: ' + genError.value)
  }

  // ---- commit ----
  async function commit() {
    const msg = commitMsg.value.trim()
    if (!msg) {
      AMessage.warning('请填写 commit message')
      return
    }
    committing.value = true
    try {
      await app.CommitDo(repo.repoArg(), msg, noVerify.value)
      AMessage.success('提交成功')
      reset()
      await refresh()
      loadAhead() // new commit → ahead likely incremented
    } catch (e) {
      AMessage.error('提交失败: ' + errMsg(e))
    } finally {
      committing.value = false
    }
  }

  // ---- push ----
  // ahead = unpushed commit count; the drawer button switches between "commit"
  // (has staged changes) and "push" (has unpushed commits) based on this + stagedCount.
  const ahead = ref(0)
  const pushing = ref(false)
  // push-readiness: hasRemote / hasUpstream / remote name / branch name
  const pushStatus = ref({ hasRemote: false, hasUpstream: false, remote: '', branch: '', ahead: 0 })
  async function loadAhead() {
    if (!app?.PushStatus) return
    try {
      const r = await app.PushStatus()
      pushStatus.value = r
      ahead.value = r.ahead
    } catch { ahead.value = 0 }
  }
  async function push(force = false) {
    pushing.value = true
    try {
      await app.CommitPush(force)
      AMessage.success(force ? '已强制推送' : '已推送')
      ahead.value = 0
      await loadAhead() // refresh (upstream may now be set after first push)
    } catch (e) {
      AMessage.error('推送失败: ' + errMsg(e))
    } finally {
      pushing.value = false
    }
  }

  return {
    // state
    status, loading, commitMsg, generating, committing, noVerify, tplScope,
    customTemplates,
    // push state
    ahead, pushing, pushStatus,
    // getters
    stagedCount, unstagedCount, untrackedCount, totalChanges,
    // templates
    applyType, persistTemplates,
    // actions
    refresh, stage, unstage, stageAll, unstageAll,
    discardTracked, discardUntracked, aiGenerate, commit, push, loadAhead,
    reset, startGlobalWatch,
  }
})
