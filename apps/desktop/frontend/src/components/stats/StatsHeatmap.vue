<script setup>
/**
 * GitHub 风格热力图组件 — 从 StatsView 拆出。
 * 展示全年每日提交热力图 + 按小时钻取面板。
 *
 * 视觉与交互保持与拆分前 StatsView 完全一致：
 *   - 顶部 block-head：图标/标题/副标题 + 年份切换器
 *   - 横向月份标签行（与周列对齐）
 *   - 全宽 15px 圆角格子，今日脉冲动画
 *   - fixed 定位的深色 hover tooltip（带箭头）
 *   - 选中日按小时柱状钻取面板（渐变柱）
 *
 * 父组件通信：
 *   - props.dayHourActivity: DayHourEntry[]
 *   - props.selectedDate:    ISO YYYY-MM-DD（外部选中，如日榜点击）
 *   - emit('select-day', iso): 内部点击单元格时触发
 */
import { ref, computed, watch } from 'vue'
import { ClockCircleOutlined } from '@ant-design/icons-vue'

const props = defineProps({
  dayHourActivity: { type: Array, default: () => [] },
  selectedDate: { type: String, default: '' },
  // repo arg passed through to the LogFetchByDate backend call; empty = use
  // the backend's current-repo fallback.
  repoArg: { type: String, default: '' },
})
const emit = defineEmits(['select-day'])

const app = window.go?.main?.ByteGitApp

const weekdayShort = ['', '周一', '', '周三', '', '周五', '']

const selectedDay = ref(null) // selected cell object
const heatYear = ref(new Date().getFullYear()) // currently displayed year

// map date -> DayHourEntry for O(1) lookup
const dayHourMap = computed(() => {
  const m = new Map()
  for (const d of props.dayHourActivity) m.set(d.date, d)
  return m
})

// local YYYY-MM-DD (avoid UTC offset drift from toISOString)
function localISO(d) {
  const y = d.getFullYear()
  const m = String(d.getMonth() + 1).padStart(2, '0')
  const day = String(d.getDate()).padStart(2, '0')
  return `${y}-${m}-${day}`
}

// min / max year available in data (for disabling switcher buttons)
const heatYearRange = computed(() => {
  if (!props.dayHourActivity.length) {
    const y = new Date().getFullYear()
    return { min: y, max: y }
  }
  let min = 9999, max = 0
  for (const d of props.dayHourActivity) {
    const y = parseInt(d.date.slice(0, 4), 10)
    if (y < min) min = y
    if (y > max) max = y
  }
  // allow navigating up to current year even if no data this year yet
  const nowY = new Date().getFullYear()
  if (nowY > max) max = nowY
  return { min, max }
})

// build full-year week columns for heatYear.
// Each column = 7 days (Sun..Sat). Covers Jan 1 (aligned to Sunday) → Dec 31
// (aligned to Saturday). Out-of-year padding cells are marked !inRange.
const heatWeeks = computed(() => {
  const year = heatYear.value
  const jan1 = new Date(year, 0, 1)
  const dec31 = new Date(year, 11, 31)
  // align to Sunday on/before Jan 1
  const start = new Date(jan1)
  start.setDate(start.getDate() - start.getDay())
  // align to Saturday on/after Dec 31
  const end = new Date(dec31)
  end.setDate(end.getDate() + (6 - end.getDay()))

  const startISO = localISO(jan1)
  const endISO = localISO(dec31)
  const todayISO = localISO(new Date())

  const weeks = []
  const cur = new Date(start)
  while (cur <= end) {
    const week = []
    for (let dow = 0; dow < 7; dow++) {
      const iso = localISO(cur)
      const entry = dayHourMap.value.get(iso)
      const inYear = iso >= startISO && iso <= endISO
      week.push({
        date: iso,
        count: entry ? entry.count : 0,
        hours: entry ? entry.hours : null,
        inRange: inYear,
        pad: !inYear,
        today: iso === todayISO,
      })
      cur.setDate(cur.getDate() + 1)
    }
    weeks.push(week)
  }
  return weeks
})

// max commit count across full history (drives color scale; global, not per-year)
const maxHeat = computed(() => Math.max(1, ...props.dayHourActivity.map((d) => d.count)))

