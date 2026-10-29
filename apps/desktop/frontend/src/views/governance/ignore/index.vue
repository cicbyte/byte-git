<script setup>
import { ref, computed, onMounted, nextTick } from 'vue'
import { useRouter } from 'vue-router'
import { message as AMessage, Modal } from 'ant-design-vue'
import {
  PlusOutlined, CheckCircleOutlined, FileTextOutlined,
  ReloadOutlined, GithubOutlined, EyeOutlined,
} from '@ant-design/icons-vue'
import AiSparkIcon from '@/components/common/AiSparkIcon.vue'
import CodeViewer from '@/components/common/CodeViewer.vue'
import IgnoreCategoriesModal from '@/components/ignore/IgnoreCategoriesModal.vue'
import IgnoreCheckModal from '@/components/ignore/IgnoreCheckModal.vue'
import IgnoredFilesDrawer from '@/components/ignore/IgnoredFilesDrawer.vue'
import IgnoreContextMenu from './components/IgnoreContextMenu.vue'
import IgnoreGithubPickerModal from './components/IgnoreGithubPickerModal.vue'

const app = window.go?.main?.ByteGitApp
const router = useRouter()

// ---- current .gitignore ----
const currentContent = ref('')
const contentLoading = ref(false)
async function loadGitignore() {
  contentLoading.value = true
  try {
    const res = await app.RepoReadFile('.gitignore')
    currentContent.value = res.notFound ? '' : res.content
  } catch { currentContent.value = '' }
  finally { contentLoading.value = false }
}

// ---- template data ----
const localTemplates = ref([])
const githubTemplates = ref([])
const githubCloned = ref(false)
const syncLoading = ref(false)

async function loadLocalList() {
  try { localTemplates.value = await app.IgnoreListTemplates() } catch {}
}
async function loadGithubList() {
  try {
    const status = await app.IgnoreSourcesStatus()
    const gh = status.find(s => s.name === 'github')
    githubCloned.value = gh?.cloned || false
    if (githubCloned.value) githubTemplates.value = await app.IgnoreListGithubTemplates()
  } catch {}
}

async function syncGithub() {
  syncLoading.value = true
  try {
    const res = await app.IgnoreGithubSync()
    AMessage.success(`同步完成，共 ${res.count} 个模板`)
    await loadGithubList()
  } catch (e) { AMessage.error('同步失败: ' + String(e?.message || e)) }
  finally { syncLoading.value = false }
}

// ---- add from template (local/github) ----
async function addFromTemplate(name, source) {
  addLoading.value = true
  diffData.value = null
  diffMode.value = 'add'
  try {
    const res = await app.IgnoreAddPreview([name], source)
    diffData.value = { original: res.original, updated: res.updated, added: res.added }
    pendingAdd = { langs: [name], source }
    if (res.added === 0) AMessage.info('所有规则已存在，无变更')
  } catch (e) { AMessage.error('预览失败: ' + String(e?.message || e)) }
  finally { addLoading.value = false }
}

// quick create from first local template (e.g. go)
async function quickAddLocal() {
  if (!localTemplates.value.length) {
    AMessage.info('无内置模板')
    return
  }
  // default to 'go' if available, otherwise first
  const lang = localTemplates.value.includes('go') ? 'go' : localTemplates.value[0]
  addFromTemplate(lang, 'local')
}

// ---- local template picker (右键菜单「本地模板」弹窗) ----
const localPickerOpen = ref(false)

function openLocalPicker() { localPickerOpen.value = true }

function pickLocalTemplate(name) {
  localPickerOpen.value = false
  addFromTemplate(name, 'local')
}

function goMgrFromPicker() {
  localPickerOpen.value = false
  router.push('/templates?tab=gitignore')
}

// ---- github template picker modal (UI state owned by IgnoreGithubPickerModal) ----
const ghPickerOpen = ref(false)

function openGithubPicker() {
  if (!githubCloned.value) { syncGithub() }
  ghPickerOpen.value = true
}

async function applyGhPick(names) {
  if (!names?.length) return
  ghPickerOpen.value = false
  addLoading.value = true
  diffData.value = null
  diffMode.value = 'add'
  try {
    const res = await app.IgnoreAddPreview(names, 'github')
    diffData.value = { original: res.original, updated: res.updated, added: res.added }
    pendingAdd = { langs: names, source: 'github' }
    if (res.added === 0) AMessage.info('所有规则已存在，无变更')
  } catch (e) { AMessage.error('预览失败: ' + String(e?.message || e)) }
  finally { addLoading.value = false }
}

