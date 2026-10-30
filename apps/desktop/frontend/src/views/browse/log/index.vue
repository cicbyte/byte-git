<script setup>
import { ref, computed, onMounted, watch, onBeforeUnmount } from 'vue'
import { useRouter } from 'vue-router'
import { useRepoStore } from '@/stores/repo'
import { message as AMessage } from 'ant-design-vue'
import { FileOutlined, HistoryOutlined, ReloadOutlined } from '@ant-design/icons-vue'
import CommitTree from '@/components/common/CommitTree.vue'
import CommitHoverCard from '@/components/common/CommitHoverCard.vue'
import FileTreeNode from '@/components/common/FileTreeNode.vue'
import DiffViewer from '@/components/common/DiffViewer.vue'
import LogContextMenu from './components/LogContextMenu.vue'
import { useCommitStat } from '@/composables/useCommitStat'
import { usePageFooter } from '@/composables/usePageFooter'
import LogFooter from '@/views/shared/LogFooter.vue'

/**
 * 提交日志 — 左：提交列表；中：该提交修改过的文件树；右：side-by-side diff。
 */
const repo = useRepoStore()
const router = useRouter()
const app = window.go?.main?.ByteGitApp

// ---- commits (left) ----
const commits = ref([])
const loading = ref(false)
const loadingMore = ref(false)
const hasMore = ref(true)
const selectedHash = ref('')
// right-click context menu
const ctxMenu = ref({ open: false, x: 0, y: 0, hash: '' })

function onTreeContextmenu(entry, evt) {
  ctxMenu.value = { open: true, x: evt.clientX, y: evt.clientY, hash: entry.hash }
}
// ctx-menu actions live in <LogContextMenu>; the few that touch parent
// state come back as events:
function onCtxSelect(h) { selectedHash.value = h }
function onCtxReload() { loadInitial() }
const totalCommits = ref(0)
const PAGE = 50

function commitNum(index) {
  return totalCommits.value - index
}

async function loadInitial() {
  if (!app) return
  loading.value = true
  commits.value = []
  hasMore.value = true
  try {
    if (app.CommitTotal) totalCommits.value = await app.CommitTotal()
    const data = await app.LogFetch(repo.repoArg(), PAGE, '')
    commits.value = data || []
    if ((data || []).length < PAGE) hasMore.value = false
    if (commits.value.length) selectCommit(commits.value[0])
  } catch (e) {
    AMessage.error('加载失败: ' + String(e?.message || e))
  } finally {
    loading.value = false
  }
}

async function onLoadMore(done) {
  if (!app || !hasMore.value || !commits.value.length) { done?.(); return }
  try {
    const data = await app.LogFetch(repo.repoArg(), PAGE, String(commits.value.length))
    const arr = data || []
    if (arr.length) commits.value.push(...arr)
    if (arr.length < PAGE) hasMore.value = false
  } catch (e) {
    AMessage.error('加载更多失败: ' + String(e?.message || e))
  } finally {
    done?.()
  }
}

// ---- changed-file tree (middle) ----
// Per-commit stat is lazy-loaded + cached via useCommitStat: hover on a row
// triggers a debounced fetch (for the tooltip card), and selecting a commit
// triggers an immediate fetch (for the file-tree panel) — both share one
// cache, so a hovered-then-selected commit shows instantly.
const {
  stat: statOf, added: addedOf, deleted: deletedOf, has: hasStat,
  fetch: fetchStat, onHover: onStatHover, onLeave: onStatLeave, clear: clearStat,
} = useCommitStat((h) => app.CommitStat(h))
const statLoading = ref(false)
const expandedDirs = ref(new Set())
const selectedFile = ref('')

// active commit's stat (reads from the shared cache)
const commitStat = computed(() => statOf(selectedHash.value))
const flatPaths = computed(() => commitStat.value.map((e) => e.file))

// path -> stat entry (for status/delta lookup)
const statMap = computed(() => {
  const m = new Map()
  for (const e of commitStat.value) m.set(e.file, e)
  return m
})

