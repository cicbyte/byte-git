<script setup>
import { ref, computed, onMounted, onBeforeUnmount, nextTick, watch } from 'vue'
import { message as AMessage } from 'ant-design-vue'
import {
  ThunderboltOutlined, ReloadOutlined,
  ExclamationCircleOutlined,
  SettingOutlined, PlusOutlined, HistoryOutlined, ProfileOutlined, ArrowRightOutlined, DeleteOutlined,
} from '@ant-design/icons-vue'
import { storeToRefs } from 'pinia'
import { useRepoStore } from '@/stores/repo'
import { useAiRewriteStore } from '@/stores/aiRewrite'
import { useRewriteStream } from '@/composables/useRewriteStream'
import { relTime } from '@/composables/useRelativeTime'
import CommitTree from '@/components/common/CommitTree.vue'
import CommitHoverCard from '@/components/common/CommitHoverCard.vue'
import PushTag from '@/components/common/PushTag.vue'
import ContextMenu from '@/components/common/ContextMenu.vue'
import CtxItem from '@/components/common/CtxItem.vue'
import AIRewriteRulesModal from '@/components/rewrite/AIRewriteRulesModal.vue'
import AiRewritePreviewModal from './components/AiRewritePreviewModal.vue'
import AiRewriteTaskDetail from './components/AiRewriteTaskDetail.vue'
import { useCommitStat } from '@/composables/useCommitStat'

/**
 * AI 改写 — 三栏分栏工作台。
 * 左：提交树（浏览，双击/行内 + /右键多选/规则匹配 → 加入任务）
 * 中：改写任务列表（竖排卡片，状态色条 + 过滤 + 批量清理）
 * 右：任务详情（原文/建议对比 + 文件 diff）
 *
 * 任务队列在 pinia（stores/aiRewrite）：跨路由保留，切仓库自动清空。
 * 键盘流（非输入态）：←/→/↑/↓ 切换任务 · G 生成 · A 采用 · S 跳过 · Esc 取消选中。
 */
const repo = useRepoStore()
const aiStore = useAiRewriteStore()
const { tasks } = storeToRefs(aiStore)
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

// ============ 任务队列（pinia，跨路由保留）============
const activeTaskIdx = ref(-1)
const activeTask = computed(() => tasks.value[activeTaskIdx.value] || null)

// 任务数变化时校正选中索引（移除/清空后不悬空）；空→选中也无任务时保持 -1
watch(() => tasks.value.length, (len) => {
  if (activeTaskIdx.value >= len) activeTaskIdx.value = len - 1
  if (len && activeTaskIdx.value < 0) activeTaskIdx.value = 0
})

// split a full commit message into subject (first line) + body (the rest)
function splitMsg(msg) {
  const s = msg || ''
  const idx = s.indexOf('\n')
  if (idx < 0) return { subject: s, body: '' }
  return { subject: s.slice(0, idx), body: s.slice(idx + 1).replace(/^\n+/, '') }
}

// 加入任务（去重；提示由调用方决定是否给）
function addToTask(entry) {
  return aiStore.addEntry(entry)
}
function addSelectedToTasks() {
  const list = entries.value.filter((e) => selectedHashes.value.has(e.hash))
  const n = aiStore.addEntries(list)
  closeTreeMenu()
  if (n) {
    AMessage.success(`已加入 ${n} 条任务`)
    activeTaskIdx.value = tasks.value.length - 1
  } else {
    AMessage.info('选中提交已在任务队列中')
  }
}
// 从队列移除选中提交（右键菜单「从任务移除」）
function removeSelectedFromTasks() {
  const before = tasks.value.length
  tasks.value = tasks.value.filter((t) => !selectedHashes.value.has(t.hash))
  const n = before - tasks.value.length
  if (activeTaskIdx.value >= tasks.value.length) {
    activeTaskIdx.value = Math.max(-1, tasks.value.length - 1)
  }
  closeTreeMenu()
  if (n) AMessage.success(`已移除 ${n} 条任务`)
  else AMessage.info('选中提交不在任务队列中')
}
// 按 hash 从队列移除单条（提交树行内 ✓ 点击）
function removeByHash(hash) {
  const idx = tasks.value.findIndex((t) => t.hash === hash)
  if (idx >= 0) removeTask(idx)
}
// 右键菜单：选中里在/不在队列的数量（决定菜单项）
const selInTask = computed(() =>
  [...selectedHashes.value].filter((h) => tasks.value.some((t) => t.hash === h)).length)
