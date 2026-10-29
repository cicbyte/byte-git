<script setup>
/**
 * 工作区浏览页 — 浏览当前仓库的代码文件。
 *
 * 解决「被迫开 IDE 只为看一眼代码」的痛点:纯只读浏览(文件树 + 搜索 +
 * 代码/Markdown 预览),不含编辑能力 —— 改代码指挥 AI 即可,看代码用本页。
 *
 * 与 RefSourcePanel 同构(文件树/搜索/CodeViewer 完全复用),数据源不同:
 *   - RefSourcePanel: 引用仓库的缓存路径
 *   - WorkspaceView:   当前仓库(GetCurrentRepo)
 */
import { ref, watch, computed, h, onBeforeUnmount } from 'vue'
import { message as AMessage } from 'ant-design-vue'
import {
  FileOutlined, FolderOpenOutlined,
  CodeOutlined, FileTextOutlined, EyeOutlined, SearchOutlined,
} from '@ant-design/icons-vue'
import CodeViewer from '@/components/common/CodeViewer.vue'
import MarkdownView from '@/components/common/MarkdownView.vue'
import FileTreeNode from '@/components/common/FileTreeNode.vue'
import { useRepoStore } from '@/stores/repo'
import { usePageFooter } from '@/composables/usePageFooter'

const repo = useRepoStore()
const app = window.go?.main?.ByteGitApp
const { setFooter, clearFooter } = usePageFooter()

// ---- tree state (lazy) ----
const tree = ref({})
const expandedDirs = ref(new Set())
const selectedFile = ref('')
const treeLoading = ref(false)

// ---- search state ----
const searchQuery = ref('')
const searchResults = ref([])
const searching = ref(false)
let searchTimer = null

// ---- file content ----
const fileContent = ref('')
const fileInfo = ref({ binary: false, notFound: false, lines: 0 })
const fileLoading = ref(false)
const mdViewMode = ref('render')

const isMarkdown = computed(() => /\.md$/i.test(selectedFile.value))

async function loadDir(subPath) {
  if (!app?.RepoFileList) return
  if (!(subPath in tree.value)) tree.value[subPath] = []
  try {
    const nodes = await app.RepoFileList(subPath)
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
    if (!(path in tree.value)) loadDir(path)
  }
}

async function openFile(node) {
  if (node.isDir) return
  if (!app?.RepoReadFile) return
  selectedFile.value = node.path
  fileLoading.value = true
  fileInfo.value = { binary: false, notFound: false, lines: 0 }
  fileContent.value = ''
  mdViewMode.value = 'render'
  try {
    const res = await app.RepoReadFile(node.path)
    fileInfo.value = { binary: !!res.binary, notFound: !!res.notFound, lines: res.lines || 0, dataUrl: res.dataUrl || '' }
    fileContent.value = res.content || ''
  } catch (e) {
    AMessage.error('读取文件失败: ' + String(e?.message || e))
  } finally {
    fileLoading.value = false
  }
}

// ---- search ----
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
  if (!q || !app?.RepoFileSearch) return
  searching.value = true
  try {
    searchResults.value = await app.RepoFileSearch(q) || []
  } catch (e) {
    AMessage.error('搜索失败: ' + String(e?.message || e))
    searchResults.value = []
  } finally {
    searching.value = false
  }
}

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

function dirOf(path) {
  const i = path.lastIndexOf('/')
  return i >= 0 ? path.slice(0, i) : ''
}

function resetBrowser() {
  tree.value = {}
  expandedDirs.value = new Set()
  selectedFile.value = ''
  fileContent.value = ''
  fileInfo.value = { binary: false, notFound: false, lines: 0 }
  searchQuery.value = ''
  searchResults.value = []
}

async function loadRoot() {
  resetBrowser()
  treeLoading.value = true
  await loadDir('')
  treeLoading.value = false
}

// initial load + reload on repo switch
loadRoot()
watch(() => repo.path, loadRoot)

const filename = computed(() => {
  const p = selectedFile.value
  const i = Math.max(p.lastIndexOf('/'), p.lastIndexOf('\\'))
  return i >= 0 ? p.slice(i + 1) : p
})

// inline footer: surfaces the current repo path so the page can drop its
// own header bar (mirrors ReferenceView's pattern).
// NOTE: must be defined BEFORE the watch with { immediate: true } below,
// otherwise the const is in TDZ when the immediate callback runs.
const RepoFooter = {
  props: ['path'],
  render() {
    return h('div', { style: 'display:flex;align-items:center;gap:8px;width:100%;font-size:12px;color:var(--color-text-tertiary)' }, [
      h(FolderOpenOutlined, { style: 'font-size:13px;color:var(--color-primary)' }),
      h('span', { style: 'font-weight:600;color:var(--color-text)' }, '工作区'),
      h('code', { style: 'font-family:Cascadia Code,monospace;font-size:11px;color:var(--color-text-tertiary);overflow:hidden;text-overflow:ellipsis;white-space:nowrap', title: this.path }, this.path),
    ])
  },
}

// footer: show current repo path (immediate, so RepoFooter must exist above)
watch(() => repo.path, (p) => {
  if (p) setFooter(RepoFooter, () => ({ path: p }))
}, { immediate: true })

