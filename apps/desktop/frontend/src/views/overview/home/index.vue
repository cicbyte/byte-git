<script setup>
import { ref, computed, onMounted, watch } from 'vue'
import { useRouter } from 'vue-router'
import dayjs from 'dayjs'
import {
  ArrowRightOutlined, FileTextOutlined, BranchesOutlined,
  CloudUploadOutlined, CodeOutlined, ThunderboltOutlined,
} from '@ant-design/icons-vue'
import MarkdownView from '@/components/common/MarkdownView.vue'
import { useRepoStore } from '@/stores/repo'
import { useScmStore } from '@/stores/scm'
import { storeToRefs } from 'pinia'

const router = useRouter()
const repo = useRepoStore()
const scm = useScmStore()
const { totalChanges, ahead } = storeToRefs(scm)

const loading = ref(true)
const summary = ref(null)
const recentCommits = ref([])
const readme = ref('')
const readmeExpanded = ref(false)
const totalCommits = ref(0)
const todayCommits = ref(0)
// 近 7 日提交数（最旧 → 今天），驱动主卡活动条形图
const weekCounts = ref([0, 0, 0, 0, 0, 0, 0])
const weekTotal = ref(0)

async function loadWeek(app) {
  if (!app?.LogFetchByDate) return
  try {
    const days = Array.from({ length: 7 }, (_, i) =>
      dayjs().subtract(6 - i, 'day').format('YYYY-MM-DD'))
    const lists = await Promise.all(
      days.map((d) => app.LogFetchByDate('', d).catch(() => [])))
    weekCounts.value = lists.map((l) => l.length)
    weekTotal.value = weekCounts.value.reduce((a, b) => a + b, 0)
    todayCommits.value = weekCounts.value[6]
  } catch {
    weekCounts.value = [0, 0, 0, 0, 0, 0, 0]
  }
}

const weekBarMax = computed(() => Math.max(1, ...weekCounts.value))

async function load() {
  loading.value = true
  try {
    if (window.go?.main?.ByteGitApp) {
      const app = window.go.main.ByteGitApp
      summary.value = await app.InfoSummary('')
      recentCommits.value = await app.LogFetch('', 10, '')
      try {
        const res = await app.RepoReadme()
        readme.value = res.content || ''
      } catch {}
      // total commit count via rev-list --count (accurate, not affected by
      // the LogFetch page size cap)
      if (app.CommitTotal) {
        try { totalCommits.value = await app.CommitTotal() } catch { totalCommits.value = 0 }
      }
      // 近 7 日逐日提交数（含今日，一并驱动主卡条形图与今日统计）
      await loadWeek(app)
      // refresh SCM state for dashboard stat cards
      scm.refresh()
      scm.loadAhead()
    }
  } catch (e) {
    console.error('Failed to load dashboard:', e)
  } finally {
    loading.value = false
  }
}

onMounted(load)
watch(() => repo.repoEpoch, load)

function goToLog() { router.push('/log') }
function goToInfo() { router.push('/info') }
function goToLive() { router.push('/live') }
function goToTerminal() { router.push('/terminal') }
function openScm() {
  // trigger SCM drawer via navbar event
  window.dispatchEvent(new CustomEvent('open-scm'))
}

function fmtDate(d) {
  if (!d) return ''
  return dayjs(d).format('MM-DD HH:mm')
}
</script>

