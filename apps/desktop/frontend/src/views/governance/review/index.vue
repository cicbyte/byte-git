<script setup>
import { ref, computed, onMounted, onBeforeUnmount, watch, nextTick } from 'vue'
import { message as AMessage } from 'ant-design-vue'
import {
  PlayCircleOutlined, FileSearchOutlined, CheckCircleOutlined,
  SettingOutlined, WarningOutlined, CopyOutlined,
} from '@ant-design/icons-vue'
import { useRepoStore } from '@/stores/repo'
import { relTime } from '@/composables/useRelativeTime'
import { useToolStatus } from '@/composables/useToolStatus'
import { fadeIn } from '@/composables/useGsapAnimations'
import { EventsOn, EventsOff } from '@bridge/runtime/runtime'
import CommitTree from '@/components/common/CommitTree.vue'
import DiffViewer from '@/components/common/DiffViewer.vue'
import MarkdownView from '@/components/common/MarkdownView.vue'
import ReviewAdvancedModal from './components/ReviewAdvancedModal.vue'

/**
 * 代码审查 — 三区布局，状态切换只动右栏：
 *   顶栏：模式切换 + 高级选项 + 开始审查
 *   左栏：范围选择（workspace=改动摘要 / range/commit=CommitTree）
 *   右栏：未运行→引导 / 运行中→分步进度 / 完成→双栏结果（文件树 + 评论流）
 */
const repo = useRepoStore()
const app = window.go?.main?.ByteGitApp

// ---- OCR availability (shared cache with the global footer) ----
const { ocr: ocrStatus, ensureLoaded } = useToolStatus()

// ---- commit tree data (LogFetch pagination) ----
const commits = ref([])
const loadingCommits = ref(false)
const hasMore = ref(true)
const totalCommits = ref(0)
const PAGE = 50

async function loadCommits() {
  if (!app) return
  loadingCommits.value = true
  commits.value = []
  hasMore.value = true
  try {
    if (app.CommitTotal) totalCommits.value = await app.CommitTotal()
    const data = await app.LogFetch(repo.repoArg(), PAGE, '')
    commits.value = data || []
    if ((data || []).length < PAGE) hasMore.value = false
  } catch {
    /* keep empty */
  } finally {
    loadingCommits.value = false
  }
}

async function onLoadMore(done) {
  if (!app || !hasMore.value || !commits.value.length) { done?.(); return }
  try {
    const data = await app.LogFetch(repo.repoArg(), PAGE, String(commits.value.length))
    const arr = data || []
    if (arr.length) commits.value.push(...arr)
    if (arr.length < PAGE) hasMore.value = false
  } catch {
    hasMore.value = false
  } finally {
    done?.()
  }
}

function commitNum(index) {
  return totalCommits.value - index
}

// ---- workspace change summary (left column in workspace mode) ----
const wsStatus = ref({ staged: [], unstaged: [], untracked: [] })
const wsLoading = ref(false)
const wsFiles = computed(() => [
  ...wsStatus.value.staged, ...wsStatus.value.unstaged, ...wsStatus.value.untracked,
])
async function loadWsStatus() {
  if (!app?.CommitStatus) return
  wsLoading.value = true
  try {
    const r = await app.CommitStatus(repo.repoArg())
    wsStatus.value = { staged: r.staged || [], unstaged: r.unstaged || [], untracked: r.untracked || [] }
  } catch {
    wsStatus.value = { staged: [], unstaged: [], untracked: [] }
  } finally {
    wsLoading.value = false
  }
}

// ---- review mode ----
const mode = ref('workspace')
const selectedHashes = ref(new Set())
const activeHash = ref('')

const selectedList = computed(() => commits.value.filter((c) => selectedHashes.value.has(c.hash)))
const rangePreview = computed(() => {
  const arr = selectedList.value
  if (arr.length < 2) return null
  return { from: arr[arr.length - 1], to: arr[0], count: arr.length }
})
const activeCommit = computed(() => commits.value.find((c) => c.hash === activeHash.value))

