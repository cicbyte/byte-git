<script setup>
/**
 * Footer injected by CheatView into the global footer slot.
 *
 * Shows: book icon + "Git 备忘录" title + entry/category counts + a refresh
 * button. All state is passed as props from the parent (CheatView owns it).
 */
import { ReloadOutlined, BookOutlined } from '@ant-design/icons-vue'

defineProps({
  totalEntries: { type: Number, default: 0 },
  catCount: { type: Number, default: 0 },
  pulling: { type: Boolean, default: false },
  onRefresh: { type: Function, default: null },
})
</script>

<template>
  <div class="cf-bar">
    <span class="cf-title"><BookOutlined /> Git 备忘录</span>
    <span class="cf-stat">{{ totalEntries }} 条 · {{ catCount }} 分类</span>
    <a-button size="small" type="text" :loading="pulling" @click="onRefresh" title="从远程更新">
      <template #icon><ReloadOutlined /></template>
    </a-button>
  </div>
</template>

<style scoped>
.cf-bar {
  display: flex;
  align-items: center;
  gap: var(--spacing-sm);
  width: 100%;
  font-size: 12px;
  color: var(--color-text-secondary);
  user-select: none;
}
.cf-title {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  font-size: 13px;
  font-weight: 500;
  color: var(--color-text);
}
.cf-stat { color: var(--color-text-tertiary); }
</style>
