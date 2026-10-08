<script setup>
/**
 * 导出向导 Step 2 — 日期重映射面板。从 ExportWizard 拆出。
 * 支持两种模式：
 *   - range：范围 + 每日上限 + 活跃时段
 *   - daily：逐日分配（可增删天数，自动按 previewCount 平衡）
 *
 * 父组件通过 v-model:remap 双向同步重映射配置对象：
 *   { enabled, mode, dateRange, dailyLimit, activeHour, dateMode, allocs }
 */
import { ref, computed, watch } from 'vue'
import dayjs from 'dayjs'

const props = defineProps({
  // 双向绑定的 remap 配置对象
  modelValue: { type: Object, required: true },
  // 当前选择将导出的提交数（用于逐日分配平衡提示）
  previewCount: { type: Number, default: 0 },
})
const emit = defineEmits(['update:modelValue'])

// 本地可变状态（与父配置双向同步）
const remap = ref({ ...props.modelValue })

// 范围模式：开始/结束（dayjs 对象），由 dateRange 字符串初始化
const dateStart = ref(props.modelValue.dateRange ? dayjs(props.modelValue.dateRange.split(',')[0]) : null)
const dateEnd = ref(props.modelValue.dateRange ? dayjs(props.modelValue.dateRange.split(',')[1]) : null)

// 逐日分配
const dailyRows = ref([])
const dailyStart = ref(null)
const dailyEnd = ref(null)

const dailyRangeDays = computed(() => {
  if (!dailyStart.value || !dailyEnd.value) return 0
  return dayjs(dailyEnd.value).diff(dayjs(dailyStart.value), 'day') + 1
})

const dailySum = computed(() => dailyRows.value.reduce((s, r) => s + (Number(r.count) || 0), 0))
const dailyDiff = computed(() => dailySum.value - props.previewCount)
const dailyHint = computed(() => {
  if (!props.previewCount) return ''
  if (dailyDiff.value === 0) return `已平衡：${dailySum.value} / ${props.previewCount}`
  if (dailyDiff.value > 0) return `超出 ${dailyDiff.value} 条（将从最后一天扣减）`
  return `不足 ${-dailyDiff.value} 条（将均匀补到现有天数）`
})

// 根据当前范围展开每天的行，按 previewCount 均分默认值
function rebuildDailyRows() {
  if (!dailyStart.value || !dailyEnd.value) { dailyRows.value = []; return }
  const s = dayjs(dailyStart.value)
  const e = dayjs(dailyEnd.value)
  if (e.isBefore(s)) { dailyRows.value = []; return }
  const days = e.diff(s, 'day') + 1
  const total = props.previewCount || 0
  const per = total > 0 ? Math.max(1, Math.ceil(total / days)) : 1
  const init = []
  for (let i = 0; i < days; i++) {
    const existing = dailyRows.value.find((r) => dayjs(r.date).isSame(s.add(i, 'day')))
    init.push({ date: s.add(i, 'day'), count: existing ? existing.count : per })
  }
  dailyRows.value = init
  commit()
}

function ensureDailyInit() {
  if (!dailyStart.value || !dailyEnd.value) {
    const today = dayjs()
    dailyStart.value = today.subtract(6, 'day')
    dailyEnd.value = today.subtract(0, 'day')
    rebuildDailyRows()
  } else if (!dailyRows.value.length) {
    rebuildDailyRows()
  }
}

function addDailyRow() {
  const last = dailyRows.value[dailyRows.value.length - 1]
  const base = last ? dayjs(last.date) : dayjs()
  dailyEnd.value = base.add(1, 'day')
}
function removeDailyRow(i) {
  dailyRows.value.splice(i, 1)
  if (dailyRows.value.length) {
    dailyEnd.value = dayjs(dailyRows.value[dailyRows.value.length - 1].date)
  } else {
    dailyEnd.value = dailyStart.value
  }
  commit()
}
function fillDailyRecent(n) {
  const today = dayjs()
  dailyStart.value = today.subtract(n - 1, 'day')
  dailyEnd.value = today
}