// 行内 + 快速加入单条（hover 出现）
function addQuick(entry) {
  if (addToTask(entry)) {
    activeTaskIdx.value = tasks.value.length - 1
    AMessage.success(`已加入 ${entry.hash.slice(0, 7)}`, 1.2)
  } else {
    AMessage.info('该提交已在任务队列中', 1.2)
  }
}
// 按规则匹配加入（调 bad 模式导出）
const matching = ref(false)
async function addMatchedToTasks() {
  if (!app?.RewriteExport) return
  matching.value = true
  try {
    const matched = (await app.RewriteExport(repo.repoArg(), 'bad', 0, 0)) || []
    const n = aiStore.addEntries(matched)
    AMessage.success(`匹配到 ${matched.length} 条，新增 ${n} 条任务`)
    if (n) activeTaskIdx.value = 0
  } catch (e) {
    AMessage.error('匹配失败: ' + String(e?.message || e))
  } finally {
    matching.value = false
  }
}

// 提交树右键
const treeContextMenu = ref({ open: false, x: 0, y: 0 })
function onTreeContextmenu(entry, evt) {
  treeContextMenu.value = { open: true, x: evt.clientX, y: evt.clientY }
}
function closeTreeMenu() { treeContextMenu.value.open = false }

// 单击切换活动行：再点同一行取消高亮（CommitTree 的 v-model 会先写入 hash，这里覆盖）
let prevActive = ''
function onTreeClick(entry) {
  activeHash.value = prevActive === entry.hash ? '' : entry.hash
  prevActive = activeHash.value
}

// 任务统计
const taskCount = computed(() => tasks.value.length)
const pendingCount = computed(() => tasks.value.filter((t) => !t.suggested).length)
const approvedCount = computed(() => tasks.value.filter((t) => t.status === 'approved').length)

// ============ AI 生成（单条 + 全部待生成）============
const { streaming, runSingle, runBatch, batchDone, batchTotal, generating } = useRewriteStream()
const singleHash = ref('')