// month label index per week column (shown when month changes vs previous column)
const heatMonths = computed(() => {
  return heatWeeks.value.map((wk) => {
    const inRangeCell = wk.find((c) => c.inRange) || wk[0]
    const d = new Date(inRangeCell.date + 'T00:00:00')
    return d.getMonth() // 0-11
  })
})

// total commits in current year view
const heatYearTotal = computed(() => {
  const y = heatYear.value
  let sum = 0
  for (const d of props.dayHourActivity) {
    if (d.date.slice(0, 4) === String(y)) sum += d.count
  }
  return sum
})

function heatLevel(count) {
  if (count <= 0) return 0
  const r = count / maxHeat.value
  if (r < 0.25) return 1
  if (r < 0.5) return 2
  if (r < 0.75) return 3
  return 4
}

function heatBg(count) {
  const lv = heatLevel(count)
  return [`var(--heat-0)`, `var(--heat-1)`, `var(--heat-2)`, `var(--heat-3)`, `var(--heat-4)`][lv]
}

function clickCell(cell) {
  if (!cell.inRange || cell.count === 0) return
  emit('select-day', cell.date)
}

// internal selection state is driven by props.selectedDate (single source of
// truth). Whenever it changes (internal click or external day-ranking click),
// update the drill-down panel + fetch the commit list for that day.
watch(() => props.selectedDate, (iso) => {
  if (!iso) { selectedDay.value = null; dayCommits.value = []; return }
  const entry = dayHourMap.value.get(iso)
  selectedDay.value = entry
    ? { date: entry.date, hours: entry.hours, count: entry.count, inRange: true }
    : { date: iso, hours: null, count: 0, inRange: true }
  loadDayCommits(iso)
})

// ---- custom hover tooltip ----
const tooltip = ref({ visible: false, x: 0, y: 0, date: '', count: 0, today: false })

function onCellEnter(cell, ev) {
  if (cell.pad) return
  const r = ev.currentTarget.getBoundingClientRect()
  tooltip.value = {
    visible: true,
    x: r.left + r.width / 2,
    y: r.top,
    date: cell.date,
    count: cell.count,
    today: cell.today,
  }
}
function onCellLeave() {
  tooltip.value.visible = false
}

function prevYear() {
  if (heatYear.value > heatYearRange.value.min) {
    heatYear.value--
    selectedDay.value = null
    emit('select-day', '')
  }
}
function nextYear() {
  if (heatYear.value < heatYearRange.value.max) {
    heatYear.value++
    selectedDay.value = null
    emit('select-day', '')
  }
}

const selectedHours = computed(() => {
  if (!selectedDay.value) return []
  const h = selectedDay.value.hours || new Array(24).fill(0)
  const max = Math.max(1, ...h)
  return h.map((c, i) => ({ hour: i, count: c, pct: c / max * 100 }))
})

// ---- commit list for the selected day ----
// loaded on demand when a day is selected; mirrors the hourly bar chart but
// shows the actual commits (hash / author / time / subject) so the user can
// see what was done, not just when.
const dayCommits = ref([])
const commitsLoading = ref(false)

async function loadDayCommits(date) {
  if (!date || !app?.LogFetchByDate) { dayCommits.value = []; return }
  commitsLoading.value = true
  try {
    dayCommits.value = (await app.LogFetchByDate(props.repoArg, date)) || []
  } catch {
    dayCommits.value = []
  } finally {
    commitsLoading.value = false
  }
}

// format RFC3339 → "HH:MM" for compact display in the commit list
function timeOf(rfc3339) {
  if (!rfc3339) return ''
  const t = rfc3339.slice(11, 16) // "HH:MM" from "2026-07-07T14:30:00..."
  return t || ''
}

function clearSelection() {
  selectedDay.value = null
  dayCommits.value = []
  emit('select-day', '')
}
</script>

