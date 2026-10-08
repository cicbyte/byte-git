<script setup>
import { useRepoData } from '../../composables/useRepoData'
import PanelState from '../common/PanelState.vue'

const { data: s, loading, error, reload } = useRepoData((repo) =>
  window.go.main.ByteGitApp.InfoSummary(repo)
)
</script>

<template>
  <PanelState :loading="loading" :error="error || ''" :empty="!s" @retry="reload">
    <div v-if="s" class="summary-grid">
      <div class="stat-card"><div class="v">{{ s.totalCommits }}</div><div class="l">总提交</div></div>
      <div class="stat-card"><div class="v">{{ s.fileCount }}</div><div class="l">文件数</div></div>
      <div class="stat-card"><div class="v">{{ s.totalSize }}</div><div class="l">总大小</div></div>
      <div class="stat-card"><div class="v">{{ s.authorCount }}</div><div class="l">贡献者</div></div>
      <div class="stat-card"><div class="v">{{ s.branchCount }}</div><div class="l">分支</div></div>
      <div class="stat-card"><div class="v">{{ s.tagCount }}</div><div class="l">标签</div></div>
      <div class="stat-card"><div class="v">{{ s.timelineDays }}</div><div class="l">项目天数</div></div>
      <div class="stat-card"><div class="v">{{ s.firstDate || '—' }}</div><div class="l">首次提交</div></div>

      <div class="wide">
        <div class="sub-title">语言分布</div>
        <div v-if="s.languages?.length" class="lang-list">
          <div v-for="l in s.languages" :key="l.name" class="lang-item">
            <div class="lang-head"><span>{{ l.name }}</span><span>{{ l.pct.toFixed(1) }}%</span></div>
            <div class="lang-bar"><div class="fill" :style="{ width: l.pct + '%' }"></div></div>
          </div>
        </div>
        <div v-else class="muted">无</div>
      </div>

      <div class="wide">
        <div class="sub-title">主要贡献者</div>
        <div v-if="s.topAuthors?.length" class="author-list">
          <div v-for="(a, i) in s.topAuthors" :key="a.name" class="author-item">
            <span class="rank">{{ i + 1 }}</span>
            <span class="name">{{ a.name }}</span>
            <span class="commits">{{ a.commits }} 次提交</span>
          </div>
        </div>
        <div v-else class="muted">无</div>
      </div>
    </div>
  </PanelState>
</template>

<style scoped>
.summary-grid {
  display: grid;
  grid-template-columns: repeat(4, 1fr);
  gap: var(--spacing-md);
}
.stat-card {
  background: var(--color-surface);
  border: 1px solid var(--color-border);
  border-radius: var(--radius-md);
  padding: var(--spacing-md);
  text-align: center;
}
.stat-card .v { font-size: 22px; font-weight: 700; color: var(--color-primary); }
.stat-card .l { font-size: 12px; color: var(--color-text-secondary); margin-top: 4px; }
.wide { grid-column: 1 / -1; }
.sub-title { font-size: 14px; font-weight: 600; color: var(--color-text); margin-bottom: var(--spacing-sm); }
.lang-list, .author-list { display: flex; flex-direction: column; gap: 6px; }
.lang-head { display: flex; justify-content: space-between; font-size: 12px; margin-bottom: 2px; }
.lang-bar { height: 6px; background: var(--color-surface-raised); border-radius: 3px; overflow: hidden; }
.fill { height: 100%; background: linear-gradient(90deg, var(--color-primary), var(--color-accent)); }
.author-item { display: flex; align-items: center; gap: var(--spacing-sm); font-size: 13px; padding: 4px 0; }
.rank { width: 20px; color: var(--color-text-tertiary); }
.name { flex: 1; color: var(--color-text); }
.commits { color: var(--color-text-secondary); }
.muted { color: var(--color-text-tertiary); font-size: 13px; }
@media (max-width: 900px) { .summary-grid { grid-template-columns: repeat(2, 1fr); } }
</style>