async function generateOne(task) {
  if (!app || !task) return
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

async function generateAll() {
  const pending = tasks.value.filter((t) => !t.suggested)
  if (!pending.length) {
    AMessage.info('没有待生成的任务')
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
  if (!task) return
  if (!task.suggested) { AMessage.warning('请先生成建议'); return }
  task.status = task.status === 'approved' ? 'suggested' : 'approved'
}
function skip(task) {
  if (!task) return
  task.status = task.status === 'skipped' ? 'pending' : 'skipped'
}
function removeTask(idx) {
  aiStore.removeAt(idx)
  if (activeTaskIdx.value === idx) activeTaskIdx.value = Math.min(idx, tasks.value.length - 1)
  else if (activeTaskIdx.value > idx) activeTaskIdx.value--
}

const batchPct = computed(() => batchTotal.value ? Math.round(batchDone.value / batchTotal.value * 100) : 0)

// ============ 任务列表（中栏）：过滤 + 批量清理 + 键盘导航 ============
// 状态过滤：列表与键盘导航均按过滤集移动
const chipFilter = ref('all')
const filteredTasks = computed(() => {
  if (chipFilter.value === 'pending') return tasks.value.filter((t) => !t.suggested)
  if (chipFilter.value === 'approved') return tasks.value.filter((t) => t.status === 'approved')
  return tasks.value
})
const filteredIdxs = computed(() => filteredTasks.value.map((t) => tasks.value.indexOf(t)))
const navPos = computed(() => filteredIdxs.value.indexOf(activeTaskIdx.value))
const skippedCount = computed(() => tasks.value.filter((t) => t.status === 'skipped').length)

function step(delta) {
  const idxs = filteredIdxs.value
  if (!idxs.length) return
  const pos = idxs.indexOf(activeTaskIdx.value)
  // 当前任务不在过滤集内：向取值方向回到过滤集首/尾
  const next = pos < 0
    ? (delta > 0 ? 0 : idxs.length - 1)
    : Math.min(idxs.length - 1, Math.max(0, pos + delta))
  activeTaskIdx.value = idxs[next]
}

// 键盘导航时活动卡片滚入视野
watch(activeTaskIdx, () => {
  nextTick(() => {
    document.querySelector('.ar-task-card.active')
      ?.scrollIntoView({ behavior: 'smooth', block: 'nearest' })
  })
})

// 卡片状态短文案
const stText = { pending: '待生成', suggested: '已生成', approved: '已采用', retry: '失败', skipped: '跳过' }

function removeSkipped() {
  const before = tasks.value.length
  tasks.value = tasks.value.filter((t) => t.status !== 'skipped')
  AMessage.success(`已移除 ${before - tasks.value.length} 条已跳过任务`)
}
function clearAll() {
  aiStore.clear()
  AMessage.success('已清空任务队列')
}

// 键盘流：非输入态生效（←/→ 切换 · G 生成 · A 采用 · S 跳过，Esc 取消树选择）。
// radio（过滤段控）点击后持有焦点但不接收文本输入，予以放行。
function onKeydown(e) {
  const t = e.target
  if (t && (t.tagName === 'TEXTAREA' || t.isContentEditable)) return
  if (t && t.tagName === 'INPUT' && t.type !== 'radio') return
  if (e.key === 'Escape') {
    selectedHashes.value = new Set()
    activeHash.value = ''
    prevActive = ''
    return
  }
  if (!activeTask.value) return
  if (e.key === 'ArrowLeft' || e.key === 'ArrowUp') { e.preventDefault(); step(-1) }
  else if (e.key === 'ArrowRight' || e.key === 'ArrowDown') { e.preventDefault(); step(1) }
  else if (e.key === 'g' || e.key === 'G') { e.preventDefault(); generateOne(activeTask.value) }
  else if (e.key === 'a' || e.key === 'A') { e.preventDefault(); approve(activeTask.value) }
  else if (e.key === 's' || e.key === 'S') { e.preventDefault(); skip(activeTask.value) }
}

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
  window.addEventListener('keydown', onKeydown)
})
onBeforeUnmount(() => {
  window.removeEventListener('keydown', onKeydown)
})
watch(() => repo.repoEpoch, () => {
  clearStat()
  loadTree()
  // 队列清空由 store 的 repoEpoch 守卫完成
})
</script>

