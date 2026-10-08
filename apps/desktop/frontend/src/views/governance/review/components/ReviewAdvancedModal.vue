<script setup>
/**
 * Advanced options modal — extracted from ReviewView.
 *
 * Lets the user override the LLM model and provide business context
 * (background) for more accurate AI code review.
 */
const props = defineProps({
  open: { type: Boolean, default: false },
  model: { type: String, default: '' },
  background: { type: String, default: '' },
})
const emit = defineEmits(['update:open', 'update:model', 'update:background'])
</script>

<template>
  <a-modal
    :open="open"
    @update:open="(v) => emit('update:open', v)"
    title="高级选项"
    width="480px"
    :footer="null"
    destroy-on-close
  >
    <div class="rv-adv-body">
      <div class="rv-adv-row">
        <span class="rv-adv-label">模型覆盖</span>
        <a-input
          :value="model"
          @update:value="(v) => emit('update:model', v)"
          placeholder="留空用默认"
          allow-clear
        />
      </div>
      <div class="rv-adv-row">
        <span class="rv-adv-label">需求背景</span>
        <a-textarea
          :value="background"
          @update:value="(v) => emit('update:background', v)"
          placeholder="可选，提供业务上下文，帮助 AI 更准确地审查"
          :auto-size="{ minRows: 2, maxRows: 5 }"
          allow-clear
        />
      </div>
      <div class="rv-adv-foot">
        <a-button @click="emit('update:open', false)">完成</a-button>
      </div>
    </div>
  </a-modal>
</template>

<style scoped>
.rv-adv-body { display: flex; flex-direction: column; gap: 14px; padding: 4px 0; }
.rv-adv-row { display: flex; flex-direction: column; gap: 6px; }
.rv-adv-label { font-size: 12px; color: var(--color-text-tertiary); }
.rv-adv-foot { margin-top: 4px; }
</style>
