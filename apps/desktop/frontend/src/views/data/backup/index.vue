<script setup>
import { ref, computed, onMounted, onBeforeUnmount, watch, h } from 'vue'
import { message as AMessage, Modal } from 'ant-design-vue'
import {
  DatabaseOutlined, ReloadOutlined, PlusOutlined,
  DeleteOutlined, ClearOutlined,
  ClockCircleOutlined, FolderOpenOutlined, FileOutlined
} from '@ant-design/icons-vue'
import CodeViewer from '../../../components/common/CodeViewer.vue'
import FileTreeNode from '../../../components/common/FileTreeNode.vue'
import MarkdownView from '../../../components/common/MarkdownView.vue'
import ContextMenu from '../../../components/common/ContextMenu.vue'
import CtxItem from '../../../components/common/CtxItem.vue'
import BackupArchiveModal from './components/BackupArchiveModal.vue'
import { usePageFooter } from '../../../composables/usePageFooter'

const app = window.go?.main?.ByteGitApp

// ---- mode switch (整库克隆 / 压缩包；ref 快照已移除) ----
const mode = ref('full') // 'full' | 'archive'

// ---- data ----
const fullBackups = ref([])     // backup.BackupEntry[]
const archives = ref([])        // archive.Entry[]
const loading = ref(false)
const activeId = ref('')        // selected full backup name / archive name

// ---- archive modal (delegated to BackupArchiveModal) ----
const archiveModalOpen = ref(false)

// ---- derived ----
const activeFull = computed(() => fullBackups.value.find((b) => b.name === activeId.value))
const activeArchive = computed(() => archives.value.find((b) => b.name === activeId.value))
const currentList = computed(() => {
  if (mode.value === 'full') return fullBackups.value
  return archives.value
})
const listCount = computed(() => currentList.value.length)

// ---- busy flags ----
const creating = ref(false)

// ---- file browser (full backup only) ----
// tree is a flat map: { [path]: { children: [], loaded, loading } }
// root children live under key "" (empty path).
const tree = ref({})                 // { [path]: FileNode[] }
const expandedDirs = ref(new Set())  // paths that are expanded
const selectedFile = ref('')         // currently selected file path
const fileContent = ref('')
const fileLoading = ref(false)
const fileInfo = ref({ binary: false, notFound: false, lines: 0 })
// markdown files render as rich HTML; everything else uses CodeViewer
const isMarkdown = computed(() => /\.(md|markdown|mdown)$/i.test(selectedFile.value))
// for markdown files: toggle between rendered HTML and raw source
const mdViewMode = ref('render') // 'render' | 'source'

async function loadDir(subPath) {
  if (!app?.FullBackupFileList || !activeId.value) return
  // mark loading
  tree.value[subPath] = tree.value[subPath] || []
  try {
    const nodes = await app.FullBackupFileList(activeId.value, subPath)
    tree.value = { ...tree.value, [subPath || '__root__']: nodes }
  } catch {
    tree.value[subPath || '__root__'] = []
  }
}

function toggleDir(node) {
  const path = node.path
  if (expandedDirs.value.has(path)) {
    expandedDirs.value.delete(path)
    expandedDirs.value = new Set(expandedDirs.value)
  } else {
    expandedDirs.value.add(path)
    expandedDirs.value = new Set(expandedDirs.value)
    // lazy-load children if not yet loaded
    if (!(path in tree.value)) loadDir(path)
  }
}

async function openFile(node) {
  // FileTreeNode emits `open` for both files and dirs; ignore dir nodes —
  // the tree's `@toggle` handler already expands/collapses them.
  if (node.isDir) return
  if (!app?.FullBackupFileRead || !activeId.value) return
  selectedFile.value = node.path
  fileLoading.value = true
  fileInfo.value = { binary: false, notFound: false, lines: 0 }
  fileContent.value = ''
  try {
    const res = await app.FullBackupFileRead(activeId.value, node.path)
    fileInfo.value = { binary: res.binary, notFound: res.notFound, lines: res.lines, dataUrl: res.dataUrl || '' }
    fileContent.value = res.content
  } catch (e) {
    AMessage.error('读取文件失败: ' + String(e?.message || e))
  } finally {
    fileLoading.value = false
  }
}

