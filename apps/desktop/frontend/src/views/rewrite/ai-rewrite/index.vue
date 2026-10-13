<script setup>
import { ref, computed, onMounted, watch } from 'vue'
import { message as AMessage } from 'ant-design-vue'
import {
  ThunderboltOutlined, ReloadOutlined,
  ExclamationCircleOutlined,
  SettingOutlined, PlusOutlined, DeleteOutlined, ArrowRightOutlined,
  HistoryOutlined, ProfileOutlined,
} from '@ant-design/icons-vue'
import { useRepoStore } from '../../../stores/repo'
import { useRewriteStream } from '../../../composables/useRewriteStream'
import { relTime } from '../../../composables/useRelativeTime'
import CommitTree from '../../../components/common/CommitTree.vue'
import CommitHoverCard from '../../../components/common/CommitHoverCard.vue'
import PushTag from '../../../components/common/PushTag.vue'
import ContextMenu from '../../../components/common/ContextMenu.vue'
import CtxItem from '../../../components/common/CtxItem.vue'
import AIRewriteRulesModal from '../../../components/rewrite/AIRewriteRulesModal.vue'
import AiRewritePreviewModal from './components/AiRewritePreviewModal.vue'
import AiRewriteTaskDetail from './components/AiRewriteTaskDetail.vue'
import { useCommitStat } from '../../../composables/useCommitStat'

/**
 * AI 改写 — 任务驱动工作流。
 * 左：提交树（浏览，右键/规则匹配→加入任务）
 * 中：改写任务列表（队列，折叠卡片，多选批量生成）
 * 右：任务详情（原始 vs 改写 + 文件 diff）
 */
const repo = useRepoStore()
const app = window.go?.main?.ByteGitApp

// per-commit change stat (lazy, for the hover card) — shared composable so
// the hover content matches git log / time-travel exactly.
const { stat: statOf, has: hasStat, onHover: onStatHover, onLeave: onStatLeave, clear: clearStat } =
  useCommitStat((h) => app.CommitStat(h))
function onCommitHover(entry, isEnter) {
  if (isEnter) onStatHover(entry.hash)
  else onStatLeave(entry.hash)
}

// ============ 提交树（左栏，浏览用）============
const entries = ref([])
const exporting = ref(false)
const loadError = ref('')
const activeHash = ref('')          // 提交树当前选中
const selectedHashes = ref(new Set()) // 提交树多选（右键批量加入用）

// ---- pagination (infinite scroll) ----
const PAGE = 100
const hasMore = ref(true)
const loadingMore = ref(false)

async function loadTree(mode = 'last', n = PAGE, skip = 0) {
  if (!app?.RewriteExport) {
    loadError.value = '后端方法不可用，请重启应用'
    return
  }
  // first page (skip === 0) → reset state; subsequent pages → append
  const isFirst = skip === 0
  if (isFirst) {
    exporting.value = true
    hasMore.value = true
  } else {
    loadingMore.value = true
  }
  loadError.value = ''
  try {
    const data = await app.RewriteExport(repo.repoArg(), mode, n, skip)
    const arr = data || []
    if (isFirst) {
      entries.value = arr
    } else {
      entries.value.push(...arr)
    }
    if (arr.length < n) hasMore.value = false
  } catch (e) {
    loadError.value = String(e?.message || e)
    if (!isFirst) hasMore.value = false
  } finally {
    exporting.value = false
    loadingMore.value = false
  }
}

// infinite-scroll callback from <CommitTree>
function loadMore(done) {
  if (!hasMore.value || loadingMore.value || !entries.value.length) { done?.(); return }
  loadTree('last', PAGE, entries.value.length).finally(() => done?.())
}

// 提交树右键
const treeContextMenu = ref({ open: false, x: 0, y: 0 })
function onTreeContextmenu(entry, evt) {
  treeContextMenu.value = { open: true, x: evt.clientX, y: evt.clientY }
}
function closeTreeMenu() { treeContextMenu.value.open = false }

// ============ 改写任务队列（中栏）============
const tasks = ref([])              // 改写任务列表（深拷贝自 entries）
const activeTaskIdx = ref(-1)      // 当前选中的任务索引
const selectedTaskIdxs = ref(new Set()) // 多选

const activeTask = computed(() => tasks.value[activeTaskIdx.value] || null)

// split a full commit message into subject (first line) + body (the rest)
function splitMsg(msg) {
  const s = msg || ''
  const idx = s.indexOf('\n')
  if (idx < 0) return { subject: s, body: '' }
  return { subject: s.slice(0, idx), body: s.slice(idx + 1).replace(/^\n+/, '') }
}