<template>
  <div class="home-view">
    <div class="dashboard-header">
      <h2>仪表盘</h2>
    </div>

    <a-spin :spinning="loading">
      <!-- Bento stats: hero (总提交 + 近 7 日活动) + 4 actionable cards -->
      <div class="stat-cards">
        <div class="stat-card hero" @click="goToLog">
          <div class="hero-head">
            <span class="stat-label">总提交</span>
            <span class="hero-sub">近 7 日 {{ weekTotal }} 次</span>
          </div>
          <div class="hero-value">{{ totalCommits || '-' }}</div>
          <div class="hero-bars" aria-hidden="true">
            <div v-for="(n, i) in weekCounts" :key="i" class="hero-bar-col">
              <div
                class="hero-bar"
                :class="{ today: i === weekCounts.length - 1 }"
                :style="{ height: Math.max(6, Math.round((n / weekBarMax) * 100)) + '%' }"
              ></div>
            </div>
          </div>
        </div>
        <div class="stat-card accent" @click="goToLive">
          <div class="stat-icon"><BranchesOutlined /></div>
          <div class="stat-body">
            <div class="stat-value">{{ totalChanges }}</div>
            <div class="stat-label">工作区变更</div>
          </div>
        </div>
        <div class="stat-card" :class="{ accent: ahead > 0 }" @click="openScm">
          <div class="stat-icon"><CloudUploadOutlined /></div>
          <div class="stat-body">
            <div class="stat-value">{{ ahead }}</div>
            <div class="stat-label">待推送提交</div>
          </div>
        </div>
        <div class="stat-card">
          <div class="stat-icon"><ThunderboltOutlined /></div>
          <div class="stat-body">
            <div class="stat-value">{{ todayCommits }}</div>
            <div class="stat-label">今日提交</div>
          </div>
        </div>
        <div class="stat-card" @click="goToInfo">
          <div class="stat-icon"><FileTextOutlined /></div>
          <div class="stat-body">
            <div class="stat-value">{{ summary?.fileCount || '-' }}</div>
            <div class="stat-label">文件数</div>
          </div>
        </div>
      </div>

      <!-- Quick Actions -->
      <div class="quick-actions">
        <button class="qa-btn" @click="openScm"><BranchesOutlined /> 提交</button>
        <button class="qa-btn" @click="goToLog">浏览日志</button>
        <button class="qa-btn" @click="goToTerminal"><CodeOutlined /> 终端</button>
        <button class="qa-btn" @click="goToInfo">仓库洞察</button>
      </div>

      <!-- Language Distribution -->
      <div class="section" v-if="summary?.languages?.length">
        <div class="section-header">
          <h3>语言分布</h3>
          <a-button type="link" size="small" @click="goToInfo">
            详情 <ArrowRightOutlined />
          </a-button>
        </div>
        <div class="lang-bars">
          <div v-for="lang in summary.languages.slice(0, 8)" :key="lang.name" class="lang-item">
            <div class="lang-info">
              <span class="lang-name">{{ lang.name }}</span>
              <span class="lang-pct">{{ lang.pct.toFixed(1) }}%</span>
            </div>
            <div class="lang-bar">
              <div class="lang-bar-fill" :style="{ width: lang.pct + '%' }"></div>
            </div>
          </div>
        </div>
      </div>

      <!-- Recent Commits -->
      <div class="section">
        <div class="section-header">
          <h3>最近提交</h3>
          <a-button type="link" size="small" @click="goToLog">
            查看全部 <ArrowRightOutlined />
          </a-button>
        </div>
        <div class="commit-list">
          <div v-for="c in recentCommits.slice(0, 8)" :key="c.hash" class="commit-item" @click="router.push('/time-travel')">
            <div class="commit-top">
              <span class="commit-hash">{{ c.hash }}</span>
              <span class="commit-type" :class="'type-' + c.type">{{ c.type || '-----' }}</span>
              <span class="commit-scope" v-if="c.scope">({{ c.scope }})</span>
              <span class="commit-time">{{ fmtDate(c.date) }}</span>
            </div>
            <div class="commit-subject">{{ c.subject }}</div>
            <div class="commit-author">{{ c.author }}</div>
          </div>
        </div>
      </div>

      <!-- README preview -->
      <div class="section" v-if="readme">
        <div class="section-header">
          <h3><FileTextOutlined /> README</h3>
          <a-button type="link" size="small" @click="readmeExpanded = !readmeExpanded">
            {{ readmeExpanded ? '收起' : '展开全部' }} <ArrowRightOutlined />
          </a-button>
        </div>
        <div class="readme-card" :class="{ expanded: readmeExpanded }">
          <MarkdownView :source="readme" />
        </div>
      </div>
    </a-spin>
  </div>