// 提交当前编辑结果到父组件
function commit() {
  const m = remap.value.mode
  const dateRangeStr = (dateStart.value && dateEnd.value)
    ? `${dateStart.value.format('YYYY-MM-DD HH:mm:ss')},${dateEnd.value.format('YYYY-MM-DD HH:mm:ss')}`
    : ''
  emit('update:modelValue', {
    ...remap.value,
    dateRange: m === 'range' ? dateRangeStr : '',
    allocs: m === 'daily'
      ? dailyRows.value
          .filter((r) => r.date && (Number(r.count) || 0) > 0)
          .sort((a, b) => (dayjs(a.date).isBefore(dayjs(b.date)) ? -1 : 1))
          .map((r) => ({ date: dayjs(r.date).format('YYYY-MM-DD'), count: Number(r.count) || 0 }))
      : [],
  })
}

// 智能修正使总数等于实际导出数
function reconcileDaily() {
  const total = props.previewCount
  if (!total) return []
  const counts = dailyRows.value.map((r) => Number(r.count) || 0)
  let cur = counts.reduce((s, c) => s + c, 0)
  if (cur > total) {
    for (let i = counts.length - 1; i >= 0 && cur > total; i--) {
      const cut = Math.min(counts[i], cur - total)
      counts[i] -= cut
      cur -= cut
    }
  } else if (cur < total) {
    const remain = total - cur
    const base = cur || counts.length
    let allocated = 0
    for (let i = 0; i < counts.length; i++) {
      if (i === counts.length - 1) counts[i] += remain - allocated
      else {
        const add = Math.round((remain * (counts[i] || 0)) / base)
        counts[i] += add
        allocated += add
      }
    }
  }
  return dailyRows.value
    .map((r, i) => ({ date: dayjs(r.date), count: counts[i] }))
    .filter((r) => r.count > 0 && r.date)
    .sort((a, b) => (dayjs(a.date).isBefore(dayjs(b.date)) ? -1 : 1))
    .map((r) => ({ date: dayjs(r.date).format('YYYY-MM-DD'), count: r.count }))
}

// 当 diff != 0 时按需触发智能修正并回写父组件
watch(dailyDiff, () => {
  if (remap.value.mode !== 'daily') return
  if (dailyDiff.value === 0) { commit(); return }
  const allocs = reconcileDaily()
  emit('update:modelValue', {
    ...remap.value,
    dateRange: '',
    allocs,
  })
})

watch(() => [dailyStart.value, dailyEnd.value], rebuildDailyRows)
watch(() => props.modelValue, (v) => { remap.value = { ...v } }, { deep: true })
watch(() => remap.value.mode, (m) => { if (m === 'daily') ensureDailyInit() })

// 初始化：若启用且为 daily 模式，确保 dailyRows 已展开
if (props.modelValue.enabled && props.modelValue.mode === 'daily') {
  ensureDailyInit()
}

// date presets
function daysAgo(n) { return dayjs().subtract(n, 'day') }
const datePresets = computed(() => {
  const today = dayjs()
  return [
    { label: '今天', start: today.startOf('day'), end: today.endOf('day') },
    { label: '昨天', start: daysAgo(1).startOf('day'), end: daysAgo(1).endOf('day') },
    { label: '最近 3 天', start: daysAgo(2).startOf('day'), end: today.endOf('day') },
    { label: '最近 7 天', start: daysAgo(6).startOf('day'), end: today.endOf('day') },
    { label: '最近 30 天', start: daysAgo(29).startOf('day'), end: today.endOf('day') },
    { label: '本月', start: today.startOf('month'), end: today.endOf('day') },
  ]
})
function presetActive(p) {
  return dateStart.value && dateEnd.value
    && dateStart.value.isSame(p.start) && dateEnd.value.isSame(p.end)
}
function applyDatePreset(p) {
  dateStart.value = p.start
  dateEnd.value = p.end
  commit()
}

