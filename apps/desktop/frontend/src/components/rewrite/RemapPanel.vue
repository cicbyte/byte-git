<script setup>
import { ref } from 'vue'
import { CalendarOutlined } from '@ant-design/icons-vue'

const emit = defineEmits(['apply'])

const expanded = ref(false)
const dateRange = ref('')
const mode = ref('spread')
const dailyLimit = ref(0)
const activeHour = ref('')

function apply() {
  if (!dateRange.value.includes(',')) {
    return
  }
  emit('apply', {
    dateRange: dateRange.value,
    mode: mode.value,
    dailyLimit: dailyLimit.value || 0,
    activeHour: activeHour.value || '',
  })
}
</script>

<template>
  <div class="remap">
    <button class="remap-toggle" @click="expanded = !expanded">
      <CalendarOutlined />
      <span>日期重映射</span>
      <span class="caret">{{ expanded ? '▾' : '▸' }}</span>
    </button>

    <div v-if="expanded" class="remap-body">
      <div class="rm-row">
        <span class="rm-label">日期范围</span>
        <a-input v-model:value="dateRange" placeholder="2024-01-01,2024-06-30" size="small" style="width:220px" />
      </div>
      <div class="rm-row">
        <span class="rm-label">分布</span>
        <a-radio-group v-model:value="mode" size="small">
          <a-radio-button value="spread">均匀</a-radio-button>
          <a-radio-button value="random">随机</a-radio-button>
        </a-radio-group>
      </div>
      <div class="rm-row">
        <span class="rm-label">每日上限</span>
        <a-input-number v-model:value="dailyLimit" :min="0" size="small" style="width:90px" />
        <span class="rm-hint">0=不限</span>
      </div>
      <div class="rm-row">
        <span class="rm-label">活跃时段</span>
        <a-input v-model:value="activeHour" placeholder="9-22" size="small" style="width:90px" />
      </div>
      <a-button type="primary" size="small" @click="apply">应用重映射</a-button>
    </div>
  </div>
</template>

<style scoped>
.remap { display: flex; flex-direction: column; gap: var(--spacing-xs); }
.remap-toggle {
  display: inline-flex; align-items: center; gap: 6px;
  background: none; border: 1px dashed var(--color-border);
  border-radius: var(--radius-sm); padding: 3px 10px;
  cursor: pointer; font-size: 12px; color: var(--color-text-secondary);
  transition: all var(--transition-fast); align-self: flex-start;
}
.remap-toggle:hover { color: var(--color-primary); border-color: var(--color-primary); }
.caret { font-size: 10px; }
.remap-body {
  display: flex; flex-wrap: wrap; align-items: center; gap: var(--spacing-sm);
  padding: var(--spacing-sm); background: var(--color-surface);
  border: 1px solid var(--color-border); border-radius: var(--radius-sm);
}
.rm-row { display: flex; align-items: center; gap: 6px; }
.rm-label { font-size: 12px; color: var(--color-text-secondary); min-width: 56px; }
.rm-hint { font-size: 11px; color: var(--color-text-tertiary); }
</style>
