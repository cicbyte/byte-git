<script setup>
/**
 * 逐日分配提交（重设计版）——核心不变量：任意时刻 Σ每天条数 === totalCount。
 *
 * - 打开/换范围/重新生成：按「每日条数形状」策略生成精确合计方案
 *   （均匀=最大余数法 / 随机波动 / 工作日优先），不再出现打开即超出。
 * - 手动改某天条数：其余天按「差额补偿」策略实时自动补偿，状态条恒显已平衡。
 * - 应用总是平衡方案，无额外确认弹窗（所见即所得）。
 *
 * emit 契约与旧版一致：{ allocs: [{date, count}], mode, activeHour }。
 */
import { ref, computed, watch } from 'vue'
import { PlusOutlined, DeleteOutlined, CalendarOutlined, SyncOutlined } from '@ant-design/icons-vue'
import dayjs from 'dayjs'
import {
  balancedEven, balancedWave, balancedWeekday, compensateAfterEdit, rebalance,
  SHAPE_LABELS, COMPENSATE_LABELS,
} from '../../../composables/useDailyAllocation'

const props = defineProps({
  open: { type: Boolean, default: false },
  totalCount: { type: Number, default: 0 },
  loading: { type: Boolean, default: false },
})
const emit = defineEmits(['update:open', 'apply'])

// rows: { date: dayjs, count: number }
const rows = ref([])
const shape = ref('even') // 每日条数形状: even | wave | weekday
const comp = ref('proportional') // 差额补偿: proportional | tail | head
const mode = ref('spread') // 天内时间排列: spread | random
const activeHour = ref('9-22')
const customDays = ref(7)

const sumCount = computed(() => rows.value.reduce((s, r) => s + (Number(r.count) || 0), 0))
const balanced = computed(() => props.totalCount > 0 && sumCount.value === props.totalCount)

/** 按当前形状策略生成 days 天（截止昨天）的精确合计方案。 */
function generate(n) {
  const total = props.totalCount || 0
  const days = Math.max(1, Math.floor(n))
  const today = dayjs()
  const dates = Array.from({ length: days }, (_, i) => today.subtract(days - i, 'day'))
  let counts
  if (shape.value === 'wave') counts = balancedWave(total, days)
  else if (shape.value === 'weekday') counts = balancedWeekday(total, dates)
  else counts = balancedEven(total, days)
  rows.value = dates.map((date, i) => ({ date, count: counts[i] }))
}

watch(
  () => props.open,
  (v) => {
    if (!v) return
    shape.value = 'even'
    comp.value = 'proportional'
    mode.value = 'spread'
    activeHour.value = '9-22'
    customDays.value = 7
    generate(7)
  },
)

function regen() {
  generate(rows.value.length || customDays.value || 7)
}
function fillRecent(n) {
  customDays.value = Math.max(1, Math.floor(n))
  generate(n)
}

function addRow() {
  const last = rows.value[rows.value.length - 1]
  const base = last ? dayjs(last.date) : dayjs().subtract(1, 'day')
  rows.value.push({ date: base.add(1, 'day'), count: 0 })
}

/** 删除某天：其条数按补偿策略摊到其余天，保持合计。 */
function removeRow(i) {
  const removed = Number(rows.value[i]?.count) || 0
  rows.value.splice(i, 1)
  if (removed > 0 && rows.value.length) {
    const counts = rebalance(
      rows.value.map((r) => Number(r.count) || 0),
      props.totalCount,
      comp.value,
      null,
    )
    rows.value = rows.value.map((r, k) => ({ ...r, count: counts[k] }))
  }
}

/** 手动编辑某天：其余天按补偿策略实时补偿，合计恒等于总数。 */
function onEditCount(i, val) {
  const counts = rows.value.map((r) => Number(r.count) || 0)
  const next = compensateAfterEdit(counts, props.totalCount, i, Number(val) || 0, comp.value)
  rows.value = rows.value.map((r, k) => ({ ...r, count: next[k] }))
}

const hourPresets = [
  { label: '工作 9-18', value: '9-18' },
  { label: '白天 8-22', value: '8-22' },
  { label: '晚间 19-24', value: '19-24' },
]

function close() {
  emit('update:open', false)
}

function doApply() {
  if (!props.totalCount || !rows.value.length) return
  if (rows.value.some((r) => !r.date)) return
  // 兜底再平衡（理论上 UI 已保持平衡）
  const counts = rebalance(
    rows.value.map((r) => Number(r.count) || 0),
    props.totalCount,
    comp.value,
    null,
  )
  const arr = rows.value
    .map((r, k) => ({ date: dayjs(r.date), count: counts[k] }))
    .filter((r) => r.count > 0)
    .sort((a, b) => (a.date.isBefore(b.date) ? -1 : 1))
  const allocs = arr.map((r) => ({ date: r.date.format('YYYY-MM-DD'), count: r.count }))
  emit('apply', { allocs, mode: mode.value, activeHour: activeHour.value || '' })
}