// ---- add from AI (prompt for language) ----
const aiInputVisible = ref(false)
const aiLangInput = ref('')
let pendingAdd = null

function showAiInput() { aiInputVisible.value = true; aiLangInput.value = '' }

async function aiGenerate() {
  const langs = aiLangInput.value.split(',').map(s => s.trim()).filter(Boolean)
  if (!langs.length) return
  aiInputVisible.value = false
  addLoading.value = true
  diffData.value = null
  diffMode.value = 'add'
  try {
    const res = await app.IgnoreAddPreview(langs, 'ai')
    diffData.value = { original: res.original, updated: res.updated, added: res.added }
    pendingAdd = { langs, source: 'ai' }
    if (res.added === 0) AMessage.info('所有规则已存在，无变更')
  } catch (e) { AMessage.error('AI 生成失败: ' + String(e?.message || e)) }
  finally { addLoading.value = false }
}

// ---- confirm / cancel diff ----
async function confirmDiff() {
  if (diffMode.value === 'add' && pendingAdd) {
    try {
      const res = await app.IgnoreAddApply(pendingAdd.langs, pendingAdd.source)
      AMessage.success(`已添加 ${res.added} 条规则`)
    } catch (e) { AMessage.error('写入失败') }
  } else if (diffMode.value === 'refactor' && diffData.value) {
    // write the EXACT content shown in the preview — don't re-call AI
    try {
      await app.IgnoreWrite(diffData.value.updated)
      AMessage.success('已整理')
    } catch (e) { AMessage.error('写入失败') }
  }
  diffData.value = null
  pendingAdd = null
  await loadGitignore()
}
function cancelDiff() { diffData.value = null; pendingAdd = null }

// ---- category & check modals (delegated to sub-components) ----
const catOpen = ref(false)
const checkOpen = ref(false)

function openCategories() { catOpen.value = true }
function openCheck() { checkOpen.value = true }

// ---- ignored files drawer（防误忽略视图）----
const ignoredOpen = ref(false)
const codeViewerRef = ref(null)
const locateHighlight = ref([])

function onLocateRule(e) {
  ignoredOpen.value = false
  locateHighlight.value = [e.line]
  nextTick(() => codeViewerRef.value?.scrollToLine(e.line))
}

// 注释掉命中规则（根 .gitignore 行），使其不再生效；写盘后刷新编辑器
async function onCommentRule(e) {
  const lines = currentContent.value.split('\n')
  const idx = e.line - 1
  const raw = lines[idx] ?? ''
  const trimmed = raw.trim()
  if (!trimmed || trimmed.startsWith('#')) {
    AMessage.warning('该行已是注释或不存在（文件可能已变更），请刷新后重试')
    return
  }
  const indent = raw.slice(0, raw.length - raw.trimStart().length)
  lines[idx] = `${indent}# ${trimmed}`
  const next = lines.join('\n')
  try {
    await app.IgnoreWrite(next)
    currentContent.value = next
    dirty.value = false
    locateHighlight.value = []
    AMessage.success('已注释该规则，相关文件不再被忽略')
  } catch (err) {
    AMessage.error('写入失败: ' + String(err?.message || err))
  }
}

// ---- refactor ----
const refactorLoading = ref(false)
async function doRefactor() {
  refactorLoading.value = true
  diffData.value = null
  diffMode.value = 'refactor'
  try {
    const res = await app.IgnoreRefactorPreview()
    if (!res.updated || res.updated === res.original) {
      AMessage.info('内容已是最佳状态，无需整理')
    } else {
      diffData.value = { original: res.original, updated: res.updated, added: 0 }
    }
  } catch (e) {
    AMessage.error('整理失败: ' + String(e?.message || e))
  } finally {
    refactorLoading.value = false
  }
}

// ---- diff rendering (side-by-side) ----
const diffData = ref(null) // { original, updated, added }
const diffMode = ref('add') // 'add' | 'refactor'
const addLoading = ref(false)

// compute highlighted line numbers (1-based) for each side using line-level LCS.
// LCS aligns lines so that truly new lines are highlighted, even if their
// content matches a line elsewhere in the other side (handles repeats/order).
const leftHighlight = computed(() => {
  if (!diffData.value) return []
  return lcsDiffLines(diffData.value.original, diffData.value.updated).removed
})
const rightHighlight = computed(() => {
  if (!diffData.value) return []
  return lcsDiffLines(diffData.value.original, diffData.value.updated).added
})

