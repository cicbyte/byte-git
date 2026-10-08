<script setup>
import { ref, computed, watch } from 'vue'
import { PlusOutlined, CloseOutlined } from '@ant-design/icons-vue'
import DateRemapPanel from './DateRemapPanel.vue'

const props = defineProps({
  open: Boolean,
  loading: Boolean,
  repoArg: { type: String, default: '' },
})
const emit = defineEmits(['update:open', 'export'])

const app = window.go?.main?.ByteGitApp
const step = ref(0)

// commit-count preview
const counts = ref({ total: 0, unpushed: 0 })
const countsLoading = ref(false)
async function loadCounts() {
  if (!app?.CommitCount) return
  countsLoading.value = true
  try {
    counts.value = await app.CommitCount(props.repoArg)
  } catch {
    // ignore — preview is best-effort
  } finally {
    countsLoading.value = false
  }
}
watch(() => props.open, (v) => { if (v) loadCounts() }, { immediate: true })

// how many commits the current selection covers
const previewCount = computed(() => {
  if (mode.value === 'all') return counts.value.total
  if (mode.value === 'unpushed') return counts.value.unpushed
  // last N → min(N, total)
  return Math.min(lastN.value || 0, counts.value.total)
})

// step 1: range
const mode = ref('last')
const lastN = ref(20)

// step 2: rules
const ruleField = ref('author')
const ruleCond = ref('')
const ruleVal = ref('')
const rules = ref([])

const fields = [
  { value: 'author', label: '作者名' },
  { value: 'email', label: '邮箱' },
  { value: 'committer-name', label: '提交者名' },
  { value: 'committer-email', label: '提交者邮箱' },
  { value: 'date-shift', label: '日期偏移' },
]
const isDateShift = computed(() => ruleField.value === 'date-shift')

function buildRule() {
  if (isDateShift.value) return `date-shift:${ruleVal.value}`
  if (ruleCond.value) return `${ruleField.value}:${ruleCond.value}=${ruleVal.value}`
  return `${ruleField.value}:=${ruleVal.value}`
}
function addRule() {
  if (!ruleVal.value) return
  rules.value.push(buildRule())
  ruleVal.value = ''
  ruleCond.value = ''
}
function removeRule(i) {
  rules.value.splice(i, 1)
}

// step 3: remap (delegated to DateRemapPanel via v-model)
const remap = ref({
  enabled: false,
  mode: 'range',     // range | daily
  dateRange: '',     // "start,end" for range mode
  dailyLimit: 0,
  activeHour: '',
  dateMode: 'spread',
  allocs: [],        // [{date, count}] for daily mode
})

// navigation
const steps = [
  { title: '范围', desc: '选择提交' },
  { title: '规则', desc: '批量改写（可选）' },
  { title: '日期', desc: '重映射（可选）' },
  { title: '确认', desc: '执行导出' },
]

function next() { if (step.value < steps.length - 1) step.value++ }
function prev() { if (step.value > 0) step.value-- }

function reset() {
  step.value = 0
  mode.value = 'last'
  lastN.value = 20
  rules.value = []
  remap.value = {
    enabled: false, mode: 'range', dateRange: '',
    dailyLimit: 0, activeHour: '', dateMode: 'spread', allocs: [],
  }
}

function close() {
  emit('update:open', false)
}

function doExport() {
  let remapOut = null
  if (remap.value.enabled) {
    remapOut = {
      dateRange: remap.value.mode === 'range' ? remap.value.dateRange : '',
      mode: remap.value.dateMode,
      dailyLimit: remap.value.mode === 'range' ? (remap.value.dailyLimit || 0) : 0,
      activeHour: remap.value.activeHour || '',
      allocs: remap.value.mode === 'daily' ? remap.value.allocs : [],
    }
  }
  const opts = {
    mode: mode.value,
    n: mode.value === 'last' ? lastN.value : 0,
    rules: [...rules.value],
    remap: remapOut,
  }
  emit('export', opts)
}