// 加入任务（去重）
function addToTask(entry) {
  if (!entry) return
  if (tasks.value.some((t) => t.hash === entry.hash)) return
  tasks.value.push(JSON.parse(JSON.stringify(entry)))
}
function addSelectedToTasks() {
  let n = 0
  entries.value.forEach((e) => {
    if (selectedHashes.value.has(e.hash)) { addToTask(e); n++ }
  })
  closeTreeMenu()
  if (n) AMessage.success(`已加入 ${n} 条任务`)
}
// 按规则匹配加入（调 bad 模式导出）
const matching = ref(false)
async function addMatchedToTasks() {
  if (!app?.RewriteExport) return
  matching.value = true
  try {
    const matched = (await app.RewriteExport(repo.repoArg(), 'bad', 0, 0)) || []
    let n = 0
    matched.forEach((e) => { addToTask(e); n++ })
    AMessage.success(`匹配到 ${matched.length} 条，新增 ${n} 条任务`)
  } catch (e) {
    AMessage.error('匹配失败: ' + String(e?.message || e))
  } finally {
    matching.value = false
  }
}

function removeTask(idx) {
  tasks.value.splice(idx, 1)
  if (activeTaskIdx.value === idx) activeTaskIdx.value = -1
  else if (activeTaskIdx.value > idx) activeTaskIdx.value--
  selectedTaskIdxs.value.delete(idx)
  selectedTaskIdxs.value = new Set(selectedTaskIdxs.value)
}
function removeSelectedTasks() {
  const idxs = [...selectedTaskIdxs.value].sort((a, b) => b - a)
  idxs.forEach((i) => tasks.value.splice(i, 1))
  selectedTaskIdxs.value = new Set()
  activeTaskIdx.value = -1
  AMessage.success('已移除选中任务')
}

// 任务统计
const taskCount = computed(() => tasks.value.length)
const generatedCount = computed(() => tasks.value.filter((t) => t.suggested).length)
const approvedCount = computed(() => tasks.value.filter((t) => t.status === 'approved').length)

// ============ AI 生成（单条 + 批量）============
const { streaming, runSingle, runBatch, batchDone, batchTotal, generating } = useRewriteStream()
const singleHash = ref('')

async function generateOne(task) {
  if (!app) return
  singleHash.value = task.hash
  try {
    const full = await runSingle(task.hash, () => app.RewriteGenerate(repo.repoArg(), task))
    task.suggested = full
    task.status = 'suggested'
  } catch (e) {
    AMessage.error('生成失败: ' + String(e?.message || e))
    task.status = 'retry'
  } finally {
    singleHash.value = ''
  }
}

async function generateSelected() {
  const pending = [...selectedTaskIdxs.value].map((i) => tasks.value[i]).filter((t) => t && !t.suggested)
  if (!pending.length) {
    AMessage.info('选中的任务无需生成')
    return
  }
  try {
    const results = await runBatch(() => app.RewriteGenerateAll(repo.repoArg(), pending))
    results.forEach((r) => {
      const t = tasks.value.find((x) => x.hash === r.hash)
      if (!t) return
      if (r.ok) { t.suggested = r.suggested; t.status = 'suggested' }
      else { t.status = 'retry' }
    })
    AMessage.success(`批量生成完成 (${batchDone.value}/${batchTotal.value})`)
  } catch (e) {
    AMessage.error('批量生成失败: ' + String(e?.message || e))
  }
}

async function cancelGenerate() {
  try { if (app?.CancelTask) await app.CancelTask() } catch {}
  AMessage.info('已取消生成')
}

// 采用/跳过
function approve(task) {
  if (!task.suggested) { AMessage.warning('请先生成建议'); return }
  task.status = task.status === 'approved' ? 'suggested' : 'approved'
}
function skip(task) {
  task.status = task.status === 'skipped' ? 'pending' : 'skipped'
}

const batchPct = computed(() => batchTotal.value ? Math.round(batchDone.value / batchTotal.value * 100) : 0)
const statusDot = { approved: '✓', suggested: '★', retry: '!', skipped: '✗', pending: '' }

// ============ 预览 + 应用 ============
const previewVisible = ref(false)
const preview = ref(null)
const previewing = ref(false)