// build nested tree from flat paths (dirs first, alpha) — reused logic
function buildTree(paths) {
  const root = []
  const dirMap = new Map()
  const ensureDir = (fullPath) => {
    if (dirMap.has(fullPath)) return dirMap.get(fullPath)
    const parts = fullPath.split('/')
    const name = parts[parts.length - 1]
    const node = { name, path: fullPath, isDir: true }
    dirMap.set(fullPath, node)
    if (parts.length === 1) {
      root.push(node)
    } else {
      const parentPath = parts.slice(0, -1).join('/')
      const parent = ensureDir(parentPath)
      if (!parent.children) parent.children = []
      parent.children.push(node)
    }
    return node
  }
  for (const p of paths) {
    const parts = p.split('/')
    if (parts.length === 1) {
      root.push({ name: parts[0], path: p, isDir: false })
    } else {
      const parentPath = parts.slice(0, -1).join('/')
      const parent = ensureDir(parentPath)
      if (!parent.children) parent.children = []
      parent.children.push({ name: parts[parts.length - 1], path: p, isDir: false })
    }
  }
  const sortNodes = (nodes) => {
    nodes.sort((a, b) => {
      if (a.isDir !== b.isDir) return a.isDir ? -1 : 1
      return a.name.toLowerCase().localeCompare(b.name.toLowerCase())
    })
    nodes.forEach((n) => { if (n.children) sortNodes(n.children) })
  }
  sortNodes(root)
  return root
}

const treeNodes = computed(() => buildTree(flatPaths.value))
const treeMap = computed(() => {
  const map = { '__root__': treeNodes.value }
  const walk = (nodes) => {
    for (const n of nodes) {
      if (n.isDir) {
        map[n.path] = n.children || []
        walk(n.children || [])
      }
    }
  }
  walk(treeNodes.value)
  return map
})

function toggleDir(node) {
  if (expandedDirs.value.has(node.path)) {
    expandedDirs.value.delete(node.path)
  } else {
    expandedDirs.value.add(node.path)
  }
  expandedDirs.value = new Set(expandedDirs.value)
}

// ---- diff (right) ----
const fileDiff = ref(null) // FileDiffResult
const diffLoading = ref(false)

async function openFile(node) {
  if (node.isDir) return
  if (!app?.CommitFileDiff || !selectedHash.value) return
  selectedFile.value = node.path
  diffLoading.value = true
  fileDiff.value = null
  try {
    fileDiff.value = await app.CommitFileDiff(selectedHash.value, node.path)
  } catch (e) {
    AMessage.error('读取差异失败: ' + String(e?.message || e))
  } finally {
    diffLoading.value = false
  }
}

// ---- selection wiring ----
// collect every directory path from the changed files so the tree can be
// fully expanded by default (one click to see all changed files).
function allDirPaths(paths) {
  const dirs = new Set()
  for (const p of paths) {
    const parts = p.split('/')
    for (let i = 1; i < parts.length; i++) {
      dirs.add(parts.slice(0, i).join('/'))
    }
  }
  return dirs
}

async function selectCommit(c) {
  selectedHash.value = c.hash
  selectedFile.value = ''
  fileDiff.value = null
  expandedDirs.value = new Set()
  // load changed-file list for this commit (shared cache; immediate fetch)
  if (app?.CommitStat) {
    statLoading.value = true
    try {
      await fetchStat(c.hash)
      // expand all directories so every changed file is visible
      expandedDirs.value = allDirPaths(flatPaths.value)
      // auto-select the last changed file and load its diff
      const paths = flatPaths.value
      if (paths.length) {
        openFile({ path: paths[paths.length - 1] })
      }
    } catch {
      // stat fetch failed; leave uncached
    } finally {
      statLoading.value = false
    }
  }
}

// CommitTree hover-item: debounced stat fetch for the tooltip card.
function onCommitHover(entry, isEnter) {
  if (isEnter) onStatHover(entry.hash)
  else onStatLeave(entry.hash)
}

const activeCommitObj = computed(() => commits.value.find((c) => c.hash === selectedHash.value))
const totalAdded = computed(() => commitStat.value.reduce((s, e) => s + e.added, 0))
const totalDeleted = computed(() => commitStat.value.reduce((s, e) => s + e.deleted, 0))

// ---- footer: inject the selected commit's full info ----
const { setFooter, clearFooter } = usePageFooter()
watch(
  () => [selectedHash.value, commitStat.value],
  () => {
    const c = activeCommitObj.value
    setFooter(LogFooter, () => ({
      hash: c?.hash || '',
      author: c?.author || '',
      date: c ? fmtFooterDate(c.date) : '',
      subject: c?.subject || '',
      added: totalAdded.value,
      deleted: totalDeleted.value,
      fileCount: commitStat.value.length,
      onViewTerminal: (hash) => {
        // send `git show <hash>` to terminal
        const app = window.go?.main?.ByteGitApp
        const cmd = `git show ${hash}\n`
        const bytes = new TextEncoder().encode(cmd)
        let b64 = ''
        bytes.forEach((b) => { b64 += String.fromCharCode(b) })
        b64 = btoa(b64)
        if (app?.TerminalInput) app.TerminalInput(b64).catch(() => {})
        router.push('/terminal')
      },
    }))
  },
  { immediate: true },
)
function fmtFooterDate(d) {
  if (!d) return ''
  try {
    const t = new Date(d)
    if (isNaN(t.getTime())) return d
    return t.toLocaleString('zh-CN', { year: 'numeric', month: '2-digit', day: '2-digit', hour: '2-digit', minute: '2-digit' })
  } catch { return d }
}

