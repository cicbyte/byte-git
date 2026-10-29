<script setup>
import { ref, computed, watch } from 'vue'
import dayjs from 'dayjs'
import {
  CalendarOutlined, FireOutlined, TrophyOutlined,
  LineChartOutlined, ReloadOutlined,
} from '@ant-design/icons-vue'
import { useRepoStore } from '@/stores/repo'
import StatsHeatmap from '@/components/stats/StatsHeatmap.vue'

const repo = useRepoStore()
const app = window.go?.main?.ByteGitApp

// date range filter: [start, end] as dayjs objects, or null = all history
const dateRange = ref(null)
// author filter: '' = all authors, otherwise a name
const author = ref('')
const authorOptions = ref([]) // [{label, value}] for the dropdown
const data = ref(null)
const loading = ref(false)
const error = ref('')

// ISO date selected externally (e.g. via day-ranking click) → passed to the
// heatmap component so it can highlight the cell + show its hourly drill-down.
const selectedHeatDate = ref('')

// quick presets for the range picker
const rangePresets = [
  { label: '近 30 天', value: [dayjs().subtract(29, 'day'), dayjs()] },
  { label: '近 90 天', value: [dayjs().subtract(89, 'day'), dayjs()] },
  { label: '今年', value: [dayjs().startOf('year'), dayjs()] },
  { label: '近 1 年', value: [dayjs().subtract(364, 'day'), dayjs()] },
]

// ---- author dropdown ----
async function loadAuthors() {
  if (!app) return
  try {
    const list = await app.InfoAuthors(repo.repoArg(), 0)
    authorOptions.value = (list || []).map((a) => ({ label: a.name, value: a.name }))
  } catch {
    authorOptions.value = []
  }
}

// ---- contribution ranking (by author / by day, multi-metric) ----
const RANK_LIMIT = 15
const rankExpanded = ref(false)
const rankDim = ref('author') // author | day
// author metrics
const rankMetric = ref('commits') // commits | activeDays | recent30 | streak
const rankMetrics = [
  { key: 'commits', label: '总提交' },
  { key: 'recent30', label: '近30天' },
  { key: 'activeDays', label: '活跃天数' },
  { key: 'streak', label: '连续天数' },
]
// author ranking (sorted by selected metric, desc)
const sortedAuthors = computed(() => {
  const list = [...(data.value?.authors || [])]
  const m = rankMetric.value
  return list.sort((a, b) => (b[m] - a[m]) || a.name.localeCompare(b.name))
})
const maxAuthorMetric = computed(() => {
  const m = rankMetric.value
  return Math.max(1, ...(data.value?.authors || []).map((a) => a[m]))
})
const totalAuthorCommits = computed(() => (data.value?.authors || []).reduce((s, a) => s + a.commits, 0))

// day ranking: top productive days by commit count (uses DailyActivity, full history)
const topDays = computed(() => {
  const list = [...(data.value?.dailyActivity || [])]
  return list.sort((a, b) => b.count - a.count)
})
const maxDayCount = computed(() => Math.max(1, ...(data.value?.dailyActivity || []).map((d) => d.count)))

const visibleAuthors = computed(() => {
  const list = sortedAuthors.value
  return rankExpanded.value ? list : list.slice(0, RANK_LIMIT)
})
const visibleDays = computed(() => {
  const list = topDays.value
  return rankExpanded.value ? list : list.slice(0, RANK_LIMIT)
})

// day dimension helpers
const todayISOStr = localISO(new Date())
function weekdayLabel(iso) {
  const d = new Date(iso + 'T00:00:00')
  return ['周日', '周一', '周二', '周三', '周四', '周五', '周六'][d.getDay()]
}
function selectDayByDate(date) {
  // toggle: clicking the same date again clears the selection
  selectedHeatDate.value = selectedHeatDate.value === date ? '' : date
}

// local YYYY-MM-DD (avoid UTC offset drift from toISOString)
function localISO(d) {
  const y = d.getFullYear()
  const m = String(d.getMonth() + 1).padStart(2, '0')
  const day = String(d.getDate()).padStart(2, '0')
  return `${y}-${m}-${day}`
}

async function load() {
  if (!app) return
  loading.value = true
  error.value = ''
  try {
    // extract since/until from the dayjs range, empty string when unbounded
    const since = dateRange.value && dateRange.value[0] ? dateRange.value[0].format('YYYY-MM-DD') : ''
    const until = dateRange.value && dateRange.value[1] ? dateRange.value[1].format('YYYY-MM-DD') : ''
    data.value = await app.StatsGet(repo.repoArg(), since, until, author.value || '')
    selectedHeatDate.value = ''
  } catch (e) {
    error.value = String(e?.message || e)
    data.value = null
  } finally {
    loading.value = false
  }
}

// initial load + reload on repo switch
load()
loadAuthors()
watch(() => repo.path, () => { load(); loadAuthors() })
</script>

