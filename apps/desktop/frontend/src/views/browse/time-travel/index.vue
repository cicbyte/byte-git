<script setup>
import { ref, computed, onMounted, onBeforeUnmount, watch, nextTick } from 'vue'
import { message as AMessage } from 'ant-design-vue'
import {
  HistoryOutlined, FileOutlined, ReloadOutlined,
} from '@ant-design/icons-vue'
import FileTreeNode from '../../../components/common/FileTreeNode.vue'
import CodeViewer from '../../../components/common/CodeViewer.vue'
import MarkdownView from '../../../components/common/MarkdownView.vue'
import CommitTree from '../../../components/common/CommitTree.vue'
import CommitHoverCard from '../../../components/common/CommitHoverCard.vue'
import TimeTravelContextMenu from './components/TimeTravelContextMenu.vue'
import { useCommitStat } from '../../../composables/useCommitStat'
import { usePageFooter } from '../../../composables/usePageFooter'
import LogFooter from '../../shared/LogFooter.vue'
import { useRepoStore } from '../../../stores/repo'
import { useRouter } from 'vue-router'

/**
 * 代码穿梭 — 浏览任意 commit 的完整源码。
 *
 * 三栏布局：
 *   左：commit 列表（<CommitTree> 分页加载 + hover 卡片）
 *   中：选中 commit 的文件树（git ls-tree，扁平路径→前端构建树）
 *   右：CodeViewer / MarkdownView 显示文件内容（git show）
 *
 * 用于理解历史代码：为什么某段代码这么写、某个 bug 何时引入、
 * 架构如何演进——比只看当前状态更有价值。
 */
const app = window.go?.main?.ByteGitApp
const repo = useRepoStore()
const router = useRouter()

// ---- commits (left column) ----
const commits = ref([])
const loadingCommits = ref(false)
const hasMore = ref(true)
const activeCommit = ref('')
// right-click context menu (mirrors Git Log)
const ctxMenu = ref({ open: false, x: 0, y: 0, hash: '' })

function onTreeContextmenu(entry, evt) {
  ctxMenu.value = { open: true, x: evt.clientX, y: evt.clientY, hash: entry.hash }
}
// ctx-menu actions live in <TimeTravelContextMenu>; the few that touch
// parent state come back as events:
function onCtxSelect(h) { activeCommit.value = h }
function onCtxReload() { loadCommits() }
const totalCommits = ref(0) // for reverse numbering (oldest = #1)

// adaptive page size: fit the visible column height so there's no wasted
// fetch (too many) nor premature "load more" clicks (too few).
const commitsColRef = ref(null) // measures the column (CommitTree owns scroll)
const ITEM_HEIGHT = 62 // approx px per commit row (padding + 3 lines)
const PAGE = ref(50)   // recalculated after mount from column height

function computePageSize() {
  const el = commitsColRef.value
  if (!el) return
  const visible = Math.ceil(el.clientHeight / ITEM_HEIGHT)
  // fill ~2 screens so there's a little scroll buffer before the next load
  PAGE.value = Math.max(visible * 2, 20)
}

async function loadCommits() {
  if (!app?.LogFetch) return
  loadingCommits.value = true
  hasMore.value = true
  try {
    if (app.CommitTotal) totalCommits.value = await app.CommitTotal()
    commits.value = await app.LogFetch('', PAGE.value, '')
    if (commits.value.length) {
      activeCommit.value = commits.value[0].hash
    }
    if (commits.value.length < PAGE.value) hasMore.value = false
  } catch (e) {
    AMessage.error('加载提交失败: ' + String(e?.message || e))
  } finally {
    loadingCommits.value = false
  }
}

// reverse number: first commit (index 0, newest) = total; last = 1
function commitNum(index) {
  return totalCommits.value - index
}

onMounted(async () => {
  await nextTick()
  computePageSize()
  await loadCommits()
})

