<script setup>
import { ref, computed, watch } from 'vue'
import { PlusOutlined, DeleteOutlined, CalendarOutlined } from '@ant-design/icons-vue'
import { Modal } from 'ant-design-vue'
import dayjs from 'dayjs'

const props = defineProps({
  open: { type: Boolean, default: false },
  totalCount: { type: Number, default: 0 },
  loading: { type: Boolean, default: false },
})
const emit = defineEmits(['update:open', 'apply'])

// rows: 每行 { date: dayjs, count: number }
const rows = ref([])
const mode = ref('spread') // spread | random
const activeHour = ref('9-22')

// 打开时初始化为最近 7 天（昨天往前），每行默认均分
watch(
  () => props.open,
  (v) => {
    if (!v) return
    const total = props.totalCount || 0
    const days = 7
    const per = Math.max(1, Math.ceil(total / days))
    const today = dayjs()
    const init = []
    for (let i = days - 1; i >= 0; i--) {
      init.push({ date: today.subtract(i + 1, 'day'), count: per })
    }
    rows.value = init
    mode.value = 'spread'
    activeHour.value = '9-22'
  },
)

const sumCount = computed(() => rows.value.reduce((s, r) => s + (Number(r.count) || 0), 0))
const diff = computed(() => sumCount.value - props.totalCount)
// diff > 0 超出; diff < 0 不足; diff == 0 平衡
const diffText = computed(() => {
  if (!props.totalCount) return '请先加载提交'
  if (diff.value === 0) return `已平衡：${sumCount.value} / ${props.totalCount}`
  if (diff.value > 0) return `超出 ${diff.value} 条，将从最后一天起逐日扣减`
  return `不足 ${-diff.value} 条，将均匀补到现有天数`
})

function addRow() {
  const last = rows.value[rows.value.length - 1]
  const base = last ? dayjs(last.date) : dayjs().subtract(1, 'day')
  rows.value.push({ date: base.add(1, 'day'), count: 1 })
}
function removeRow(i) {
  rows.value.splice(i, 1)
}

// 快捷预设：覆盖式填充最近 N 天
function fillRecent(n) {
  const total = props.totalCount || 0
  const per = Math.max(1, Math.ceil(total / n))
  const today = dayjs()
  const init = []
  for (let i = n - 1; i >= 0; i--) {
    init.push({ date: today.subtract(i + 1, 'day'), count: per })
  }
  rows.value = init
}

const hourPresets = [
  { label: '工作 9-18', value: '9-18' },
  { label: '白天 8-22', value: '8-22' },
  { label: '晚间 19-24', value: '19-24' },
]

// 智能修正：使 sumCount === totalCount
function reconcile() {
  const total = props.totalCount
  if (!total) return rows.value.map((r) => ({ date: r.date, count: r.count }))
  // 用副本计算
  const counts = rows.value.map((r) => Number(r.count) || 0)
  let cur = counts.reduce((s, c) => s + c, 0)
  let action = '' // 'trim' | 'spread'
  if (cur > total) {
    action = 'trim'
    // 从最后一天起往前扣减
    for (let i = counts.length - 1; i >= 0 && cur > total; i--) {
      const cut = Math.min(counts[i], cur - total)
      counts[i] -= cut
      cur -= cut
    }
  } else if (cur < total) {
    action = 'spread'
    // 按现有各天 count 比例摊分剩余
    const remain = total - cur
    const base = counts.reduce((s, c) => s + c, 0) || counts.length
    let allocated = 0
    for (let i = 0; i < counts.length; i++) {
      if (i === counts.length - 1) {
        counts[i] += remain - allocated
      } else {
        const add = Math.round((remain * (counts[i] || 0)) / base)
        counts[i] += add
        allocated += add
      }
    }
  }
  return rows.value
    .map((r, i) => ({ date: dayjs(r.date), count: counts[i] }))
    .filter((r) => r.count > 0)
}

function close() {
  emit('update:open', false)
}