<template>
  <div class="block daily-block">
    <div class="block-head">
      <ClockCircleOutlined class="bh-icon" />
      <span class="block-title">提交热力图</span>
      <span class="block-sub">{{ heatYear }} 年 · {{ heatYearTotal }} 次提交 · 点击格子查看每小时详情</span>
      <div class="head-spacer"></div>
      <div class="year-switcher">
        <button class="yb" :disabled="heatYear <= heatYearRange.min" @click="prevYear">‹</button>
        <span class="year-label">{{ heatYear }}</span>
        <button class="yb" :disabled="heatYear >= heatYearRange.max" @click="nextYear">›</button>
      </div>
    </div>

    <div class="heat-scroll">
      <!-- custom hover tooltip -->
      <div v-show="tooltip.visible" class="heat-tip" :style="{ left: tooltip.x + 'px', top: tooltip.y + 'px' }">
        <span class="tip-date">{{ tooltip.date }}<span v-if="tooltip.today" class="tip-today">今天</span></span>
        <span class="tip-count">{{ tooltip.count }} 次提交</span>
      </div>
      <!-- month labels row (aligned with week columns) -->
      <div class="heat-months">
        <span
          v-for="(mi, wi) in heatMonths" :key="wi"
          class="hm-cell"
        >{{ (wi === 0 || mi !== heatMonths[wi-1]) ? (mi+1)+'月' : '' }}</span>
      </div>
      <div class="heatmap">
        <!-- weekday labels -->
        <div class="heat-dows">
          <span></span>
          <span v-for="(w, i) in weekdayShort" :key="i">{{ w }}</span>
        </div>
        <!-- weeks -->
        <div class="heat-weeks">
          <div v-for="(wk, wi) in heatWeeks" :key="wi" class="heat-week">
            <div
              v-for="cell in wk" :key="cell.date"
              class="heat-cell"
              :class="{
                pad: cell.pad,
                today: cell.today,
                active: selectedDay?.date === cell.date,
                clickable: cell.inRange && cell.count > 0,
              }"
              :style="{ background: heatBg(cell.count) }"
              @click="clickCell(cell)"
              @mouseenter="onCellEnter(cell, $event)"
              @mouseleave="onCellLeave"
            ></div>
          </div>
        </div>
      </div>
      <!-- legend -->
      <div class="heat-legend">
        <span>少</span>
        <span class="lc" :style="{background: heatBg(0)}"></span>
        <span class="lc" :style="{background: heatBg(maxHeat*0.2)}"></span>
        <span class="lc" :style="{background: heatBg(maxHeat*0.5)}"></span>
        <span class="lc" :style="{background: heatBg(maxHeat*0.8)}"></span>
        <span class="lc" :style="{background: heatBg(maxHeat)}"></span>
        <span>多</span>
      </div>
    </div>

    <!-- per-hour drill-down panel -->
    <transition name="slide">
      <div v-if="selectedDay" class="day-detail">
        <div class="dd-head">
          <span class="dd-date">{{ selectedDay.date }}</span>
          <span class="dd-count">共 {{ selectedDay.count }} 次提交</span>
          <span class="dd-close" @click="clearSelection">✕</span>
        </div>
        <div class="dd-hours">
          <div
            v-for="h in selectedHours" :key="h.hour"
            class="dd-hcol"
            :title="`${h.hour}时: ${h.count} 次`"
          >
            <div class="dd-hbar" :style="{ height: h.pct + '%' }">
              <span v-if="h.count > 0" class="dd-hnum">{{ h.count }}</span>
            </div>
          </div>
        </div>
        <div class="dd-haxis">
          <span>0</span><span>3</span><span>6</span><span>9</span><span>12</span>
          <span>15</span><span>18</span><span>21</span><span>23</span>
        </div>

        <!-- commit list for the selected day -->
        <div class="dd-commits">
          <div class="ddc-head">
            <span>提交列表</span>
            <span class="ddc-count">{{ dayCommits.length }} 条</span>
          </div>
          <div v-if="commitsLoading" class="ddc-loading"><a-spin size="small" /></div>
          <div v-else-if="!dayCommits.length" class="ddc-empty">无提交记录</div>
          <div v-else class="ddc-list">
            <div v-for="(c, i) in dayCommits" :key="c.hash" class="ddc-item" :title="c.subject">
              <span class="ddc-idx">{{ i + 1 }}</span>
              <span class="ddc-hash">{{ c.hash }}</span>
              <span class="ddc-time">{{ timeOf(c.date) }}</span>
              <span class="ddc-author">{{ c.author }}</span>
              <span class="ddc-subject">{{ c.subject }}</span>
            </div>
          </div>
        </div>
      </div>
    </transition>

    <div v-if="!dayHourActivity?.length" class="muted">无数据</div>
  </div>