// reset file browser when switching backup or mode
function resetBrowser() {
  tree.value = {}
  expandedDirs.value = new Set()
  selectedFile.value = ''
  fileContent.value = ''
  fileInfo.value = { binary: false, notFound: false, lines: 0 }
}
// resolve a relative image inside a full-clone backup → data URL, for
// MarkdownView's local-image inlining. Backups live outside the current repo
// so RepoReadImage can't reach them — read via FullBackupFileRead instead.
async function resolveBackupImage(rel) {
  if (!app?.FullBackupFileRead || !activeId.value) return ''
  try {
    const r = await app.FullBackupFileRead(activeId.value, rel)
    return r.dataUrl || ''
  } catch { return '' }
}

// ---- load ----
async function loadAll() {
  loading.value = true
  try {
    await Promise.all([loadFullBackups(), loadArchives()])
  } finally {
    loading.value = false
  }
}
async function loadFullBackups() {
  if (!app?.FullBackupList) return
  try {
    fullBackups.value = await app.FullBackupList()
    if (!activeId.value && fullBackups.value.length && mode.value === 'full') {
      activeId.value = fullBackups.value[0].name
    }
  } catch { fullBackups.value = [] }
}
async function loadArchives() {
  if (!app?.ArchiveList) return
  try {
    archives.value = await app.ArchiveList()
    if (!activeId.value && archives.value.length && mode.value === 'archive') {
      activeId.value = archives.value[0].name
    }
  } catch { archives.value = [] }
}
onMounted(loadAll)

function switchMode(m) {
  mode.value = m
  resetBrowser()
  // pick first item of the new mode
  const list = currentList.value
  activeId.value = list.length ? list[0].name : ''
  // for full mode, load the root file tree of the selected backup
  if (m === 'full' && activeId.value) loadDir('')
}

// when activeId changes in full mode, reset + load root tree of the new backup
watch(activeId, (id, old) => {
  if (mode.value === 'full' && id && id !== old) {
    resetBrowser()
    loadDir('')
  }
})

// ---- create ----
function handleCreate() {
  if (mode.value === 'full') {
    Modal.confirm({
      title: '创建整库克隆备份？',
      content: '将执行 git clone 完整复制当前仓库，可能需要一些时间（取决于仓库大小）。',
      okText: '开始克隆',
      onOk: createFull,
    })
  } else {
    // archive mode: open the pack modal
    archiveModalOpen.value = true
  }
}
async function createFull() {
  creating.value = true
  try {
    await app.FullBackupCreate()
    AMessage.success('整库克隆备份完成')
    await loadFullBackups()
  } catch (e) {
    AMessage.error('克隆失败: ' + String(e?.message || e))
  } finally { creating.value = false }
}
async function handleCancel() {
  try { if (app?.CancelTask) await app.CancelTask() } catch {}
  creating.value = false
  AMessage.info('已取消')
}

// ---- context menu (right-click a backup node to delete) ----
const ctxMenu = ref({ open: false, x: 0, y: 0, name: '', path: '' })
function onItemContextmenu(b, evt) {
  evt.preventDefault()
  ctxMenu.value = { open: true, x: evt.clientX, y: evt.clientY, name: b.name, path: b.path || '' }
}
function closeCtx() { ctxMenu.value.open = false }

// "Reveal in file manager" — label adapted to the host OS. Wails webview's
// userAgent reliably reflects the real platform, so we show the native file
// manager name (Finder / 资源管理器 / 文件管理器) instead of a generic term.
const _fmUA = navigator.userAgent
const revealLabel = /Mac/.test(_fmUA) ? '在 Finder 中显示'
  : /Windows/.test(_fmUA) ? '在文件资源管理器中显示'
  : '在文件管理器中显示'

async function ctxReveal() {
  const p = ctxMenu.value.path
  closeCtx()
  if (!p || !app?.OpenInExplorer) return
  try { await app.OpenInExplorer(p) } catch {}
}

// ---- delete ----
// name: the backup to delete (from right-click). Falls back to the active one.
function handleDelete(name) {
  const target = name || (mode.value === 'full' ? activeFull.value?.name : activeArchive.value?.name)
  if (!target) return
  Modal.confirm({
    title: '确认删除此备份？',
    content: `「${target}」将被永久删除，此操作不可撤销。`,
    okText: '删除',
    okType: 'danger',
    cancelText: '取消',
    async onOk() {
      try {
        if (mode.value === 'full') {
          await app.FullBackupDelete(target)
          await loadFullBackups()
        } else {
          await app.ArchiveDelete(target)
          await loadArchives()
        }
        AMessage.success('已删除')
        // reselect first if current was deleted
        const list = currentList.value
        if (!list.find((x) => x.name === activeId.value)) {
          activeId.value = list.length ? list[0].name : ''
        }
      } catch (e) {
        AMessage.error('删除失败: ' + String(e?.message || e))
      }
    },
  })
}

