<script setup>
import { ref, computed, onMounted, watch } from 'vue'
import { message as AMessage } from 'ant-design-vue'
import {
  ExportOutlined, ExclamationCircleOutlined, UndoOutlined,
  EditOutlined, CloseOutlined, FolderOpenOutlined, CalendarOutlined,
  ReloadOutlined, HistoryOutlined
} from '@ant-design/icons-vue'
import { useRepoStore } from '@/stores/repo'
import ExportWizard from '@/components/rewrite/ExportWizard.vue'
import DailyDistributeModal from '@/components/rewrite/DailyDistributeModal.vue'
import { relTime } from '@/composables/useRelativeTime'
import CommitTree from '@/components/common/CommitTree.vue'
import CommitHoverCard from '@/components/common/CommitHoverCard.vue'
import PushTag from '@/components/common/PushTag.vue'
import ContextMenu from '@/components/common/ContextMenu.vue'
import CtxItem from '@/components/common/CtxItem.vue'
import MetaPreviewModal from './components/MetaPreviewModal.vue'
import { useCommitStat } from '@/composables/useCommitStat'
const repo = useRepoStore()
const app = window.go?.main?.ByteGitApp
// per-commit change stat (lazy, for the hover card) — same composable the
// log / time-travel views use, so hover content is identical across the app.
const { stat: statOf, has: hasStat, onHover: onStatHover, onLeave: onStatLeave, clear: clearStat } =
  useCommitStat((h) => app.CommitStat(h))
function onCommitHover(entry, isEnter) {
  if (isEnter) onStatHover(entry.hash)
  else onStatLeave(entry.hash)
}
const entries = ref([])
const exporting = ref(false)
const activeHash = ref('')
const wizardVisible = ref(false)
// ---- pagination (infinite scroll) ----
const PAGE = 50
const hasMore = ref(true)
const loadingMore = ref(false)

// ---- multi-select (selection state only; logic lives in <CommitTree>) ----
const selectedHashes = ref(new Set())   // bulk-selected hashes
const contextMenu = ref({ open: false, x: 0, y: 0 })
const selectedEntries = computed(() => entries.value.filter((e) => selectedHashes.value.has(e.hash)))
const selectedCount = computed(() => selectedHashes.value.size)
// CommitTree emits the raw contextmenu event; we position the menu.
function onTreeContextmenu(entry, evt) {
  contextMenu.value = { open: true, x: evt.clientX, y: evt.clientY }
}
function closeContextMenu() { contextMenu.value.open = false }
// ---- bulk operations ----
function bulkRestore() {
  selectedEntries.value.forEach((e) => {
    e.rewrite = JSON.parse(JSON.stringify(e.original))
  })
  AMessage.success(`已恢复 ${selectedCount.value} 条`)
  closeContextMenu()
}
// quick reload a range without opening the wizard (skips step 1)
async function quickExport(modeName, n = 50) {
  closeContextMenu()
  await runExport({ mode: modeName, n, rules: [], remap: null })
}
function bulkClear() {
  // alias of restore for clarity in menu
  bulkRestore()
}
function bulkApplyRule() {
  const rule = window.prompt('输入规则（如 author:=新名 / email:旧=新 / date-shift:+2h）')
  if (!rule) { closeContextMenu(); return }
  applyRuleToSelection(rule)
}
async function applyRuleToSelection(rule) {
  if (!app) return
  try {
    const subset = selectedEntries.value
    const updated = await app.MetaRewriteApplyRule(JSON.parse(JSON.stringify(subset)), rule)
    // merge back by hash
    const map = new Map(updated.map((u) => [u.hash, u]))
    entries.value.forEach((e, i) => {
      if (map.has(e.hash)) entries.value[i] = map.get(e.hash)
    })
    AMessage.success(`已对 ${selectedCount.value} 条应用规则`)
  } catch (e) {
    AMessage.error('规则应用失败: ' + String(e?.message || e))
  } finally {
    closeContextMenu()
  }
}