<template>
  <div class="ar-view">
    <!-- 顶部工具栏（批量操作收敛于此） -->
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

      <!-- 任务/已生成/已采用计数由中栏过滤段控与底栏承载，工具栏不重复 -->
      <a-button v-if="generating" size="small" danger @click="cancelGenerate">取消生成</a-button>
      <a-button v-else size="small" :disabled="!pendingCount" @click="generateAll" title="为所有未生成的任务生成 AI 建议">
        <template #icon><ThunderboltOutlined /></template>生成全部{{ pendingCount ? ` (${pendingCount})` : '' }}
      </a-button>
      <a-button size="small" :loading="previewing" :disabled="!approvedCount" @click="openPreview">
        <template #icon><ExclamationCircleOutlined /></template>预览结果{{ approvedCount ? ` (${approvedCount})` : '' }}
      </a-button>
    </div>

    <!-- 批量进度条 -->
    <div v-if="generating && batchTotal" class="ar-batch-bar">
      <a-progress :percent="batchPct" size="small" :format="() => `${batchDone}/${batchTotal}`" />
    </div>

    <!-- 两栏 -->
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
          @click-item="onTreeClick"
          @hover-item="onCommitHover"
        >
          <template #header>
            <span class="ar-head-title"><HistoryOutlined /> 提交树</span>
            <span v-if="selectedHashes.size" class="ar-sel-hint">已选 {{ selectedHashes.size }} · Esc 取消</span>
            <span v-else :title="'双击加入任务 · Ctrl/Shift 多选后右键批量加入'">{{ entries.length }} 条</span>
          </template>
          <template #item="{ entry: e, index }">
            <div
              class="ar-tree-row"
              :class="{ 'need-ai': e.needAi, 'in-task': tasks.some(t => t.hash === e.hash) }"
              title="双击加入改写任务"
              @dblclick="addQuick(e)"
            >
              <span class="ar-num">#{{ index + 1 }}</span>
              <span class="ar-ai-dot" v-if="e.needAi" title="命中规则">●</span>
              <span class="ar-ai-dot dim" v-else>·</span>
              <div class="ar-tree-body">
                <div class="ar-tree-line">
                  <span class="ar-hash">{{ e.hash.slice(0, 7) }}</span>
                  <PushTag :pushed="e.pushed" />
                  <span class="ar-time">{{ relTime(e.date || '') }}</span>
                </div>
                <div class="ar-subject" :title="e.original">{{ e.original }}</div>
                <div class="ar-author">{{ e.author }}</div>
              </div>
              <!-- 行内快速加入/移除（hover 浮现，绝对定位不挤压布局） -->
              <button
                v-if="!tasks.some(t => t.hash === e.hash)"
                class="ar-row-add"
                title="加入改写任务"
                @click.stop="addQuick(e)"
                @dblclick.stop
              ><PlusOutlined /></button>
              <button
                v-else
                class="ar-row-in"
                title="已在任务队列，点击移除"
                @click.stop="removeByHash(e.hash)"
                @dblclick.stop
              >✓</button>
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

      <!-- 中栏：改写任务列表（分栏竖排） -->
      <aside class="ar-tasks">
        <div class="ar-tasks-head">
          <span class="ar-head-title"><ProfileOutlined /> 改写任务</span>
          <span>{{ taskCount ? `${filteredTasks.length}/${taskCount}` : '0 条' }}</span>
        </div>
        <div v-if="taskCount" class="ar-tasks-filter">
          <a-radio-group v-model:value="chipFilter" size="small" button-style="solid" class="ar-filter">
            <a-radio-button value="all">全部 {{ taskCount }}</a-radio-button>
            <a-radio-button value="pending">待生成 {{ pendingCount }}</a-radio-button>
            <a-radio-button value="approved">已采用 {{ approvedCount }}</a-radio-button>
          </a-radio-group>
        </div>
        <div class="ar-tasks-body">
          <div v-if="!taskCount" class="ar-tasks-empty">
            <PlusOutlined style="font-size:24px;opacity:0.3" />
            <div style="margin-top:8px">双击提交或悬停点 ＋ 加入<br/>右键支持 Ctrl/Shift 多选<br/>或点工具栏「按规则匹配」</div>
          </div>
          <template v-else>
            <div
              v-for="t in filteredTasks" :key="t.hash"
              class="ar-task-card"
              :class="['st-' + (t.status || 'pending'), { active: tasks.indexOf(t) === activeTaskIdx }]"
              @click="activeTaskIdx = tasks.indexOf(t)"
            >
              <div class="atc-bar"></div>
              <div class="atc-main">
                <div class="atc-line">
                  <span class="atc-num">#{{ tasks.indexOf(t) + 1 }}</span>
                  <span class="atc-hash">{{ t.hash.slice(0, 7) }}</span>
                  <span v-if="t.needAi" class="atc-flag" title="命中规则">⚠</span>
                  <span class="atc-st">{{ stText[t.status || 'pending'] }}</span>
                </div>
                <div class="atc-orig" :title="t.original">{{ t.original }}</div>
                <div v-if="t.suggested" class="atc-new" :title="t.suggested"><ArrowRightOutlined /> {{ t.suggested }}</div>
              </div>
              <button class="atc-x" title="移除" @click.stop="removeTask(tasks.indexOf(t))">×</button>
            </div>
            <div v-if="!filteredTasks.length" class="ar-tasks-empty">此筛选下无任务</div>
          </template>
        </div>
        <div v-if="taskCount" class="ar-tasks-foot">
          <span>已采用 <b class="ok">{{ approvedCount }}</b>/{{ taskCount }}</span>
          <div class="ar-foot-spacer" />
          <button v-if="skippedCount" class="ar-foot-btn" @click="removeSkipped">移除已跳过 {{ skippedCount }}</button>
          <button class="ar-foot-btn danger" @click="clearAll">清空</button>
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
        @quick-match="addMatchedToTasks"
      />
    </div>

    <!-- 提交树右键菜单（按选中状态动态：未入队→加入，已入队→移除） -->
    <ContextMenu v-model:open="treeContextMenu.open" :x="treeContextMenu.x" :y="treeContextMenu.y" :title="selectedHashes.size ? `已选 ${selectedHashes.size} 条` : '操作'">
      <CtxItem v-if="selectedHashes.size > selInTask" @click="addSelectedToTasks">
        <PlusOutlined /> 加入改写任务{{ selectedHashes.size - selInTask > 1 ? ` (${selectedHashes.size - selInTask})` : '' }}
      </CtxItem>
      <CtxItem v-if="selInTask" @click="removeSelectedFromTasks">
        <DeleteOutlined /> 从任务移除{{ selInTask > 1 ? ` (${selInTask})` : '' }}
      </CtxItem>
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
.ar-batch-bar { flex-shrink: 0; }