// CommitTree infinite-scroll callback: fetch the next page (offset = loaded
// count) and append; call `done()` when finished so the tree re-arms.
async function loadMore(done) {
  if (!app?.LogFetch || !commits.value.length || !hasMore.value) { done?.(); return }
  try {
    const more = await app.LogFetch('', PAGE.value, String(commits.value.length))
    if (more?.length) {
      commits.value.push(...more)
      if (more.length < PAGE.value) hasMore.value = false
    } else {
      hasMore.value = false
    }
  } catch (e) {
    AMessage.error('加载更多失败: ' + String(e?.message || e))
  } finally {
    done?.()
  }
}

// ---- per-commit change stat (lazy, shared by hover card + active column) ----
// `fetch` is immediate (for the active commit's file-tree summary);
// `onHover/onLeave` debounce (for the tooltip card). One cache, both paths.
const {
  stat: statOf, added: addedOf, deleted: deletedOf, has: hasStat,
  fetch: fetchStat, onHover: onStatHover, onLeave: onStatLeave, clear: clearStat,
} = useCommitStat((h) => app.CommitStat(h))

// separate flag for the active column's summary bar so a hover fetch doesn't
// flip it on/off
const activeStatLoading = ref(false)

// ---- file tree (middle column) ----
// flatPaths: string[] from CommitFileTree → build a nested tree for FileTreeNode
const flatPaths = ref([])
const loadingTree = ref(false)
const expandedDirs = ref(new Set())
const selectedFile = ref('')