const rangeLabel = computed(() => {
  if (mode.value === 'all') return '全部历史'
  if (mode.value === 'unpushed') return '未推送'
  return `最近 ${lastN.value} 条`
})

// 确认页摘要：日期重映射
const remapSummary = computed(() => {
  if (!remap.value.enabled) return '不启用'
  if (remap.value.mode === 'range') {
    if (!remap.value.dateRange) return '未配置'
    const parts = [remap.value.dateRange]
    parts.push(remap.value.dateMode === 'spread' ? '均匀' : '随机')
    if (remap.value.dailyLimit) parts.push(`每日${remap.value.dailyLimit}`)
    if (remap.value.activeHour) parts.push(remap.value.activeHour)
    return parts.join(' ')
  }
  // daily
  const parts = [`逐日分配 ${remap.value.allocs.length} 天，合计 ${remap.value.allocs.reduce((s, a) => s + a.count, 0)} / ${previewCount.value} 条`]
  parts.push(remap.value.dateMode === 'spread' ? '均匀' : '随机')
  if (remap.value.activeHour) parts.push(remap.value.activeHour)
  return parts.join(' ')
})
</script>

<template>
  <a-modal
    :open="open"
    @update:open="emit('update:open', $event)"
    title="导出向导"
    width="760px"
    :footer="null"
    :mask-closable="false"
    destroy-on-close
    @cancel="reset"
  >
    <a-steps :current="step" size="small" class="wiz-steps">
      <a-step v-for="s in steps" :key="s.title" :title="s.title" :description="s.desc" />
    </a-steps>

    <div class="wiz-body">
      <!-- Step 0: range -->
      <div v-show="step === 0" class="step-pane">
        <div class="count-summary">
          <span class="cs-stat">仓库共 <b>{{ counts.total }}</b> 条提交</span>
          <span class="cs-sep">·</span>
          <span class="cs-stat" :class="{ warn: counts.unpushed > 0 }">未推送 <b>{{ counts.unpushed }}</b> 条</span>
        </div>
        <a-radio-group v-model:value="mode" class="col-radios">
          <a-radio value="all">全部历史 <span class="opt-count">{{ counts.total }} 条</span></a-radio>
          <a-radio value="last">最近 N 条</a-radio>
          <a-radio value="unpushed">未推送 <span class="opt-count">{{ counts.unpushed }} 条</span></a-radio>
        </a-radio-group>
        <div v-if="mode === 'last'" class="row">
          <span class="lbl">N</span>
          <a-input-number v-model:value="lastN" :min="1" :max="counts.total || 9999" />
        </div>
        <div class="tip">当前选择将导出 <b class="preview-num">{{ previewCount }}</b> 条提交</div>
      </div>

      <!-- Step 1: rules -->
      <div v-show="step === 1" class="step-pane">
        <div class="rule-add">
          <a-select v-model:value="ruleField" size="small" style="width:120px" :options="fields" />
          <a-input v-if="!isDateShift" v-model:value="ruleCond" placeholder="旧值(空=全部)" size="small" style="width:130px" />
          <span v-if="!isDateShift" class="arrow">→</span>
          <a-input v-model:value="ruleVal" :placeholder="isDateShift ? '+2h30m / -1d' : '新值'" size="small" style="width:140px" @press-enter="addRule" />
          <a-button size="small" type="primary" @click="addRule"><PlusOutlined /></a-button>
        </div>
        <div class="rule-list" v-if="rules.length">
          <div class="rl-hint">已添加规则（导出时批量应用）:</div>
          <div v-for="(r, i) in rules" :key="i" class="rule-tag">
            <code>{{ r }}</code>
            <a-button size="small" type="text" @click="removeRule(i)"><CloseOutlined /></a-button>
          </div>
        </div>
        <div v-else class="tip">未添加规则，可跳过此步。导出后仍可手动编辑单条。</div>
      </div>

      <!-- Step 2: remap (delegated to DateRemapPanel) -->
      <div v-show="step === 2" class="step-pane">
        <DateRemapPanel v-model="remap" :preview-count="previewCount" />
      </div>

      <!-- Step 3: confirm -->
      <div v-show="step === 3" class="step-pane">
        <a-descriptions :column="1" size="small" bordered>
          <a-descriptions-item label="范围">{{ rangeLabel }}</a-descriptions-item>
          <a-descriptions-item label="规则">
            <span v-if="rules.length">{{ rules.length }} 条: <code v-for="r in rules" :key="r" class="sum-rule">{{ r }} </code></span>
            <span v-else class="muted">无</span>
          </a-descriptions-item>
          <a-descriptions-item label="日期重映射">
            <span :class="{ muted: !remap.enabled }">{{ remapSummary }}</span>
          </a-descriptions-item>
        </a-descriptions>
        <div class="tip">确认后将拉取提交并应用上述配置。</div>
      </div>
    </div>

    <!-- nav -->
    <div class="wiz-foot">
      <a-button v-if="step > 0" @click="prev">上一步</a-button>
      <span class="foot-sep"></span>
      <a-button @click="close">取消</a-button>
      <a-button v-if="step < steps.length - 1" type="primary" @click="next">下一步</a-button>
      <a-button v-else type="primary" :loading="loading" @click="doExport">执行导出</a-button>
    </div>
  </a-modal>