onBeforeUnmount(() => {
  clearFooter()
  if (searchTimer) clearTimeout(searchTimer)
})
</script>

<template>
  <div class="ws-view">
    <!-- main split: tree/search + content -->
    <div class="ws-main">
      <!-- left: file tree / search -->
      <aside class="ws-tree">
        <div class="wt-head">
          <FolderOpenOutlined class="wt-icon" />
          <span>文件</span>
        </div>
        <div class="wt-search">
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
        <div class="wt-body">
          <!-- search results mode -->
          <div v-if="searchQuery.trim()">
            <div v-if="searching" class="wt-loading"><a-spin size="small" /></div>
            <div v-else-if="!searchResults.length" class="wt-empty">无匹配文件</div>
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
          <!-- tree mode -->
          <template v-else>
            <div v-if="treeLoading" class="wt-loading"><a-spin size="small" /></div>
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
            <div v-if="!treeLoading && !((tree['__root__'] || []).length)" class="wt-empty">
              暂无文件
            </div>
          </template>
        </div>
      </aside>

      <!-- right: file content (read-only) -->
      <div class="ws-content">
        <div class="wc-bar" v-if="selectedFile">
          <span class="wc-name" :title="selectedFile">
            <FileTextOutlined class="wc-name-icon" />
            {{ filename }}
          </span>
          <span class="wc-path">{{ selectedFile }}</span>
          <div class="wc-spacer"></div>
          <a-radio-group v-if="isMarkdown" v-model:value="mdViewMode" size="small" button-style="solid">
            <a-radio-button value="render"><EyeOutlined /> 渲染</a-radio-button>
            <a-radio-button value="source"><CodeOutlined /> 源码</a-radio-button>
          </a-radio-group>
        </div>

        <div class="wc-body">
          <div v-if="fileLoading" class="wc-loading"><a-spin tip="读取中..." /></div>
          <div v-else-if="!selectedFile" class="wc-placeholder">
            <FileOutlined class="wc-placeholder-icon" />
            <div>从左侧选择文件查看内容</div>
          </div>
          <div v-else-if="fileInfo.notFound" class="wc-state">该文件不存在</div>
          <img v-else-if="fileInfo.dataUrl" :src="fileInfo.dataUrl" style="max-width:100%;max-height:100%;object-fit:contain;padding:16px;display:block;margin:auto" />
          <div v-else-if="fileInfo.binary" class="wc-state">二进制文件，无法显示</div>
          <MarkdownView
            v-else-if="isMarkdown && mdViewMode === 'render'"
            :source="fileContent"
            :filename="selectedFile"
            class="wc-md"
          />
          <CodeViewer
            v-else
            :code="fileContent"
            :filename="filename"
            class="wc-code"
          />
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.ws-view { width: calc(100% + var(--spacing-lg) * 2); height: calc(100% + var(--spacing-lg) * 2); margin: calc(-1 * var(--spacing-lg)); display: flex; flex-direction: column; min-height: 0; background: transparent; }
.ws-main { flex: 1; display: flex; min-height: 0; gap: 1px; background: var(--color-border-light); }

/* left tree */
.ws-tree { width: 260px; flex-shrink: 0; background: var(--color-surface); display: flex; flex-direction: column; overflow: hidden; }
.wt-head {
  display: flex; align-items: center; gap: 8px;
  padding: 12px 14px; border-bottom: 1px solid var(--color-border-light);
  font-size: 13px; font-weight: 600; color: var(--color-text); flex-shrink: 0;
}
.wt-icon { color: var(--color-primary); }
.wt-search { padding: 8px 10px; border-bottom: 1px solid var(--color-border-light); flex-shrink: 0; }
.wt-body { flex: 1; overflow-y: auto; padding: 6px 4px; }
.wt-body::-webkit-scrollbar { width: 6px; }
.wt-body::-webkit-scrollbar-thumb { background: var(--color-border); border-radius: 3px; }
.wt-loading { padding: 20px; text-align: center; }
.wt-empty { padding: 16px; font-size: 12px; color: var(--color-text-tertiary); text-align: center; }

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
.ws-content { flex: 1; min-width: 0; background: var(--color-surface); display: flex; flex-direction: column; overflow: hidden; }
.wc-bar {
  display: flex; align-items: center; gap: 10px;
  padding: 8px 14px; border-bottom: 1px solid var(--color-border-light);
  flex-shrink: 0; font-size: 12px;
}
.wc-name { display: inline-flex; align-items: center; gap: 6px; font-weight: 600; color: var(--color-text); font-size: 13px; }
.wc-name-icon { color: var(--color-primary); }
.wc-path {
  font-family: 'Cascadia Code', monospace; font-size: 11px;
  color: var(--color-text-tertiary);
  overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
}
.wc-spacer { flex: 1; }

.wc-body { flex: 1; min-height: 0; overflow: hidden; display: flex; flex-direction: column; }
.wc-loading, .wc-placeholder, .wc-state {
  flex: 1; display: flex; flex-direction: column;
  align-items: center; justify-content: center; gap: 12px;
  color: var(--color-text-tertiary); font-size: 13px;
}
.wc-placeholder-icon { font-size: 36px; opacity: 0.4; }
.wc-md { flex: 1; overflow-y: auto; padding: var(--spacing-lg); }
.wc-code { flex: 1; overflow: hidden; }
</style>