const activeEntry = computed(() => entries.value.find((e) => e.hash === activeHash.value))
const hasExported = computed(() => entries.value.length > 0)
// split a full commit message into subject (first line) + body (the rest)
function splitMsg(msg) {
  const s = msg || ''
  const idx = s.indexOf('\n')
  if (idx < 0) return { subject: s, body: '' }
  return { subject: s.slice(0, idx), body: s.slice(idx + 1).replace(/^\n+/, '') }
}
// count changed fields for an entry
function changedFields(e) {
  if (!e) return []
  const o = e.original, r = e.rewrite
  const fields = [
    ['作者名', o.authorName, r.authorName],
    ['作者邮箱', o.authorEmail, r.authorEmail],
    ['作者日期', o.authorDate, r.authorDate],
    ['提交者名', o.committerName, r.committerName],
    ['提交者邮箱', o.committerEmail, r.committerEmail],
    ['提交者日期', o.committerDate, r.committerDate],
    ['message', o.message, r.message],
  ]
  return fields.filter(([, a, b]) => a !== b)
}

const isChanged = (e) => changedFields(e).length > 0
const changedCount = computed(() => entries.value.filter(isChanged).length)
// run the export with options collected by the wizard (resets to first page)
async function runExport(opts) {
  if (!app) return
  exporting.value = true
  hasMore.value = true
  try {
    // persist: true — this is an explicit export (wizard / quick export), so
    // write rewrite.json for the data editor to read. Passive loads (entry /
    // pagination) pass persist:false and must not create the file.
    const fetchOpts = { ...opts, skip: 0, persist: true }
    const data = await app.MetaRewriteExport(repo.repoArg(), fetchOpts)
    entries.value = data || []
    if ((data || []).length < PAGE) hasMore.value = false
    activeHash.value = entries.value[0]?.hash || ''
    wizardVisible.value = false
    AMessage.success(`已加载 ${entries.value.length} 条提交`)
  } catch (e) {
    AMessage.error('导出失败: ' + String(e?.message || e))
  } finally {
    exporting.value = false
  }
}

// load next page (called by <CommitTree> infinite scroll sentinel)
async function loadMore(done) {
  if (!app || !hasMore.value || loadingMore.value || !entries.value.length) { done?.(); return }
  loadingMore.value = true
  try {
    const data = await app.MetaRewriteExport(repo.repoArg(), {
      mode: 'last', n: PAGE, skip: entries.value.length, rules: [], remap: null, persist: false,
    })
    const arr = data || []
    if (arr.length) entries.value.push(...arr)
    if (arr.length < PAGE) hasMore.value = false
  } catch {
    hasMore.value = false
  } finally {
    loadingMore.value = false
    done?.()
  }
}
// auto-load the commit tree on entry (first page, no rules/remap)
async function autoLoad() {
  if (!app) return
  exporting.value = true
  hasMore.value = true
  try {
    const data = await app.MetaRewriteExport(repo.repoArg(), { mode: 'last', n: PAGE, skip: 0, rules: [], remap: null, persist: false })
    entries.value = data || []
    if ((data || []).length < PAGE) hasMore.value = false
    activeHash.value = entries.value[0]?.hash || ''
  } catch {
    // silent — tree just stays empty, user can retry via wizard
  } finally {
    exporting.value = false
  }
}
onMounted(() => {
  autoLoad()
})

// reload when the repo is switched (repoEpoch bumps on each switch)
watch(() => repo.repoEpoch, () => { clearStat(); if (hasExported.value) autoLoad() })

// ---- auto-save edited rewrite data to rewrite.json ----
// v-model edits mutate entries in place; debounce-persist so the data editor
// and later sessions see the rewrite work. flush:'sync' + exporting/loadingMore
// guards ensure passive loads (entry / pagination) — which also mutate entries —
// do NOT write the file; only real edits (field edit / restore / apply-rule) do.
let saveTimer = null
function scheduleSave() {
  if (exporting.value || loadingMore.value || !hasData.value || !app?.JsonEditorWrite) return
  if (saveTimer) clearTimeout(saveTimer)
  saveTimer = setTimeout(async () => {
    if (exporting.value || loadingMore.value) return  // a load started during the debounce
    try { await app.JsonEditorWrite(JSON.stringify(entries.value, null, 2)) }
    catch { /* best-effort auto-save */ }
  }, 600)
}
watch(entries, scheduleSave, { deep: true, flush: 'sync' })
function clearAll() {
  entries.value = []
  activeHash.value = ''
}
// restore active entry to original
function restoreActive() {
  if (!activeEntry.value) return
  activeEntry.value.rewrite = JSON.parse(JSON.stringify(activeEntry.value.original))
}

