<script setup>
import { ref, watch, nextTick, onBeforeUnmount } from 'vue'

/**
 * Floating context-menu container.
 *
 * Renders a positioned panel at fixed (x, y), teleported to <body> — 逃出
 * 调用方祖先的层叠上下文/overflow 裁切（如 ProjectRail z-index:2 会被
 * 后续兄弟 Sidebar 盖住）与滚动容器裁切。打开后按视口边缘翻转收拢。
 *
 * The container chrome (header, separators, shadow, theming) is unified
 * here; the menu *items* come from the default slot — they differ per page
 * (MetaRewrite vs AIRewrite), so hard-coding them would defeat the purpose.
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

const panel = ref(null)
const pos = ref({ x: 0, y: 0 })

/** 视口边缘翻转：右/下溢出时向内收拢（留 8px 边距，不小于 8）。 */
function clampToViewport() {
  const node = panel.value
  if (!node) return
  const r = node.getBoundingClientRect()
  const vw = window.innerWidth
  const vh = window.innerHeight
  let { x, y } = pos.value
  if (x + r.width > vw - 8) x = Math.max(8, vw - r.width - 8)
  if (y + r.height > vh - 8) y = Math.max(8, vh - r.height - 8)
  pos.value = { x, y }
}

function close() {
  emit('update:open', false)
}

// register/deregister global dismiss listeners only while open
function onDocClick() { close() }
function onScroll() { close() }
function onKey(e) { if (e.key === 'Escape') close() }

watch(() => props.open, async (v) => {
  if (v) {
    pos.value = { x: props.x, y: props.y }
    await nextTick()
    clampToViewport()
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
  <Teleport to="body">
    <div
      v-if="open"
      ref="panel"
      class="ctx-menu"
      :style="{ left: pos.x + 'px', top: pos.y + 'px' }"
      @click.stop
    >
      <div v-if="title" class="ctx-head">{{ title }}</div>
      <slot />
    </div>
  </Teleport>
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
