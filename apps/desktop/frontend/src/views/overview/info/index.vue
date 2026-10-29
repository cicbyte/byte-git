<script setup>
import { ref } from 'vue'
import SummaryTab from '@/components/info/SummaryTab.vue'
import ActivityTab from '@/components/info/ActivityTab.vue'
import HealthTab from '@/components/info/HealthTab.vue'
import BranchesTab from '@/components/info/BranchesTab.vue'
import SizeTab from '@/components/info/SizeTab.vue'
import ChangelogTab from '@/components/info/ChangelogTab.vue'
import HotspotsTab from '@/components/info/HotspotsTab.vue'
import LargeFilesTab from '@/components/info/LargeFilesTab.vue'
import TodosTab from '@/components/info/TodosTab.vue'
import ReadmeTab from '@/components/info/ReadmeTab.vue'

const activeTab = ref('summary')
</script>

<template>
  <div class="info-view">
    <a-tabs v-model:activeKey="activeTab" tab-position="left" class="info-tabs">
      <a-tab-pane key="summary" tab="概览"><SummaryTab /></a-tab-pane>
      <a-tab-pane key="readme" tab="README"><ReadmeTab /></a-tab-pane>
      <a-tab-pane key="activity" tab="活动"><ActivityTab /></a-tab-pane>
      <a-tab-pane key="health" tab="健康"><HealthTab /></a-tab-pane>
      <a-tab-pane key="branches" tab="分支"><BranchesTab /></a-tab-pane>
      <a-tab-pane key="size" tab="体积"><SizeTab /></a-tab-pane>
      <a-tab-pane key="changelog" tab="日志"><ChangelogTab /></a-tab-pane>
      <a-tab-pane key="hotspots" tab="热点"><HotspotsTab /></a-tab-pane>
      <a-tab-pane key="largefiles" tab="大文件"><LargeFilesTab /></a-tab-pane>
      <a-tab-pane key="todos" tab="TODO"><TodosTab /></a-tab-pane>
    </a-tabs>
  </div>
</template>

<style scoped>
.info-view {
  width: 100%;
  height: 100%;
}

.info-tabs {
  height: 100%;
}

.info-tabs :deep(.ant-tabs-content) {
  height: 100%;
  overflow-y: auto;
  padding-right: var(--spacing-sm);
}

.info-tabs :deep(.ant-tab-pane) {
  padding: var(--spacing-sm) var(--spacing-md);
}
</style>