// ---- preview & apply ----
const previewVisible = ref(false)
const preview = ref(null)
const previewing = ref(false)

async function openPreview() {
  if (!changedCount.value) {
    AMessage.warning('没有需要改写的提交')
    return
  }
  previewing.value = true
  try {
    preview.value = await app.MetaRewritePreview(repo.repoArg(), entries.value)
    previewVisible.value = true
  } catch (e) {
    AMessage.error('预览失败: ' + String(e?.message || e))
  } finally {
    previewing.value = false
  }
}

// 改写应用完成（来自 MetaPreviewModal）：历史已变更，内存数据失效，重新加载。
async function onApplied() {
  selectedHashes.value = new Set()
  await autoLoad()
}

// ---- daily distribute (per-day commit count allocation) ----
const distributeOpen = ref(false)
const distributing = ref(false)

function openDistribute() {
  closeContextMenu()
  if (!entries.value.length) {
    AMessage.warning('请先加载提交')
    return
  }
  distributeOpen.value = true
}

async function onDistributeApply({ allocs, mode: dm, activeHour, sizeWeight }) {
  if (!app) return
  distributing.value = true
  try {
    const updated = await app.MetaRewriteDistributeByDays(
      JSON.parse(JSON.stringify(entries.value)), allocs, dm, activeHour, !!sizeWeight,
    )
    // 合并：按 hash 回填，仅 rewrite 字段更新（original/hash/pushed 保持）
    const map = new Map(updated.map((u) => [u.hash, u]))
    entries.value.forEach((e, i) => {
      if (map.has(e.hash)) entries.value[i] = map.get(e.hash)
    })
    distributeOpen.value = false
    AMessage.success(`已按 ${allocs.length} 天分配 ${entries.value.length} 条提交日期`)
  } catch (e) {
    AMessage.error('逐日分配失败: ' + String(e?.message || e))
  } finally {
    distributing.value = false
  }
}

// field config for the editor form
const authorFields = [
  { key: 'authorName', label: '作者名' },
  { key: 'authorEmail', label: '作者邮箱' },
  { key: 'authorDate', label: '作者日期' },
]
const committerFields = [
  { key: 'committerName', label: '提交者名' },
  { key: 'committerEmail', label: '提交者邮箱' },
  { key: 'committerDate', label: '提交者日期' },
]
</script>