<template>
  <div class="stats-view">
    <!-- filter bar -->
    <div class="stats-head">
      <CalendarOutlined class="head-icon" />
      <span class="head-title">时间范围</span>
      <a-range-picker
        v-model:value="dateRange"
        size="small"
        style="width: 240px"
        :presets="rangePresets"
        :allow-clear="true"
        format="YYYY-MM-DD"
      />
      <button
        v-if="dateRange"
        class="range-clear"
        title="清空时间范围"
        @click="dateRange = null"
      >✕</button>
      <span class="div"></span>
      <span class="lbl">作者</span>
      <a-select
        v-model:value="author"
        size="small"
        style="width: 200px"
        placeholder="全部贡献者"
        allow-clear
        :options="authorOptions"
      />
      <div class="head-spacer"></div>
      <a-button size="small" :loading="loading" @click="load">
        <template #icon><ReloadOutlined /></template>统计
      </a-button>
    </div>

    <a-spin :spinning="loading">
      <div v-if="error" class="error">{{ error }}</div>
      <div v-else-if="data" class="stats-body">
        <!-- stat cards -->
        <div class="cards">
          <div class="card">
            <LineChartOutlined class="c-icon" />
            <div class="c-body"><div class="v">{{ data.totalCommits }}</div><div class="l">总提交</div></div>
          </div>
          <div class="card">
            <CalendarOutlined class="c-icon" />
            <div class="c-body"><div class="v">{{ data.activeDays }}</div><div class="l">活跃天数</div></div>
          </div>
          <div class="card">
            <FireOutlined class="c-icon" />
            <div class="c-body"><div class="v">{{ data.currentStreak }}</div><div class="l">当前连续</div></div>
          </div>
          <div class="card">
            <TrophyOutlined class="c-icon" />
            <div class="c-body"><div class="v">{{ data.longestStreak }}</div><div class="l">最长连续</div></div>
          </div>
        </div>

        <!-- daily activity heatmap (full-width, self-contained block) -->
        <StatsHeatmap
          :day-hour-activity="data.dayHourActivity || []"
          :selected-date="selectedHeatDate"
          :repo-arg="repo.repoArg()"
          @select-day="selectDayByDate"
        />

        <!-- contribution ranking (by author / by day) -->
        <div class="block rank-block" v-if="data.authors?.length || data.dailyActivity?.length">
          <div class="block-head">
            <TrophyOutlined class="bh-icon" />
            <span class="block-title">贡献排行</span>
            <div class="rank-dims">
              <button class="metric-chip" :class="{ on: rankDim === 'author' }" @click="rankDim = 'author'; rankExpanded = false">按作者</button>
              <button class="metric-chip" :class="{ on: rankDim === 'day' }" @click="rankDim = 'day'; rankExpanded = false">按日期</button>
            </div>
            <!-- author metrics -->
            <div class="rank-metrics" v-if="rankDim === 'author'">
              <button
                v-for="m in rankMetrics" :key="m.key"
                class="metric-chip" :class="{ on: rankMetric === m.key }"
                @click="rankMetric = m.key"
              >{{ m.label }}</button>
            </div>
            <span class="block-sub" v-if="rankDim === 'author' && data.authors?.length">共 {{ data.authors.length }} 人</span>
            <span class="block-sub" v-else-if="rankDim === 'day' && data.dailyActivity?.length">共 {{ data.dailyActivity.length }} 个活跃日</span>
          </div>

          <!-- author ranking -->
          <div class="rank-list" v-if="rankDim === 'author'">
            <div
              v-for="(a, i) in visibleAuthors" :key="a.name"
              class="rank-item"
              :class="{ me: author === a.name }"
              @click="author = (author === a.name ? '' : a.name); load()"
            >
              <span class="rank-no">{{ i + 1 }}</span>
              <span class="rank-name">{{ a.name }}</span>
              <div class="rank-track">
                <div class="rank-fill" :style="{ width: (a[rankMetric] / maxAuthorMetric * 100) + '%' }"></div>
              </div>
              <span class="rank-count">{{ a[rankMetric] }}</span>
              <span class="rank-pct" v-if="rankMetric === 'commits'">{{ (a.commits / totalAuthorCommits * 100).toFixed(1) }}%</span>
              <span class="rank-pct" v-else>{{ a.lastCommit }}</span>
            </div>
          </div>

          <!-- day ranking -->
          <div class="rank-list" v-else>
            <div
              v-for="(d, i) in visibleDays" :key="d.date"
              class="rank-item"
              @click="selectDayByDate(d.date)"
            >
              <span class="rank-no">{{ i + 1 }}</span>
              <span class="rank-name">{{ d.date }}</span>
              <div class="rank-track">
                <div class="rank-fill" :style="{ width: (d.count / maxDayCount * 100) + '%' }"></div>
              </div>
              <span class="rank-count">{{ d.count }}</span>
              <span class="rank-pct">{{ d.date === todayISOStr ? '今天' : weekdayLabel(d.date) }}</span>
            </div>
          </div>

          <a-button
            v-if="rankDim === 'author' && data.authors?.length > RANK_LIMIT || rankDim === 'day' && data.dailyActivity?.length > RANK_LIMIT"
            type="link" size="small" class="rank-more"
            @click="rankExpanded = !rankExpanded"
          >{{ rankExpanded ? '收起' : `展开全部` }}</a-button>
        </div>
      </div>
    </a-spin>
  </div>