</template>

<style scoped>
.home-view {
  width: 100%;
}

.dashboard-header {
  margin-bottom: var(--spacing-lg);
}

.dashboard-header h2 {
  font-size: 20px;
  font-weight: 600;
  color: var(--color-text);
}

/* Bento stats: hero 占左侧 2×2，四张操作卡填右侧 2×2 */
.stat-cards {
  display: grid;
  grid-template-columns: repeat(4, 1fr);
  grid-auto-rows: 1fr;
  gap: var(--spacing-md);
  margin-bottom: var(--spacing-md);
}

.stat-card {
  background: var(--color-surface);
  border: 1px solid var(--color-border);
  border-radius: var(--radius-md);
  padding: var(--spacing-md);
  display: flex;
  align-items: center;
  gap: var(--spacing-sm);
  cursor: pointer;
  transition: border-color var(--transition-fast), background-color var(--transition-fast);
}

.stat-card:hover {
  border-color: color-mix(in srgb, var(--color-primary) 45%, var(--color-border));
  background: var(--color-hover);
}

.stat-card.accent {
  border-color: var(--color-primary);
}

/* 主卡：大数字 + 近 7 日活动条形图 + 全局唯一品牌微光 */
.stat-card.hero {
  grid-column: span 2;
  grid-row: span 2;
  flex-direction: column;
  align-items: stretch;
  justify-content: space-between;
  gap: var(--spacing-sm);
  background:
    linear-gradient(135deg, rgba(var(--color-primary-rgb), 0.09), transparent 55%),
    var(--color-surface);
}

.hero-head {
  display: flex;
  justify-content: space-between;
  align-items: baseline;
}

.hero-sub {
  font-size: 12px;
  color: var(--color-text-tertiary);
}

.hero-value {
  font-size: 44px;
  font-weight: 650;
  line-height: 1.1;
  letter-spacing: -0.5px;
  color: var(--color-text);
  font-variant-numeric: tabular-nums;
}

.hero-bars {
  display: flex;
  align-items: flex-end;
  gap: 6px;
  height: 64px;
  margin-top: var(--spacing-sm);
}

.hero-bar-col {
  flex: 1;
  display: flex;
  align-items: flex-end;
  height: 100%;
}

.hero-bar {
  width: 100%;
  min-height: 3px;
  border-radius: 2px;
  background: rgba(var(--color-primary-rgb), 0.35);
  transition: height var(--transition-normal);
}

.hero-bar.today {
  background: var(--color-primary);
}

.stat-icon {
  width: 40px; height: 40px;
  border-radius: var(--radius-sm);
  background: var(--color-primary-bg);
  color: var(--color-primary);
  display: flex; align-items: center; justify-content: center;
  font-size: 18px; flex-shrink: 0;
}

.stat-card.accent .stat-icon {
  background: var(--color-primary);
  color: var(--color-surface);
}

.stat-body { display: flex; flex-direction: column; }

.stat-value {
  font-size: 22px;
  font-weight: 700;
  color: var(--color-text);
  line-height: 1.2;
}

.stat-card.accent .stat-value { color: var(--color-primary); }

.stat-label {
  font-size: 12px;
  color: var(--color-text-secondary);
  margin-top: 2px;
}

/* quick actions bar */
.quick-actions {
  display: flex;
  gap: 8px;
  margin-bottom: var(--spacing-xl);
  flex-wrap: wrap;
}
.qa-btn {
  -webkit-appearance: none; appearance: none;
  display: inline-flex; align-items: center; gap: 6px;
  padding: 6px 14px;
  border: 1px solid var(--color-border); border-radius: var(--radius-sm);
  background: var(--color-surface); color: var(--color-text-secondary);
  font-size: 13px; cursor: pointer;
  transition: all var(--transition-fast);
}
.qa-btn:hover {
  background: var(--color-primary-bg); color: var(--color-primary);
  border-color: var(--color-primary);
}