</template>

<style scoped>
/* block shell (mirrors StatsView's .block styling so the component is self-contained) */
.block { background: var(--color-surface); border: 1px solid var(--color-border); border-radius: var(--radius-md); padding: var(--spacing-md); box-shadow: var(--shadow-xs); }
.block-head { display: flex; align-items: center; gap: 8px; margin-bottom: var(--spacing-sm); }
.bh-icon { color: var(--color-primary); font-size: 15px; }
.block-title { font-size: 14px; font-weight: 600; color: var(--color-text); }
.block-sub { font-size: 11px; color: var(--color-text-tertiary); }
.head-spacer { flex: 1; }

.daily-block { margin-bottom: var(--spacing-md); }
.muted { color: var(--color-text-tertiary); font-size: 13px; }

.heat-scroll { position: relative; overflow-x: auto; overflow-y: hidden; padding: 6px 2px 8px; }
.heat-scroll::-webkit-scrollbar { height: 8px; }
.heat-scroll::-webkit-scrollbar-thumb { background: var(--color-border); border-radius: 4px; }
.heat-scroll::-webkit-scrollbar-track { background: transparent; }

/* year switcher */
.year-switcher { display: flex; align-items: center; gap: 6px; }
.yb {
  width: 24px; height: 24px; border-radius: var(--radius-sm);
  border: 1px solid var(--color-border); background: var(--color-background);
  color: var(--color-text-secondary); cursor: pointer; font-size: 14px; line-height: 1;
  display: flex; align-items: center; justify-content: center;
  transition: all var(--transition-fast);
}
.yb:hover:not(:disabled) { color: var(--color-primary); border-color: var(--color-primary); }
.yb:disabled { opacity: 0.35; cursor: not-allowed; }
.year-label { font-size: 13px; font-weight: 600; color: var(--color-text); min-width: 38px; text-align: center; }

/* custom hover tooltip */
.heat-tip {
  position: fixed; z-index: 9999;
  transform: translate(-50%, -100%) translateY(-8px);
  background: var(--color-text); color: var(--color-background);
  padding: 6px 10px; border-radius: var(--radius-sm);
  font-size: 11px; line-height: 1.5; white-space: nowrap;
  pointer-events: none;
  box-shadow: var(--shadow-md);
  display: flex; flex-direction: column; gap: 2px; align-items: flex-start;
}
.heat-tip::after {
  content: ''; position: absolute; left: 50%; bottom: -4px;
  transform: translateX(-50%) rotate(45deg);
  width: 8px; height: 8px; background: var(--color-text);
}
.tip-date { font-weight: 600; display: inline-flex; align-items: center; gap: 6px; }
.tip-today {
  font-size: 9px; font-weight: 700; padding: 1px 5px; border-radius: 999px;
  background: var(--color-accent); color: #fff;
}
.tip-count { color: var(--color-background); opacity: 0.65; font-size: 10px; }

/* top axis: month labels row — fills width to align with weeks */
.heat-months {
  display: flex; gap: 3px; padding-left: 34px;
  margin-bottom: 4px; font-size: 10px; color: var(--color-text-tertiary);
}
.heat-months .hm-cell { flex: 1; min-width: 0; text-align: left; white-space: nowrap; }
.heatmap { display: flex; gap: 4px; width: 100%; }
/* weekday labels column */
.heat-dows { display: flex; flex-direction: column; gap: 3px; flex: 0 0 auto; }
.heat-dows span { height: 15px; line-height: 15px; font-size: 10px; color: var(--color-text-tertiary); width: 30px; }