<template>
  <div class="mr-view">
    <!-- Status bar (always visible — the page renders immediately and the
         commit tree loads asynchronously into <CommitTree>, like ai-rewrite) -->
    <div class="mr-statusbar">
        <span class="sb-count">已加载 <b>{{ entries.length }}</b> 条 · 已改 <b>{{ changedCount }}</b></span>
        <a-button size="small" type="text" @click="wizardVisible = true" title="重新导出">重新导出</a-button>
        <a-button size="small" type="text" :loading="exporting" @click="autoLoad" title="刷新">
          <template #icon><ReloadOutlined /></template>刷新
        </a-button>
        <a-button size="small" type="text" @click="clearAll">清空</a-button>
      </div>

      <div class="mr-cols">
      <!-- Left: commit tree -->
      <aside class="mr-list">
        <CommitTree
          :items="entries"
          :loading="exporting && !hasExported"
          v-model:active="activeHash"
          v-model:selected="selectedHashes"
          :has-more="hasMore"
          tooltip-placement="right"
          tooltip-overlay-class="mr-tip"
          @load-more="loadMore"
          @contextmenu="onTreeContextmenu"
          @hover-item="onCommitHover"
        >
          <template #header>
            <span class="mr-head-title"><HistoryOutlined /> 提交树</span>
            <span>{{ changedCount }}/{{ entries.length }} 已改</span>
          </template>

          <template #item="{ entry: e, index }">
            <div class="mr-item-row" :class="{ changed: isChanged(e) }">
              <span class="mr-num">#{{ index + 1 }}</span>
              <span class="mr-status">{{ isChanged(e) ? '●' : '·' }}</span>
              <div class="mr-item-body">
                <div class="mr-tree-line">
                  <span class="mr-hash">{{ e.hash.slice(0, 7) }}</span>
                  <span class="mr-author">{{ e.original.authorName }}</span>
                  <span class="mr-time">{{ relTime(e.original.authorDate) }}</span>
                  <PushTag :pushed="e.pushed" />
                </div>
                <div class="mr-subject" :title="e.original.message">{{ (e.original.message || '').split('\n')[0] }}</div>
                <div v-if="isChanged(e)" class="mr-changes">改 {{ changedFields(e).length }} 项: {{ changedFields(e).map(f => f[0]).join('·') }}</div>
              </div>
            </div>
          </template>

          <template #tooltip="{ entry: e, index }">
            <CommitHoverCard
              :commit="{
                hash: e.hash,
                subject: splitMsg(e.original.message).subject,
                body: splitMsg(e.original.message).body,
                author: e.original.authorName,
                date: e.original.authorDate,
              }"
              :num="'#' + (index + 1)"
              :stat="statOf(e.hash)"
              :stat-loading="!hasStat(e.hash)"
            />
          </template>
        </CommitTree>
      </aside>

      <!-- Right: rewrite data editor -->
      <section class="mr-editor" v-if="activeEntry">
        <div class="mr-ed-head">
          <span class="mr-ed-title"><EditOutlined /> 改写数据</span>
          <span class="mr-hash">{{ activeEntry.hash.slice(0, 7) }}</span>
          <span class="mr-date">{{ relTime(activeEntry.original.authorDate) }}</span>
          <a-button size="small" type="text" @click="restoreActive" title="恢复原值">
            <template #icon><UndoOutlined /></template>
          </a-button>
        </div>

        <div class="mr-ed-body">
        <!-- two-column compare: original (read-only) | rewrite (editable) -->
        <div class="mr-compare-head">
          <span>原始数据</span>
          <span>改写预览</span>
        </div>

        <div class="mr-ed-group">
          <div class="mr-ed-gtitle">作者 (Author)</div>
          <div v-for="f in authorFields" :key="f.key" class="mr-cmp-row" :class="{ diff: activeEntry.original[f.key] !== activeEntry.rewrite[f.key] }">
            <span class="mr-orig-val" :title="activeEntry.original[f.key]">{{ activeEntry.original[f.key] || '—' }}</span>
            <a-input v-model:value="activeEntry.rewrite[f.key]" size="small" />
          </div>
        </div>

        <div class="mr-ed-group">
          <div class="mr-ed-gtitle">提交者 (Committer)</div>
          <div v-for="f in committerFields" :key="f.key" class="mr-cmp-row" :class="{ diff: activeEntry.original[f.key] !== activeEntry.rewrite[f.key] }">
            <span class="mr-orig-val" :title="activeEntry.original[f.key]">{{ activeEntry.original[f.key] || '—' }}</span>
            <a-input v-model:value="activeEntry.rewrite[f.key]" size="small" />
          </div>
        </div>

        <div class="mr-ed-group">
          <div class="mr-ed-gtitle">Message</div>
          <div class="mr-cmp-row subject" :class="{ diff: activeEntry.original.message !== activeEntry.rewrite.message }">
            <span class="mr-orig-val subject" :title="activeEntry.original.message">{{ activeEntry.original.message || '—' }}</span>
            <a-textarea v-model:value="activeEntry.rewrite.message" :auto-size="{ minRows: 2, maxRows: 12 }" placeholder="首行主题，空行后写正文" size="small" />
          </div>
        </div>

        <div class="mr-ed-changes" v-if="isChanged(activeEntry)">
          改动 {{ changedFields(activeEntry).length }} 项: {{ changedFields(activeEntry).map(f => f[0]).join('、') }}
        </div>
        </div>
      </section>
    </div>

    <!-- Bottom: apply -->
    <div class="mr-bottom">
      <div class="mr-spacer"></div>
      <span class="mr-stat">已改 {{ changedCount }} / 共 {{ entries.length }}</span>
      <a-button :loading="distributing" :disabled="!entries.length" @click="openDistribute">
        <template #icon><CalendarOutlined /></template>逐日分配
      </a-button>
      <a-button :loading="previewing" :disabled="!changedCount" @click="openPreview">
        <template #icon><ExclamationCircleOutlined /></template>预览改写影响
      </a-button>
    </div>

    <!-- Context menu -->
    <ContextMenu
      v-model:open="contextMenu.open"
      :x="contextMenu.x"
      :y="contextMenu.y"
      :title="`已选 ${selectedCount} 条`"
    >
      <CtxItem @click="bulkRestore"><UndoOutlined /> 恢复原值</CtxItem>
      <CtxItem @click="bulkApplyRule"><EditOutlined /> 应用规则...</CtxItem>
      <CtxItem @click="openDistribute"><CalendarOutlined /> 逐日分配...</CtxItem>
      <CtxItem danger @click="bulkClear"><CloseOutlined /> 清空改动</CtxItem>
      <div class="ctx-sep"></div>
      <CtxItem @click="quickExport('all')"><FolderOpenOutlined /> 加载全部</CtxItem>
      <CtxItem @click="quickExport('last', 50)">最近 50 条</CtxItem>
      <CtxItem @click="quickExport('last', 100)">最近 100 条</CtxItem>
      <CtxItem @click="quickExport('unpushed')">仅未推送</CtxItem>
      <div class="ctx-sep"></div>
      <CtxItem @click="wizardVisible = true; closeContextMenu()"><ExportOutlined /> 导出向导...</CtxItem>
    </ContextMenu>

    <!-- Export wizard -->
    <ExportWizard
      v-model:open="wizardVisible"
      :loading="exporting"
      :repo-arg="repo.repoArg()"
      @export="runExport"
    />

    <!-- Daily distribute modal -->
    <DailyDistributeModal
      v-model:open="distributeOpen"
      :total-count="entries.length"
      :loading="distributing"
      @apply="onDistributeApply"
    />

    <!-- Preview modal -->
    <MetaPreviewModal
      v-model:open="previewVisible"
      :preview="preview"
      :repo-arg="repo.repoArg()"
      :entries="entries"
      @applied="onApplied"
    />
  </div>
