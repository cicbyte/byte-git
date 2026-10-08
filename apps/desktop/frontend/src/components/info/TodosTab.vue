<script setup>
import { ref, computed } from 'vue'
import { useRepoData } from '../../composables/useRepoData'
import PanelState from '../common/PanelState.vue'

const { data: list, loading, error, reload } = useRepoData((repo) =>
  window.go.main.ByteGitApp.InfoTodos(repo)
)

const filterTag = ref()
const tags = computed(() => {
  const set = new Set((list.value || []).map((t) => t.tag))
  return [...set].map((v) => ({ label: v, value: v }))
})
const filtered = computed(() =>
  filterTag.value ? (list.value || []).filter((t) => t.tag === filterTag.value) : list.value || []
)

const tagColor = { TODO: 'blue', FIXME: 'orange', HACK: 'magenta', XXX: 'red' }
const cols = [
  { title: '标签', dataIndex: 'tag', width: 90 },
  { title: '文件', dataIndex: 'file' },
  { title: '行', dataIndex: 'line', width: 70 },
  { title: '内容', dataIndex: 'content' },
]
</script>

<template>
  <div class="todo-head" v-if="tags.length">
    <span class="lbl">筛选</span>
    <a-select v-model:value="filterTag" :options="tags" placeholder="全部" allow-clear size="small" style="width:140px" />
    <span class="cnt">共 {{ filtered.length }} 条</span>
  </div>
  <PanelState :loading="loading" :error="error || ''" :empty="!filtered.length" empty-text="未发现 TODO/FIXME" @retry="reload">
    <a-table :columns="cols" :data-source="filtered" row-key="line" size="small" :pagination="{ pageSize: 20 }">
      <template #bodyCell="{ column, record }">
        <template v-if="column.dataIndex === 'tag'">
          <a-tag :color="tagColor[record.tag] || 'default'">{{ record.tag }}</a-tag>
        </template>
      </template>
    </a-table>
  </PanelState>
</template>

<style scoped>
.todo-head { display: flex; align-items: center; gap: var(--spacing-sm); margin-bottom: var(--spacing-md); }
.lbl { font-size: 13px; color: var(--color-text-secondary); }
.cnt { font-size: 12px; color: var(--color-text-tertiary); }
</style>