onMounted(loadInitial)
watch(() => repo.repoEpoch, () => { clearStat(); loadInitial() })
onBeforeUnmount(() => { clearFooter() })
</script>

<template>
  <div class="log-view">
    <!-- Column 1: commit list -->
    <div class="commit-panel">
      <CommitTree
        :items="commits"
        v-model:active="selectedHash"
        :loading="loading"
        :multi-select="false"
        :context-menu="true"
        :has-more="hasMore"
        empty-text="无提交记录"
        tooltip-placement="right"
        tooltip-overlay-class="lv-tip"
        @click-item="selectCommit"
        @load-more="onLoadMore"
        @hover-item="onCommitHover"
        @contextmenu="onTreeContextmenu"
      >
        <template #header>
          <span class="commit-head-title">
            <HistoryOutlined /> 提交
          </span>
          <a-button size="small" type="text" :loading="loading" title="刷新" @click="loadInitial"><ReloadOutlined /></a-button>
        </template>
        <template #item="{ entry: c, index }">
          <div class="commit-row">
            <div class="commit-line1">
              <span class="commit-num">#{{ commitNum(index) }}</span>
              <span class="hash">{{ c.hash }}</span>
              <span v-if="c.type" class="type" :class="'type-' + c.type">{{ c.type }}</span>
              <span v-if="c.scope" class="scope">({{ c.scope }})</span>
            </div>
            <div class="subject" :title="c.subject">{{ c.subject }}</div>
            <div class="commit-meta">
              <span class="author">{{ c.author }}</span>
              <span v-if="c.hash === selectedHash && commitStat.length" class="stat">
                <span class="add">+{{ totalAdded }}</span>
                <span class="del">-{{ totalDeleted }}</span>
                <span class="files">{{ commitStat.length }} 文件</span>
              </span>
            </div>
          </div>
        </template>

        <template #tooltip="{ entry: c, index }">
          <CommitHoverCard
            :commit="c"
            :num="'#' + commitNum(index)"
            :stat="statOf(c.hash)"
            :stat-loading="!hasStat(c.hash)"
          />
        </template>

        <template #empty>
          <div class="center-pad muted">无提交记录</div>
        </template>
      </CommitTree>
    </div>

    <!-- Column 2: changed-file tree -->
    <aside class="files-panel">
      <div class="files-head">
        <FileOutlined />
        <span>文件</span>
      </div>
      <div class="files-body">
        <div v-if="statLoading" class="center-pad"><a-spin size="small" /></div>
        <div v-else-if="!selectedHash" class="center-pad muted">← 选择一个提交</div>
        <FileTreeNode
          v-else
          :nodes="treeMap['__root__'] || []"
          :depth="0"
          :tree="treeMap"
          :expanded="expandedDirs"
          :selected="selectedFile"
          @toggle="toggleDir"
          @open="openFile"
        >
          <template #extra="{ node }">
            <span v-if="!node.isDir && statMap.get(node.path)" class="fstat">
              <span class="fstatus" :class="'st-' + statMap.get(node.path).status">{{ statMap.get(node.path).status }}</span>
            </span>
          </template>
        </FileTreeNode>
      </div>
    </aside>

    <!-- Column 3: side-by-side diff -->
    <section class="diff-panel">
      <div class="diff-head">
        <span class="diff-path">{{ selectedFile || '（未选择文件）' }}</span>
      </div>
      <div class="diff-body">
        <div v-if="!selectedFile" class="center-pad muted">
          <FileOutlined style="font-size:32px;opacity:0.4" />
          <div style="margin-top:8px">选择文件查看原始 vs 修改后</div>
        </div>
        <DiffViewer
          v-else
          :loading="diffLoading"
          :before="fileDiff?.before || ''"
          :after="fileDiff?.after || ''"
          :filename="selectedFile"
          :status="fileDiff?.status || 'M'"
          :binary="fileDiff?.binary || false"
          :not-found="fileDiff?.notFound || false"
        />
      </div>
    </section>

    <!-- right-click context menu for commits -->
    <LogContextMenu
      v-model:open="ctxMenu.open"
      :x="ctxMenu.x"
      :y="ctxMenu.y"
      :hash="ctxMenu.hash"
      :commits="commits"
      @select="onCtxSelect"
      @goto-time-travel="router.push('/time-travel')"
      @reload="onCtxReload"
      @branch-created="repo.detectCurrent()"
    />
  </div>