// line-level LCS diff: returns { added: [1-based line nums in updated], removed: [in original] }
function lcsDiffLines(original, updated) {
  const a = (original || '').split('\n')
  const b = (updated || '').split('\n')
  const m = a.length, n = b.length

  // build LCS table
  const dp = []
  for (let i = 0; i <= m; i++) dp.push(new Int16Array(n + 1))
  for (let i = m - 1; i >= 0; i--) {
    for (let j = n - 1; j >= 0; j--) {
      dp[i][j] = a[i] === b[j] ? dp[i + 1][j + 1] + 1 : Math.max(dp[i + 1][j], dp[i][j + 1])
    }
  }

  // backtrack to find added/removed line indices
  const removed = []
  const added = []
  let i = 0, j = 0
  while (i < m && j < n) {
    if (a[i] === b[j]) {
      // common line — no highlight
      i++; j++
    } else if (dp[i + 1][j] >= dp[i][j + 1]) {
      // a[i] is removed (exists in original, not in updated at this position)
      removed.push(i + 1) // 1-based
      i++
    } else {
      // b[j] is added
      added.push(j + 1) // 1-based
      j++
    }
  }
  while (i < m) { removed.push(i + 1); i++ }
  while (j < n) { added.push(j + 1); j++ }

  return { removed, added }
}

// ---- template manager（独立页 /templates?tab=gitignore）----
function openMgr() { router.push('/templates?tab=gitignore') }

// ---- editor save (Ctrl+S) ----
const dirty = ref(false)
const saving = ref(false)

function onContentChange(newContent) {
  currentContent.value = newContent
  dirty.value = true
  locateHighlight.value = []
}

async function saveGitignore() {
  if (!dirty.value || saving.value) return
  saving.value = true
  try {
    await app.IgnoreWrite(currentContent.value)
    dirty.value = false
    AMessage.success('已保存')
  } catch (e) {
    AMessage.error('保存失败: ' + String(e?.message || e))
  } finally {
    saving.value = false
  }
}

function onKeydown(e) {
  if ((e.ctrlKey || e.metaKey) && e.key === 's') {
    e.preventDefault()
    saveGitignore()
  }
}

onMounted(async () => {
  await loadGitignore()
  await loadLocalList()
  await loadGithubList()
})
</script>

