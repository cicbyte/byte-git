<script setup>
/**
 * 推送状态标签 — 统一在提交列表/树中显示 commit 的推送状态。
 *
 * 用于：MetaRewriteView、AIRewriteView 的提交树 item slot。
 * 替代各页面各自写的 mr-push-tag / ar-push-tag。
 */
defineProps({
  pushed: { type: Boolean, default: false },
})
</script>

<template>
  <span class="push-tag" :class="pushed ? 'is-pushed' : 'is-local'">
    {{ pushed ? '已推送' : '本地' }}
  </span>
</template>

<style scoped>
.push-tag {
  font-size: 10px;
  padding: 1px 6px;
  border-radius: 8px;
  flex-shrink: 0;
  font-weight: 500;
  line-height: 16px;
}
.push-tag.is-pushed {
  color: var(--color-text-tertiary);
  background: var(--color-surface-raised);
}
.push-tag.is-local {
  color: var(--color-success);
  background: var(--color-success-bg);
}
</style>