.section {
  margin-bottom: var(--spacing-xl);
}

.section-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: var(--spacing-md);
}

.section-header h3 {
  font-size: 16px;
  font-weight: 600;
  color: var(--color-text);
}

.lang-bars { display: flex; flex-direction: column; gap: var(--spacing-sm); }
.lang-item { display: flex; flex-direction: column; gap: 4px; }
.lang-info { display: flex; justify-content: space-between; font-size: 13px; }
.lang-name { color: var(--color-text); font-weight: 500; }
.lang-pct { color: var(--color-text-secondary); }
.lang-bar { height: 6px; background: var(--color-surface-raised); border-radius: 3px; overflow: hidden; }
.lang-bar-fill { height: 100%; background: linear-gradient(90deg, var(--color-primary), var(--color-accent)); border-radius: 3px; transition: width var(--transition-normal); }

.commit-list {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.commit-item {
  display: flex;
  flex-direction: column;
  gap: 4px;
  padding: var(--spacing-sm) var(--spacing-md);
  border-radius: var(--radius-sm);
  border-left: 2px solid transparent;
  transition: all var(--transition-fast);
}

.commit-item:hover {
  background: var(--color-hover);
  border-left-color: var(--color-primary);
}

.commit-top {
  display: flex;
  align-items: center;
  gap: var(--spacing-sm);
}

.commit-hash {
  font-family: 'Cascadia Code', 'Fira Code', monospace;
  font-size: 12px;
  color: var(--color-hash);
  flex-shrink: 0;
}

.commit-type {
  font-size: 11px;
  font-weight: 600;
  padding: 1px 6px;
  border-radius: var(--radius-xs);
  min-width: 56px;
  text-align: center;
  flex-shrink: 0;
}

.type-feat { color: var(--color-feat); background: rgba(80,250,123,0.1); }
.type-fix { color: var(--color-fix); background: rgba(255,184,108,0.1); }
.type-refactor { color: var(--color-refactor); background: rgba(189,147,249,0.1); }
.type-docs { color: var(--color-docs); background: rgba(139,233,253,0.1); }
.type-perf { color: var(--color-perf); background: rgba(255,121,198,0.1); }

.commit-scope {
  font-size: 11px;
  color: var(--color-text-tertiary);
  flex-shrink: 0;
}

.commit-time {
  margin-left: auto;
  font-size: 11px;
  color: var(--color-text-tertiary);
  flex-shrink: 0;
  font-family: 'Cascadia Code', 'Fira Code', monospace;
}

.commit-subject {
  font-size: 13px;
  color: var(--color-text);
  line-height: 1.4;
  word-break: break-word;
}

.commit-author {
  font-size: 11px;
  color: var(--color-text-tertiary);
}

@media (max-width: 768px) {
  .stat-cards {
    grid-template-columns: repeat(2, 1fr);
  }
}

@media (max-width: 480px) {
  .stat-cards {
    grid-template-columns: 1fr;
  }
}

/* README preview card */
.readme-card {
  background: var(--color-surface);
  border: 1px solid var(--color-border);
  border-radius: var(--radius-md);
  padding: var(--spacing-md) var(--spacing-lg);
  max-height: 320px;
  overflow: hidden;
  position: relative;
  transition: max-height var(--transition-normal);
}
.readme-card:not(.expanded)::after {
  content: '';
  position: absolute;
  bottom: 0;
  left: 0;
  right: 0;
  height: 48px;
  background: linear-gradient(transparent, var(--color-surface));
  pointer-events: none;
}
.readme-card.expanded {
  max-height: none;
}
</style>