function doApply() {
  if (!props.totalCount) return
  // 校验日期完整
  if (rows.value.some((r) => !r.date)) return
  if (rows.value.length === 0) return

  // 若已经平衡，直接 emit
  if (diff.value === 0) {
    emitFinal(rows.value.map((r) => ({ date: dayjs(r.date), count: Number(r.count) || 0 })))
    return
  }

  // 否则先弹框提示修正方案
  const fixed = reconcile()
  const fixedSum = fixed.reduce((s, r) => s + r.count, 0)
  const msg =
    diff.value > 0
      ? `当前合计 ${sumCount.value} 条，超出 ${diff.value} 条。将从最后一天起逐日扣减。`
      : `当前合计 ${sumCount.value} 条，不足 ${-diff.value} 条。将按比例均匀补到现有 ${rows.value.length} 天。`
  Modal.confirm({
    title: '智能修正提交分配',
    content: `${msg}\n修正后合计 ${fixedSum} / ${props.totalCount} 条，共 ${fixed.length} 天。`,
    okText: '确认并应用',
    cancelText: '我再改改',
    onOk() {
      emitFinal(fixed)
    },
  })
}

function emitFinal(arr) {
  // 按日期升序
  arr.sort((a, b) => (dayjs(a.date).isBefore(dayjs(b.date)) ? -1 : 1))
  const allocs = arr.map((r) => ({ date: dayjs(r.date).format('YYYY-MM-DD'), count: r.count }))
  emit('apply', { allocs, mode: mode.value, activeHour: activeHour.value || '' })
}
</script>

<template>
  <a-modal
    :open="open"
    @update:open="emit('update:open', $event)"
    title="逐日分配提交"
    width="640px"
    :footer="null"
    :mask-closable="false"
    destroy-on-close
  >
    <div class="dd-head">
      <span class="dd-stat">共 <b>{{ totalCount }}</b> 条提交待分配</span>
    </div>

    <!-- 全局参数 -->
    <div class="dd-global">
      <div class="dd-gitem">
        <span class="lbl">分布</span>
        <a-radio-group v-model:value="mode" size="small">
          <a-radio-button value="spread">均匀</a-radio-button>
          <a-radio-button value="random">随机</a-radio-button>
        </a-radio-group>
      </div>
      <div class="dd-gitem">
        <span class="lbl">活跃时段</span>
        <a-input v-model:value="activeHour" placeholder="9-22" size="small" style="width:72px" />
        <button v-for="p in hourPresets" :key="p.value" class="chip" :class="{ on: activeHour === p.value }" @click="activeHour = p.value">{{ p.label }}</button>
      </div>
    </div>

    <!-- 分配表 -->
    <div class="dd-table">
      <div class="dd-thead">
        <span class="col-date">日期</span>
        <span class="col-count">提交数</span>
        <span class="col-op">操作</span>
      </div>
      <div class="dd-tbody">
        <div v-for="(r, i) in rows" :key="i" class="dd-row">
          <a-date-picker
            v-model:value="r.date"
            format="YYYY-MM-DD"
            size="small"
            style="width:150px"
            :allow-clear="false"
          />
          <a-input-number v-model:value="r.count" :min="0" size="small" style="width:90px" />
          <a-button size="small" type="text" danger @click="removeRow(i)" title="删除该天">
            <DeleteOutlined />
          </a-button>
        </div>
        <div v-if="!rows.length" class="dd-empty">未添加任何天数，点下方添加</div>
      </div>

      <div class="dd-tfoot">
        <a-button size="small" type="dashed" @click="addRow">
          <PlusOutlined /> 增加一天
        </a-button>
        <span class="preset-label">快捷填充:</span>
        <button class="chip" @click="fillRecent(7)">最近 7 天</button>
        <button class="chip" @click="fillRecent(14)">最近 14 天</button>
        <button class="chip" @click="fillRecent(30)">最近 30 天</button>
      </div>
    </div>

    <!-- 合计状态条 -->
    <div class="dd-sum" :class="{ ok: diff === 0 && totalCount > 0, over: diff > 0, under: diff < 0 }">
      <CalendarOutlined />
      <span>合计 <b>{{ sumCount }}</b> / {{ totalCount }}</span>
      <span class="dd-hint" v-if="totalCount">· {{ diffText }}</span>
    </div>

    <!-- 底部操作 -->
    <div class="dd-foot">
      <span class="dd-note">分配仅改写日期（作者+提交者），其他字段不变</span>
      <div class="dd-spacer"></div>
      <a-button @click="close">取消</a-button>
      <a-button type="primary" :loading="loading" :disabled="!totalCount || !rows.length" @click="doApply">
        智能修正并分配
      </a-button>
    </div>
  </a-modal>