// re-fetch left-column data when the mode changes
watch(mode, (m) => {
  if (m === 'workspace') loadWsStatus()
  else if (!commits.value.length) loadCommits()
})

// ---- advanced options (modal) ----
const advModalOpen = ref(false)
const model = ref('')
const background = ref('')

// ---- review result + progress ----
const running = ref(false)
const result = ref(null)
// stepwise progress (from backend "review:progress" events) + elapsed timer
const progress = ref({ stage: '', message: '' })
const elapsed = ref(0)
let elapsedTimer = null
// ref to the progress panel so we can fadeIn when review starts
const progressRef = ref(null)
// when a review starts, fade-in the progress panel (after the DOM updates)
watch(running, (v) => {
  if (v) nextTick(() => fadeIn(progressRef.value?.$el || progressRef.value))
})

const STAGES = [
  { key: 'collecting', label: '收集改动' },
  { key: 'analyzing', label: 'AI 分析' },
  { key: 'parsing', label: '解析结果' },
]
const stageIndex = computed(() => {
  const i = STAGES.findIndex((s) => s.key === progress.value.stage)
  return i < 0 ? 0 : i
})

function onReviewProgress(ev) {
  if (ev?.stage) progress.value = { stage: ev.stage, message: ev.message || '' }
}

const canRun = computed(() => {
  if (mode.value === 'workspace') return true
  if (mode.value === 'range') return (selectedHashes.value.size || 0) >= 2
  if (mode.value === 'commit') return !!activeHash.value
  return false
})
const runDisabledReason = computed(() => {
  if (!ocrStatus.value.available) return '未检测到 OCR'
  if (mode.value === 'range' && (selectedHashes.value.size || 0) < 2) return '请至少选择 2 条提交'
  if (mode.value === 'commit' && !activeHash.value) return '请选择一条提交'
  return ''
})

async function runReview() {
  if (!app?.ReviewRun) return
  if (!ocrStatus.value.available) {
    AMessage.warning('未检测到 OCR，请在设置中安装或指定路径')
    return
  }
  if (!canRun.value) {
    AMessage.warning(runDisabledReason.value)
    return
  }
  running.value = true
  result.value = null
  progress.value = { stage: 'collecting', message: '收集改动差异…' }
  elapsed.value = 0
  elapsedTimer = setInterval(() => { elapsed.value += 1 }, 1000)

  const dto = { from: '', to: '', commit: '', model: model.value, background: background.value }
  if (mode.value === 'range' && rangePreview.value) {
    dto.from = rangePreview.value.from.hash
    dto.to = rangePreview.value.to.hash
  } else if (mode.value === 'commit' && activeHash.value) {
    dto.commit = activeHash.value
  }
  try {
    result.value = await app.ReviewRun(dto)
    if (fileGroups.value.length) activePath.value = fileGroups.value[0].path
    if (result.value.comments?.length) {
      AMessage.success(`审查完成，发现 ${result.value.comments.length} 条意见`)
    } else {
      AMessage.info(result.value.message || '审查完成，无意见')
    }
  } catch (e) {
    AMessage.error('审查失败: ' + String(e?.message || e))
  } finally {
    running.value = false
    if (elapsedTimer) { clearInterval(elapsedTimer); elapsedTimer = null }
  }
}

async function cancelReview() {
  try {
    if (app?.CancelTask) await app.CancelTask()
  } catch {}
  running.value = false
  if (elapsedTimer) { clearInterval(elapsedTimer); elapsedTimer = null }
  AMessage.info('审查已取消')
}

// ---- result shaping ----
// group comments by file, sorted by comment count desc (most problems first)
const fileGroups = computed(() => {
  if (!result.value?.comments) return []
  const map = new Map()
  for (const c of result.value.comments) {
    if (!map.has(c.path)) map.set(c.path, [])
    map.get(c.path).push(c)
  }
  const arr = [...map.entries()].map(([path, comments]) => ({ path, comments }))
  arr.sort((a, b) => b.comments.length - a.comments.length)
  return arr
})