.heat-weeks { display: flex; gap: 3px; flex: 1; min-width: 0; }
.heat-week { display: flex; flex-direction: column; gap: 3px; flex: 1; min-width: 0; }
.heat-cell {
  height: 15px; width: 100%; border-radius: 3px;
  background: var(--heat-0);
  border: 1px solid var(--color-border-light);
  transition: transform var(--transition-fast), box-shadow var(--transition-fast);
}
.heat-cell.pad { opacity: 0; border-color: transparent; }
.heat-cell.today {
  outline: 1.5px solid var(--color-accent);
  outline-offset: -1px;
  box-shadow: 0 0 0 2px var(--color-accent-bg);
  animation: today-pulse 2s ease-in-out infinite;
}
.heat-cell.today.active { outline-color: var(--color-primary); }
.heat-cell.clickable { cursor: pointer; }
@keyframes today-pulse {
  0%, 100% { box-shadow: 0 0 0 2px rgba(255, 121, 198, 0.25); }
  50% { box-shadow: 0 0 0 4px rgba(255, 121, 198, 0.4); }
}
.heat-cell.clickable:hover {
  transform: scale(1.25); z-index: 2;
  box-shadow: 0 0 0 2px var(--color-primary);
}
.heat-cell.active { box-shadow: 0 0 0 2px var(--color-primary); }

.heat-legend { display: flex; align-items: center; gap: 4px; margin-top: 10px; justify-content: flex-end; font-size: 10px; color: var(--color-text-tertiary); }
.heat-legend .lc { width: 12px; height: 12px; border-radius: 2px; display: inline-block; }

/* day detail drill-down */
.day-detail {
  margin-top: var(--spacing-sm); padding: 12px 14px;
  background: var(--color-surface-raised);
  border: 1px solid var(--color-border);
  border-radius: var(--radius-sm);
}
.dd-head { display: flex; align-items: center; gap: 10px; margin-bottom: 10px; }
.dd-date { font-size: 13px; font-weight: 600; color: var(--color-text); }
.dd-count { font-size: 12px; color: var(--color-text-secondary); }
.dd-close { margin-left: auto; cursor: pointer; color: var(--color-text-tertiary); font-size: 14px; }
.dd-close:hover { color: var(--color-error); }
.dd-hours { display: flex; align-items: flex-end; gap: 2px; height: 80px; }
.dd-hcol { flex: 1; display: flex; flex-direction: column; justify-content: flex-end; height: 100%; min-width: 0; }
.dd-hbar {
  width: 70%; margin: 0 auto; min-height: 2px;
  background: linear-gradient(180deg, var(--color-primary), var(--color-accent));
  border-radius: 2px 2px 0 0;
  display: flex; justify-content: center; align-items: flex-start;
  position: relative;
}
.dd-hnum {
  position: absolute; top: -14px; font-size: 9px; color: var(--color-text-secondary);
}
.dd-haxis { display: flex; justify-content: space-between; font-size: 10px; color: var(--color-text-tertiary); margin-top: 4px; }

/* commit list under the hourly chart */
.dd-commits { margin-top: 14px; border-top: 1px solid var(--color-border-light); padding-top: 10px; }
.ddc-head { display: flex; align-items: center; justify-content: space-between; font-size: 11px; font-weight: 600; color: var(--color-text-secondary); margin-bottom: 6px; }
.ddc-count { font-weight: 400; color: var(--color-text-tertiary); }
.ddc-loading, .ddc-empty { padding: 12px; text-align: center; font-size: 11px; color: var(--color-text-tertiary); }
.ddc-list { display: flex; flex-direction: column; gap: 2px; max-height: 280px; overflow-y: auto; }
.ddc-list::-webkit-scrollbar { width: 5px; }
.ddc-list::-webkit-scrollbar-thumb { background: var(--color-border); border-radius: 3px; }
.ddc-item {
  display: grid; grid-template-columns: 28px 52px 38px 80px 1fr; gap: 8px;
  padding: 5px 8px; border-radius: var(--radius-xs); font-size: 11px;
  align-items: center; transition: background var(--transition-fast);
}
.ddc-item:hover { background: var(--color-hover); }
.ddc-idx { font-family: 'Cascadia Code', monospace; color: var(--color-text-tertiary); font-size: 10px; text-align: right; }
.ddc-hash { font-family: 'Cascadia Code', monospace; color: var(--color-hash); font-size: 10px; }
.ddc-time { font-family: 'Cascadia Code', monospace; color: var(--color-text-tertiary); font-size: 10px; }
.ddc-author { color: var(--color-text-secondary); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.ddc-subject { color: var(--color-text); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }

.slide-enter-active, .slide-leave-active { transition: all var(--transition-fast); }
.slide-enter-from, .slide-leave-to { opacity: 0; transform: translateY(-6px); }
</style>