// build nested node structure from flat paths: { name, path, isDir }
function buildTree(paths) {
  const root = [] // top-level nodes
  const dirMap = new Map() // path → node (for quick child lookup)
  const ensureDir = (fullPath) => {
    if (dirMap.has(fullPath)) return dirMap.get(fullPath)
    const parts = fullPath.split('/')
    const name = parts[parts.length - 1]
    const node = { name, path: fullPath, isDir: true }
    dirMap.set(fullPath, node)
    // attach to parent
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
    // skip .git and node_modules
    if (parts[0] === '.git' || parts[0] === 'node_modules') continue
    if (parts.length === 1) {
      root.push({ name: parts[0], path: p, isDir: false })
    } else {
      const parentPath = parts.slice(0, -1).join('/')
      const parent = ensureDir(parentPath)
      if (!parent.children) parent.children = []
      parent.children.push({ name: parts[parts.length - 1], path: p, isDir: false })
    }
  }
  // sort: dirs first, then alpha (VSCode style)
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
// FileTreeNode expects { [path]: children[] } map + flat render; adapt:
const treeMap = computed(() => {
  const map = { '__root__': treeNodes.value }
  // flatten all dirs into the map for FileTreeNode's lookup
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

async function loadFileTree() {
  if (!activeCommit.value || !app?.CommitFileTree) return
  loadingTree.value = true
  selectedFile.value = ''
  fileContent.value = ''
  expandedDirs.value = new Set()
  try {
    const entries = await app.CommitFileTree(activeCommit.value)
    flatPaths.value = (entries || []).map((e) => e.path)
  } catch (e) {
    AMessage.error('加载文件树失败: ' + String(e?.message || e))
    flatPaths.value = []
  } finally {
    loadingTree.value = false
  }
}

// when the active commit changes: refresh the file tree + load its stat
// (shared cache means a hovered-then-selected commit shows instantly).
watch(activeCommit, async (h) => {
  loadFileTree()
  activeStatLoading.value = true
  await fetchStat(h)
  activeStatLoading.value = false
})

// CommitTree hover-item: trigger debounced stat fetch for the tooltip card.
function onCommitHover(entry, isEnter) {
  if (isEnter) onStatHover(entry.hash)
  else onStatLeave(entry.hash)
}

function toggleDir(node) {
  if (expandedDirs.value.has(node.path)) {
    expandedDirs.value.delete(node.path)
  } else {
    expandedDirs.value.add(node.path)
  }
  expandedDirs.value = new Set(expandedDirs.value)
}

// ---- code view (right column) ----
const fileContent = ref('')
const fileLoading = ref(false)
const fileInfo = ref({ binary: false, notFound: false, lines: 0 })
const mdViewMode = ref('render')

const isMarkdown = computed(() => /\.(md|markdown|mdown)$/i.test(selectedFile.value))

async function openFile(node) {
  if (node.isDir) return
  if (!app?.CommitReadFile || !activeCommit.value) return
  selectedFile.value = node.path
  fileLoading.value = true
  fileInfo.value = { binary: false, notFound: false, lines: 0 }
  fileContent.value = ''
  try {
    const res = await app.CommitReadFile(activeCommit.value, node.path)
    fileInfo.value = { binary: res.binary, notFound: res.notFound, lines: res.lines }
    fileContent.value = res.content
  } catch (e) {
    AMessage.error('读取文件失败: ' + String(e?.message || e))
  } finally {
    fileLoading.value = false
  }
}

// ---- commit display helpers ----
const activeCommitObj = computed(() => commits.value.find((c) => c.hash === activeCommit.value))
function shortHash(h) { return h && h.length > 7 ? h.slice(0, 7) : (h || '') }
function fmtDate(d) {
  if (!d) return ''
  try { return new Date(d).toLocaleString('zh-CN', { month: '2-digit', day: '2-digit', hour: '2-digit', minute: '2-digit' }) }
  catch { return d }
}
function fmtFooterDate(d) {
  if (!d) return ''
  try {
    const t = new Date(d)
    if (isNaN(t.getTime())) return d
    return t.toLocaleString('zh-CN', { year: 'numeric', month: '2-digit', day: '2-digit', hour: '2-digit', minute: '2-digit' })
  } catch { return d }
}
function fileCount() {
  return flatPaths.value.length
}

// ---- footer: inject the selected commit's full info (same as LogView) ----
const { setFooter, clearFooter } = usePageFooter()
watch(
  () => [activeCommit.value, statOf(activeCommit.value)],
  () => {
    const c = activeCommitObj.value
    const stat = statOf(activeCommit.value)
    setFooter(LogFooter, () => ({
      hash: c?.hash || '',
      author: c?.author || '',
      date: c ? fmtFooterDate(c.date) : '',
      subject: c?.subject || '',
      added: addedOf(activeCommit.value),
      deleted: deletedOf(activeCommit.value),
      fileCount: stat.length,
      onViewTerminal: (hash) => {
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

// reload everything when the user switches repo
watch(() => repo.repoEpoch, () => {
  commits.value = []
  activeCommit.value = ''
  flatPaths.value = []
  selectedFile.value = ''
  fileContent.value = ''
  clearStat()
  loadCommits()
})

onBeforeUnmount(() => { clearFooter() })
</script>

<template>
  <div class="tt-view">
    <!-- Column 1: commits -->
    <aside class="tt-commits" ref="commitsColRef">
      <div class="tt-col-head">
        <span><HistoryOutlined /> 提交历史</span>
        <a-button size="small" type="text" @click="loadCommits" :loading="loadingCommits"><ReloadOutlined /></a-button>
      </div>
      <CommitTree
        class="tt-commits-tree"
        :items="commits"
        v-model:active="activeCommit"
        :loading="loadingCommits"
        :has-more="hasMore"
        :multi-select="false"
        :context-menu="true"
        tooltip-placement="right"
        tooltip-overlay-class="tt-tip"
        @load-more="loadMore"
        @hover-item="onCommitHover"
        @contextmenu="onTreeContextmenu"
      >
        <template #item="{ entry: c, index }">
          <div class="tt-commit">
            <div class="tt-commit-line1">
              <span class="tt-commit-num">#{{ commitNum(index) }}</span>
              <span class="tt-commit-hash">{{ shortHash(c.hash) }}</span>
              <span v-if="c.type" class="tt-commit-type" :class="'type-' + c.type">{{ c.type }}</span>
              <span v-if="c.scope" class="tt-commit-scope">({{ c.scope }})</span>
              <span class="tt-commit-date">{{ fmtDate(c.date) }}</span>
            </div>
            <div class="tt-commit-subject" :title="c.subject">{{ c.subject }}</div>
            <div class="tt-commit-meta">
              <span class="tt-commit-author">{{ c.author }}</span>
              <span v-if="c.hash === activeCommit && hasStat(c.hash)" class="tt-commit-stat">
                <span class="stat-add">+{{ addedOf(c.hash) }}</span>
                <span class="stat-del">-{{ deletedOf(c.hash) }}</span>
                <span class="stat-files">{{ statOf(c.hash).length }} 文件</span>
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

        <template #footer>
          <div v-if="commits.length && !hasMore" class="tt-end">已到底（共 {{ totalCommits }} 个提交）</div>
        </template>
      </CommitTree>
    </aside>

    <!-- right-click context menu (mirrors Git Log) -->
    <TimeTravelContextMenu
      v-model:open="ctxMenu.open"
      :x="ctxMenu.x"
      :y="ctxMenu.y"
      :hash="ctxMenu.hash"
      :commits="commits"
      @select="onCtxSelect"
      @goto-log="router.push('/log')"
      @reload="onCtxReload"
      @branch-created="repo.detectCurrent()"
    />

    <!-- Column 2: file tree -->
    <aside class="tt-files">
      <div class="tt-files-body">
        <div v-if="loadingTree" class="tt-center"><a-spin size="small" /></div>
        <div v-else-if="!activeCommit" class="tt-center muted">← 选择一个提交</div>
        <FileTreeNode
          v-else
          :nodes="treeMap['__root__'] || []"
          :depth="0"
          :tree="treeMap"
          :expanded="expandedDirs"
          :selected="selectedFile"
          @toggle="toggleDir"
          @open="openFile"
        />
      </div>
    </aside>

    <!-- Column 3: code -->
    <section class="tt-code">
      <div class="tt-code-head">
        <span class="tt-code-path">{{ selectedFile || '（未选择文件）' }}</span>
        <span v-if="fileInfo.lines" class="tt-code-lines">{{ fileInfo.lines }} 行</span>
        <a-segmented
          v-if="isMarkdown && selectedFile && !fileLoading"
          v-model:value="mdViewMode"
          :options="[{ label: '渲染', value: 'render' }, { label: '源码', value: 'source' }]"
          size="small"
          class="tt-md-toggle"
        />
      </div>
      <div class="tt-code-body">
        <div v-if="fileLoading" class="tt-center"><a-spin /></div>
        <div v-else-if="!selectedFile" class="tt-center muted">
          <FileOutlined style="font-size:32px;opacity:0.4" />
          <div style="margin-top:8px">选择文件查看源码</div>
        </div>
        <div v-else-if="fileInfo.binary" class="tt-center">⚠️ 二进制文件</div>
        <div v-else-if="fileInfo.notFound" class="tt-center">⚠️ 文件不存在</div>
        <MarkdownView v-else-if="isMarkdown && mdViewMode === 'render'" :source="fileContent" class="tt-md" />
        <CodeViewer v-else :code="fileContent" :filename="selectedFile" :highlight-lines="[]" />
      </div>
    </section>
  </div>
</template>

<style scoped>
.tt-view { width: 100%; height: 100%; display: flex; gap: 1px; background: var(--color-border); min-height: 0; }

/* shared column chrome */
.tt-commits, .tt-files, .tt-code { background: var(--color-surface); display: flex; flex-direction: column; min-height: 0; }
.tt-col-head { display: flex; align-items: center; gap: 8px; padding: 0 12px; height: 34px; flex-shrink: 0; font-size: 12px; font-weight: 600; color: var(--color-text-secondary); border-bottom: 1px solid var(--color-border); background: var(--color-surface); }

/* column 1: commits (narrow) */
.tt-commits { width: 240px; flex-shrink: 0; }
.tt-commits-tree { flex: 1; min-height: 0; border: none; border-radius: 0; }
/* the row sits inside <CommitTree>'s .ct-item wrapper */
.tt-commit { padding: 8px 12px; }
.tt-commit-line1 { display: flex; align-items: center; gap: 5px; margin-bottom: 3px; }
.tt-commit-num { font-family: 'Cascadia Code', monospace; font-size: 10px; font-weight: 700; color: var(--color-text-tertiary); flex-shrink: 0; min-width: 26px; }
.tt-commit-hash { font-family: 'Cascadia Code', monospace; font-size: 11px; color: var(--color-hash); flex-shrink: 0; }
.tt-commit-type { font-size: 9px; font-weight: 700; padding: 1px 5px; border-radius: var(--radius-xs); flex-shrink: 0; text-transform: uppercase; }
.tt-commit-scope { font-size: 10px; color: var(--color-text-tertiary); flex-shrink: 0; }
.tt-commit-date { font-size: 10px; color: var(--color-text-tertiary); margin-left: auto; flex-shrink: 0; }
.tt-commit-subject { font-size: 12px; color: var(--color-text); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; margin: 1px 0 3px; line-height: 1.3; }
.tt-commit-meta { display: flex; justify-content: space-between; align-items: center; font-size: 10px; color: var(--color-text-tertiary); }
.tt-commit-stat { display: flex; gap: 6px; font-family: 'Cascadia Code', monospace; }
.stat-add { color: var(--color-success); }
.stat-del { color: var(--color-error); }
.stat-files { color: var(--color-text-tertiary); }
.tt-end { padding: 8px; text-align: center; font-size: 11px; color: var(--color-text-tertiary); opacity: 0.6; }

/* conventional commit type colors */
.type-feat { color: var(--color-feat); background: rgba(80,250,123,0.12); }
.type-fix { color: var(--color-fix); background: rgba(255,184,108,0.12); }
.type-refactor { color: var(--color-refactor); background: rgba(189,147,249,0.12); }
.type-docs { color: var(--color-docs); background: rgba(139,233,253,0.12); }
.type-perf { color: var(--color-perf); background: rgba(255,121,198,0.12); }
.type-chore, .type-build, .type-ci { color: var(--color-text-tertiary); background: var(--color-surface-raised); }
.type-test { color: var(--color-warning); background: rgba(255,184,108,0.08); }

/* column 2: files (medium) */
.tt-files { width: 260px; flex-shrink: 0; background: var(--color-background); }
.tt-files-body { flex: 1; overflow-y: auto; padding: 4px; }

/* column 3: code (fills rest) */
.tt-code { flex: 1; min-width: 0; }
.tt-code-head { display: flex; align-items: center; gap: 8px; padding: 0 12px; height: 34px; flex-shrink: 0; border-bottom: 1px solid var(--color-border); }
.tt-code-path { font-family: 'Cascadia Code', monospace; font-size: 12px; color: var(--color-text); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; flex: 1; }
.tt-code-lines { font-size: 11px; color: var(--color-text-tertiary); flex-shrink: 0; }
.tt-md-toggle { flex-shrink: 0; }
.tt-code-body { flex: 1; min-height: 0; overflow: hidden; position: relative; }
.tt-code-body :deep(.cv-host) { height: 100%; }
.tt-md { height: 100%; overflow-y: auto; padding: var(--spacing-md); }

/* shared states */
.tt-center { flex: 1; display: flex; flex-direction: column; align-items: center; justify-content: center; height: 100%; color: var(--color-text-tertiary); font-size: 13px; gap: 4px; }
.muted { opacity: 0.6; }
</style>
