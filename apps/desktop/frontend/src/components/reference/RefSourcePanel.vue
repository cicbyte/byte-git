<script setup>
/**
 * 源码浏览面板 — 展示引用仓库缓存中的代码文件。
 *
 * 左侧文件树(懒加载目录,VSCode explorer 风格) + 内置搜索框
 * (搜索时切换为扁平结果列表,模糊匹配文件名),右侧代码/Markdown 预览。
 * 后端走 ReferenceFileList/Read/Search,直接读取磁盘上的缓存仓库
 * (remote → CachePath;local → Source 原路径)。
 */
import { ref, watch, computed, onBeforeUnmount } from 'vue'
import { message as AMessage } from 'ant-design-vue'
import {
  FileOutlined, FolderOutlined, FolderOpenOutlined,
  CodeOutlined, FileTextOutlined, EyeOutlined, SearchOutlined,
  CopyOutlined, FolderOpenFilled,
} from '@ant-design/icons-vue'
import CodeViewer from '../common/CodeViewer.vue'
import MarkdownView from '../common/MarkdownView.vue'
import FileTreeNode from '../common/FileTreeNode.vue'

const props = defineProps({
  repoName: { type: String, default: '' },
})

const app = window.go?.main?.ByteGitApp

// ---- tree state (lazy) ----
// tree: { [path]: FileNode[] } — root uses '__root__' key
const tree = ref({})
const expandedDirs = ref(new Set())
const selectedFile = ref('')
const treeLoading = ref(false)

// ---- search state ----
// when searchQuery is non-empty, the panel swaps the tree view for a flat
// results list (debounced backend walk via ReferenceFileSearch). clearing the
// query restores the tree at its previous expand/collapse state.
const searchQuery = ref('')
const searchResults = ref([])
const searching = ref(false)
let searchTimer = null

// ---- file content ----
const fileContent = ref('')
const fileInfo = ref({ binary: false, notFound: false, lines: 0 })
const fileLoading = ref(false)
// markdown render/source toggle (only for .md files)
const mdViewMode = ref('render') // 'render' | 'source'

const isMarkdown = computed(() => /\.md$/i.test(selectedFile.value))

// resolve a relative image inside the referenced repo cache → data URL, for
// MarkdownView's local-image inlining. The cache lives outside the current
// repo, so read via ReferenceFileRead instead of RepoReadImage.
async function resolveRefImage(rel) {
  if (!app?.ReferenceFileRead || !props.repoName) return ''
  try {
    const r = await app.ReferenceFileRead(props.repoName, rel)
    return r.dataUrl || ''
  } catch { return '' }
}

async function loadDir(subPath) {
  if (!app?.ReferenceFileList || !props.repoName) return
  // mark this dir as loading by setting an empty array first
  if (!(subPath in tree.value)) tree.value[subPath] = []
  try {
    const nodes = await app.ReferenceFileList(props.repoName, subPath)
    tree.value = { ...tree.value, [subPath || '__root__']: nodes }
  } catch (e) {
    tree.value[subPath || '__root__'] = []
    AMessage.error('读取目录失败: ' + String(e?.message || e))
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
    // lazy-load: fetch children on first expand
    if (!(path in tree.value)) loadDir(path)
  }
}

async function openFile(node) {
  // FileTreeNode emits `open` for both files and dirs; dirs are handled by
  // @toggle, so skip them here.
  if (node.isDir) return
  if (!app?.ReferenceFileRead || !props.repoName) return
  selectedFile.value = node.path
  fileLoading.value = true
  fileInfo.value = { binary: false, notFound: false, lines: 0 }
  fileContent.value = ''
  mdViewMode.value = 'render' // reset to render mode on each open
  try {
    const res = await app.ReferenceFileRead(props.repoName, node.path)
    fileInfo.value = { binary: !!res.binary, notFound: !!res.notFound, lines: res.lines || 0 }
    fileContent.value = res.content || ''
  } catch (e) {
    AMessage.error('读取文件失败: ' + String(e?.message || e))
  } finally {
    fileLoading.value = false
  }
}