function ctxDelete() {
  const name = ctxMenu.value.name
  closeCtx()
  handleDelete(name)
}

function handleCleanAll() {
  if (!listCount.value) return
  Modal.confirm({
    title: `确认清理全部 ${listCount.value} 个${mode.value === 'full' ? '整库克隆' : '压缩包'}？`,
    content: '将删除当前类型的所有备份，此操作不可撤销。',
    okText: '全部删除',
    okType: 'danger',
    cancelText: '取消',
    async onOk() {
      try {
        if (mode.value === 'full') {
          for (const b of fullBackups.value) await app.FullBackupDelete(b.name)
          await loadFullBackups()
        } else {
          for (const a of archives.value) await app.ArchiveDelete(a.name)
          await loadArchives()
        }
        AMessage.success('已清理全部')
        activeId.value = ''
      } catch (e) {
        AMessage.error('清理失败: ' + String(e?.message || e))
      }
    },
  })
}

// ---- helpers ----
function fmtSize(bytes) {
  if (!bytes) return '0 B'
  if (bytes < 1024) return bytes + ' B'
  if (bytes < 1048576) return (bytes / 1024).toFixed(1) + ' KB'
  return (bytes / 1048576).toFixed(1) + ' MB'
}

// ---- global footer: archive note (moved here from the detail a-alert) ----
// 打包压缩是独立归档这条提示放在全局 footer 一行,详情区更干净。
const ArchiveNoteFooter = () => h('span',
  { style: 'font-size:12px;color:var(--color-text-tertiary);user-select:none' },
  '打包压缩文件是独立的归档，可直接解压查看，无需 git')
const { setFooter, clearFooter } = usePageFooter()
watch(mode, (m) => {
  if (m === 'archive') setFooter(ArchiveNoteFooter, () => ({}))
  else clearFooter()
}, { immediate: true })
onBeforeUnmount(() => clearFooter())
</script>