<template>
  <div class="ig-page">
    <!-- header bar -->
    <div class="ig-bar">
      <span class="ig-bar-title"><FileTextOutlined /> .gitignore</span>
      <span v-if="currentContent" class="ig-bar-lines">{{ currentContent.split('\n').length }} 行</span>
      <span class="ig-bar-hint">右键打开操作菜单</span>
      <div class="ig-bar-spacer" />
      <a-button size="small" type="text" @click="ignoredOpen = true"><EyeOutlined /> 已忽略</a-button>
      <a-button size="small" type="text" @click="loadGitignore" :loading="contentLoading"><ReloadOutlined /></a-button>
    </div>

    <!-- main: CodeViewer with context menu, or diff overlay -->
    <a-dropdown :trigger="['contextmenu']" placement="bottomLeft">
      <div class="ig-canvas">
        <!-- diff mode: side-by-side -->
        <div v-if="diffData" class="ig-diff">
          <div class="ig-diff-head">
            <span>变更预览 {{ diffData.added ? `（新增 ${diffData.added} 条）` : '' }}</span>
            <div class="ig-diff-acts">
              <a-button size="small" @click="cancelDiff">取消</a-button>
              <a-button size="small" type="primary" :loading="addLoading || refactorLoading" @click="confirmDiff">
                <CheckCircleOutlined /> 确认写入
              </a-button>
            </div>
          </div>
          <div class="ig-diff-cols">
            <div class="ig-diff-col">
              <div class="ig-diff-col-head">原始 <span class="hl-count">{{ leftHighlight.length }} 处移除</span></div>
              <CodeViewer :code="diffData.original || '(空)'" filename=".gitignore" :highlight-lines="leftHighlight" highlight-class="cv-diff-del" :show-context-menu="false" />
            </div>
            <div class="ig-diff-col">
              <div class="ig-diff-col-head updated">预览 <span class="hl-count">{{ rightHighlight.length }} 处新增</span></div>
              <CodeViewer :code="diffData.updated" filename=".gitignore" :highlight-lines="rightHighlight" highlight-class="cv-diff-add" :show-context-menu="false" />
            </div>
          </div>
        </div>

        <!-- loading -->
        <div v-else-if="addLoading || refactorLoading" class="ig-center">
          <a-spin :tip="diffMode === 'refactor' ? '整理中…' : '生成中…'" />
        </div>

        <!-- file view -->
        <div v-else-if="currentContent" class="ig-edit-wrap" @keydown="onKeydown">
          <CodeViewer ref="codeViewerRef" :code="currentContent" filename=".gitignore" :highlight-lines="locateHighlight" highlight-class="cv-locate" editable @update:modelValue="onContentChange" :show-context-menu="false" />
          <div v-if="dirty" class="ig-save-bar">
            <span class="ig-dirty-dot">● 未保存</span>
            <div class="ig-save-spacer" />
            <a-button size="small" type="primary" :loading="saving" @click="saveGitignore">
              保存 (Ctrl+S)
            </a-button>
          </div>
        </div>

        <!-- empty: no .gitignore — guided creation -->
        <div v-else-if="!contentLoading && !currentContent" class="ig-empty-guide">
          <FileTextOutlined style="font-size:48px;color:var(--color-primary);opacity:0.5" />
          <div class="ig-empty-title">无 .gitignore 文件</div>
          <div class="ig-empty-desc">选择一种方式快速创建</div>
          <div class="ig-empty-actions">
            <a-button type="primary" @click="quickAddLocal">
              <PlusOutlined /> 从内置模板创建
            </a-button>
            <a-button @click="openGithubPicker">
              <GithubOutlined /> 从 GitHub 模板
            </a-button>
            <a-button @click="showAiInput">
              <AiSparkIcon /> AI 生成
            </a-button>
          </div>
          <div class="ig-empty-hint">或右键空白处 → 添加规则</div>
        </div>
      </div>

      <!-- context menu -->
      <template #overlay>
        <IgnoreContextMenu
          :local-templates="localTemplates"
          :github-cloned="githubCloned"
          :github-template-count="githubTemplates.length"
          @open-local-picker="openLocalPicker"
          @open-github-picker="openGithubPicker"
          @show-ai-input="showAiInput"
          @open-mgr="openMgr"
          @do-refactor="doRefactor"
          @open-categories="openCategories"
          @open-check="openCheck"
          @open-ignored="ignoredOpen = true"
          @load-gitignore="loadGitignore"
        />
      </template>
    </a-dropdown>

    <!-- local template picker modal -->
    <a-modal v-model:open="localPickerOpen" title="本地模板" width="420px" :footer="null" destroy-on-close>
      <div v-if="localTemplates.length" class="ig-local-list">
        <a-button
          v-for="t in localTemplates"
          :key="t"
          size="small"
          @click="pickLocalTemplate(t)"
        >{{ t }}</a-button>
      </div>
      <a-empty v-else description="无内置模板" :image-style="{ height: '48px' }" style="margin:8px 0" />
      <div class="ig-local-foot">
        <a-button size="small" type="link" @click="goMgrFromPicker">管理模板（新建 / 编辑 / 删除）…</a-button>
      </div>
    </a-modal>

    <!-- GitHub template picker modal -->
    <IgnoreGithubPickerModal
      v-model:open="ghPickerOpen"
      :templates="githubTemplates"
      :github-cloned="githubCloned"
      :sync-loading="syncLoading"
      :add-loading="addLoading"
      @sync-github="syncGithub"
      @apply="applyGhPick"
    />

    <!-- AI input modal -->
    <a-modal v-model:open="aiInputVisible" title="AI 生成 .gitignore" width="420px" :footer="null" destroy-on-close>
      <div class="ai-form">
        <p style="font-size:13px;color:var(--color-text-secondary);margin:0 0 12px">
          输入语言/框架名（逗号分隔），AI 将生成对应的 .gitignore 规则。
        </p>
        <a-input v-model:value="aiLangInput" placeholder="如: python, go, node" @press-enter="aiGenerate" />
        <div style="display:flex;justify-content:flex-end;gap:8px;margin-top:12px">
          <a-button size="small" @click="aiInputVisible = false">取消</a-button>
          <a-button size="small" type="primary" @click="aiGenerate">生成</a-button>
        </div>
      </div>
    </a-modal>

    <!-- template manager modal → 独立页 /templates?tab=gitignore -->
    <!-- category rules modal -->
    <IgnoreCategoriesModal v-model:open="catOpen" />

    <!-- check file modal -->
    <IgnoreCheckModal v-model:open="checkOpen" />

    <!-- ignored files drawer（防误忽略） -->
    <IgnoredFilesDrawer v-model:open="ignoredOpen" @locate="onLocateRule" @comment-rule="onCommentRule" />
  </div>
</template>

<style scoped>
.ig-page { width: calc(100% + var(--spacing-lg) * 2); height: calc(100% + var(--spacing-lg) * 2); margin: calc(-1 * var(--spacing-lg)); display: flex; flex-direction: column; min-height: 0; background: transparent; }

