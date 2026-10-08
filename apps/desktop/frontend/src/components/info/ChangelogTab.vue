<script setup>
import { ref, watch } from 'vue'
import { useRepoData } from '../../composables/useRepoData'
import PanelState from '../common/PanelState.vue'

const fromTag = ref('')
const { data: list, loading, error, reload } = useRepoData((repo) =>
  window.go.main.ByteGitApp.InfoChangelog(repo, fromTag.value || '')
)
watch(fromTag, reload)

const typeColor = {
  feat: 'green', fix: 'orange', docs: 'blue', style: 'cyan',
  refactor: 'purple', perf: 'magenta', test: 'gold', chore: 'default', build: 'default', ci: 'default',
}
</script>

<template>
  <div class="cl-head">
    <span class="lbl">起始 (tag/commit，可空)</span>
    <a-input v-model:value="fromTag" placeholder="如 v1.0.0" style="width: 200px" allow-clear size="small" />
  </div>
  <PanelState :loading="loading" :error="error || ''" :empty="!list?.length" empty-text="无变更记录" @retry="reload">
    <div class="cl-list">
      <div v-for="c in list" :key="c.hash" class="cl-item">
        <a-tag :color="typeColor[c.type] || 'default'">{{ c.type || 'other' }}</a-tag>
        <span v-if="c.scope" class="scope">({{ c.scope }})</span>
        <span class="desc">{{ c.description }}</span>
        <a-tag v-if="c.breaking" color="red">BREAKING</a-tag>
        <span class="date">{{ c.date }}</span>
      </div>
    </div>
  </PanelState>
</template>

<style scoped>
.cl-head { display: flex; align-items: center; gap: var(--spacing-sm); margin-bottom: var(--spacing-md); }
.lbl { font-size: 13px; color: var(--color-text-secondary); }
.cl-list { display: flex; flex-direction: column; gap: 2px; }
.cl-item { display: flex; align-items: center; gap: var(--spacing-sm); padding: 8px var(--spacing-sm); font-size: 13px; border-radius: var(--radius-sm); }
.cl-item:hover { background: var(--color-hover); }
.scope { color: var(--color-text-tertiary); }
.desc { flex: 1; color: var(--color-text); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.date { color: var(--color-text-tertiary); font-size: 12px; flex-shrink: 0; }
</style>