const activePath = ref('')
const activeComments = computed(() =>
  activePath.value ? (fileGroups.value.find((g) => g.path === activePath.value)?.comments || []) : []
)

function fmtToken(n) {
  if (!n) return '0'
  if (n >= 1000) return (n / 1000).toFixed(1) + 'k'
  return String(n)
}
function locStr(c) {
  if (!c.start_line) return c.path
  if (c.end_line > c.start_line) return `${c.path}:${c.start_line}-${c.end_line}`
  return `${c.path}:${c.start_line}`
}
function shortPath(p) {
  // show last 2 segments when long, full path on hover via title
  const parts = p.split('/')
  if (parts.length <= 2) return p
  return '…/' + parts.slice(-2).join('/')
}
async function copyText(t, hint = '已复制') {
  if (!t) return
  try { await navigator.clipboard.writeText(t); AMessage.success(hint) } catch {}
}

const statusTag = computed(() => {
  if (!result.value) return null
  const s = result.value.status
  if (s === 'success') return { color: 'green', text: '成功', bar: 'var(--color-success)' }
  if (s === 'completed_with_warnings') return { color: 'orange', text: '有警告', bar: 'var(--color-warning)' }
  if (s === 'completed_with_errors') return { color: 'red', text: '有错误', bar: 'var(--color-error)' }
  if (s === 'skipped') return { color: 'default', text: '跳过', bar: 'var(--color-text-tertiary)' }
  return { color: 'default', text: s, bar: 'var(--color-text-tertiary)' }
})

onMounted(() => {
  ensureLoaded()
  loadWsStatus() // workspace is the default mode
  EventsOn('review:progress', onReviewProgress)
})
onBeforeUnmount(() => {
  EventsOff('review:progress', onReviewProgress)
  if (elapsedTimer) clearInterval(elapsedTimer)
})

// reload on repo switch
watch(() => repo.repoEpoch, () => {
  loadWsStatus()
  loadCommits()
})
</script>