</template>

<style scoped>
/* Full-bleed: negate .content-wrapper's 24px padding so the page is flush
   with the navbar / Sidebar / footer, matching Settings + reference-ops. */
.mr-view {
  width: calc(100% + var(--spacing-lg) * 2);
  height: calc(100% + var(--spacing-lg) * 2);
  margin: calc(-1 * var(--spacing-lg));
  display: flex; flex-direction: column;
  min-height: 0;
  background: transparent; /* 1px gap separator between columns */
}
/* empty state */
.mr-empty { flex: 1; display: flex; align-items: center; justify-content: center; }
.empty-card { display: flex; flex-direction: column; align-items: center; gap: var(--spacing-sm); padding: 48px 40px; text-align: center; max-width: 420px; }
.empty-icon { font-size: 40px; color: var(--color-primary); opacity: 0.8; }
.empty-title { font-size: 18px; font-weight: 600; color: var(--color-text); }
.empty-desc { font-size: 13px; color: var(--color-text-secondary); line-height: 1.6; }
/* status bar (after export) — flush strip, no card chrome */
.mr-statusbar {
  display: flex; align-items: center; gap: var(--spacing-sm);
  flex-shrink: 0;
  height: 38px;
  padding: 0 var(--spacing-md);
  background: var(--color-surface);
  border-bottom: 1px solid var(--color-border);
}
.sb-count { font-size: 13px; color: var(--color-text-secondary); }
.sb-count b { color: var(--color-text); }
.mr-spacer { flex: 1; }
.mr-head-title { display: inline-flex; align-items: center; gap: 6px; }
.mr-head-title .anticon { font-size: 13px; opacity: 0.85; }
/* columns — 1px gap (the parent's border colour shows through as divider) */
.mr-cols { flex: 1; display: flex; gap: 1px; min-height: 0; }
/* list — width only; chrome lives in <CommitTree> */
.mr-list { width: 320px; flex-shrink: 0; background: var(--color-surface); display: flex; flex-direction: column; min-height: 0; }
/* Flatten CommitTree's inner card so column 1 is flush with column 2. */
.mr-list :deep(.ct-list) { border-radius: 0; border: none; }
/* Force CommitTree's header to 34px so it aligns with .mr-ed-head. */
.mr-list :deep(.ct-head) { height: 34px; padding: 0 12px; }
.mr-item-row { display: flex; gap: 6px; padding: 6px 8px; }
.mr-num { font-family: 'Cascadia Code', monospace; font-size: 10px; font-weight: 700; color: var(--color-text-tertiary); flex-shrink: 0; min-width: 26px; margin-top: 1px; }
.mr-status { color: var(--color-text-tertiary); font-size: 10px; margin-top: 3px; }
.mr-item-row.changed .mr-status { color: var(--color-primary); }
.mr-item-body { flex: 1; min-width: 0; }
.mr-tree-line { display: flex; align-items: center; gap: 6px; margin-bottom: 2px; }
.mr-hash { font-family: 'Cascadia Code', monospace; font-size: 11px; color: var(--color-hash); }
.mr-author { font-size: 11px; color: var(--color-text-secondary); flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.mr-time { font-size: 10px; color: var(--color-text-tertiary); flex-shrink: 0; }
.mr-subject { font-size: 12px; color: var(--color-text); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.mr-changes { font-size: 10px; color: var(--color-primary); margin-top: 2px; }
/* editor — flush column, no card chrome. */
.mr-editor { flex: 1; min-width: 0; background: var(--color-surface); padding: 0; overflow-y: auto; display: flex; flex-direction: column; }
.mr-ed-head {
  display: flex; align-items: center; gap: var(--spacing-sm);
  height: 34px; padding: 0 var(--spacing-md);
  flex-shrink: 0;
  border-bottom: 1px solid var(--color-border);
}
.mr-ed-title { font-size: 12px; font-weight: 600; color: var(--color-text-secondary); margin-right: auto; display: inline-flex; align-items: center; gap: 6px; }
.mr-ed-title .anticon { font-size: 13px; opacity: 0.85; }
.mr-date { font-size: 11px; color: var(--color-text-tertiary); flex: 1; }
/* editor body gets its own scroll + padding (head is now fixed) */
.mr-ed-body { padding: var(--spacing-md); flex: 1; overflow-y: auto; }
.mr-ed-group { margin-bottom: var(--spacing-md); }
.mr-ed-gtitle { font-size: 12px; font-weight: 600; color: var(--color-text-secondary); margin-bottom: var(--spacing-xs); }
.mr-compare-head { display: grid; grid-template-columns: 1fr 1fr; gap: var(--spacing-sm); padding: var(--spacing-xs) 0 var(--spacing-xs); font-size: 11px; font-weight: 600; color: var(--color-text-tertiary); border-bottom: 1px solid var(--color-border-light); margin-bottom: var(--spacing-sm); }
.mr-cmp-row { display: grid; grid-template-columns: 1fr 1fr; gap: var(--spacing-sm); align-items: center; margin-bottom: 6px; }
.mr-cmp-row.diff { background: rgba(125, 108, 248, 0.06); border-radius: var(--radius-xs); padding: 2px 0; }
.mr-orig-val { font-size: 12px; color: var(--color-text-secondary); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; padding: 4px 6px; border-radius: var(--radius-xs); background: var(--color-background); }
.mr-cmp-row.diff .mr-orig-val { color: var(--color-text-tertiary); text-decoration: line-through; }
.mr-cmp-row.diff :deep(.ant-input) { border-color: var(--color-primary); }
.mr-cmp-row.subject { align-items: stretch; }
.mr-cmp-row.subject .mr-orig-val { white-space: pre-wrap; word-break: break-word; overflow: visible; text-overflow: clip; display: flex; align-items: flex-start; }
.mr-cmp-row.subject.diff .mr-orig-val.subject { white-space: pre-wrap; }
.mr-ed-changes { font-size: 12px; color: var(--color-primary); padding-top: var(--spacing-sm); border-top: 1px solid var(--color-border-light); }
/* bottom */
.mr-bottom { display: flex; align-items: center; gap: var(--spacing-md); flex-shrink: 0; padding-top: var(--spacing-xs); border-top: 1px solid var(--color-border); }
.mr-backup { display: none; }
.mr-stat { font-size: 12px; color: var(--color-text-secondary); }
</style>
