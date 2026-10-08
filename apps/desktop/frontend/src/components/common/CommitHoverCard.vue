<script setup>
/**
 * Hover card for a commit — rendered inside an <a-tooltip> #title slot.
 *
 * Shows: hash (+ optional reverse number) · type badge · scope · subject ·
 * author · date · change summary (+added -deleted, N files) · file list with
 * per-file status + delta. Pass `stat` empty + `statLoading` to show a
 * "loading…" placeholder (the caller lazy-loads CommitStat on hover).
 *
 * Reusable: paired with <CommitTree>'s `#tooltip` slot and the
 * `useCommitStat` composable, but has no hard dependency on either — pass
 * plain props and it renders.
 *
 * Scoped styles survive ant-design's portal (Vue 3 data-v attrs travel with
 * the nodes), so no global CSS block is needed.
 */
import { computed } from 'vue'

const props = defineProps({
  /** Commit object: { hash, subject, body?, author?, date?, type?, scope? } */
  commit: { type: Object, required: true },
  /** Optional reverse number label, e.g. "#42" (oldest = #1). */
  num: { type: [String, Number], default: '' },
  /** CommitStat entries (file/status/added/deleted). Empty = not loaded. */
  stat: { type: Array, default: () => [] },
  /** Show the "loading…" placeholder instead of the empty state. */
  statLoading: { type: Boolean, default: false },
  /** Max files to list before "…还有 N 个文件". */
  maxFiles: { type: Number, default: 8 },
})

// show a short hash (7 chars) regardless of what the caller passes — callers
// pass the full 40-char hash, but every row elsewhere shows the short form, so
// the tooltip must match to avoid a layout-busting long hash line.
const shortHash = computed(() => {
  const h = props.commit.hash || ''
  return h.length > 7 ? h.slice(0, 7) : h
})
const totalAdded = computed(() => props.stat.reduce((s, e) => s + e.added, 0))
const totalDeleted = computed(() => props.stat.reduce((s, e) => s + e.deleted, 0))
const shownFiles = computed(() => props.stat.slice(0, props.maxFiles))
const hiddenCount = computed(() => Math.max(0, props.stat.length - props.maxFiles))
</script>

<template>
  <div class="chc-card">
    <div class="chc-hash">
      <span v-if="num !== ''" class="chc-num">{{ num }}</span>
      <span v-if="commit.hash" class="chc-hash-val">{{ shortHash }}</span>
      <span v-if="commit.type" class="chc-type" :class="'type-' + commit.type">{{ commit.type }}</span>
      <span v-if="commit.scope" class="chc-scope">({{ commit.scope }})</span>
    </div>
    <div v-if="commit.subject" class="chc-subject">{{ commit.subject }}</div>
    <div v-if="commit.body" class="chc-body">{{ commit.body }}</div>
    <div v-if="commit.author" class="chc-row"><span class="chc-k">作者</span><span class="chc-v">{{ commit.author }}</span></div>
    <div v-if="commit.date" class="chc-row"><span class="chc-k">时间</span><span class="chc-v">{{ commit.date }}</span></div>

    <template v-if="stat.length">
      <div class="chc-row">
        <span class="chc-k">变更</span>
        <span class="chc-v">
          <span class="stat-add">+{{ totalAdded }}</span>
          <span class="stat-del">-{{ totalDeleted }}</span>
          <span class="stat-files">{{ stat.length }} 文件</span>
        </span>
      </div>
      <div class="chc-files">
        <div v-for="f in shownFiles" :key="f.file" class="chc-file">
          <span class="chc-file-status" :class="'st-' + f.status">{{ f.status }}</span>
          <span class="chc-file-name" :title="f.file">{{ f.file }}</span>
          <span class="chc-file-delta"><span class="stat-add">+{{ f.added }}</span> <span class="stat-del">-{{ f.deleted }}</span></span>
        </div>
        <div v-if="hiddenCount" class="chc-more">…还有 {{ hiddenCount }} 个文件</div>
      </div>
    </template>
    <div v-else class="chc-loading">{{ statLoading ? '加载统计中…' : '—' }}</div>
  </div>
</template>

<style scoped>
/*
 * Fixed dark palette — ant-design's tooltip is always rendered on a dark
 * overlay regardless of the app theme, so the card must use light text in
 * both light and dark mode. CSS vars would flip with the theme and turn the
 * text dark-on-dark (unreadable) in light mode.
 */
.chc-card { font-size: 12px; color: #e2e8f0; }
.chc-hash {
  font-family: 'Cascadia Code', monospace;
  font-size: 11px;
  color: #BD93F9;
  margin-bottom: 6px;
  display: flex;
  align-items: center;
  gap: 6px;
}
.chc-num { font-weight: 700; color: #94a3b8; }
.chc-hash-val { color: #BD93F9; }
.chc-type {
  font-size: 9px;
  font-weight: 700;
  padding: 1px 5px;
  border-radius: 3px;
  text-transform: uppercase;
}
.chc-scope { color: #cbd5e1; }
.chc-subject {
  color: #f1f5f9;
  font-weight: 500;
  margin-bottom: 8px;
  line-height: 1.4;
  word-break: break-word;
}
/* message body (everything after the subject line) — preserves newlines */
.chc-body {
  color: #cbd5e1;
  font-size: 11px;
  line-height: 1.5;
  white-space: pre-wrap;
  word-break: break-word;
  margin: -4px 0 8px;
  padding-top: 4px;
  border-top: 1px solid rgba(255, 255, 255, 0.12);
}
.chc-row { display: flex; gap: 8px; margin-bottom: 4px; line-height: 1.5; }
.chc-k { color: #94a3b8; width: 36px; flex-shrink: 0; }
.chc-v { color: #cbd5e1; display: flex; gap: 8px; }

/* conventional-commit type colors — Dracula neons, fixed (dark bg) */
.type-feat { color: #50FA7B; background: rgba(80, 250, 123, 0.16); }
.type-fix { color: #FFB86C; background: rgba(255, 184, 108, 0.16); }
.type-refactor { color: #BD93F9; background: rgba(189, 147, 249, 0.16); }
.type-docs { color: #8BE9FD; background: rgba(139, 233, 253, 0.16); }
.type-perf { color: #FF79C6; background: rgba(255, 121, 198, 0.16); }
.type-chore, .type-build, .type-ci {
  color: #94a3b8;
  background: rgba(148, 163, 184, 0.16);
}
.type-test { color: #FFB86C; background: rgba(255, 184, 108, 0.12); }

/* change summary + file list */
.stat-add { color: #50FA7B; }
.stat-del { color: #FF6E6E; }
.stat-files { color: #94a3b8; }
.chc-files {
  margin-top: 6px;
  padding-top: 6px;
  border-top: 1px solid rgba(255, 255, 255, 0.12);
  display: flex;
  flex-direction: column;
  gap: 2px;
}
.chc-file { display: flex; align-items: center; gap: 6px; font-size: 11px; }
.chc-file-status { width: 14px; text-align: center; font-weight: 700; flex-shrink: 0; }
.st-A { color: #50FA7B; }
.st-M { color: #FFB86C; }
.st-D { color: #FF6E6E; }
.chc-file-name {
  flex: 1;
  color: #cbd5e1;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-family: 'Cascadia Code', monospace;
  font-size: 10px;
}
.chc-file-delta { flex-shrink: 0; display: flex; gap: 4px; }
.chc-more { font-size: 10px; color: #94a3b8; margin-top: 2px; }
.chc-loading { font-size: 11px; color: #94a3b8; margin-top: 4px; }
</style>