<template>
  <div class="rv-view">
    <!-- OCR status banner (missing only — OK state lives in footer) -->
    <a-alert
      v-if="!ocrStatus.available"
      type="warning"
      show-icon
      message="未检测到 OpenCodeReview"
      :description="ocrStatus.message + '。安装: npm i -g @alibaba-group/open-code-review，或在设置 → 代码审查中指定路径'"
    />

    <!-- top bar -->
    <div class="rv-top">
      <a-segmented v-model:value="mode" :options="[
        { label: '工作区', value: 'workspace' },
        { label: '提交范围', value: 'range' },
        { label: '单个提交', value: 'commit' },
      ]" size="small" />
      <div class="rv-spacer"></div>
      <a-button size="small" type="text" @click="advModalOpen = true" title="高级选项">
        <template #icon><SettingOutlined /></template>
      </a-button>
      <a-button v-if="running" danger size="small" @click="cancelReview">
        取消审查
      </a-button>
      <a-tooltip v-else :title="runDisabledReason" :disabled="!runDisabledReason">
        <a-button :disabled="!ocrStatus.available || !canRun" @click="runReview">
          <template #icon><PlayCircleOutlined /></template>开始审查
        </a-button>
      </a-tooltip>
    </div>

    <!-- two-column body: left = scope, right = result/progress/guide -->
    <div class="rv-body">
      <!-- ============ LEFT: scope picker ============ -->
      <aside class="rv-scope">
        <!-- workspace mode: working-tree change summary -->
        <template v-if="mode === 'workspace'">
          <div class="rv-scope-head">
            <span>工作区改动</span>
            <span v-if="wsFiles.length" class="rv-scope-count">{{ wsFiles.length }} 文件</span>
          </div>
          <div class="rv-scope-list">
            <div v-if="wsLoading" class="rv-scope-center"><a-spin size="small" /></div>
            <div v-else-if="!wsFiles.length" class="rv-scope-empty">工作区干净，没有未提交改动</div>
            <div v-for="f in wsFiles" :key="f.path" class="rv-ws-item">
              <span class="rv-ws-status" :class="'st-' + f.status">{{ f.status[0].toUpperCase() }}</span>
              <span class="rv-ws-path" :title="f.path">{{ f.path }}</span>
            </div>
          </div>
        </template>

        <!-- range / commit mode: commit tree -->
        <template v-else>
          <CommitTree
            :items="commits"
            :loading="loadingCommits"
            :has-more="hasMore"
            :multi-select="mode === 'range'"
            :context-menu="false"
            v-model:active="activeHash"
            v-model:selected="selectedHashes"
            @load-more="onLoadMore"
          >
            <template #header>
              <span>{{ mode === 'range' ? '选择提交范围' : '选择提交' }}</span>
              <span v-if="mode === 'range'">{{ selectedHashes.size }} 已选</span>
              <span v-else>{{ totalCommits }} 条</span>
            </template>
            <template #item="{ entry: c, index }">
              <div class="rv-row">
                <span class="rv-num">#{{ commitNum(index) }}</span>
                <div class="rv-row-body">
                  <div class="rv-row-line">
                    <span class="rv-hash">{{ c.hash.slice(0, 7) }}</span>
                    <span class="rv-author">{{ c.author }}</span>
                    <span class="rv-time">{{ relTime(c.date) }}</span>
                  </div>
                  <div class="rv-subject" :title="c.subject">{{ c.subject }}</div>
                </div>
              </div>
            </template>
          </CommitTree>

          <!-- selection status bar (inside left column) -->
          <div class="rv-selbar">
            <template v-if="mode === 'range'">
              <template v-if="rangePreview">
                <span class="rv-sel-count">已选 {{ rangePreview.count }} 条</span>
                <span class="rv-sel-range">{{ rangePreview.from.hash.slice(0,7) }} …→… {{ rangePreview.to.hash.slice(0,7) }}</span>
              </template>
              <span v-else class="rv-sel-muted">Shift 范围选 / Ctrl 多选（≥2 条）</span>
            </template>
            <template v-else>
              <template v-if="activeCommit">
                <span class="rv-sel-count">已选</span>
                <span class="rv-sel-range">{{ activeCommit.hash.slice(0,7) }}</span>
              </template>
              <span v-else class="rv-sel-muted">点击选择一条提交</span>
            </template>
          </div>
        </template>
      </aside>

      <!-- ============ RIGHT: result area ============ -->
      <section class="rv-result">
        <!-- guide (idle) -->
        <div v-if="!result && !running" class="rv-placeholder">
          <FileSearchOutlined class="rv-ph-icon" />
          <div class="rv-ph-title">配置范围后开始审查</div>
          <div class="rv-ph-desc">左侧选择审查范围（工作区 / 提交范围 / 单提交），点击「开始审查」</div>
        </div>

        <!-- progress (running) -->
        <div v-else-if="running" class="rv-progress" ref="progressRef">
          <a-steps :current="stageIndex" size="small" direction="vertical" class="rv-steps">
            <a-step v-for="s in STAGES" :key="s.key" :title="s.label" />
          </a-steps>
          <div class="rv-prog-msg">
            <a-spin size="small" />
            <span>{{ progress.message || '审查中…' }}</span>
          </div>
          <div class="rv-prog-elapsed">已耗时 {{ elapsed }}s · AI 审查通常需要 10-60s</div>
        </div>

        <!-- result -->
        <template v-else-if="result">
          <!-- summary bar with status color border -->
          <div v-if="result.summary" class="rv-summary" :style="{ borderLeftColor: statusTag?.bar }">
            <span class="sm-item"><FileSearchOutlined /> {{ result.summary.files_reviewed }} 文件</span>
            <span class="sm-item"><WarningOutlined /> {{ result.summary.comments }} 意见</span>
            <span class="sm-item">⏱ {{ result.summary.elapsed }}</span>
            <span class="sm-item">↓ {{ fmtToken(result.summary.input_tokens) }}</span>
            <span class="sm-item">↑ {{ fmtToken(result.summary.output_tokens) }}</span>
            <a-tag v-if="statusTag" :color="statusTag.color">{{ statusTag.text }}</a-tag>
          </div>

          <!-- has comments: two columns (file tree + comment stream) -->
          <div v-if="fileGroups.length" class="rv-cols">
            <aside class="rv-files">
              <div class="rv-files-head">
                <span>审查结果</span>
                <span>{{ fileGroups.length }} 文件 / {{ result.comments.length }} 意见</span>
              </div>
              <div class="rv-files-body">
                <div
                  v-for="g in fileGroups"
                  :key="g.path"
                  class="rv-file-item"
                  :class="{ active: g.path === activePath }"
                  @click="activePath = g.path"
                >
                  <span class="rv-file-badge">{{ g.comments.length }}</span>
                  <div class="rv-file-body">
                    <div class="rv-file-path" :title="g.path">{{ shortPath(g.path) }}</div>
                    <div class="rv-file-preview">{{ g.comments[0].content }}</div>
                  </div>
                </div>
              </div>
            </aside>

            <div class="rv-stream">
              <div class="rv-stream-head">
                <span class="rv-stream-path" :title="activePath">{{ activePath }}</span>
                <a-button size="small" type="text" @click="copyText(activePath, '已复制路径')" title="复制文件路径">
                  <template #icon><CopyOutlined /></template>
                </a-button>
              </div>
              <div class="rv-stream-body">
                <div v-for="(c, i) in activeComments" :key="i" class="rv-comment">
                  <div class="cm-head">
                    <span class="cm-loc" @click="copyText(locStr(c), '已复制定位')">{{ locStr(c) }}</span>
                    <a-button
                      v-if="c.suggestion_code"
                      size="small" type="text" class="cm-copy"
                      @click="copyText(c.suggestion_code, '已复制建议代码')"
                    ><CopyOutlined /> 复制建议</a-button>
                  </div>
                  <MarkdownView :source="c.content" class="cm-content" />
                  <DiffViewer
                    v-if="c.existing_code || c.suggestion_code"
                    :before="c.existing_code || ''"
                    :after="c.suggestion_code || ''"
                    :filename="c.path"
                    :not-found="!c.existing_code"
                    class="cm-diff"
                  />
                  <details v-if="c.thinking" class="cm-thinking">
                    <summary>AI 推理</summary>
                    <div class="th-body">{{ c.thinking }}</div>
                  </details>
                </div>
              </div>
            </div>
          </div>

          <!-- no comments -->
          <div v-else class="rv-placeholder rv-ok">
            <CheckCircleOutlined class="rv-ph-icon ok" />
            <div class="rv-ph-title">{{ result.message || '审查完成，未发现问题' }}</div>
          </div>
        </template>
      </section>
    </div>

    <!-- advanced options modal (delegated to component) -->
    <ReviewAdvancedModal
      v-model:open="advModalOpen"
      v-model:model="model"
      v-model:background="background"
    />
  </div>