function pct(c) {
  if (!props.totalCount) return ''
  return `${Math.round((c / props.totalCount) * 100)}%`
}
</script>

<template>
  <a-modal
    :open="open"
    @update:open="emit('update:open', $event)"
    title="逐日分配提交"
    width="680px"
    :footer="null"
    :mask-closable="false"
    destroy-on-close
  >
    <div class="dd-head">
      <span class="dd-stat">共 <b>{{ totalCount }}</b> 条提交待分配</span>
      <span class="dd-badge" :class="{ ok: balanced }">
        <CalendarOutlined /> {{ balanced ? `已平衡 ${sumCount} / ${totalCount}` : '未平衡' }}
      </span>
    </div>

    <!-- 生成策略 -->
    <div class="dd-global">
      <div class="dd-gitem">
        <span class="lbl">每日条数</span>
        <a-radio-group v-model:value="shape" size="small" @change="regen">
          <a-radio-button v-for="(l, k) in SHAPE_LABELS" :key="k" :value="k">{{ l }}</a-radio-button>
        </a-radio-group>
        <span class="dd-tip">均匀=每天相近 · 波动=自然起伏 · 工作日优先=周末少量</span>
      </div>
      <div class="dd-gitem">
        <span class="lbl">范围</span>
        <button class="chip" @click="fillRecent(7)">最近 7 天</button>
        <button class="chip" @click="fillRecent(14)">最近 14 天</button>
        <button class="chip" @click="fillRecent(30)">最近 30 天</button>
        <span class="dd-custom">
          最近 <a-input-number v-model:value="customDays" :min="1" :max="365" size="small" style="width:64px" @pressEnter="fillRecent(customDays)" /> 天
          <a-button size="small" type="dashed" @click="fillRecent(customDays)">生成</a-button>
        </span>
        <a-button size="small" @click="regen" title="按当前形状策略重新生成">
          <SyncOutlined /> 重新生成
        </a-button>
      </div>
      <div class="dd-gitem">
        <span class="lbl">时间排列</span>
        <a-radio-group v-model:value="mode" size="small">
          <a-radio-button value="spread">顺序</a-radio-button>
          <a-radio-button value="random">随机</a-radio-button>
        </a-radio-group>
        <span class="lbl" style="width:auto;margin-left:12px">活跃时段</span>
        <a-input v-model:value="activeHour" placeholder="9-22" size="small" style="width:72px" />
        <button
          v-for="p in hourPresets" :key="p.value"
          class="chip" :class="{ on: activeHour === p.value }"
          @click="activeHour = p.value"
        >{{ p.label }}</button>
      </div>
    </div>

    <!-- 分配表 -->
    <div class="dd-table">
      <div class="dd-thead">
        <span class="col-date">日期</span>
        <span class="col-week">星期</span>
        <span class="col-count">提交数</span>
        <span class="col-pct">占比</span>
        <span class="col-op">操作</span>
      </div>
      <div class="dd-tbody">
        <div v-for="(r, i) in rows" :key="i" class="dd-row">
          <a-date-picker v-model:value="r.date" format="YYYY-MM-DD" size="small" style="width:140px" :allow-clear="false" />
          <span class="dd-week" :class="{ wk: [0, 6].includes(r.date?.day?.() ?? -1) }">
            {{ ['日', '一', '二', '三', '四', '五', '六'][r.date?.day?.() ?? 0] }}
          </span>
          <a-input-number
            :value="r.count" :min="0" size="small" style="width:86px"
            @change="(v) => onEditCount(i, v)"
          />
          <span class="dd-pct">{{ pct(r.count) }}</span>
          <a-button size="small" type="text" danger @click="removeRow(i)" title="删除该天（条数自动摊到其余天）">
            <DeleteOutlined />
          </a-button>
        </div>
        <div v-if="!rows.length" class="dd-empty">未添加任何天数，点下方添加或选择范围生成</div>
      </div>
      <div class="dd-tfoot">
        <a-button size="small" type="dashed" @click="addRow">
          <PlusOutlined /> 增加一天
        </a-button>
        <span class="dd-note">改任意天条数，其余天按补偿策略自动找平</span>
      </div>
    </div>

    <!-- 合计状态条 -->
    <div class="dd-sum" :class="{ ok: balanced }">
      <CalendarOutlined />
      <span>合计 <b>{{ sumCount }}</b> / {{ totalCount || '—' }}</span>
      <span class="dd-hint">· 差额补偿：
        <button
          v-for="(l, k) in COMPENSATE_LABELS" :key="k"
          class="chip mini" :class="{ on: comp === k }"
          @click="comp = k"
        >{{ l }}</button>
      </span>
    </div>

    <div class="dd-foot">
      <span class="dd-note">分配仅改写日期（作者+提交者），其他字段不变</span>
      <div class="dd-spacer"></div>
      <a-button @click="close">取消</a-button>
      <a-button type="primary" :loading="loading" :disabled="!totalCount || !rows.length" @click="doApply">
        应用分配
      </a-button>
    </div>
  </a-modal>
