<script setup>
/**
 * AI 改写 — 右栏任务详情面板，从 ai-rewrite/index.vue 拆出。
 * 展示原始 vs 改写对比、操作按钮、文件树与单文件 diff。
 *
 * 数据流：
 *  - props.activeTask  当前选中任务（对象，v-model.suggested 由父级通过响应式数组直接改）
 *  - props.singleHash  正在单条生成的 hash（用于显示“生成中…”、禁用输入）
 *  - emit generate/approve/skip/remove  操作委托回父级
 *  - 文件列表与 diff 为本组件内部懒加载（按 activeTask.hash 重置）
 */
import { ref, computed, watch } from 'vue'
import {
  ThunderboltOutlined, CheckOutlined, CloseOutlined, DeleteOutlined, EditOutlined,
  FileTextOutlined,
} from '@ant-design/icons-vue'
import FileTreeNode from '../../../../components/common/FileTreeNode.vue'
import DiffViewer from '../../../../components/common/DiffViewer.vue'

const props = defineProps({
  /** 当前选中的任务对象，可为 null */
  activeTask: { type: Object, default: null },
  /** 正在生成的任务 hash（来自父级 singleHash） */
  singleHash: { type: String, default: '' },
  /** 当前任务在 tasks 数组中的索引（用于 remove 事件回传） */
  index: { type: Number, default: -1 },
  /** 仓库参数（repo.repoArg()），用于后端 RewriteEntryDiff 调用 */
  repoArg: { type: [Object, Array, String], default: null },
})

const emit = defineEmits(['generate', 'approve', 'skip', 'remove'])

const app = window.go?.main?.ByteGitApp

// ---- 文件列表 + diff（按任务 hash 懒加载）----
const filesLoading = ref(false)
const commitFiles = ref([])
const selectedFile = ref('')
const fileDiff = ref(null)
const diffLoading = ref(false)
const filesCache = ref(new Map())
const expandedDirs = ref(new Set())

function buildFileTree(paths) {
  const root = []
  const dirMap = new Map()
  const ensureDir = (fullPath) => {
    if (dirMap.has(fullPath)) return dirMap.get(fullPath)
    const parts = fullPath.split('/')
    const name = parts[parts.length - 1]
    const node = { name, path: fullPath, isDir: true }
    dirMap.set(fullPath, node)
    if (parts.length === 1) root.push(node)
    else { const p = ensureDir(parts.slice(0, -1).join('/')); if (!p.children) p.children = []; p.children.push(node) }
    return node
  }
  for (const p of paths) {
    const parts = p.split('/')
    if (parts.length === 1) root.push({ name: parts[0], path: p, isDir: false })
    else { const par = ensureDir(parts.slice(0, -1).join('/')); if (!par.children) par.children = []; par.children.push({ name: parts[parts.length - 1], path: p, isDir: false }) }
  }
  const sort = (nodes) => {
    nodes.sort((a, b) => (a.isDir !== b.isDir ? (a.isDir ? -1 : 1) : a.name.toLowerCase().localeCompare(b.name.toLowerCase())))
    nodes.forEach((n) => n.children && sort(n.children))
  }
  sort(root)
  return root
}
const fileTreeNodes = computed(() => buildFileTree(commitFiles.value))
const fileTreeMap = computed(() => {
  const map = { '__root__': fileTreeNodes.value }
  const walk = (nodes) => { for (const n of nodes) { if (n.isDir) { map[n.path] = n.children || []; walk(n.children || []) } } }
  walk(fileTreeNodes.value)
  return map
})
function toggleDir(node) {
  if (expandedDirs.value.has(node.path)) expandedDirs.value.delete(node.path)
  else expandedDirs.value.add(node.path)
  expandedDirs.value = new Set(expandedDirs.value)
}
async function loadFiles(hash) {
  if (filesCache.value.has(hash)) { commitFiles.value = filesCache.value.get(hash); return }
  filesLoading.value = true
  try {
    const r = await app.RewriteEntryDiff(props.repoArg, hash)
    const paths = r.files || []
    filesCache.value.set(hash, paths)
    commitFiles.value = paths
    const dirs = new Set()
    for (const p of paths) { const parts = p.split('/'); for (let i = 1; i < parts.length; i++) dirs.add(parts.slice(0, i).join('/')) }
    expandedDirs.value = dirs
  } catch { commitFiles.value = [] } finally { filesLoading.value = false }
}