</template>

<style scoped>
.wiz-steps { margin-bottom: var(--spacing-lg); padding: 0 var(--spacing-sm); }
.wiz-steps :deep(.ant-steps-item-title) { white-space: nowrap; }
.wiz-steps :deep(.ant-steps-item-description) { white-space: nowrap; overflow: hidden; text-overflow: ellipsis; max-width: 140px; }
.step-pane { min-height: 320px; padding: var(--spacing-sm) 0; display: flex; flex-direction: column; align-items: center; }
.step-pane > * { width: 100%; max-width: 480px; }
.step-pane > :deep(.remap-form) { max-width: 480px; }
.rule-add { max-width: none; }
.col-radios { display: flex; flex-direction: column; gap: var(--spacing-sm); margin-bottom: var(--spacing-md); }
.opt-count { font-size: 11px; color: var(--color-text-tertiary); margin-left: 4px; }
.count-summary { display: flex; align-items: center; justify-content: center; gap: var(--spacing-sm); margin-bottom: var(--spacing-md); font-size: 13px; color: var(--color-text-secondary); }
.cs-stat.warn { color: var(--color-warning); }
.cs-sep { color: var(--color-text-tertiary); }
.preview-num { color: var(--color-primary); font-size: 14px; }
.row { display: flex; align-items: center; gap: var(--spacing-sm); margin-bottom: var(--spacing-sm); }
.lbl { font-size: 13px; color: var(--color-text-secondary); }
.arrow { color: var(--color-text-tertiary); }
.tip { font-size: 12px; color: var(--color-text-tertiary); margin-top: var(--spacing-sm); text-align: center; }
.muted { color: var(--color-text-tertiary); }

.rule-add { display: flex; align-items: center; gap: 6px; flex-wrap: nowrap; margin-bottom: var(--spacing-md); }
.rule-list { display: flex; flex-direction: column; gap: 4px; }
.rl-hint { font-size: 12px; color: var(--color-text-secondary); margin-bottom: 4px; }
.rule-tag { display: flex; align-items: center; justify-content: space-between; padding: 4px 8px; background: var(--color-surface); border: 1px solid var(--color-border); border-radius: var(--radius-sm); }
.rule-tag code { font-size: 12px; color: var(--color-primary); font-family: 'Cascadia Code', monospace; }

.sum-rule { font-size: 11px; color: var(--color-primary); margin-right: 4px; }

.wiz-foot { display: flex; align-items: center; gap: var(--spacing-sm); margin-top: var(--spacing-lg); padding-top: var(--spacing-md); border-top: 1px solid var(--color-border); }
.foot-sep { flex: 1; }
</style>
