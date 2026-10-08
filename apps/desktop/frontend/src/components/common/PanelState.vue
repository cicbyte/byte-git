<script setup>
defineProps({
  loading: { type: Boolean, default: false },
  error: { type: String, default: '' },
  empty: { type: Boolean, default: false },
  emptyText: { type: String, default: '暂无数据' },
})
defineEmits(['retry'])
</script>

<template>
  <div class="panel-state">
    <div v-if="loading" class="state-loading">
      <a-spin />
    </div>
    <div v-else-if="error" class="state-error">
      <div class="error-text">{{ error }}</div>
      <a-button size="small" @click="$emit('retry')">重试</a-button>
    </div>
    <div v-else-if="empty" class="state-empty">
      {{ emptyText }}
    </div>
    <slot v-else />
  </div>
</template>

<style scoped>
.panel-state {
  min-height: 200px;
}
.state-loading,
.state-error,
.state-empty {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 12px;
  min-height: 200px;
  color: var(--color-text-tertiary);
  font-size: 13px;
}
.error-text {
  color: var(--color-error);
  word-break: break-all;
  text-align: center;
  max-width: 80%;
}
</style>