async function openFile(node) {
  if (node.isDir) return
  if (!app?.CommitFileDiff || !props.activeTask) return
  selectedFile.value = node.path
  diffLoading.value = true
  fileDiff.value = null
  try { fileDiff.value = await app.CommitFileDiff(props.activeTask.hash, node.path) }
  catch { fileDiff.value = null } finally { diffLoading.value = false }
}

// 切换任务时重置并加载文件
watch(() => props.activeTask?.hash, (hash) => {
  selectedFile.value = ''
  fileDiff.value = null
  if (hash) loadFiles(hash)
})
</script>

<template>
  <section class="ar-detail">
    <!-- 34px column header — aligns with the left/middle column headers. -->
    <div class="ar-detail-head">
      <FileTextOutlined />
      <span>任务详情</span>
      <span v-if="activeTask" class="ar-detail-hash">{{ activeTask.hash.slice(0, 7) }}</span>
    </div>
    <template v-if="activeTask">
      <!-- 原始 vs 改写 -->
      <div class="ar-msg-area">
        <div class="ar-msg-head">
          <span class="ar-msg-title">Commit Message</span>
          <span class="ar-hash">{{ activeTask.hash.slice(0, 7) }}</span>
          <span v-if="singleHash === activeTask.hash" class="ar-typing">生成中…</span>
        </div>
        <div class="ar-msg-cols">
          <div class="ar-msg-side">
            <div class="ar-msg-label">原始</div>
            <div class="ar-msg-orig">{{ activeTask.original || '—' }}</div>
          </div>
          <div class="ar-msg-side">
            <div class="ar-msg-label">改写 <span class="ar-status-tag" :class="'st-' + (activeTask.status||'pending')">{{ activeTask.status }}</span></div>
            <textarea
              class="ar-msg-input"
              v-model="activeTask.suggested"
              :readonly="singleHash === activeTask.hash"
              placeholder="点击「生成」获取 AI 建议，或手动输入"
              rows="4"
            ></textarea>
          </div>
        </div>
        <div class="ar-msg-actions">
          <a-button size="small" type="primary" :loading="singleHash === activeTask.hash" @click="emit('generate', activeTask)">
            <template #icon><ThunderboltOutlined /></template>{{ activeTask.suggested ? '重新生成' : '生成' }}
          </a-button>
          <a-button size="small" :type="activeTask.status === 'approved' ? 'primary' : 'default'" @click="emit('approve', activeTask)">
            <template #icon><CheckOutlined /></template>采用
          </a-button>
          <a-button size="small" @click="emit('skip', activeTask)">
            <template #icon><CloseOutlined /></template>跳过
          </a-button>
          <a-button size="small" type="text" @click="emit('remove', index)">
            <template #icon><DeleteOutlined /></template>移除任务
          </a-button>
        </div>
      </div>

      <!-- 文件 diff -->
      <div class="ar-diff-area">
        <div class="ar-files-panel">
          <div class="ar-files-head"><span>文件</span><span class="ar-file-count">{{ commitFiles.length }}</span></div>
          <div class="ar-files-body">
            <div v-if="filesLoading" class="ar-center"><a-spin size="small" /></div>
            <div v-else-if="!commitFiles.length" class="ar-muted-sm">无文件变更</div>
            <FileTreeNode v-else :nodes="fileTreeMap['__root__'] || []" :depth="0" :tree="fileTreeMap" :expanded="expandedDirs" :selected="selectedFile" @toggle="toggleDir" @open="openFile" />
          </div>
        </div>
        <div class="ar-diff-panel">
          <div class="ar-diff-head">
            <span class="ar-msg-title">Diff</span>
            <span class="ar-diff-path">{{ selectedFile || '' }}</span>
          </div>
          <div class="ar-diff-body">
            <div v-if="!selectedFile" class="ar-center-muted">选择文件查看 diff</div>
            <div v-else-if="diffLoading" class="ar-center"><a-spin /></div>
            <DiffViewer v-else :loading="false" :before="fileDiff?.before || ''" :after="fileDiff?.after || ''" :filename="selectedFile" :status="fileDiff?.status || 'M'" :binary="fileDiff?.binary || false" :not-found="fileDiff?.notFound || false" />
          </div>
        </div>
      </div>
    </template>
    <div v-else class="ar-detail-empty">
      <EditOutlined style="font-size:28px;opacity:0.3" />
      <div style="margin-top:8px">选择一个任务查看详情</div>
    </div>
  </section>