</template>

<style scoped>
.rv-view {
  width: calc(100% + var(--spacing-lg) * 2);
  height: calc(100% + var(--spacing-lg) * 2);
  margin: calc(-1 * var(--spacing-lg));
  display: flex; flex-direction: column;
  min-height: 0;
  background: transparent;
}

/* top bar */
.rv-top { display: flex; align-items: center; gap: var(--spacing-sm); flex-shrink: 0; height: 38px; padding: 0 var(--spacing-md); background: var(--color-surface); border-bottom: 1px solid var(--color-border); }
.rv-spacer { flex: 1; }

/* body: left scope + right result */
.rv-body { flex: 1; display: flex; gap: 1px; min-height: 0; background: var(--color-border); }

/* left: scope picker */
.rv-scope {
  width: 340px; flex-shrink: 0; display: flex; flex-direction: column; gap: var(--spacing-xs);
  background: var(--color-surface);
  min-height: 0; overflow: hidden;
}
.rv-scope-head { display: flex; align-items: center; justify-content: space-between; height: 34px; padding: 0 var(--spacing-md); border-bottom: 1px solid var(--color-border); font-size: 12px; font-weight: 600; color: var(--color-text-secondary); flex-shrink: 0; }
.rv-scope-count { font-weight: 400; color: var(--color-text-tertiary); }
.rv-scope-list { flex: 1; overflow-y: auto; padding: var(--spacing-xs); min-height: 0; }
.rv-scope-center { display: flex; justify-content: center; padding: var(--spacing-xl); }
.rv-scope-empty { padding: var(--spacing-lg); text-align: center; color: var(--color-text-tertiary); font-size: 12px; }
.rv-ws-item { display: flex; align-items: center; gap: 6px; padding: 4px 8px; border-radius: var(--radius-xs); }
.rv-ws-item:hover { background: var(--color-hover); }
.rv-ws-status { width: 16px; height: 16px; border-radius: 3px; font-size: 9px; font-weight: 700; display: inline-flex; align-items: center; justify-content: center; flex-shrink: 0; }
.st-modified, .st-M { color: var(--color-warning); background: var(--color-warning-bg); }
.st-added, .st-A { color: var(--color-success); background: var(--color-success-bg); }
.st-deleted, .st-D { color: var(--color-error); background: var(--color-error-bg); }
.st-untracked, .st-U { color: var(--color-text-tertiary); background: var(--color-surface-raised); }
.rv-ws-path { font-family: 'Cascadia Code', monospace; font-size: 11px; color: var(--color-text-secondary); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }

/* commit tree rows (inside CommitTree's slot) */
.rv-row { display: flex; gap: 6px; padding: 6px 8px; }
.rv-num { font-family: 'Cascadia Code', monospace; font-size: 10px; font-weight: 700; color: var(--color-text-tertiary); flex-shrink: 0; min-width: 26px; margin-top: 1px; }
.rv-row-body { flex: 1; min-width: 0; }
.rv-row-line { display: flex; align-items: center; gap: 6px; margin-bottom: 2px; }
.rv-hash { font-family: 'Cascadia Code', monospace; font-size: 11px; color: var(--color-hash); }
.rv-author { font-size: 11px; color: var(--color-text-secondary); flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.rv-time { font-size: 10px; color: var(--color-text-tertiary); }
.rv-subject { font-size: 12px; color: var(--color-text); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }

.rv-selbar { display: flex; align-items: center; gap: var(--spacing-sm); padding: 6px 12px; border-top: 1px solid var(--color-border); font-size: 12px; flex-shrink: 0; }
.rv-sel-count { font-weight: 600; color: var(--color-text); }
.rv-sel-range { font-family: 'Cascadia Code', monospace; color: var(--color-primary); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.rv-sel-muted { color: var(--color-text-tertiary); }

/* right: result area */
.rv-result { flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 1px; min-height: 0; background: var(--color-border); }

/* placeholder / guide */
.rv-placeholder { flex: 1; display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 8px; color: var(--color-text-tertiary); }
.rv-ph-icon { font-size: 40px; opacity: 0.3; }
.rv-ph-icon.ok { color: var(--color-success); opacity: 0.8; }
.rv-ph-title { font-size: 15px; font-weight: 600; color: var(--color-text); }
.rv-ph-desc { font-size: 12px; color: var(--color-text-tertiary); text-align: center; max-width: 320px; }

/* progress */
.rv-progress { flex: 1; display: flex; flex-direction: column; align-items: center; justify-content: center; gap: var(--spacing-md); }
.rv-steps { width: 240px; }
.rv-prog-msg { display: flex; align-items: center; gap: 8px; font-size: 13px; color: var(--color-text); }
.rv-prog-elapsed { font-size: 11px; color: var(--color-text-tertiary); }

/* summary bar with status-colored left border */
.rv-summary { display: flex; align-items: center; gap: var(--spacing-md); padding: var(--spacing-xs) var(--spacing-sm) var(--spacing-xs) var(--spacing-md); background: var(--color-surface); border: 1px solid var(--color-border); border-left: 3px solid var(--color-text-tertiary); border-radius: var(--radius-md); flex-shrink: 0; }
.sm-item { font-size: 12px; color: var(--color-text-secondary); display: inline-flex; align-items: center; gap: 4px; }

/* result columns */
.rv-cols { flex: 1; display: flex; gap: 1px; min-height: 0; background: var(--color-border); }

/* file tree (left of result) */
.rv-files { width: 260px; flex-shrink: 0; background: var(--color-surface); display: flex; flex-direction: column; min-height: 0; }
.rv-files-head { display: flex; justify-content: space-between; height: 34px; padding: 0 var(--spacing-md); border-bottom: 1px solid var(--color-border); font-size: 12px; font-weight: 600; color: var(--color-text-secondary); flex-shrink: 0; }
.rv-files-body { flex: 1; overflow-y: auto; padding: var(--spacing-xs); min-height: 0; }
.rv-file-item { display: flex; gap: 8px; padding: 6px 8px; border-radius: var(--radius-sm); cursor: pointer; border-left: 2px solid transparent; }
.rv-file-item:hover { background: var(--color-hover); }
.rv-file-item.active { background: var(--color-primary-bg); border-left-color: var(--color-primary); }
.rv-file-badge { width: 18px; height: 18px; border-radius: 9px; background: var(--color-warning-bg); color: var(--color-warning); font-size: 10px; font-weight: 700; display: inline-flex; align-items: center; justify-content: center; flex-shrink: 0; }
.rv-file-body { flex: 1; min-width: 0; }
.rv-file-path { font-size: 12px; color: var(--color-text); font-family: 'Cascadia Code', monospace; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.rv-file-preview { font-size: 10px; color: var(--color-text-tertiary); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; margin-top: 1px; }

/* comment stream (right of result) */
.rv-stream { flex: 1; min-width: 0; background: var(--color-surface); display: flex; flex-direction: column; min-height: 0; }
.rv-stream-head { display: flex; align-items: center; gap: var(--spacing-sm); height: 34px; padding: 0 var(--spacing-md); border-bottom: 1px solid var(--color-border); flex-shrink: 0; }
.rv-stream-path { font-family: 'Cascadia Code', monospace; font-size: 12px; color: var(--color-text); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; flex: 1; }
.rv-stream-body { flex: 1; overflow-y: auto; padding: var(--spacing-sm) var(--spacing-md); min-height: 0; }

.rv-comment { margin-bottom: var(--spacing-md); padding: var(--spacing-sm); background: var(--color-background); border-radius: var(--radius-sm); border-left: 3px solid var(--color-warning); }
.cm-head { display: flex; align-items: center; justify-content: space-between; margin-bottom: 6px; }
.cm-loc { font-family: 'Cascadia Code', monospace; font-size: 12px; color: var(--color-primary); font-weight: 600; cursor: pointer; }
.cm-loc:hover { text-decoration: underline; }
.cm-copy { font-size: 11px; color: var(--color-text-tertiary); }
.cm-content { font-size: 13px; color: var(--color-text); line-height: 1.6; }
.cm-content :deep(p) { margin: 0 0 6px; }
.cm-content :deep(p:last-child) { margin-bottom: 0; }
.cm-diff { margin-top: 8px; border-radius: var(--radius-sm); overflow: hidden; }
.cm-thinking { margin-top: 8px; font-size: 12px; }
.cm-thinking summary { cursor: pointer; color: var(--color-text-tertiary); }
.th-body { padding: var(--spacing-xs) 0; color: var(--color-text-secondary); white-space: pre-wrap; }
</style>
