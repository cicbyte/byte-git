<script setup>
import { watch, onBeforeUnmount } from 'vue'

/**
 * Floating context-menu container.
 *
 * Renders a positioned panel at fixed (x, y). The container chrome (header,
 * separators, shadow, theming) is unified here; the menu *items* come from
 * the default slot — they differ per page (MetaRewrite vs AIRewrite), so
 * hard-coding them would defeat the purpose.
 *
 * Use <CtxItem> for each entry to get consistent styling.
 *
 * Open/close is two-way bound (`v-model:open`). The component closes itself
 * on outside click / scroll / Esc, so callers no longer need to register
 * their own global listeners.
 */
const props = defineProps({
  open: { type: Boolean, default: false },
  x: { type: Number, default: 0 },
  y: { type: Number, default: 0 },
  title: { type: String, default: '' },
})
const emit = defineEmits(['update:open'])

function close() {
  emit('update:open', false)
}

// register/deregister global dismiss listeners only while open
function onDocClick() { close() }
function onScroll() { close() }
function onKey(e) { if (e.key === 'Escape') close() }

watch(() => props.open, (v) => {
  if (v) {
    // defer so the opening click itself doesn't immediately close it
    setTimeout(() => {
      window.addEventListener('click', onDocClick)
      window.addEventListener('scroll', onScroll, true)
      window.addEventListener('keydown', onKey)
    }, 0)
  } else {
    window.removeEventListener('click', onDocClick)
    window.removeEventListener('scroll', onScroll, true)
    window.removeEventListener('keydown', onKey)
  }
})

onBeforeUnmount(() => {
  window.removeEventListener('click', onDocClick)
  window.removeEventListener('scroll', onScroll, true)
  window.removeEventListener('keydown', onKey)
})
</script>

<template>
  <div
    v-if="open"
    class="ctx-menu"
    :style="{ left: x + 'px', top: y + 'px' }"
    @click.stop
  >
    <div v-if="title" class="ctx-head">{{ title }}</div>
    <slot />
  </div>
</template>

<style scoped>
.ctx-menu {
  position: fixed;
  z-index: 1050;
  min-width: 160px;
  background: var(--color-surface);
  border: 1px solid var(--color-border);
  border-radius: var(--radius-md);
  box-shadow: var(--shadow-md);
  padding: 4px;
}
.ctx-head {
  font-size: 11px;
  color: var(--color-text-tertiary);
  padding: 4px 10px;
  border-bottom: 1px solid var(--color-border-light);
  margin-bottom: 4px;
}
</style>

<style>
/* menu items: unscoped so <CtxItem> (a sibling component) can share styles,
   and so callers can drop in plain <button class="ctx-item"> if they prefer. */
.ctx-sep {
  height: 1px;
  background: var(--color-border-light);
  margin: 4px 0;
}
.ctx-item {
  display: flex;
  align-items: center;
  gap: 8px;
  width: 100%;
  padding: 6px 10px;
  background: none;
  border: none;
  cursor: pointer;
  font-size: 13px;
  color: var(--color-text);
  border-radius: var(--radius-sm);
  text-align: left;
}
.ctx-item:hover {
  background: var(--color-hover);
  color: var(--color-primary);
}
.ctx-item.danger:hover {
  background: var(--color-error-bg);
  color: var(--color-error);
}
</style>
