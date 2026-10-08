<script setup>
import { useRepoData } from '../../composables/useRepoData'
import PanelState from '../common/PanelState.vue'

const { data: r, loading, error, reload } = useRepoData((repo) =>
  window.go.main.ByteGitApp.InfoBranches(repo)
)

const groups = [
  { key: 'current', label: '当前' },
  { key: 'active', label: '活跃' },
  { key: 'merged', label: '已合并' },
  { key: 'stale', label: '过期' },
]
const cols = [
  { title: '分支', dataIndex: 'name' },
  { title: '最后提交', dataIndex: 'lastCommit' },
  { title: '领先', dataIndex: 'ahead', width: 80 },
  { title: '落后', dataIndex: 'behind', width: 80 },
]
</script>

<template>
  <PanelState :loading="loading" :error="error || ''" :empty="!r" @retry="reload">
    <div v-if="r" class="branch-groups">
      <div v-for="g in groups" :key="g.key" class="branch-group">
        <div class="group-head">{{ g.label }} <span class="cnt">({{ r[g.key]?.length || 0 }})</span></div>
        <a-table
          v-if="r[g.key]?.length"
          :columns="cols"
          :data-source="r[g.key]"
          row-key="name"
          size="small"
          :pagination="false"
        >
          <template #bodyCell="{ column, record }">
            <template v-if="column.dataIndex === 'name'">
              <span :class="{ 'cur-tag': record.isCurrent }">{{ record.name }}</span>
              <a-tag v-if="record.isCurrent" color="green" style="margin-left:6px">HEAD</a-tag>
            </template>
          </template>
        </a-table>
        <div v-else class="muted">无</div>
      </div>
    </div>
  </PanelState>
</template>

<style scoped>
.branch-groups { display: flex; flex-direction: column; gap: var(--spacing-md); }
.group-head { font-size: 14px; font-weight: 600; color: var(--color-text); margin-bottom: var(--spacing-sm); }
.cnt { color: var(--color-text-tertiary); font-weight: 400; font-size: 12px; }
.cur-tag { color: var(--color-primary); font-weight: 600; }
.muted { color: var(--color-text-tertiary); font-size: 13px; }
</style>