// ---- search ----
// debounced: user types → wait 300ms → backend walk → flat result list.
// clearing the input restores the tree view without losing expand state.
function onSearchInput(v) {
  searchQuery.value = v
  if (searchTimer) clearTimeout(searchTimer)
  if (!v.trim()) {
    searchResults.value = []
    searching.value = false
    return
  }
  searchTimer = setTimeout(runSearch, 300)
}

async function runSearch() {
  const q = searchQuery.value.trim()
  if (!q || !app?.ReferenceFileSearch) return
  searching.value = true
  try {
    searchResults.value = await app.ReferenceFileSearch(props.repoName, q) || []
  } catch (e) {
    AMessage.error('搜索失败: ' + String(e?.message || e))
    searchResults.value = []
  } finally {
    searching.value = false
  }
}

// highlight matched substring within a filename (returns HTML segments)
function highlight(name) {
  const q = searchQuery.value.trim().toLowerCase()
  if (!q) return [{ text: name, match: false }]
  const lower = name.toLowerCase()
  const idx = lower.indexOf(q)
  if (idx < 0) return [{ text: name, match: false }]
  return [
    { text: name.slice(0, idx), match: false },
    { text: name.slice(idx, idx + q.length), match: true },
    { text: name.slice(idx + q.length), match: false },
  ]
}

// display the directory part of a path (everything except the filename),
// muted — helps disambiguate same-named files in different folders
function dirOf(path) {
  const i = path.lastIndexOf('/')
  return i >= 0 ? path.slice(0, i) : ''
}

// reset everything when the selected repo changes
function resetBrowser() {
  tree.value = {}
  expandedDirs.value = new Set()
  selectedFile.value = ''
  fileContent.value = ''
  fileInfo.value = { binary: false, notFound: false, lines: 0 }
  searchQuery.value = ''
  searchResults.value = []
}

// initial load + reset on repo change
watch(
  () => props.repoName,
  async (name) => {
    resetBrowser()
    if (!name) return
    treeLoading.value = true
    await loadDir('') // loads root
    treeLoading.value = false
  },
  { immediate: true },
)

const filename = computed(() => {
  const p = selectedFile.value
  const i = Math.max(p.lastIndexOf('/'), p.lastIndexOf('\\'))
  return i >= 0 ? p.slice(i + 1) : p
})

// ---- toolbar actions (path copy / reveal / send to terminal) ----
// these need the absolute OS path of the cached file, fetched once per action
// from ReferenceCachePath. base64 + TerminalInput mirrors the LogView /
// CommandPalette pattern for piping a command into the live PTY.

async function copyPath() {
  if (!app?.ReferenceCachePath) return
  try {
    const abs = await app.ReferenceCachePath(props.repoName, selectedFile.value)
    await navigator.clipboard.writeText(abs)
    AMessage.success('已复制路径')
  } catch (e) {
    AMessage.error('复制失败: ' + String(e?.message || e))
  }
}

async function revealInExplorer() {
  if (!app?.ReferenceCachePath || !app?.OpenInExplorer) return
  try {
    const abs = await app.ReferenceCachePath(props.repoName, selectedFile.value)
    await app.OpenInExplorer(abs)
  } catch (e) {
    AMessage.error('打开失败: ' + String(e?.message || e))
  }
}

// clean up pending search timer to avoid a stale callback firing after the
// panel is destroyed (e.g. switching to a different reference repo)
onBeforeUnmount(() => {
  if (searchTimer) clearTimeout(searchTimer)
})
</script>

