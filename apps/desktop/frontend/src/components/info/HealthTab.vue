<script setup>
import { useRepoData } from '@/composables/useRepoData'
import PanelState from '@/components/common/PanelState.vue'

const { data: r, loading, error, reload } = useRepoData((repo) =>
  window.go.main.ByteGitApp.InfoHealth(repo)
)

const levelColor = { error: 'var(--color-error)', warning: 'var(--color-warning)', ok: 'var(--color-success)', info: 'var(--color-info)' }
</script>

<template>
  <PanelState :loading="loading" :error="error || ''" :empty="!r" @retry="reload">
    <div v-if="r" class="health">
      <div class="grade-banner">
        <span class="grade-label">健康评级</span>
        <span class="grade">{{ r.grade || '—' }}</span>
      </div>
      <div class="item-list">
        <div v-for="(it, i) in r.items" :key="i" class="health-item">
          <span class="dot" :style="{ background: levelColor[it.level] || levelColor.info }"></span>
          <div class="item-body">
            <div class="item-title">{{ it.title }}</div>
            <div v-if="it.details?.length" class="item-details">
              <div v-for="(d, j) in it.details" :key="j" class="detail-line">{{ d }}</div>
            </div>
          </div>
        </div>
      </div>
    </div>
  </PanelState>
</template>

<style scoped>
.grade-banner {
  display: flex;
  align-items: center;
  gap: var(--spacing-sm);
  padding: var(--spacing-md);
  background: var(--color-surface);
  border: 1px solid var(--color-border);
  border-radius: var(--radius-md);
  margin-bottom: var(--spacing-md);
}
.grade { font-size: 28px; font-weight: 800; color: var(--color-primary); }
.grade-label { font-size: 13px; color: var(--color-text-secondary); }
.item-list { display: flex; flex-direction: column; gap: var(--spacing-sm); }
.health-item {
  display: flex;
  gap: var(--spacing-sm);
  padding: var(--spacing-sm) var(--spacing-md);
  background: var(--color-surface);
  border: 1px solid var(--color-border);
  border-radius: var(--radius-md);
}
.dot { width: 8px; height: 8px; border-radius: 50%; flex-shrink: 0; margin-top: 6px; }
.item-title { font-size: 13px; font-weight: 500; color: var(--color-text); }
.item-details { margin-top: 4px; }
.detail-line { font-size: 12px; color: var(--color-text-secondary); }
</style>
