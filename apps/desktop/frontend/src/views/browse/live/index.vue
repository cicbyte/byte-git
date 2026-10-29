<script setup>
import { ref, computed, onMounted, onUnmounted, watch } from 'vue'
import { useRepoStore } from '@/stores/repo'
import { message as AMessage } from 'ant-design-vue'
import { FileOutlined, ReloadOutlined, EyeOutlined } from '@ant-design/icons-vue'
import FileTreeNode from '@/components/common/FileTreeNode.vue'
import DiffViewer from '@/components/common/DiffViewer.vue'
import { useLiveStream } from '@/composables/useLiveStream'

/**
 * 实时变更 — 只读监控工作区相对 HEAD 的未提交改动。
 * 左：变更文件树（含未跟踪 ??）；右：side-by-side diff。
 * 后端 fsnotify 监听文件系统，防抖后 emit "live:changed"，前端刷新。
 *
 * 仅查看，提交/暂存/放弃更改全部走 Navbar 的 SCM 抽屉（stores/scm）。
 */
const repo = useRepoStore()
const app = window.go?.main?.ByteGitApp

// ---- changed-file tree (left) ----
const stat = ref([]) // CommitStatEntry[]
const statLoading = ref(false)
const expandedDirs = ref(new Set())
const selectedFile = ref('')

const flatPaths = computed(() => stat.value.map((e) => e.file))
const statMap = computed(() => {
  const m = new Map()
  for (const e of stat.value) m.set(e.file, e)
  return m
})
const totalAdded = computed(() => stat.value.reduce((s, e) => s + e.added, 0))
const totalDeleted = computed(() => stat.value.reduce((s, e) => s + e.deleted, 0))

// build nested tree from flat paths (dirs first, alpha)
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

function toggleDir(node) {
  if (expandedDirs.value.has(node.path)) {
    expandedDirs.value.delete(node.path)
  } else {
    expandedDirs.value.add(node.path)
  }
  expandedDirs.value = new Set(expandedDirs.value)
}

// ---- diff (right) ----
const fileDiff = ref(null)
const diffLoading = ref(false)
// 'worktree' = HEAD vs 工作区 (LiveFileDiff)
// 'staged'    = HEAD vs 暂存区 (LiveStagedFileDiff) — 查看将要提交的内容
const diffTab = ref('worktree')

async function openFile(node) {
  selectedFile.value = node.path
  await loadDiff()
}

async function loadDiff() {
  if (!selectedFile.value) {
    fileDiff.value = null
    return
  }
  diffLoading.value = true
  fileDiff.value = null
  try {
    if (diffTab.value === 'staged' && app?.LiveStagedFileDiff) {
      fileDiff.value = await app.LiveStagedFileDiff(selectedFile.value)
    } else if (app?.LiveFileDiff) {
      fileDiff.value = await app.LiveFileDiff(selectedFile.value)
    }
  } catch (e) {
    AMessage.error('读取差异失败: ' + String(e?.message || e))
  } finally {
    diffLoading.value = false
  }
}

// 切换 Tab → 重新加载当前文件的对应 diff
watch(diffTab, () => loadDiff())

// 暂存区 tab 下，若 HEAD 与暂存区内容一致（文件未暂存 / 暂存内容=HEAD），
// 显示明确提示而非两个相同面板，避免用户误以为 diff 坏了。
const stagedUnchanged = computed(() => {
  if (diffTab.value !== 'staged' || !fileDiff.value) return false
  const d = fileDiff.value
  // 纯新增文件 staged 时 notFound=true 且 after 有内容 → 有差异，不提示
  if (d.notFound && d.after) return false
  return d.before === d.after
})

// ---- data loading ----
async function loadStat() {
  if (!app?.LiveStat) return
  statLoading.value = true
  try {
    stat.value = (await app.LiveStat()) || []
    expandedDirs.value = allDirPaths(flatPaths.value)
    // auto-select last file if none selected or selection vanished
    const paths = flatPaths.value
    if (paths.length) {
      if (!selectedFile.value || !statMap.value.has(selectedFile.value)) {
        openFile({ path: paths[paths.length - 1] })
      } else {
        loadDiff()
      }
    } else {
      selectedFile.value = ''
      fileDiff.value = null
    }
  } catch (e) {
    AMessage.error('加载变更失败: ' + String(e?.message || e))
    stat.value = []
  } finally {
    statLoading.value = false
  }
}

async function startWatch() {
  if (!app?.LiveStartWatch) return
  try {
    await app.LiveStartWatch()
  } catch {}
}

async function stopWatch() {
  if (!app?.LiveStopWatch) return
  try {
    await app.LiveStopWatch()
  } catch {}
}

// fsnotify → live:changed event → reload
useLiveStream(() => {
  loadStat()
})

// lifecycle
onMounted(async () => {
  await loadStat()
  await startWatch()
})

onUnmounted(() => {
  stopWatch()
})

// repo switch → reload + restart watcher
watch(() => repo.repoEpoch, async () => {
  selectedFile.value = ''
  fileDiff.value = null
  await loadStat()
  await startWatch()
})
</script>