<template>
  <div class="src-panel">
    <!-- left: file tree / search results -->
    <aside class="sp-tree">
      <div class="st-head">
        <FolderOpenOutlined class="st-icon" />
        <span>文件</span>
      </div>
      <!-- search box -->
      <div class="st-search">
        <a-input
          :value="searchQuery"
          placeholder="搜索文件名..."
          size="small"
          allow-clear
          @change="(e) => onSearchInput(e.target.value)"
        >
          <template #prefix><SearchOutlined style="color: var(--color-text-tertiary)" /></template>
        </a-input>
      </div>
      <div class="st-body">
        <!-- search results mode -->
        <div v-if="searchQuery.trim()">
          <div v-if="searching" class="st-loading"><a-spin size="small" /></div>
          <div v-else-if="!searchResults.length" class="st-empty">无匹配文件</div>
          <div v-else class="sr-list">
            <div
              v-for="r in searchResults" :key="r.path"
              class="sr-item"
              :class="{ active: selectedFile === r.path }"
              @click="openFile(r)"
            >
              <FileTextOutlined class="sr-icon" />
              <div class="sr-body">
                <div class="sr-name">
                  <template v-for="(seg, i) in highlight(r.name)" :key="i">
                    <mark v-if="seg.match" class="sr-mark">{{ seg.text }}</mark>
                    <span v-else>{{ seg.text }}</span>
                  </template>
                </div>
                <div class="sr-dir" v-if="dirOf(r.path)">{{ dirOf(r.path) }}</div>
              </div>
            </div>
            <div v-if="searchResults.length >= 500" class="sr-cap">
              结果过多,仅显示前 500 条,请细化关键词
            </div>
          </div>
        </div>
        <!-- tree mode (default) -->
        <template v-else>
          <div v-if="treeLoading" class="st-loading"><a-spin size="small" /></div>
          <FileTreeNode
            v-else
            :nodes="tree['__root__'] || []"
            :depth="0"
            :tree="tree"
            :expanded="expandedDirs"
            :selected="selectedFile"
            @toggle="toggleDir"
            @open="openFile"
          />
          <div v-if="!treeLoading && !((tree['__root__'] || []).length)" class="st-empty">
            暂无文件
          </div>
        </template>
      </div>
    </aside>

    <!-- right: file content -->
    <div class="sp-content">
      <!-- toolbar: filename + md view toggle + actions -->
      <div class="sc-bar" v-if="selectedFile">
        <span class="sc-name" :title="selectedFile">
          <FileTextOutlined class="sc-name-icon" />
          {{ filename }}
        </span>
        <span class="sc-path">{{ selectedFile }}</span>
        <div class="sc-spacer"></div>
        <!-- per-file actions: copy path / reveal / send to terminal -->
        <a-tooltip title="复制绝对路径">
          <a-button size="small" type="text" @click="copyPath">
            <template #icon><CopyOutlined /></template>
          </a-button>
        </a-tooltip>
        <a-tooltip title="在文件管理器中显示">
          <a-button size="small" type="text" @click="revealInExplorer">
            <template #icon><FolderOpenFilled /></template>
          </a-button>
        </a-tooltip>
        <a-radio-group v-if="isMarkdown" v-model:value="mdViewMode" size="small" button-style="solid" style="margin-left: 8px">
          <a-radio-button value="render"><EyeOutlined /> 渲染</a-radio-button>
          <a-radio-button value="source"><CodeOutlined /> 源码</a-radio-button>
        </a-radio-group>
      </div>

      <!-- content area -->
      <div class="sc-body">
        <div v-if="fileLoading" class="sc-loading"><a-spin tip="读取中..." /></div>
        <div v-else-if="!selectedFile" class="sc-placeholder">
          <FileOutlined class="sc-placeholder-icon" />
          <div>从左侧选择文件查看内容</div>
        </div>
        <div v-else-if="fileInfo.notFound" class="sc-state">该文件不存在</div>
        <div v-else-if="fileInfo.binary" class="sc-state">二进制文件，无法显示</div>
        <!-- markdown render mode -->
        <MarkdownView
          v-else-if="isMarkdown && mdViewMode === 'render'"
          :source="fileContent"
          :filename="selectedFile"
          :resolve-image="resolveRefImage"
          class="sc-md"
        />
        <!-- source / code -->
        <CodeViewer
          v-else
          :code="fileContent"
          :filename="filename"
          class="sc-code"
        />
      </div>
    </div>
  </div>
