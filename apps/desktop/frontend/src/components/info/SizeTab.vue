<script setup>
import { useRepoData } from '../../composables/useRepoData'
import PanelState from '../common/PanelState.vue'

const { data: r, loading, error, reload } = useRepoData((repo) =>
  window.go.main.ByteGitApp.InfoSize(repo)
)

const typeCols = [
  { title: '扩展名', dataIndex: 'ext' },
  { title: '文件数', dataIndex: 'count', width: 100 },
  { title: '大小', dataIndex: 'sizeStr', width: 120 },
]
const largeCols = [
  { title: '文件', dataIndex: 'path' },
  { title: '大小', dataIndex: 'sizeStr', width: 120 },
]
</script>

<template>
  <PanelState :loading="loading" :error="error || ''" :empty="!r" @retry="reload">
    <div v-if="r" class="size-view">
      <div class="summary-row">
        <div class="s-item"><span class="l">总大小</span><span class="v">{{ r.totalSize }}</span></div>
        <div class="s-item"><span class="l">总文件数</span><span class="v">{{ r.totalFiles }}</span></div>
      </div>

      <div class="block">
        <div class="block-title">按类型</div>
        <a-table :columns="typeCols" :data-source="r.byType" row-key="ext" size="small" :pagination="{ pageSize: 10 }" />
      </div>

      <div class="block">
        <div class="block-title">大文件</div>
        <a-table v-if="r.largeFiles?.length" :columns="largeCols" :data-source="r.largeFiles" row-key="path" size="small" :pagination="{ pageSize: 10 }" />
        <div v-else class="muted">无大文件</div>
      </div>
    </div>
  </PanelState>
</template>

<style scoped>
.size-view { display: flex; flex-direction: column; gap: var(--spacing-md); }
.summary-row { display: flex; gap: var(--spacing-lg); padding: var(--spacing-md); background: var(--color-surface); border: 1px solid var(--color-border); border-radius: var(--radius-md); }
.s-item { display: flex; flex-direction: column; }
.s-item .l { font-size: 12px; color: var(--color-text-secondary); }
.s-item .v { font-size: 18px; font-weight: 700; color: var(--color-primary); }
.block { background: var(--color-surface); border: 1px solid var(--color-border); border-radius: var(--radius-md); padding: var(--spacing-md); }
.block-title { font-size: 14px; font-weight: 600; color: var(--color-text); margin-bottom: var(--spacing-sm); }
.muted { color: var(--color-text-tertiary); font-size: 13px; }
</style>