/* local template picker modal */
.ig-local-list { display: flex; flex-wrap: wrap; gap: 8px; padding: 4px 0 8px; }
.ig-local-foot { display: flex; justify-content: flex-end; border-top: 1px solid var(--color-border-light); padding-top: 8px; }

/* top bar */
.ig-bar { display: flex; align-items: center; gap: 8px; padding: 0 12px; height: 34px; border-bottom: 1px solid var(--color-border); flex-shrink: 0; }
.ig-bar-title { font-size: 13px; font-weight: 600; color: var(--color-text); display: flex; align-items: center; gap: 6px; }
.ig-bar-lines { font-size: 11px; color: var(--color-text-tertiary); }
.ig-bar-hint { font-size: 10px; color: var(--color-text-tertiary); opacity: 0.6; }
.ig-bar-spacer { flex: 1; }

/* canvas (fills remaining) */
.ig-canvas { flex: 1; min-height: 0; overflow: hidden; position: relative; }
.ig-canvas :deep(.cv-host) { height: 100%; }
.ig-edit-wrap { height: 100%; display: flex; flex-direction: column; position: relative; }
.ig-edit-wrap :deep(.cv-host) { flex: 1; min-height: 0; }
/* 定位规则行高亮（主题色，区别于泄漏行的红色） */
.ig-edit-wrap :deep(.cv-locate) { background: rgba(var(--color-primary-rgb), 0.14); border-left: 3px solid var(--color-primary); }
.ig-save-bar { display: flex; align-items: center; gap: 8px; padding: 4px 12px; border-top: 1px solid var(--color-border); background: var(--color-warning-bg); flex-shrink: 0; }
.ig-dirty-dot { font-size: 11px; color: var(--color-warning); font-weight: 600; }
.ig-save-spacer { flex: 1; }
.ig-center { flex: 1; display: flex; flex-direction: column; align-items: center; justify-content: center; height: 100%; gap: 8px; color: var(--color-text-tertiary); font-size: 13px; }

/* empty guided creation */
.ig-empty-guide { flex: 1; display: flex; flex-direction: column; align-items: center; justify-content: center; height: 100%; gap: 10px; color: var(--color-text-tertiary); }
.ig-empty-title { font-size: 18px; font-weight: 600; color: var(--color-text-secondary); margin-top: 8px; }
.ig-empty-desc { font-size: 13px; color: var(--color-text-tertiary); }
.ig-empty-actions { display: flex; gap: 12px; margin-top: 12px; }
.ig-empty-hint { font-size: 11px; color: var(--color-text-tertiary); opacity: 0.6; margin-top: 8px; }

/* diff overlay */
.ig-diff { height: 100%; display: flex; flex-direction: column; }
.ig-diff-head { display: flex; align-items: center; justify-content: space-between; padding: 6px 12px; background: var(--color-primary-bg); border-bottom: 1px solid var(--color-border); font-size: 12px; color: var(--color-text-secondary); flex-shrink: 0; }
.ig-diff-acts { display: flex; gap: 6px; }
.ig-diff-cols { flex: 1; display: flex; gap: 1px; background: var(--color-border); min-height: 0; }
.ig-diff-col { flex: 1; min-width: 0; display: flex; flex-direction: column; background: var(--color-surface); }
.ig-diff-col-head { padding: 4px 12px; font-size: 11px; font-weight: 600; color: var(--color-text-tertiary); border-bottom: 1px solid var(--color-border); flex-shrink: 0; text-transform: uppercase; letter-spacing: 0.5px; }
.ig-diff-col-head.updated { color: var(--color-primary); }
.ig-diff-col :deep(.cv-host) { flex: 1; min-height: 0; height: auto; }
/* diff highlight: green for added (right), red for removed (left) */
.ig-diff-col :deep(.cv-diff-add) { background: rgba(80,250,123,0.12); border-left: 3px solid var(--color-success); }
.ig-diff-col :deep(.cv-diff-del) { background: rgba(255,85,85,0.12); border-left: 3px solid var(--color-error); }
.hl-count { font-weight: 400; font-size: 10px; opacity: 0.7; margin-left: 4px; }

/* AI input modal */
.ai-form { padding: 4px 0; }
</style>

<!-- unscoped: ant-design menu/modal render to body -->
<style>
/* cap the GitHub template submenu so 300+ items scroll instead of stretching */
.ant-menu-submenu-popup .ant-menu-vertical,
.ant-menu-submenu-popup .ant-menu-sub {
  max-height: 320px;
  overflow-y: auto;
}
</style>