</template>

<style scoped>
.src-panel { display: flex; height: 100%; min-height: 0; gap: 1px; background: var(--color-border-light); }

/* left tree */
.sp-tree { width: 260px; flex-shrink: 0; background: var(--color-surface); display: flex; flex-direction: column; overflow: hidden; }
.st-head {
  display: flex; align-items: center; gap: 8px;
  padding: 12px 14px; border-bottom: 1px solid var(--color-border-light);
  font-size: 13px; font-weight: 600; color: var(--color-text); flex-shrink: 0;
}
.st-icon { color: var(--color-primary); }
.st-body { flex: 1; overflow-y: auto; padding: 6px 4px; }
.st-body::-webkit-scrollbar { width: 6px; }
.st-body::-webkit-scrollbar-thumb { background: var(--color-border); border-radius: 3px; }
.st-loading { padding: 20px; text-align: center; }
.st-empty { padding: 16px; font-size: 12px; color: var(--color-text-tertiary); text-align: center; }

/* search box */
.st-search { padding: 8px 10px; border-bottom: 1px solid var(--color-border-light); flex-shrink: 0; }

/* search results */
.sr-list { display: flex; flex-direction: column; gap: 1px; }
.sr-item {
  display: flex; gap: 8px; padding: 6px 10px; cursor: pointer;
  border-radius: var(--radius-xs); transition: background var(--transition-fast);
}
.sr-item:hover { background: var(--color-hover); }
.sr-item.active { background: var(--color-primary-bg); }
.sr-icon { font-size: 13px; color: var(--color-text-tertiary); flex-shrink: 0; margin-top: 2px; }
.sr-item.active .sr-icon { color: var(--color-primary); }
.sr-body { flex: 1; min-width: 0; }
.sr-name { font-size: 12px; color: var(--color-text); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.sr-item.active .sr-name { color: var(--color-primary); font-weight: 500; }
.sr-mark { background: var(--color-accent-bg, rgba(255, 121, 198, 0.25)); color: var(--color-accent, #ff79c6); padding: 0 1px; border-radius: 2px; }
.sr-dir {
  font-size: 10px; color: var(--color-text-tertiary); margin-top: 1px;
  overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
  font-family: 'Cascadia Code', monospace;
}
.sr-cap { padding: 8px 10px; font-size: 10px; color: var(--color-text-tertiary); text-align: center; font-style: italic; }

/* right content */
.sp-content { flex: 1; min-width: 0; background: var(--color-surface); display: flex; flex-direction: column; overflow: hidden; }
.sc-bar {
  display: flex; align-items: center; gap: 10px;
  padding: 8px 14px; border-bottom: 1px solid var(--color-border-light);
  flex-shrink: 0; font-size: 12px;
}
.sc-name { display: inline-flex; align-items: center; gap: 6px; font-weight: 600; color: var(--color-text); font-size: 13px; }
.sc-name-icon { color: var(--color-primary); }
.sc-path {
  font-family: 'Cascadia Code', monospace; font-size: 11px;
  color: var(--color-text-tertiary);
  overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
}
.sc-spacer { flex: 1; }

.sc-body { flex: 1; min-height: 0; overflow: hidden; display: flex; flex-direction: column; }
.sc-loading, .sc-placeholder, .sc-state {
  flex: 1; display: flex; flex-direction: column;
  align-items: center; justify-content: center; gap: 12px;
  color: var(--color-text-tertiary); font-size: 13px;
}
.sc-placeholder-icon { font-size: 36px; opacity: 0.4; }
.sc-md { flex: 1; overflow-y: auto; padding: var(--spacing-lg); }
.sc-code { flex: 1; overflow: hidden; }
</style>
