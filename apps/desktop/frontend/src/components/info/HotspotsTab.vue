<script setup>
import { ref, watch } from 'vue'
import { useRepoData } from '../../composables/useRepoData'
import PanelState from '../common/PanelState.vue'

const days = ref(90)
const limit = ref(20)
const { data: list, loading, error, reload } = useRepoData((repo) =>
  window.go.main.ByteGitApp.InfoHotspots(repo, days.value, limit.value)
)
watch([days, limit], reload)

const cols = [
  { title: '文件', dataIndex: 'file' },
  { title: '提交数', dataIndex: 'commits', width: 100 },
  { title: '新增', dataIndex: 'additions', width: 100 },
  { title: '删除', dataIndex: 'deletions', width: 100 },
]
</script>

<template>
  <div class="hs-head">
    <span class="lbl">范围</span>
    <a-input-number v-model:value="days" :min="1" size="small" style="width:90px" /> 天
    <span class="lbl" style="margin-left:12px">Top</span>
    <a-input-number v-model:value="limit" :min="1" :max="100" size="small" style="width:80px" />
  </div>
  <PanelState :loading="loading" :error="error || ''" :empty="!list?.length" empty-text="无热点数据" @retry="reload">
    <a-table :columns="cols" :data-source="list" row-key="file" size="small" :pagination="{ pageSize: 15 }" />
  </PanelState>
</template>

<style scoped>
.hs-head { display: flex; align-items: center; gap: 6px; margin-bottom: var(--spacing-md); font-size: 13px; color: var(--color-text-secondary); }
</style>