<template>
  <div class="bv-view">
    <!-- Top toolbar -->
    <div class="bv-top">
      <a-segmented v-model:value="mode" :options="[
        { label: '整库克隆', value: 'full' },
        { label: '打包压缩', value: 'archive' },
      ]" size="small" @change="(e) => switchMode(e.target ? e.target.value : e)" />
      <span class="bv-count">{{ listCount }} 个备份</span>
      <div class="bv-spacer"></div>
      <a-button size="small" @click="loadAll" :loading="loading"><template #icon><ReloadOutlined /></template>刷新</a-button>
      <template v-if="creating">
        <a-button size="small" danger @click="handleCancel">取消</a-button>
      </template>
      <template v-else>
        <a-button size="small" @click="handleCreate">
          <template #icon><PlusOutlined /></template>{{ mode === 'full' ? '克隆备份' : '打包' }}
        </a-button>
      </template>
      <a-button size="small" :disabled="!listCount" @click="handleCleanAll">
        <template #icon><ClearOutlined /></template>清理全部
      </a-button>
    </div>

    <!-- Empty state -->
    <div v-if="loading" class="bv-empty"><a-spin /></div>
    <div v-else-if="!listCount" class="bv-empty">
      <DatabaseOutlined style="font-size:40px;color:var(--color-text-tertiary);opacity:0.5" />
      <div class="bv-empty-title">暂无{{ mode === 'full' ? '整库克隆' : '压缩包' }}</div>
      <div class="bv-empty-desc">{{ mode === 'full' ? '手动克隆完整仓库用于灾难恢复' : '将仓库打包为 zip / tar.gz 归档' }}</div>
      <a-button type="primary" size="small" @click="handleCreate" :loading="creating">
        <template #icon><PlusOutlined /></template>创建第一个备份
      </a-button>
    </div>

    <!-- Two columns -->
    <div v-else class="bv-cols">
      <!-- Left: backup list -->
      <aside class="bv-list">
        <div class="bv-list-head">
          <span>{{ mode === 'full' ? '克隆列表' : '打包列表' }}</span>
        </div>
        <div class="bv-list-body">
          <!-- full backup items -->
          <template v-if="mode === 'full'">
            <div
              v-for="b in fullBackups" :key="b.name"
              class="bv-item" :class="{ active: b.name === activeId }"
              @click="activeId = b.name"
              @contextmenu="onItemContextmenu(b, $event)"
            >
              <FolderOpenOutlined class="bv-icon" />
              <div class="bv-item-body">
                <div class="bv-item-id">{{ b.timestamp }}</div>
                <div class="bv-item-meta">
                  <span class="bv-item-size">{{ fmtSize(b.size) }}</span>
                </div>
              </div>
            </div>
          </template>
          <!-- archive items -->
          <template v-else>
            <div
              v-for="b in archives" :key="b.name"
              class="bv-item" :class="{ active: b.name === activeId }"
              @click="activeId = b.name"
              @contextmenu="onItemContextmenu(b, $event)"
            >
              <FileZipOutlined class="bv-icon" />
              <div class="bv-item-body">
                <div class="bv-item-id">{{ b.name }}</div>
                <div class="bv-item-meta">
                  <span class="bv-item-size">{{ fmtSize(b.size) }}</span>
                  <span class="bv-item-time">{{ b.modTime }}</span>
                </div>
              </div>
            </div>
          </template>
        </div>
      </aside>

      <!-- Right: detail -->
      <section class="bv-detail">
        <!-- full backup detail: VSCode-style read-only browser -->
        <template v-if="mode === 'full' && activeFull">
          <div class="bv-detail-head">
            <span class="bv-detail-id">{{ activeFull.timestamp }}</span>
            <a-tag color="cyan">整库克隆</a-tag>
            <span class="bv-detail-meta">{{ fmtSize(activeFull.size) }}</span>
          </div>
          <div class="fb-browser">
            <!-- left: file tree -->
            <div class="fb-tree">
              <div class="fb-tree-head">
                <span>文件</span>
                <span class="fb-tree-path" :title="activeFull.path">{{ activeFull.name }}</span>
              </div>
              <div class="fb-tree-body">
                <FileTreeNode
                  :nodes="tree['__root__'] || []"
                  :depth="0"
                  :tree="tree"
                  :expanded="expandedDirs"
                  :selected="selectedFile"
                  @toggle="toggleDir"
                  @open="openFile"
                />
                <div v-if="tree['__root__'] && !tree['__root__'].length" class="fb-tree-empty">空目录</div>
                <div v-if="!tree['__root__']" class="fb-tree-empty"><a-spin size="small" /></div>
              </div>
            </div>

            <!-- right: code viewer -->
            <div class="fb-code">
              <div class="fb-code-head">
                <span class="fb-code-path">{{ selectedFile || '选择左侧文件查看' }}</span>
                <span v-if="fileInfo.lines" class="fb-code-lines">{{ fileInfo.lines }} 行</span>
                <a-segmented
                  v-if="isMarkdown && selectedFile && !fileLoading"
                  v-model:value="mdViewMode"
                  :options="[
                    { label: '渲染', value: 'render' },
                    { label: '源码', value: 'source' },
                  ]"
                  size="small"
                  class="fb-md-toggle"
                />
              </div>
              <div class="fb-code-body">
                <div v-if="fileLoading" class="fb-code-state"><a-spin /></div>
                <div v-else-if="!selectedFile" class="fb-code-state">
                  <FileOutlined style="font-size:32px;color:var(--color-text-tertiary);opacity:0.4" />
                  <div style="margin-top:8px">只读浏览备份代码</div>
                </div>
                <img v-else-if="fileInfo.dataUrl" :src="fileInfo.dataUrl" style="max-width:100%;max-height:100%;object-fit:contain;padding:16px;display:block;margin:auto" />
                <div v-else-if="fileInfo.binary" class="fb-code-state">⚠️ 二进制文件，无法显示</div>
                <div v-else-if="fileInfo.notFound" class="fb-code-state">⚠️ 文件不存在</div>
                <MarkdownView
                  v-else-if="isMarkdown && mdViewMode === 'render'"
                  :source="fileContent"
                  :filename="selectedFile"
                  :resolve-image="resolveBackupImage"
                  class="fb-md-render"
                />
                <CodeViewer
                  v-else
                  :code="fileContent"
                  :filename="selectedFile"
                  :highlight-lines="[]"
                />
              </div>
            </div>
          </div>
        </template>

        <!-- archive detail -->
        <template v-else-if="mode === 'archive' && activeArchive">
          <div class="bv-detail-head">
            <span class="bv-detail-id">{{ activeArchive.name }}</span>
            <a-tag color="geekblue">打包压缩</a-tag>
            <span class="bv-detail-meta">{{ fmtSize(activeArchive.size) }}</span>
          </div>
          <div class="bv-detail-body">
            <div class="bd-row"><span class="bd-label">文件名</span><span class="bd-val" style="font-family:'Cascadia Code',monospace;font-size:12px">{{ activeArchive.name }}</span></div>
            <div class="bd-row"><span class="bd-label">大小</span><span class="bd-val">{{ fmtSize(activeArchive.size) }}</span></div>
            <div class="bd-row"><span class="bd-label">打包时间</span><span class="bd-val"><ClockCircleOutlined /> {{ activeArchive.modTime }}</span></div>
            <div class="bd-row"><span class="bd-label">路径</span><span class="bd-val" style="font-family:'Cascadia Code',monospace;font-size:11px;word-break:break-all">{{ activeArchive.path }}</span></div>
          </div>
        </template>

        <!-- no selection -->
        <div v-else class="bv-detail-empty">选择左侧的备份查看详情</div>
      </section>
    </div>

    <!-- archive pack modal (delegated to component) -->
    <BackupArchiveModal v-model:open="archiveModalOpen" @done="loadArchives" />

    <!-- right-click context menu on a backup node -->
    <ContextMenu
      v-model:open="ctxMenu.open"
      :x="ctxMenu.x"
      :y="ctxMenu.y"
      :title="ctxMenu.name"
    >
      <CtxItem @click="ctxReveal"><FolderOpenOutlined /> {{ revealLabel }}</CtxItem>
      <CtxItem danger @click="ctxDelete"><DeleteOutlined /> 删除此备份</CtxItem>
    </ContextMenu>
  </div>