const hourPresets = [
  { label: '工作时间 9-18', value: '9-18' },
  { label: '白天 8-22', value: '8-22' },
  { label: '晚间 19-24', value: '19-24' },
  { label: '深夜 22-2', value: '22-2' },
  { label: '凌晨 0-6', value: '0-6' },
]
</script>

<template>
  <a-checkbox v-model:checked="remap.enabled" @change="commit">启用日期重映射</a-checkbox>
  <div v-if="remap.enabled" class="remap-form">
    <div class="row">
      <span class="lbl w80">模式</span>
      <a-radio-group v-model:value="remap.mode" size="small" @change="commit">
        <a-radio-button value="range">范围分配</a-radio-button>
        <a-radio-button value="daily">逐日分配</a-radio-button>
      </a-radio-group>
    </div>

    <!-- 范围模式 -->
    <template v-if="remap.mode === 'range'">
      <div class="row">
        <span class="lbl w80">开始</span>
        <a-date-picker
          v-model:value="dateStart"
          show-time
          format="YYYY-MM-DD HH:mm:ss"
          placeholder="开始时间"
          size="small"
          style="width:200px"
          @change="commit"
        />
      </div>
      <div class="row">
        <span class="lbl w80">结束</span>
        <a-date-picker
          v-model:value="dateEnd"
          show-time
          format="YYYY-MM-DD HH:mm:ss"
          placeholder="结束时间"
          size="small"
          style="width:200px"
          @change="commit"
        />
      </div>
      <div class="preset-row">
        <span class="lbl w80">快捷</span>
        <div class="preset-chips">
          <button v-for="p in datePresets" :key="p.label" class="chip" :class="{ on: presetActive(p) }" @click="applyDatePreset(p)">{{ p.label }}</button>
        </div>
      </div>
      <div class="row">
        <span class="lbl w80">每日上限</span>
        <a-input-number v-model:value="remap.dailyLimit" :min="0" size="small" style="width:90px" @change="commit" />
        <span class="hint">0=不限</span>
      </div>
    </template>

    <!-- 逐日分配模式 -->
    <template v-else>
      <div class="row">
        <span class="lbl w80">开始</span>
        <a-date-picker v-model:value="dailyStart" format="YYYY-MM-DD" size="small" style="width:150px" :allow-clear="false" />
        <span class="lbl">结束</span>
        <a-date-picker v-model:value="dailyEnd" format="YYYY-MM-DD" size="small" style="width:150px" :allow-clear="false" />
        <span class="hint" v-if="dailyRangeDays > 0">共 {{ dailyRangeDays }} 天</span>
      </div>
      <div class="preset-row">
        <span class="lbl w80">快捷</span>
        <div class="preset-chips">
          <button class="chip" @click="fillDailyRecent(7)">最近 7 天</button>
          <button class="chip" @click="fillDailyRecent(14)">最近 14 天</button>
          <button class="chip" @click="fillDailyRecent(30)">最近 30 天</button>
        </div>
      </div>

      <div class="daily-table">
        <div class="dd-thead">
          <span class="col-date">日期</span>
          <span class="col-count">提交数</span>
          <span class="col-op"></span>
        </div>
        <div class="dd-tbody">
          <div v-for="(r, i) in dailyRows" :key="i" class="dd-row">
            <span class="dd-date">{{ dayjs(r.date).format('YYYY-MM-DD') }}</span>
            <a-input-number v-model:value="r.count" :min="0" size="small" style="width:80px" @change="commit" />
            <a-button size="small" type="text" danger @click="removeDailyRow(i)" title="移除该天">✕</a-button>
          </div>
          <div v-if="!dailyRows.length" class="dd-empty">请先选择开始与结束日期</div>
        </div>
        <div class="dd-sum" :class="{ ok: dailyDiff === 0, over: dailyDiff > 0, under: dailyDiff < 0 }">
          合计 <b>{{ dailySum }}</b> / {{ previewCount }}
          <span class="dd-hint" v-if="dailyHint">· {{ dailyHint }}</span>
        </div>
      </div>
    </template>

    <div class="row">
      <span class="lbl w80">分布</span>
      <a-radio-group v-model:value="remap.dateMode" size="small" @change="commit">
        <a-radio-button value="spread">均匀</a-radio-button>
        <a-radio-button value="random">随机</a-radio-button>
      </a-radio-group>
    </div>
    <div class="row">
      <span class="lbl w80">活跃时段</span>
      <a-input v-model:value="remap.activeHour" placeholder="9-22" size="small" style="width:70px" @change="commit" />
    </div>
    <div class="preset-row">
      <span class="lbl w80">快捷</span>
      <div class="preset-chips">
        <button v-for="p in hourPresets" :key="p.label" class="chip" :class="{ on: remap.activeHour === p.value }" @click="remap.activeHour = p.value; commit()">{{ p.label }}</button>
      </div>
    </div>
  </div>
  <div v-if="!remap.enabled" class="tip">不启用可跳过。导出后仍可手动改日期。</div>