async function openPreview() {
  if (!approvedCount.value) {
    AMessage.warning('请先采用至少一条改写')
    return
  }
  previewing.value = true
  try {
    preview.value = await app.RewriteApplyPreview(repo.repoArg(), tasks.value)
    previewVisible.value = true
  } catch (e) {
    AMessage.error('预览失败: ' + String(e?.message || e))
  } finally {
    previewing.value = false
  }
}

// 改写应用完成后刷新提交树（来自 AiRewritePreviewModal）
function onApplied() {
  loadTree()
}

// ============ 规则管理（委托给 AIRewriteRulesModal） ============
const rulesOpen = ref(false)

// ============ lifecycle ============
onMounted(() => {
  loadTree()
})
watch(() => repo.repoEpoch, () => { clearStat(); loadTree() })
</script>

<template>
  <div class="ar-view">
    <!-- 顶部工具栏 -->
    <div class="ar-toolbar">
      <a-button size="small" :loading="matching" @click="addMatchedToTasks" title="按规则匹配并加入任务">
        <template #icon><ThunderboltOutlined /></template>按规则匹配
      </a-button>
      <a-button size="small" type="text" @click="rulesOpen = true" title="检测规则管理">
        <template #icon><SettingOutlined /></template>规则
      </a-button>
      <a-button size="small" type="text" :loading="exporting" @click="loadTree()" title="刷新提交树">
        <template #icon><ReloadOutlined /></template>刷新
      </a-button>

      <div class="ar-spacer"></div>

      <span class="ar-stat">任务 <b>{{ taskCount }}</b> · 已生成 <b>{{ generatedCount }}</b> · 已采用 <b class="ok">{{ approvedCount }}</b></span>
      <a-button size="small" :loading="previewing" :disabled="!approvedCount" @click="openPreview">
        <template #icon><ExclamationCircleOutlined /></template>预览结果
      </a-button>
    </div>

    <!-- 批量进度条 -->
    <div v-if="generating && batchTotal" class="ar-batch-bar">
      <a-progress :percent="batchPct" size="small" :format="() => `${batchDone}/${batchTotal}`" />
    </div>

    <!-- 三栏 -->
    <div class="ar-cols">
      <!-- 左栏：提交树 -->
      <aside class="ar-tree">
        <CommitTree
          :items="entries"
          v-model:active="activeHash"
          v-model:selected="selectedHashes"
          :has-more="hasMore"
          tooltip-placement="right"
          tooltip-overlay-class="ar-tip"
          @load-more="loadMore"
          @contextmenu="onTreeContextmenu"
          @hover-item="onCommitHover"
        >
          <template #header>
            <span class="ar-head-title"><HistoryOutlined /> 提交树</span>
            <span>{{ entries.length }} 条</span>
          </template>
          <template #item="{ entry: e, index }">
            <div class="ar-tree-row" :class="{ 'need-ai': e.needAi, 'in-task': tasks.some(t => t.hash === e.hash) }">
              <span class="ar-num">#{{ index + 1 }}</span>
              <span class="ar-ai-dot" v-if="e.needAi" title="命中规则">●</span>
              <span class="ar-ai-dot dim" v-else>·</span>
              <div class="ar-tree-body">
                <div class="ar-tree-line">
                  <span class="ar-hash">{{ e.hash.slice(0, 7) }}</span>
                  <span class="ar-author">{{ e.author }}</span>
                  <span class="ar-time">{{ relTime(e.date || '') }}</span>
                  <PushTag :pushed="e.pushed" />
                </div>
                <div class="ar-subject" :title="e.original">{{ e.original }}</div>
              </div>
            </div>
          </template>
          <template #tooltip="{ entry: e, index }">
            <CommitHoverCard
              :commit="{
                hash: e.hash,
                subject: splitMsg(e.original).subject,
                body: splitMsg(e.original).body,
                date: e.date,
              }"
              :num="'#' + (index + 1)"
              :stat="statOf(e.hash)"
              :stat-loading="!hasStat(e.hash)"
            />
          </template>
        </CommitTree>
      </aside>

      <!-- 中栏：改写任务列表 -->
      <aside class="ar-tasks">
        <div class="ar-tasks-head">
          <span class="ar-head-title"><ProfileOutlined /> 改写任务</span>
          <span>{{ taskCount }} 条</span>
        </div>
        <div class="ar-tasks-body">
          <div v-if="!taskCount" class="ar-tasks-empty">
            <PlusOutlined style="font-size:24px;opacity:0.3" />
            <div style="margin-top:8px">右键提交或按规则匹配<br/>加入改写任务</div>
          </div>
          <div
            v-for="(t, idx) in tasks" :key="t.hash"
            class="ar-task-card"
            :class="{ active: activeTaskIdx === idx, ['st-' + (t.status||'pending')]: true }"
            @click="activeTaskIdx = idx"
          >
            <div class="atc-bar"></div>
            <input type="checkbox" class="atc-check" :checked="selectedTaskIdxs.has(idx)" @click.stop @change="() => { selectedTaskIdxs.has(idx) ? selectedTaskIdxs.delete(idx) : selectedTaskIdxs.add(idx); selectedTaskIdxs = new Set(selectedTaskIdxs) }" />
            <div class="atc-main">
              <div class="atc-line">
                <span class="atc-num">#{{ idx + 1 }}</span>
                <span class="atc-status">{{ statusDot[t.status] || '' }}</span>
                <span class="atc-hash">{{ t.hash.slice(0, 7) }}</span>
                <span class="atc-flag" v-if="t.needAi" title="命中规则">⚠</span>
              </div>
              <div class="atc-orig" :title="t.original">{{ t.original }}</div>
              <div v-if="t.suggested" class="atc-new" :title="t.suggested">
                <ArrowRightOutlined /> {{ t.suggested }}
              </div>
            </div>
          </div>
        </div>
        <!-- 多选操作栏 -->
        <div v-if="selectedTaskIdxs.size" class="ar-tasks-bulk">
          <span>已选 {{ selectedTaskIdxs.size }}</span>
          <template v-if="generating">
            <a-button size="small" danger @click="cancelGenerate">取消生成</a-button>
          </template>
          <template v-else>
            <a-button size="small" @click="generateSelected">
              <template #icon><ThunderboltOutlined /></template>一键生成
            </a-button>
          </template>
          <a-button size="small" danger @click="removeSelectedTasks">
            <template #icon><DeleteOutlined /></template>移除
          </a-button>
        </div>
      </aside>

      <!-- 右栏：任务详情 -->
      <AiRewriteTaskDetail
        :active-task="activeTask"
        :single-hash="singleHash"
        :index="activeTaskIdx"
        :repo-arg="repo.repoArg()"
        @generate="generateOne"
        @approve="approve"
        @skip="skip"
        @remove="removeTask"
      />
    </div>

    <!-- 提交树右键菜单 -->
    <ContextMenu v-model:open="treeContextMenu.open" :x="treeContextMenu.x" :y="treeContextMenu.y" :title="selectedHashes.size ? `已选 ${selectedHashes.size} 条` : '操作'">
      <CtxItem @click="addSelectedToTasks"><PlusOutlined /> 加入改写任务</CtxItem>
      <CtxItem @click="closeTreeMenu"><ReloadOutlined /> 取消</CtxItem>
    </ContextMenu>

    <!-- 规则管理 modal -->
    <AIRewriteRulesModal v-model:open="rulesOpen" />

    <!-- 预览 modal -->
    <AiRewritePreviewModal
      v-model:open="previewVisible"
      :preview="preview"
      :repo-arg="repo.repoArg()"
      :tasks="tasks"
      @applied="onApplied"
    />
  </div>
