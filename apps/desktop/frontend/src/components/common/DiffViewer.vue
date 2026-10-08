<script setup>
import { computed } from 'vue'
import CodeViewer from './CodeViewer.vue'

/**
 * Side-by-side diff view: 原始 (before) vs 修改后 (after)。
 * 用 LCS 行级对比算出增/删行号，分别高亮（左红=删除，右绿=新增）。
 */
const props = defineProps({
  before: { type: String, default: '' },
  after: { type: String, default: '' },
  filename: { type: String, default: '' },
  status: { type: String, default: 'M' }, // A/M/D
  binary: { type: Boolean, default: false },
  notFound: { type: Boolean, default: false }, // before 不存在（纯新增）
  loading: { type: Boolean, default: false },
})

// before 是否为空：新增文件、父版本不存在、或内容确实为空
const beforeEmpty = computed(() => props.status === 'A' || props.notFound || !props.before)

// ---- LCS line diff ----
// returns { beforeLines: [1-based line numbers to highlight as removed],
//           afterLines:  [1-based line numbers to highlight as added] }
function computeLineDiff(a, b) {
  // normalize CRLF → LF so Windows files (CRLF) vs git output (LF) compare
  // correctly; otherwise every line looks "changed" and the whole file lights up.
  const norm = (s) => s.replace(/\r\n/g, '\n').replace(/\r/g, '\n')
  const A = a.length ? norm(a).split('\n') : []
  const B = b.length ? norm(b).split('\n') : []
  const n = A.length, m = B.length
  // dp table for LCS length
  const dp = Array.from({ length: n + 1 }, () => new Int32Array(m + 1))
  for (let i = n - 1; i >= 0; i--) {
    for (let j = m - 1; j >= 0; j--) {
      dp[i][j] = A[i] === B[j] ? dp[i + 1][j + 1] + 1 : Math.max(dp[i + 1][j], dp[i][j + 1])
    }
  }
  const dels = [] // removed line numbers (1-based) in A
  const adds = [] // added line numbers (1-based) in B
  let i = 0, j = 0
  while (i < n && j < m) {
    if (A[i] === B[j]) { i++; j++ }
    else if (dp[i + 1][j] >= dp[i][j + 1]) { dels.push(i + 1); i++ }
    else { adds.push(j + 1); j++ }
  }
  while (i < n) { dels.push(i + 1); i++ }
  while (j < m) { adds.push(j + 1); j++ }
  return { beforeLines: dels, afterLines: adds }
}

// only compute diff for modified files (not A/D/binary/empty)
const diff = computed(() => {
  if (props.status !== 'M' || beforeEmpty.value || !props.after || props.binary) {
    return { beforeLines: [], afterLines: [] }
  }
  return computeLineDiff(props.before, props.after)
})
</script>

<template>
  <div class="dv-wrap">
    <div v-if="loading" class="dv-center"><a-spin /></div>

    <div v-else-if="binary" class="dv-center">⚠️ 二进制文件，无法展示差异</div>

    <div v-else class="dv-panels">
      <!-- 左：原始 -->
      <div class="dv-side">
        <div class="dv-head left">
          <span class="dv-label">原始</span>
          <span class="dv-file">{{ filename }}</span>
          <span class="dv-delta" v-if="diff.beforeLines.length">-{{ diff.beforeLines.length }}</span>
          <span v-if="status === 'A'" class="dv-tag new">新文件</span>
        </div>
        <div class="dv-body">
          <div v-if="beforeEmpty" class="dv-empty">（此版本无内容）</div>
          <CodeViewer
            v-else
            :key="'before-' + filename"
            :code="before"
            :filename="filename"
            :highlight-lines="diff.beforeLines"
            highlight-class="dv-del-line"
          />
        </div>
      </div>

      <!-- 右：修改后 -->
      <div class="dv-side">
        <div class="dv-head right">
          <span class="dv-label">修改后</span>
          <span class="dv-file">{{ filename }}</span>
          <span class="dv-delta add" v-if="diff.afterLines.length">+{{ diff.afterLines.length }}</span>
          <span v-if="status === 'D'" class="dv-tag del">已删除</span>
        </div>
        <div class="dv-body">
          <div v-if="status === 'D'" class="dv-empty">（文件已删除）</div>
          <CodeViewer
            v-else
            :key="'after-' + filename"
            :code="after"
            :filename="filename"
            :highlight-lines="diff.afterLines"
            highlight-class="dv-add-line"
          />
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.dv-wrap { width: 100%; height: 100%; display: flex; min-height: 0; }
.dv-center { flex: 1; display: flex; align-items: center; justify-content: center; color: var(--color-text-tertiary); font-size: 13px; }

.dv-panels { flex: 1; display: flex; gap: 1px; background: var(--color-border); min-width: 0; min-height: 0; }
.dv-side { flex: 1; min-width: 0; min-height: 0; display: flex; flex-direction: column; background: var(--color-surface); }

.dv-head {
  display: flex; align-items: center; gap: 8px;
  padding: 0 12px; height: 32px; flex-shrink: 0;
  font-size: 12px; border-bottom: 1px solid var(--color-border);
  background: var(--color-surface);
}
.dv-head.left { border-right: 1px solid var(--color-border); }
.dv-label { font-weight: 600; color: var(--color-text-secondary); }
.dv-file { font-family: 'Cascadia Code', monospace; font-size: 11px; color: var(--color-text-tertiary); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.dv-delta { margin-left: auto; font-family: 'Cascadia Code', monospace; font-size: 11px; font-weight: 700; color: var(--color-error); flex-shrink: 0; }
.dv-delta.add { color: var(--color-success); }
.dv-tag { font-size: 10px; font-weight: 700; padding: 1px 6px; border-radius: var(--radius-xs); margin-left: auto; flex-shrink: 0; }
.dv-tag.new { color: var(--color-success); background: rgba(80,250,123,0.12); }
.dv-tag.del { color: var(--color-error); background: rgba(255,85,85,0.12); }

.dv-body { flex: 1; min-height: 0; overflow: hidden; position: relative; }
.dv-body :deep(.cv-host) { height: 100%; }
.dv-empty {
  height: 100%; display: flex; align-items: center; justify-content: center;
  color: var(--color-text-tertiary); font-size: 13px; font-style: italic;
  background: var(--color-surface-raised); opacity: 0.6;
}
</style>

<!-- diff line highlight classes (global, not scoped, so CodeMirror picks them up) -->
<style>
.dv-del-line { background: rgba(255, 85, 85, 0.14) !important; }
.dv-add-line { background: rgba(80, 250, 123, 0.14) !important; }
</style>
