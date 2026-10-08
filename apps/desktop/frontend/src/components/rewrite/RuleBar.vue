<script setup>
import { ref } from 'vue'
import { PlusOutlined } from '@ant-design/icons-vue'

const emit = defineEmits(['apply'])

// field presets
const fields = [
  { value: 'author', label: '作者名' },
  { value: 'email', label: '邮箱' },
  { value: 'committer-name', label: '提交者名' },
  { value: 'committer-email', label: '提交者邮箱' },
  { value: 'date-shift', label: '日期偏移' },
]

const field = ref('author')
const condition = ref('') // 旧值（条件匹配），空=无条件
const newVal = ref('')

function buildRule() {
  // date-shift 不支持条件，直接 date-shift:新值
  if (field.value === 'date-shift') {
    return `date-shift:${newVal.value}`
  }
  // 有条件: field:旧=新 ; 无条件: field:=新
  if (condition.value) {
    return `${field.value}:${condition.value}=${newVal.value}`
  }
  return `${field.value}:=${newVal.value}`
}

function add() {
  if (!newVal.value && field.value !== 'date-shift') {
    return
  }
  if (field.value === 'date-shift' && !newVal.value) return
  emit('apply', buildRule())
  // reset value but keep field for batch editing
  newVal.value = ''
  condition.value = ''
}

const isDateShift = () => field.value === 'date-shift'
</script>

<template>
  <div class="rule-bar">
    <span class="rb-label">规则</span>
    <a-select v-model:value="field" size="small" style="width:120px" :options="fields" />
    <template v-if="!isDateShift()">
      <a-input v-model:value="condition" placeholder="旧值(可空=全部)" size="small" style="width:140px" />
      <span class="rb-arrow">→</span>
    </template>
    <a-input
      v-model:value="newVal"
      :placeholder="isDateShift() ? '+2h30m / -1d' : '新值'"
      size="small"
      style="width:160px"
      @press-enter="add"
    />
    <a-button type="primary" size="small" @click="add">
      <template #icon><PlusOutlined /></template>应用
    </a-button>
  </div>
</template>

<style scoped>
.rule-bar { display: flex; align-items: center; gap: var(--spacing-xs); flex-wrap: wrap; }
.rb-label { font-size: 12px; color: var(--color-text-secondary); margin-right: 4px; }
.rb-arrow { color: var(--color-text-tertiary); font-size: 12px; }
</style>