<template>
  <div class="live-view">
    <!-- Column 1: changed-file tree (read-only) -->
    <aside class="files-panel">
      <div class="col-head">
        <span class="watch-indicator" title="文件系统监听中">
          <EyeOutlined />
          <span class="dot"></span>
        </span>
        <span class="col-title">实时变更</span>
        <a-button size="small" type="text" @click="loadStat" :loading="statLoading"><ReloadOutlined /></a-button>
      </div>
      <div v-if="stat.length" class="changes-bar">
        <span class="cb-add">+{{ totalAdded }}</span>
        <span class="cb-del">-{{ totalDeleted }}</span>
        <span class="cb-files">{{ stat.length }} 个文件</span>
      </div>
      <div class="files-body">
        <div v-if="statLoading && !stat.length" class="center-pad"><a-spin size="small" /></div>
        <div v-else-if="!stat.length" class="center-pad muted">
          <FileOutlined style="font-size:28px;opacity:0.4" />
          <div style="margin-top:8px">工作区无未提交改动</div>
        </div>
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

    <!-- Column 2: side-by-side diff (Tab: worktree / staged) -->
    <section class="diff-panel">
      <div class="diff-head">
        <a-tabs v-model:activeKey="diffTab" size="small" class="diff-tabs">
          <a-tab-pane key="worktree" tab="工作区" />
          <a-tab-pane key="staged" tab="暂存区" />
        </a-tabs>
        <span class="diff-path">{{ selectedFile || '（未选择文件）' }}</span>
      </div>
      <div class="diff-body">
        <div v-if="!selectedFile" class="center-pad muted">
          <FileOutlined style="font-size:32px;opacity:0.4" />
          <div style="margin-top:8px">选择文件查看 HEAD vs {{ diffTab === 'staged' ? '暂存区' : '工作区' }}</div>
        </div>
        <div v-else-if="stagedUnchanged && !diffLoading" class="center-pad muted">
          <FileOutlined style="font-size:32px;opacity:0.4" />
          <div style="margin-top:8px">此文件未暂存</div>
          <div style="font-size:11px;margin-top:4px">暂存区与 HEAD 一致，暂存后可在此查看将提交的差异</div>
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
  </div>
</template>

<style scoped>
.live-view {
  width: calc(100% + var(--spacing-lg) * 2);
  height: calc(100% + var(--spacing-lg) * 2);
  margin: calc(-1 * var(--spacing-lg));
  display: flex;
  gap: 1px;
  background: transparent;
  min-height: 0;
}

/* column 1: files */
.files-panel { width: 280px; flex-shrink: 0; display: flex; flex-direction: column; background: var(--color-background); min-height: 0; }
.col-head { display: flex; align-items: center; gap: 8px; padding: 0 12px; height: 38px; flex-shrink: 0; font-size: 13px; font-weight: 600; color: var(--color-text); border-bottom: 1px solid var(--color-border); background: var(--color-surface); }
.col-title { flex: 1; }
.watch-indicator { display: flex; align-items: center; gap: 4px; color: var(--color-success); }
.watch-indicator .dot { width: 7px; height: 7px; border-radius: 50%; background: var(--color-success); animation: live-pulse 1.5s ease-in-out infinite; }
@keyframes live-pulse { 0%,100% { opacity: 1; } 50% { opacity: 0.3; } }

.changes-bar { display: flex; align-items: center; gap: 10px; padding: 4px 12px; font-family: 'Cascadia Code', monospace; font-size: 11px; border-bottom: 1px solid var(--color-border-light); flex-shrink: 0; background: var(--color-surface); }
.cb-add { color: var(--color-success); font-weight: 600; }
.cb-del { color: var(--color-error); font-weight: 600; }
.cb-files { color: var(--color-text-tertiary); margin-left: auto; }

.files-body { flex: 1; overflow-y: auto; padding: 4px; min-height: 0; }
.fstat { margin-left: auto; }
.fstatus { font-size: 9px; font-weight: 700; padding: 0 4px; border-radius: 3px; }
.st-A { color: var(--color-success); }
.st-M { color: var(--color-warning); }
.st-D { color: var(--color-error); }

/* column 2: diff */
.diff-panel { flex: 1; min-width: 0; display: flex; flex-direction: column; background: var(--color-surface); }
.diff-head { display: flex; align-items: center; gap: 12px; padding: 0 12px; height: 38px; flex-shrink: 0; border-bottom: 1px solid var(--color-border); }
.diff-tabs { flex-shrink: 0; }
.diff-tabs :deep(.ant-tabs-nav) { margin: 0; }
.diff-tabs :deep(.ant-tabs-tab) { padding: 6px 0 !important; }
.diff-path { font-family: 'Cascadia Code', monospace; font-size: 12px; color: var(--color-text); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; flex: 1; text-align: right; }
.diff-body { flex: 1; min-height: 0; overflow: hidden; }

.center-pad { display: flex; flex-direction: column; align-items: center; justify-content: center; height: 100%; min-height: 120px; color: var(--color-text-tertiary); font-size: 13px; gap: 4px; }
.muted { color: var(--color-text-tertiary); }
</style>
