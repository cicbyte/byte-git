<script setup>
import { ref, computed } from 'vue'
import { useRepoData } from '@/composables/useRepoData'
import PanelState from '@/components/common/PanelState.vue'

const days = ref(30)
const { data: list, loading, error, reload } = useRepoData((repo) =>
  window.go.main.ByteGitApp.InfoActivity(repo, days.value)
)

// re-fetch when days changes
import { watch } from 'vue'
watch(days, reload)

const maxCount = computed(() => Math.max(1, ...(list.value || []).map((d) => d.count)))
</script>

<template>
  <div class="activity-head">
    <a-radio-group v-model:value="days" size="small">
      <a-radio-button :value="7">7天</a-radio-button>
      <a-radio-button :value="30">30天</a-radio-button>
      <a-radio-button :value="90">90天</a-radio-button>
      <a-radio-button :value="365">1年</a-radio-button>
    </a-radio-group>
  </div>
  <PanelState :loading="loading" :error="error || ''" :empty="!list?.length" empty-text="该时间段无提交" @retry="reload">
    <div class="chart">
      <div v-for="d in list" :key="d.date" class="bar-col" :title="`${d.date}: ${d.count} 次`">
        <div class="bar" :style="{ height: (d.count / maxCount * 100) + '%' }"></div>
        <span class="count" v-if="d.count > 0">{{ d.count }}</span>
      </div>
    </div>
  </PanelState>
</template>

<style scoped>
.activity-head { margin-bottom: var(--spacing-md); }
.chart {
  display: flex;
  align-items: flex-end;
  gap: 2px;
  height: 220px;
  padding: var(--spacing-sm) 0;
  border-bottom: 1px solid var(--color-border);
}
.bar-col {
  flex: 1;
  min-width: 4px;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: flex-end;
  height: 100%;
  position: relative;
}
.bar {
  width: 100%;
  background: linear-gradient(180deg, var(--color-primary), var(--color-accent));
  border-radius: 2px 2px 0 0;
  min-height: 2px;
  transition: height var(--transition-fast);
}
.bar:hover { filter: brightness(1.15); }
.count { font-size: 10px; color: var(--color-text-tertiary); position: absolute; bottom: 100%; }
</style>