</template>

<style scoped>
.bv-view {
  width: calc(100% + var(--spacing-lg) * 2);
  height: calc(100% + var(--spacing-lg) * 2);
  margin: calc(-1 * var(--spacing-lg));
  display: flex; flex-direction: column;
  min-height: 0;
  background: transparent;
}

/* toolbar — flush strip rather than floating */
.bv-top {
  display: flex; align-items: center; gap: var(--spacing-sm);
  flex-shrink: 0;
  height: 38px;
  padding: 0 var(--spacing-md);
  background: var(--color-surface);
  border-bottom: 1px solid var(--color-border);
}
.bv-spacer { flex: 1; }
.bv-count { font-size: 12px; color: var(--color-text-tertiary); }

/* empty */
.bv-empty { flex: 1; display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 8px; color: var(--color-text-tertiary); }
.bv-empty-title { font-size: 16px; font-weight: 600; color: var(--color-text-secondary); }
.bv-empty-desc { font-size: 13px; color: var(--color-text-tertiary); max-width: 320px; text-align: center; }

/* two columns */
.bv-cols { flex: 1; display: flex; gap: 1px; min-height: 0; background: var(--color-border); }

/* left list */
.bv-list { width: 300px; flex-shrink: 0; background: var(--color-surface); display: flex; flex-direction: column; }
.bv-list-head { display: flex; align-items: center; height: 34px; padding: 0 var(--spacing-md); border-bottom: 1px solid var(--color-border); font-size: 12px; font-weight: 600; color: var(--color-text-secondary); flex-shrink: 0; }
.bv-list-body { flex: 1; overflow-y: auto; padding: var(--spacing-xs); }
.bv-item { display: flex; gap: 8px; padding: 8px 10px; border-radius: var(--radius-sm); cursor: pointer; border-left: 2px solid transparent; transition: background var(--transition-fast); margin-bottom: 2px; }
.bv-item:hover { background: var(--color-hover); }
.bv-item.active { background: var(--color-primary-bg); border-left-color: var(--color-primary); }
.bv-icon { color: var(--color-primary); font-size: 14px; margin-top: 2px; flex-shrink: 0; }
.bv-item-body { flex: 1; min-width: 0; }
.bv-item-id { font-size: 12px; font-family: 'Cascadia Code', monospace; color: var(--color-text); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.bv-item-meta { display: flex; align-items: center; gap: 6px; margin-top: 3px; }
.bv-type-tag { margin: 0; font-size: 10px; line-height: 14px; padding: 0 4px; }
.bv-item-time { font-size: 10px; color: var(--color-text-tertiary); display: flex; align-items: center; gap: 3px; }
.bv-item-size { font-size: 10px; color: var(--color-text-tertiary); }

/* right detail */
.bv-detail { flex: 1; min-width: 0; background: var(--color-surface); display: flex; flex-direction: column; overflow: hidden; }
.bv-detail-head { display: flex; align-items: center; gap: 8px; height: 34px; padding: 0 var(--spacing-md); border-bottom: 1px solid var(--color-border); flex-shrink: 0; }
.bv-detail-id { font-size: 13px; font-weight: 600; color: var(--color-text); font-family: 'Cascadia Code', monospace; flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.bv-detail-body { flex: 1; overflow-y: auto; padding: var(--spacing-md); }

.bd-row { display: flex; align-items: baseline; gap: var(--spacing-sm); margin-bottom: 10px; }
.bd-label { font-size: 12px; color: var(--color-text-tertiary); width: 70px; flex-shrink: 0; }
.bd-val { font-size: 13px; color: var(--color-text); display: flex; align-items: center; gap: 4px; }

.bd-section-title { font-size: 12px; font-weight: 600; color: var(--color-text-secondary); margin: var(--spacing-md) 0 var(--spacing-sm); padding-bottom: 6px; border-bottom: 1px solid var(--color-border-light); }
.bd-refs { display: flex; flex-direction: column; gap: 4px; }
.bd-ref { display: flex; align-items: center; gap: var(--spacing-sm); padding: 4px 8px; background: var(--color-background); border-radius: var(--radius-xs); }
.bd-ref-name { flex: 1; font-size: 12px; font-family: 'Cascadia Code', monospace; color: var(--color-text-secondary); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.bd-ref-hash { font-size: 12px; font-family: 'Cascadia Code', monospace; color: var(--color-primary); flex-shrink: 0; }

.bv-detail-actions { display: flex; gap: var(--spacing-sm); padding: var(--spacing-sm) var(--spacing-md); border-top: 1px solid var(--color-border); flex-shrink: 0; }
.bv-detail-empty { flex: 1; display: flex; align-items: center; justify-content: center; color: var(--color-text-tertiary); font-size: 13px; }

/* full backup: detail head with inline meta + delete */
.bv-detail-meta { font-size: 11px; color: var(--color-text-tertiary); margin-left: 8px; }
.bv-head-del { margin-left: auto; }

/* VSCode-style file browser */
.fb-browser { flex: 1; display: flex; min-height: 0; }
.fb-tree { width: 260px; flex-shrink: 0; border-right: 1px solid var(--color-border); display: flex; flex-direction: column; min-height: 0; background: var(--color-background); }
.fb-tree-head { display: flex; align-items: center; gap: 8px; height: 34px; padding: 0 12px; font-size: 12px; font-weight: 600; color: var(--color-text-secondary); border-bottom: 1px solid var(--color-border); flex-shrink: 0; }
.fb-tree-path { margin-left: auto; font-family: 'Cascadia Code', monospace; font-weight: 400; font-size: 11px; color: var(--color-text-tertiary); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.fb-tree-body { flex: 1; overflow-y: auto; padding: 4px; }
.fb-tree-empty { padding: 24px 12px; text-align: center; font-size: 12px; color: var(--color-text-tertiary); }

/* code viewer pane */
.fb-code { flex: 1; min-width: 0; display: flex; flex-direction: column; min-height: 0; background: var(--color-surface); }
.fb-code-head { display: flex; align-items: center; gap: 8px; padding: 0 12px; height: 34px; border-bottom: 1px solid var(--color-border); flex-shrink: 0; background: var(--color-surface); }
.fb-md-toggle { margin-left: auto; }
.fb-code-path { font-family: 'Cascadia Code', monospace; font-size: 12px; color: var(--color-text); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.fb-code-lines { font-size: 11px; color: var(--color-text-tertiary); flex-shrink: 0; }
.fb-code-body { flex: 1; min-height: 0; overflow: hidden; position: relative; }
.fb-code-state { flex: 1; display: flex; flex-direction: column; align-items: center; justify-content: center; color: var(--color-text-tertiary); font-size: 13px; height: 100%; gap: 8px; }
.fb-code-body :deep(.cv-host) { height: 100%; }
.fb-md-render { height: 100%; overflow-y: auto; padding: var(--spacing-md); }
</style>