/* shared column-header title (icon + label) */
.ar-head-title { display: inline-flex; align-items: center; gap: 6px; }
.ar-head-title .anticon { font-size: 13px; opacity: 0.85; }

/* 三栏 — 1px gap (parent border colour shows as divider) */
.ar-cols { flex: 1; display: grid; grid-template-columns: 300px 280px 1fr; gap: 1px; background: var(--color-border); min-height: 0; overflow: hidden; }

/* 左栏：提交树 */
.ar-tree { background: var(--color-background); overflow: hidden; display: flex; flex-direction: column; }
/* Flatten CommitTree's card chrome + force its header to 34px to align with
   the workbench strip. */
.ar-tree :deep(.ct-list) { border-radius: 0; border: none; }
.ar-tree :deep(.ct-head) { height: 34px; padding: 0 12px; }
.ar-tree :deep(.ct-host) { flex: 1; }
/* 活动行 = 左侧细条光标（无底色），多选 = 填充底——两者视觉区分，
   Esc/再点一次即可全部取消（否则活动行看起来像"取消不掉的选中"） */
.ar-tree :deep(.ct-item.active) { background: transparent; border-left-color: var(--color-primary); }
.ar-tree :deep(.ct-item.selected) { background: rgba(var(--color-primary-rgb), 0.14); }
.ar-tree :deep(.ct-item.selected.active) { background: rgba(var(--color-primary-rgb), 0.14); }
.ar-tree-row { position: relative; display: flex; gap: 6px; padding: 6px 26px 6px 8px; }
.ar-num { font-family: 'Cascadia Code', monospace; font-size: 10px; font-weight: 700; color: var(--color-text-tertiary); flex-shrink: 0; min-width: 26px; margin-top: 1px; }
.ar-ai-dot { font-size: 10px; flex-shrink: 0; color: var(--color-warning); margin-top: 3px; }
.ar-ai-dot.dim { color: var(--color-text-tertiary); opacity: 0.4; }
.ar-tree-row.in-task { background: rgba(80, 250, 123, 0.06); }
.ar-tree-row.in-task .ar-ai-dot { color: var(--color-success); }
.ar-tree-body { flex: 1; min-width: 0; }
.ar-tree-line { display: flex; align-items: center; gap: 6px; margin-bottom: 2px; }
.ar-hash { font-family: 'Cascadia Code', monospace; font-size: 11px; color: var(--color-hash); }
.ar-time { font-size: 10px; color: var(--color-text-tertiary); margin-left: auto; flex-shrink: 0; }
.ar-subject { font-size: 12px; color: var(--color-text); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; margin-bottom: 1px; }
.ar-author { font-size: 10px; color: var(--color-text-tertiary); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.ar-sel-hint { font-size: 11px; color: var(--color-primary); }
/* 行内快速加入：绝对定位右侧，hover 浮现（不挤压布局避免抖动） */
.ar-row-add {
  position: absolute; right: 6px; top: 50%; transform: translateY(-50%);
  width: 20px; height: 20px;
  display: flex; align-items: center; justify-content: center;
  border: 1px solid var(--color-border); border-radius: var(--radius-xs);
  background: var(--color-surface); color: var(--color-text-secondary);
  cursor: pointer; opacity: 0; font-size: 11px;
  transition: opacity var(--transition-fast);
}
.ar-tree-row:hover .ar-row-add { opacity: 1; }
.ar-row-add:hover { color: var(--color-primary); border-color: var(--color-primary); }
.ar-row-in {
  position: absolute; right: 8px; top: 50%; transform: translateY(-50%);
  width: 20px; height: 20px;
  display: flex; align-items: center; justify-content: center;
  border: none; background: transparent;
  font-size: 11px; color: var(--color-success); opacity: 0.8;
  cursor: pointer; border-radius: var(--radius-xs);
  transition: color var(--transition-fast), opacity var(--transition-fast);
}
.ar-row-in:hover { color: var(--color-error); opacity: 1; }

/* 中栏：改写任务列表（分栏竖排） */
.ar-tasks { background: var(--color-background); display: flex; flex-direction: column; overflow: hidden; }
.ar-tasks-head { display: flex; align-items: center; justify-content: space-between; padding: 0 12px; height: 34px; flex-shrink: 0; font-size: 12px; font-weight: 600; border-bottom: 1px solid var(--color-border); background: var(--color-surface); }
.ar-tasks-filter { padding: 8px 10px; flex-shrink: 0; border-bottom: 1px solid var(--color-border-light); }
.ar-filter { display: flex; width: 100%; }
.ar-filter :deep(.ant-radio-button-wrapper) { flex: 1; padding: 0 4px; font-size: 11px; height: 22px; line-height: 20px; text-align: center; }
.ar-tasks-body { flex: 1; overflow-y: auto; padding: 4px; }
.ar-tasks-empty { flex: 1; display: flex; flex-direction: column; align-items: center; justify-content: center; color: var(--color-text-tertiary); font-size: 12px; text-align: center; padding: 40px 12px; }

.ar-task-card { display: flex; align-items: stretch; gap: 0; border-radius: var(--radius-xs); cursor: pointer; margin-bottom: 2px; position: relative; transition: background var(--transition-fast); }
.ar-task-card:hover { background: var(--color-hover); }
.ar-task-card.active { background: var(--color-primary-bg); }
.atc-bar { width: 3px; flex-shrink: 0; border-radius: 2px; background: var(--color-text-tertiary); }
.ar-task-card.st-suggested .atc-bar { background: var(--color-primary); }
.ar-task-card.st-approved .atc-bar { background: var(--color-success); }
.ar-task-card.st-retry .atc-bar { background: var(--color-error); }
.ar-task-card.st-skipped .atc-bar { background: var(--color-text-tertiary); opacity: 0.4; }
.atc-main { flex: 1; min-width: 0; padding: 5px 22px 5px 0; }
.atc-line { display: flex; align-items: center; gap: 5px; margin-bottom: 2px; }
.atc-num { font-family: 'Cascadia Code', monospace; font-size: 10px; color: var(--color-text-tertiary); }
.atc-hash { font-family: 'Cascadia Code', monospace; font-size: 11px; color: var(--color-hash); }
.atc-flag { color: var(--color-warning); font-size: 11px; }
.atc-st { margin-left: auto; font-size: 10px; color: var(--color-text-tertiary); flex-shrink: 0; }
.ar-task-card.st-approved .atc-st { color: var(--color-success); }
.ar-task-card.st-suggested .atc-st { color: var(--color-primary); }
.atc-orig { font-size: 12px; color: var(--color-text-secondary); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.atc-new { font-size: 11px; color: var(--color-primary); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; margin-top: 1px; }
.atc-x {
  position: absolute; right: 4px; top: 4px;
  width: 16px; height: 16px;
  border: none; background: transparent; color: var(--color-text-tertiary);
  font-size: 12px; line-height: 1; cursor: pointer; border-radius: 3px;
  opacity: 0; transition: opacity var(--transition-fast);
}
.ar-task-card:hover .atc-x { opacity: 1; }
.atc-x:hover { color: var(--color-error); background: var(--color-hover); }

.ar-tasks-foot { display: flex; align-items: center; gap: 8px; padding: 6px 12px; border-top: 1px solid var(--color-border); flex-shrink: 0; font-size: 12px; color: var(--color-text-secondary); background: var(--color-surface); }
.ar-tasks-foot .ok { color: var(--color-success); }
.ar-foot-spacer { flex: 1; }
.ar-foot-btn { background: transparent; border: none; padding: 2px 4px; font-size: 11px; color: var(--color-text-tertiary); cursor: pointer; border-radius: 3px; }
.ar-foot-btn:hover { color: var(--color-primary); }
.ar-foot-btn.danger:hover { color: var(--color-error); }
</style>