</template>

<style scoped>
/* 右栏：详情 */
.ar-detail { background: var(--color-surface); display: flex; flex-direction: column; overflow: hidden; }
.ar-detail-head {
  display: flex; align-items: center; gap: 6px;
  height: 34px;
  padding: 0 12px;
  flex-shrink: 0;
  border-bottom: 1px solid var(--color-border);
  font-size: 12px;
  font-weight: 600;
  color: var(--color-text-secondary);
}
.ar-detail-head .anticon { font-size: 13px; opacity: 0.85; }
.ar-detail-hash {
  margin-left: auto;
  font-family: 'Cascadia Code', monospace;
  font-size: 11px;
  font-weight: 400;
  color: var(--color-hash);
}
.ar-detail-empty { flex: 1; display: flex; flex-direction: column; align-items: center; justify-content: center; color: var(--color-text-tertiary); font-size: 13px; }

/* message 编辑区 */
.ar-msg-area { flex-shrink: 0; padding: 12px; border-bottom: 1px solid var(--color-border); }
.ar-msg-head { display: flex; align-items: center; gap: 8px; margin-bottom: 8px; }
.ar-msg-title { font-size: 13px; font-weight: 600; color: var(--color-text); }
.ar-typing { font-size: 11px; color: var(--color-primary); }
.ar-msg-cols { display: grid; grid-template-columns: 1fr 1fr; gap: 12px; margin-bottom: 8px; }
.ar-msg-side { display: flex; flex-direction: column; gap: 4px; }
.ar-msg-label { font-size: 11px; color: var(--color-text-tertiary); display: flex; align-items: center; gap: 6px; }
.ar-status-tag { font-size: 9px; padding: 0 4px; border-radius: 3px; font-weight: 600; }
.ar-status-tag.st-approved { color: var(--color-success); background: var(--color-success-bg); }
.ar-status-tag.st-suggested { color: var(--color-primary); background: var(--color-primary-bg); }
.ar-msg-orig { font-size: 12px; color: var(--color-text-secondary); background: var(--color-background); border-radius: var(--radius-sm); padding: 8px; white-space: pre-wrap; word-break: break-word; flex: 1; min-height: 60px; overflow-y: auto; }
.ar-msg-input { width: 100%; resize: vertical; padding: 8px; border: 1px solid var(--color-border); border-radius: var(--radius-sm); background: var(--color-background); color: var(--color-text); font-family: 'Cascadia Code', monospace; font-size: 12px; outline: none; min-height: 60px; }
.ar-msg-input:focus { border-color: var(--color-primary); }
.ar-msg-actions { display: flex; gap: 6px; flex-wrap: wrap; }

/* diff 区 */
.ar-diff-area { flex: 1; display: flex; gap: 1px; background: var(--color-border); min-height: 0; overflow: hidden; }
.ar-files-panel { width: 180px; flex-shrink: 0; display: flex; flex-direction: column; background: var(--color-background); min-height: 0; }
.ar-files-head { display: flex; align-items: center; justify-content: space-between; padding: 0 10px; height: 30px; font-size: 11px; font-weight: 600; border-bottom: 1px solid var(--color-border); background: var(--color-surface); }
.ar-file-count { color: var(--color-text-tertiary); }
.ar-files-body { flex: 1; overflow-y: auto; padding: 4px; }
.ar-diff-panel { flex: 1; display: flex; flex-direction: column; background: var(--color-surface); min-width: 0; min-height: 0; }
.ar-diff-head { display: flex; align-items: center; gap: 8px; padding: 0 12px; height: 30px; border-bottom: 1px solid var(--color-border); }
.ar-hash { font-family: 'Cascadia Code', monospace; font-size: 11px; color: var(--color-hash); }
.ar-diff-path { font-family: 'Cascadia Code', monospace; font-size: 11px; color: var(--color-text-tertiary); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.ar-diff-body { flex: 1; min-height: 0; overflow: hidden; }
.ar-center { flex: 1; display: flex; align-items: center; justify-content: center; }
.ar-center-muted { flex: 1; display: flex; align-items: center; justify-content: center; color: var(--color-text-tertiary); font-size: 13px; }
.ar-muted-sm { padding: 16px; text-align: center; color: var(--color-text-tertiary); font-size: 12px; }
</style>