</template>

<style scoped>
/* Full-bleed: negate .content-wrapper's 24px padding — flush with navbar /
   Sidebar / footer, matching the other multi-column pages. */
.ar-view {
  width: calc(100% + var(--spacing-lg) * 2);
  height: calc(100% + var(--spacing-lg) * 2);
  margin: calc(-1 * var(--spacing-lg));
  display: flex; flex-direction: column;
  min-height: 0;
  background: transparent;
}

/* 顶部工具栏 — flush strip rather than floating */
.ar-toolbar {
  display: flex; align-items: center; gap: 8px;
  flex-shrink: 0;
  height: 38px;
  padding: 0 var(--spacing-md);
  border-bottom: 1px solid var(--color-border);
}
.ar-spacer { flex: 1; }
.ar-stat { font-size: 12px; color: var(--color-text-secondary); }
.ar-stat b { color: var(--color-text); }
.ar-stat .ok { color: var(--color-success); }
.ar-batch-bar { flex-shrink: 0; }

/* shared column-header title (icon + label) */
.ar-head-title { display: inline-flex; align-items: center; gap: 6px; }
.ar-head-title .anticon { font-size: 13px; opacity: 0.85; }

/* 三栏 — 1px gap (parent border colour shows as divider) */
.ar-cols { flex: 1; display: grid; grid-template-columns: 280px 300px 1fr; gap: 1px; background: var(--color-border); min-height: 0; overflow: hidden; }