</template>

<style scoped>
.dd-head { display: flex; align-items: center; justify-content: space-between; margin-bottom: var(--spacing-sm); font-size: 13px; color: var(--color-text-secondary); }
.dd-head b { color: var(--color-text); }
.dd-badge { display: inline-flex; align-items: center; gap: 4px; padding: 2px 10px; border-radius: 10px; font-size: 12px; border: 1px solid var(--color-border-light); color: var(--color-text-tertiary); }
.dd-badge.ok { color: var(--color-success, #52c41a); border-color: var(--color-success, #52c41a); }

.dd-global { display: flex; flex-direction: column; gap: var(--spacing-xs); padding: var(--spacing-sm); background: var(--color-surface); border: 1px solid var(--color-border-light); border-radius: var(--radius-md); margin-bottom: var(--spacing-sm); }
.dd-gitem { display: flex; align-items: center; gap: var(--spacing-xs); flex-wrap: wrap; }
.lbl { font-size: 12px; color: var(--color-text-secondary); width: 56px; flex-shrink: 0; }
.dd-tip { font-size: 11px; color: var(--color-text-tertiary); }
.dd-custom { display: inline-flex; align-items: center; gap: 4px; font-size: 12px; color: var(--color-text-secondary); }

.dd-table { border: 1px solid var(--color-border-light); border-radius: var(--radius-md); overflow: hidden; }
.dd-thead { display: grid; grid-template-columns: 150px 40px 100px 48px 48px; gap: 6px; padding: 6px 10px; background: var(--color-surface); font-size: 12px; color: var(--color-text-secondary); border-bottom: 1px solid var(--color-border-light); }
.dd-tbody { max-height: 260px; overflow-y: auto; }
.dd-row { display: grid; grid-template-columns: 150px 40px 100px 48px 48px; gap: 6px; padding: 6px 10px; align-items: center; border-bottom: 1px solid var(--color-border-light); }
.dd-row:last-child { border-bottom: none; }
.dd-week { font-size: 12px; color: var(--color-text-tertiary); text-align: center; }
.dd-week.wk { color: var(--color-warning, #faad14); }
.dd-pct { font-size: 12px; color: var(--color-text-tertiary); text-align: right; }
.dd-empty { padding: 16px; text-align: center; font-size: 12px; color: var(--color-text-tertiary); }

.dd-tfoot { display: flex; align-items: center; gap: 6px; padding: 8px 10px; flex-wrap: wrap; background: var(--color-surface); }
.dd-note { font-size: 11px; color: var(--color-text-tertiary); }

.chip { font-size: 11px; padding: 2px 8px; cursor: pointer; border: 1px solid var(--color-border); border-radius: 10px; background: var(--color-background); color: var(--color-text-secondary); transition: all var(--transition-fast); }
.chip:hover { color: var(--color-primary); border-color: var(--color-primary); }
.chip.on { background: var(--color-primary); border-color: var(--color-primary); color: #fff; }
.chip.mini { padding: 1px 6px; }

.dd-sum { display: flex; align-items: center; gap: 6px; margin-top: var(--spacing-sm); padding: 8px 12px; border-radius: var(--radius-sm); font-size: 13px; background: var(--color-surface); border: 1px solid var(--color-border-light); flex-wrap: wrap; }
.dd-sum b { font-size: 14px; }
.dd-sum.ok { color: var(--color-success, #52c41a); border-color: var(--color-success, #52c41a); }
.dd-hint { color: var(--color-text-tertiary); font-size: 12px; display: inline-flex; align-items: center; gap: 4px; }
.dd-sum.ok .dd-hint { color: var(--color-success, #52c41a); }

.dd-foot { display: flex; align-items: center; gap: var(--spacing-sm); margin-top: var(--spacing-md); padding-top: var(--spacing-sm); border-top: 1px solid var(--color-border-light); }
.dd-spacer { flex: 1; }
</style>
