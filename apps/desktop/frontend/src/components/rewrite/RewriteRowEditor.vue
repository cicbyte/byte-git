<script setup>
/**
 * 改写行内编辑面板 — 从 RewriteDataEditor 拆出。
 * 在 a-table 展开行中渲染：原始值 vs 改写值对比、字段分组、单行恢复。
 */
import { UndoOutlined } from '@ant-design/icons-vue'
import AiSparkIcon from '@/components/common/AiSparkIcon.vue'

const props = defineProps({
  // 改写行对象（需包含 original/rewrite/hash）
  row: { type: Object, default: null },
  // 该行 message 的 AI 整理进行中
  aiLoading: { type: Boolean, default: false },
})

const emit = defineEmits(['dirty', 'restore', 'ai-message'])

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

function isChanged(row) {
  if (!row) return false
  const o = row.original, r = row.rewrite
  return o.authorName !== r.authorName ||
    o.authorEmail !== r.authorEmail ||
    o.authorDate !== r.authorDate ||
    o.committerName !== r.committerName ||
    o.committerEmail !== r.committerEmail ||
    o.committerDate !== r.committerDate ||
    o.message !== r.message
}
function fieldChanged(row, key) {
  return row && row.original[key] !== row.rewrite[key]
}
function changedFieldNames(row) {
  if (!row) return []
  const all = [
    ['作者名', 'authorName'], ['作者邮箱', 'authorEmail'], ['作者日期', 'authorDate'],
    ['提交者名', 'committerName'], ['提交者邮箱', 'committerEmail'], ['提交者日期', 'committerDate'],
    ['message', 'message'],
  ]
  return all.filter(([, k]) => row.original[k] !== row.rewrite[k]).map(([n]) => n)
}
function onInput() { emit('dirty') }
</script>

<template>
  <div v-if="row" class="rde-edit-panel">
    <div class="rde-ep-head">
      <span class="rde-ep-hash">{{ row.hash.slice(0, 7) }}</span>
      <span v-if="isChanged(row)" class="rde-ep-changes">
        改 {{ changedFieldNames(row).length }} 项: {{ changedFieldNames(row).join('、') }}
      </span>
      <a-button size="small" type="text" :disabled="!isChanged(row)" title="恢复此行原值" @click="emit('restore', row)">
        <template #icon><UndoOutlined /></template>恢复
      </a-button>
    </div>

    <div class="rde-cmp-head">
      <span>原始值</span>
      <span>改写值</span>
    </div>

    <div class="rde-ep-group">
      <div class="rde-ep-gtitle">作者 (Author)</div>
      <div v-for="f in authorFields" :key="f.key" class="rde-cmp-row" :class="{ diff: fieldChanged(row, f.key) }">
        <span class="rde-orig" :title="row.original[f.key]">{{ row.original[f.key] || '—' }}</span>
        <a-input v-model:value="row.rewrite[f.key]" size="small" @input="onInput" />
      </div>
    </div>

    <div class="rde-ep-group">
      <div class="rde-ep-gtitle">提交者 (Committer)</div>
      <div v-for="f in committerFields" :key="f.key" class="rde-cmp-row" :class="{ diff: fieldChanged(row, f.key) }">
        <span class="rde-orig" :title="row.original[f.key]">{{ row.original[f.key] || '—' }}</span>
        <a-input v-model:value="row.rewrite[f.key]" size="small" @input="onInput" />
      </div>
    </div>

    <div class="rde-ep-group">
      <div class="rde-ep-gtitle">Message</div>
      <div class="rde-cmp-row subject" :class="{ diff: fieldChanged(row, 'message') }">
        <span class="rde-orig subject" :title="row.original.message">{{ row.original.message || '—' }}</span>
        <div class="rde-msg-wrap">
          <a-textarea
            v-model:value="row.rewrite.message"
            :auto-size="{ minRows: 2, maxRows: 12 }"
            placeholder="首行主题，空行后写正文"
            size="small"
            @input="onInput"
          />
          <button
            class="rde-ai-btn"
            :class="{ busy: aiLoading }"
            :disabled="aiLoading"
            title="AI 整理此条 message（按「提示词」页规则）"
            @click="emit('ai-message', row)"
          >
            <AiSparkIcon />
          </button>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.rde-edit-panel {
  padding: 12px 16px;
  background: var(--color-background);
  border-radius: var(--radius-sm);
  margin: 4px 0;
}
.rde-ep-head { display: flex; align-items: center; gap: 10px; margin-bottom: 10px; padding-bottom: 8px; border-bottom: 1px solid var(--color-border-light); }
.rde-ep-hash { font-family: 'Cascadia Code', monospace; font-size: 12px; color: var(--color-primary); font-weight: 600; }
.rde-ep-changes { font-size: 11px; color: var(--color-primary); flex: 1; }
.rde-cmp-head { display: grid; grid-template-columns: 1fr 1fr; gap: 12px; font-size: 11px; font-weight: 600; color: var(--color-text-tertiary); margin-bottom: 6px; }

.rde-ep-group { margin-bottom: 12px; }
.rde-ep-gtitle { font-size: 11px; font-weight: 600; color: var(--color-text-secondary); margin-bottom: 4px; }
.rde-cmp-row { display: grid; grid-template-columns: 1fr 1fr; gap: 12px; align-items: center; margin-bottom: 4px; }
.rde-cmp-row.diff { background: rgba(125, 108, 248, 0.06); border-radius: var(--radius-xs); padding: 2px 4px; }
.rde-orig { font-size: 12px; color: var(--color-text-secondary); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; padding: 4px 6px; border-radius: var(--radius-xs); background: var(--color-surface); }
.rde-cmp-row.diff .rde-orig { color: var(--color-text-tertiary); text-decoration: line-through; }
.rde-cmp-row.diff :deep(.ant-input) { border-color: var(--color-primary); }
.rde-cmp-row.subject { align-items: stretch; }
.rde-orig.subject { white-space: pre-wrap; word-break: break-word; overflow: visible; text-overflow: clip; display: flex; align-items: flex-start; }

/* Message 输入框右上角 AI 整理按钮 */
.rde-msg-wrap { position: relative; }
.rde-msg-wrap :deep(.ant-input) { padding-right: 30px; }
.rde-ai-btn {
  position: absolute;
  top: 5px;
  right: 5px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 22px;
  height: 22px;
  border: none;
  border-radius: var(--radius-xs);
  background: transparent;
  color: var(--color-text-tertiary);
  cursor: pointer;
  transition: all var(--transition-fast);
  z-index: 1;
}
.rde-ai-btn:hover { color: var(--color-primary); background: rgba(125, 108, 248, 0.12); }
.rde-ai-btn.busy { color: var(--color-primary); animation: rde-ai-pulse 1s ease-in-out infinite; pointer-events: none; }
@keyframes rde-ai-pulse { 0%, 100% { opacity: 1; } 50% { opacity: 0.35; } }
</style>