</template>

<style scoped>
.stats-view { width: 100%; }

/* filter bar */
.stats-head { display: flex; align-items: center; gap: var(--spacing-xs); flex-wrap: wrap; margin-bottom: var(--spacing-md); }
.head-icon { color: var(--color-text-tertiary); font-size: 14px; }
.head-title { font-size: 13px; color: var(--color-text-secondary); margin-right: 4px; }
.head-spacer { flex: 1; }
.lbl { font-size: 12px; color: var(--color-text-tertiary); margin: 0 2px; }
.div { width: 1px; height: 18px; background: var(--color-border); margin: 0 6px; }

/* standalone clear button for the range picker — always visible when a range
   is set, unlike the picker's built-in clear (hover-only, easy to miss). */
.range-clear {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 18px;
  height: 22px;
  border: none;
  background: transparent;
  color: var(--color-text-tertiary);
  cursor: pointer;
  font-size: 11px;
  line-height: 1;
  border-radius: var(--radius-xs);
  transition: all var(--transition-fast);
  margin-left: -4px;
}
.range-clear:hover {
  color: var(--color-error);
  background: var(--color-error-bg);
}

.error { color: var(--color-error); font-size: 13px; }

/* cards */
.cards { display: grid; grid-template-columns: repeat(4, 1fr); gap: var(--spacing-md); margin-bottom: var(--spacing-md); }
.card { display: flex; align-items: center; gap: 12px; background: var(--color-surface); border: 1px solid var(--color-border); border-radius: var(--radius-md); padding: var(--spacing-md) var(--spacing-lg); box-shadow: var(--shadow-xs); }
.c-icon { font-size: 28px; color: var(--color-primary); opacity: 0.8; }
.c-body .v { font-size: 26px; font-weight: 700; color: var(--color-text); line-height: 1.1; }
.c-body .l { font-size: 12px; color: var(--color-text-tertiary); margin-top: 2px; }

/* blocks */
.block { background: var(--color-surface); border: 1px solid var(--color-border); border-radius: var(--radius-md); padding: var(--spacing-md); box-shadow: var(--shadow-xs); }
.block-head { display: flex; align-items: center; gap: 8px; margin-bottom: var(--spacing-sm); }
.bh-icon { color: var(--color-primary); font-size: 15px; }
.block-title { font-size: 14px; font-weight: 600; color: var(--color-text); }
.block-sub { font-size: 11px; color: var(--color-text-tertiary); }

/* ===== contributors ranking ===== */
.rank-block { margin-top: var(--spacing-md); }
.rank-dims { display: flex; gap: 4px; margin-left: 6px; }
.rank-metrics { display: flex; gap: 4px; margin-left: 6px; }
.metric-chip {
  font-size: 11px; padding: 2px 9px; cursor: pointer;
  border: 1px solid var(--color-border); border-radius: 999px;
  background: var(--color-background); color: var(--color-text-secondary);
  transition: all var(--transition-fast);
}
.metric-chip:hover { color: var(--color-primary); border-color: var(--color-primary); }
.metric-chip.on { background: var(--color-primary); border-color: var(--color-primary); color: #fff; }
.rank-list { display: flex; flex-direction: column; gap: 4px; }
.rank-item {
  display: flex; align-items: center; gap: 10px;
  padding: 6px 8px; border-radius: var(--radius-sm);
  cursor: pointer; transition: background var(--transition-fast);
}
.rank-item:hover { background: var(--color-hover); }
.rank-item.me { background: var(--color-primary-bg); }
.rank-no {
  width: 22px; flex-shrink: 0; text-align: center;
  font-size: 12px; font-weight: 700; color: var(--color-text-tertiary);
}
.rank-item:nth-child(1) .rank-no { color: #FFD700; }
.rank-item:nth-child(2) .rank-no { color: #C0C0C0; }
.rank-item:nth-child(3) .rank-no { color: #CD7F32; }
.rank-name {
  width: 130px; flex-shrink: 0; font-size: 13px; color: var(--color-text);
  overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
}
.rank-track { flex: 1; height: 8px; background: var(--color-surface-raised); border-radius: 4px; overflow: hidden; }
.rank-fill { height: 100%; background: linear-gradient(90deg, var(--color-primary), var(--color-accent)); border-radius: 4px; transition: width var(--transition-normal); }
.rank-count { width: 50px; flex-shrink: 0; text-align: right; font-size: 12px; font-weight: 600; color: var(--color-text); }
.rank-pct { width: 48px; flex-shrink: 0; text-align: right; font-size: 11px; color: var(--color-text-tertiary); }
.rank-more { margin-top: 4px; padding: 0; }

@media (max-width: 900px) { .cards { grid-template-columns: repeat(2, 1fr); } }
</style>