/* 左栏：提交树 */
.ar-tree { background: var(--color-background); overflow: hidden; display: flex; flex-direction: column; }
/* Flatten CommitTree's card chrome + force its header to 34px to align with
   the middle column's .ar-tasks-head. */
.ar-tree :deep(.ct-list) { border-radius: 0; border: none; }
.ar-tree :deep(.ct-head) { height: 34px; padding: 0 12px; }
.ar-tree :deep(.ct-host) { flex: 1; }
.ar-tree-row { display: flex; gap: 6px; padding: 6px 8px; }
.ar-num { font-family: 'Cascadia Code', monospace; font-size: 10px; font-weight: 700; color: var(--color-text-tertiary); flex-shrink: 0; min-width: 26px; margin-top: 1px; }
.ar-ai-dot { font-size: 10px; flex-shrink: 0; color: var(--color-warning); margin-top: 3px; }
.ar-ai-dot.dim { color: var(--color-text-tertiary); opacity: 0.4; }
.ar-tree-row.in-task { background: rgba(80, 250, 123, 0.06); }
.ar-tree-row.in-task .ar-ai-dot { color: var(--color-success); }
.ar-tree-body { flex: 1; min-width: 0; }
.ar-tree-line { display: flex; align-items: center; gap: 6px; margin-bottom: 2px; }
.ar-hash { font-family: 'Cascadia Code', monospace; font-size: 11px; color: var(--color-hash); }
.ar-author { font-size: 11px; color: var(--color-text-secondary); flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.ar-time { font-size: 10px; color: var(--color-text-tertiary); flex-shrink: 0; }
.ar-subject { font-size: 12px; color: var(--color-text); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }

/* 中栏：任务列表 */
.ar-tasks { background: var(--color-background); display: flex; flex-direction: column; overflow: hidden; }
.ar-tasks-head { display: flex; align-items: center; justify-content: space-between; padding: 0 12px; height: 34px; flex-shrink: 0; font-size: 12px; font-weight: 600; border-bottom: 1px solid var(--color-border); background: var(--color-surface); }
.ar-tasks-body { flex: 1; overflow-y: auto; padding: 4px; }
.ar-tasks-empty { flex: 1; display: flex; flex-direction: column; align-items: center; justify-content: center; color: var(--color-text-tertiary); font-size: 12px; text-align: center; padding: 40px 12px; }

.ar-task-card { display: flex; align-items: stretch; gap: 0; padding: 0; border-radius: var(--radius-xs); cursor: pointer; margin-bottom: 2px; transition: background var(--transition-fast); position: relative; }
.ar-task-card:hover { background: var(--color-hover); }
.ar-task-card.active { background: var(--color-primary-bg); }
.atc-bar { width: 3px; flex-shrink: 0; border-radius: 2px; background: var(--color-text-tertiary); }
.ar-task-card.st-suggested .atc-bar { background: var(--color-primary); }
.ar-task-card.st-approved .atc-bar { background: var(--color-success); }
.ar-task-card.st-retry .atc-bar { background: var(--color-error); }
.ar-task-card.st-skipped .atc-bar { background: var(--color-text-tertiary); opacity: 0.4; }
.atc-check { margin: 8px 4px 8px 6px; flex-shrink: 0; cursor: pointer; }
.atc-main { flex: 1; min-width: 0; padding: 4px 8px 4px 0; }
.atc-line { display: flex; align-items: center; gap: 5px; margin-bottom: 2px; }
.atc-num { font-family: 'Cascadia Code', monospace; font-size: 10px; color: var(--color-text-tertiary); }
.atc-status { font-size: 11px; width: 12px; text-align: center; }
.ar-task-card.st-approved .atc-status { color: var(--color-success); }
.ar-task-card.st-suggested .atc-status { color: var(--color-primary); }
.atc-hash { font-family: 'Cascadia Code', monospace; font-size: 11px; color: var(--color-hash); }
.atc-flag { color: var(--color-warning); font-size: 11px; }
.atc-orig { font-size: 12px; color: var(--color-text-secondary); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.atc-new { font-size: 11px; color: var(--color-primary); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; margin-top: 1px; }

.ar-tasks-bulk { display: flex; align-items: center; gap: 8px; padding: 6px 12px; border-top: 1px solid var(--color-border); flex-shrink: 0; font-size: 12px; color: var(--color-text-secondary); background: var(--color-surface); }
</style>