</template>

<style scoped>
.remap-form { margin-top: var(--spacing-sm); padding-top: var(--spacing-sm); border-top: 1px solid var(--color-border-light); }
.row { display: flex; align-items: center; gap: var(--spacing-sm); margin-bottom: var(--spacing-sm); }
.lbl { font-size: 13px; color: var(--color-text-secondary); }
.w80 { width: 64px; }
.hint { font-size: 11px; color: var(--color-text-tertiary); }
.tip { font-size: 12px; color: var(--color-text-tertiary); margin-top: var(--spacing-sm); text-align: center; }
.preset-row { display: flex; align-items: flex-start; gap: var(--spacing-sm); margin-bottom: var(--spacing-sm); }
.preset-chips { display: flex; flex-wrap: wrap; gap: 6px; flex: 1; }
.chip {
  font-size: 11px; padding: 2px 8px; cursor: pointer;
  border: 1px solid var(--color-border); border-radius: 10px;
  background: var(--color-background); color: var(--color-text-secondary);
  transition: all var(--transition-fast);
}
.chip:hover { color: var(--color-primary); border-color: var(--color-primary); }
.chip.on { background: var(--color-primary); border-color: var(--color-primary); color: #fff; }

/* 逐日分配表 */
.daily-table { border: 1px solid var(--color-border-light); border-radius: var(--radius-sm); overflow: hidden; max-width: none; }
.dd-thead { display: grid; grid-template-columns: 1fr 90px 32px; gap: 6px; padding: 5px 8px; background: var(--color-surface); font-size: 11px; color: var(--color-text-secondary); border-bottom: 1px solid var(--color-border-light); }
.dd-tbody { max-height: 180px; overflow-y: auto; }
.dd-row { display: grid; grid-template-columns: 1fr 90px 32px; gap: 6px; padding: 5px 8px; align-items: center; border-bottom: 1px solid var(--color-border-light); }
.dd-row:last-child { border-bottom: none; }
.dd-empty { padding: 12px; text-align: center; font-size: 12px; color: var(--color-text-tertiary); }
.dd-date { font-family: 'Cascadia Code', monospace; font-size: 12px; color: var(--color-text); }
.dd-sum { padding: 6px 8px; font-size: 12px; background: var(--color-background); border-top: 1px solid var(--color-border-light); }
.dd-sum b { font-size: 13px; }
.dd-sum.ok { color: var(--color-success, #52c41a); }
.dd-sum.over { color: var(--color-error, #ff4d4f); }
.dd-sum.under { color: var(--color-warning, #faad14); }
.dd-hint { color: var(--color-text-tertiary); font-size: 11px; }
</style>