</template>

<style scoped>
.dd-head { margin-bottom: var(--spacing-sm); font-size: 13px; color: var(--color-text-secondary); }
.dd-head b { color: var(--color-text); }

.dd-global { display: flex; flex-direction: column; gap: var(--spacing-xs); padding: var(--spacing-sm); background: var(--color-surface); border: 1px solid var(--color-border-light); border-radius: var(--radius-md); margin-bottom: var(--spacing-sm); }
.dd-gitem { display: flex; align-items: center; gap: var(--spacing-xs); flex-wrap: wrap; }
.lbl { font-size: 12px; color: var(--color-text-secondary); width: 56px; flex-shrink: 0; }

.dd-table { border: 1px solid var(--color-border-light); border-radius: var(--radius-md); overflow: hidden; }
.dd-thead { display: grid; grid-template-columns: 1fr 110px 48px; gap: 8px; padding: 6px 10px; background: var(--color-surface); font-size: 12px; color: var(--color-text-secondary); border-bottom: 1px solid var(--color-border-light); }
.dd-tbody { max-height: 260px; overflow-y: auto; }
.dd-row { display: grid; grid-template-columns: 1fr 110px 48px; gap: 8px; padding: 6px 10px; align-items: center; border-bottom: 1px solid var(--color-border-light); }
.dd-row:last-child { border-bottom: none; }
.dd-empty { padding: 16px; text-align: center; font-size: 12px; color: var(--color-text-tertiary); }

.dd-tfoot { display: flex; align-items: center; gap: 6px; padding: 8px 10px; flex-wrap: wrap; background: var(--color-surface); }
.preset-label { font-size: 11px; color: var(--color-text-tertiary); margin-left: var(--spacing-xs); }

.chip { font-size: 11px; padding: 2px 8px; cursor: pointer; border: 1px solid var(--color-border); border-radius: 10px; background: var(--color-background); color: var(--color-text-secondary); transition: all var(--transition-fast); }
.chip:hover { color: var(--color-primary); border-color: var(--color-primary); }
.chip.on { background: var(--color-primary); border-color: var(--color-primary); color: #fff; }

.dd-sum { display: flex; align-items: center; gap: 6px; margin-top: var(--spacing-sm); padding: 8px 12px; border-radius: var(--radius-sm); font-size: 13px; background: var(--color-surface); border: 1px solid var(--color-border-light); }
.dd-sum b { font-size: 14px; }
.dd-sum.ok { color: var(--color-success, #52c41a); border-color: var(--color-success, #52c41a); }
.dd-sum.over { color: var(--color-error, #ff4d4f); border-color: var(--color-error, #ff4d4f); }
.dd-sum.under { color: var(--color-warning, #faad14); border-color: var(--color-warning, #faad14); }
.dd-hint { color: var(--color-text-tertiary); font-size: 12px; }
.dd-sum.ok .dd-hint { color: var(--color-success, #52c41a); }

.dd-foot { display: flex; align-items: center; gap: var(--spacing-sm); margin-top: var(--spacing-md); padding-top: var(--spacing-sm); border-top: 1px solid var(--color-border-light); }
.dd-note { font-size: 11px; color: var(--color-text-tertiary); }
.dd-spacer { flex: 1; }
</style>