</template>

<style scoped>
.log-view {
  width: calc(100% + var(--spacing-lg) * 2);
  height: calc(100% + var(--spacing-lg) * 2);
  margin: calc(-1 * var(--spacing-lg));
  display: flex;
  gap: 1px;
  background: transparent;
  min-height: 0;
}

/* column 1: commits */
.commit-panel { width: 360px; flex-shrink: 0; display: flex; flex-direction: column; background: var(--color-surface); }
/* Flatten CommitTree's inner container so column 1 is flush with the
   borderless, square column 2/3 — no rounded card look, no outer border
   (the 1px gap on .log-view already separates the columns). */
.commit-panel :deep(.ct-list) { border-radius: 0; border: none; }
/* Force CommitTree's header to 34px so it aligns with .files-head / .diff-head. */
.commit-panel :deep(.ct-head) { height: 34px; padding: 0 12px; }
.commit-head-title { display: inline-flex; align-items: center; gap: 6px; }
.commit-head-title .anticon { font-size: 13px; opacity: 0.85; }
.commit-row { padding: 8px 12px; }
.commit-line1 { display: flex; align-items: center; gap: 5px; margin-bottom: 3px; }
.commit-num { font-family: 'Cascadia Code', monospace; font-size: 10px; font-weight: 700; color: var(--color-text-tertiary); min-width: 26px; }
.hash { font-family: 'Cascadia Code', monospace; font-size: 11px; color: var(--color-hash); }
.type { font-size: 9px; font-weight: 700; padding: 1px 5px; border-radius: var(--radius-xs); text-transform: uppercase; }
.scope { font-size: 10px; color: var(--color-text-tertiary); }
.subject { font-size: 12px; color: var(--color-text); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; margin: 1px 0 3px; line-height: 1.3; }
.commit-meta { display: flex; justify-content: space-between; align-items: center; font-size: 10px; color: var(--color-text-tertiary); }
.author { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.stat { display: flex; gap: 6px; font-family: 'Cascadia Code', monospace; flex-shrink: 0; }
.add { color: var(--color-success); }
.del { color: var(--color-error); }
.files { color: var(--color-text-tertiary); }

.type-feat { color: var(--color-feat); background: rgba(80,250,123,0.12); }
.type-fix { color: var(--color-fix); background: rgba(255,184,108,0.12); }
.type-refactor { color: var(--color-refactor); background: rgba(189,147,249,0.12); }
.type-docs { color: var(--color-docs); background: rgba(139,233,253,0.12); }
.type-perf { color: var(--color-perf); background: rgba(255,121,198,0.12); }
.type-chore, .type-build, .type-ci { color: var(--color-text-tertiary); background: var(--color-surface-raised); }

/* column 2: files */
.files-panel { width: 260px; flex-shrink: 0; display: flex; flex-direction: column; background: var(--color-background); min-height: 0; }
.files-head {
  display: flex;
  align-items: center;
  gap: 6px;
  height: 34px;          /* align with .diff-head */
  padding: 0 12px;
  flex-shrink: 0;
  border-bottom: 1px solid var(--color-border);
  font-size: 12px;
  font-weight: 600;
  color: var(--color-text-secondary);
}
.files-head .anticon { font-size: 13px; opacity: 0.85; }
.files-body { flex: 1; overflow-y: auto; padding: 4px; min-height: 0; }
.fstat { margin-left: auto; }
.fstatus { font-size: 9px; font-weight: 700; padding: 0 4px; border-radius: 3px; }
.st-A { color: var(--color-success); }
.st-M { color: var(--color-warning); }
.st-D { color: var(--color-error); }

/* column 3: diff */
.diff-panel { flex: 1; min-width: 0; display: flex; flex-direction: column; background: var(--color-surface); }
.diff-head { display: flex; align-items: center; gap: 8px; padding: 0 12px; height: 34px; flex-shrink: 0; border-bottom: 1px solid var(--color-border); }
.diff-path { font-family: 'Cascadia Code', monospace; font-size: 12px; color: var(--color-text); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.diff-body { flex: 1; min-height: 0; overflow: hidden; }

/* shared */
.center-pad { display: flex; flex-direction: column; align-items: center; justify-content: center; height: 100%; min-height: 120px; color: var(--color-text-tertiary); font-size: 13px; gap: 4px; }
.muted { color: var(--color-text-tertiary); }
</style>
