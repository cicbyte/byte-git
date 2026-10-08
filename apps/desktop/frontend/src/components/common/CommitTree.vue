<script setup>
import { computed, useSlots, watch, ref, onMounted, onBeforeUnmount, nextTick } from 'vue'
import { useCommitSelection } from '../../composables/useCommitSelection'
import { staggerIn } from '../../composables/useGsapAnimations'

/**
 * Reusable commit-list "shell".
 *
 * This component owns everything *around* a commit entry — the scroll
 * container, loading / empty states, active highlight, multi-selection
 * (shift range / ctrl toggle), right-click dispatch and the optional hover
 * tooltip wrapper. The *contents* of each row and tooltip are deliberately
 * left to the caller via scoped slots, because commit shapes vary widely:
 *
 *   - MetaRewrite : { hash, pushed, original: {...}, rewrite: {...} }
 *   - AIRewrite   : { hash, pushed, date, original, suggested, status, ... }
 *   - LogView     : { hash, type, subject, author }   (flat, single-select)
 *
 * Slots
 *   #item( { entry, index, active, selected } )  row content   — required
 *   #tooltip( { entry, index } )                  hover card    — omit to disable
 *   #header                                list header   — optional
 *   #footer                                below the list (e.g. "load more")
 *   #empty                                 custom empty state
 *
 * Events
 *   update:active / update:selected   — v-model bindings
 *   click-item(entry, evt)
 *   hover-item(entry, isEnter)         — mouseenter/leave on a row; the caller
 *                                         uses this to lazy-load hover-only
 *                                         data (e.g. CommitStat for the tooltip)
 *   contextmenu(entry, evt)            — fired only when `context-menu` is on;
 *                                         the caller renders <ContextMenu>
 */
const props = defineProps({
  items: { type: Array, required: true },          // entries, each with a `hash`
  active: { type: String, default: '' },           // v-model:active  — single hash
  selected: { type: Set, default: () => new Set() }, // v-model:selected — hash set
  loading: { type: Boolean, default: false },
  emptyText: { type: String, default: '暂无提交' },
  multiSelect: { type: Boolean, default: true },   // false → ignore shift/ctrl
  contextMenu: { type: Boolean, default: true },   // false → let browser menu be
  tooltipOverlayClass: { type: String, default: 'ct-tip' },
  tooltipPlacement: { type: String, default: 'rightTop' },
  tooltipDelay: { type: Number, default: 0.3 },
  hasMore: { type: Boolean, default: false }, // more pages available → enable infinite scroll
})

const emit = defineEmits([
  'update:active', 'update:selected',
  'click-item', 'contextmenu', 'load-more', 'hover-item',
])

const slots = useSlots()
const hasTooltip = computed(() => !!slots.tooltip)

// keep an internal reactive view of the list for range calculations; the
// selection state itself is driven by the v-model props so the caller stays
// in control (e.g. to clear selection after applying a rule).
const sel = useCommitSelection(
  computed(() => props.items),
  props.multiSelect,
)

// two-way bind the selection set: mirror prop → internal, push internal → emit
watch(() => props.selected, (v) => { sel.selectedHashes.value = v }, { immediate: true })
function syncSelected() { emit('update:selected', sel.selectedHashes.value) }

function onClick(evt, entry) {
  sel.onItemClick(evt, entry.hash)
  syncSelected()
  emit('update:active', entry.hash)
  emit('click-item', entry, evt)
}

function onCtx(evt, entry) {
  if (!props.contextMenu) return              // leave the native menu be
  sel.onContextMenu(evt, entry.hash)
  syncSelected()
  emit('update:active', entry.hash)
  emit('contextmenu', entry, evt)
}

// ---- infinite scroll ----
// A sentinel at the list bottom is watched by IntersectionObserver. When it
// scrolls into view and there are more pages, we emit `load-more` — the caller
// fetches the next page and appends to `items`.
const bodyRef = ref(null)
const sentinelRef = ref(null)
const loadingMore = ref(false)
let ioObserver = null

function onSentinelVisible(entries) {
  if (entries[0].isIntersecting && props.hasMore && !loadingMore.value) {
    loadingMore.value = true
    emit('load-more', () => { loadingMore.value = false }) // caller calls this when done
  }
}

// (Re)bind the IntersectionObserver to the sentinel. Must be called after the
// sentinel enters the DOM (i.e. after items load — the body is v-if'd on
// items.length, so the sentinel node doesn't exist during initial loading).
function bindObserver() {
  nextTick(() => {
    if (ioObserver) { ioObserver.disconnect() }
    if (!sentinelRef.value || !bodyRef.value) return
    ioObserver = new IntersectionObserver(onSentinelVisible, {
      root: bodyRef.value,
      rootMargin: '80px',
    })
    ioObserver.observe(sentinelRef.value)
  })
}

onMounted(() => {
  bindObserver()
  // If the tree mounts already populated (e.g. the parent v-if'd it behind a
  // hasExported flag, so the empty→populated transition happened before mount),
  // the items.length watch never sees 0→N and won't fire — stagger-in on mount.
  // Empty-at-mount callers are unaffected and still rely on the watch below.
  if (props.items.length > 0) {
    nextTick(() => staggerIn(bodyRef.value, '.ct-item'))
  }
})
// re-bind whenever the list transitions from empty → populated, or grows
// (new page appended), because the sentinel DOM node is recreated by v-for/v-if.
watch(() => props.items.length, (n, old) => {
  bindObserver()
  // first population (empty → populated) → stagger-in the rows for a smooth entrance
  if ((!old || old === 0) && n > 0) {
    nextTick(() => staggerIn(bodyRef.value, '.ct-item'))
  }
})
onBeforeUnmount(() => {
  if (ioObserver) { ioObserver.disconnect(); ioObserver = null }
})

</script>

<template>
  <div class="ct-list">
    <div v-if="slots.header" class="ct-head"><slot name="header" /></div>

    <div v-if="loading && !items.length" class="ct-loading">
      <a-spin tip="加载提交树..." />
    </div>

    <div v-else-if="!items.length" class="ct-empty">
      <slot name="empty">{{ emptyText }}</slot>
    </div>

    <div v-else class="ct-body" ref="bodyRef">
      <a-tooltip
        v-for="(entry, index) in items"
        :key="entry.hash"
        :placement="tooltipPlacement"
        :overlay-class-name="tooltipOverlayClass"
        :mouse-enter-delay="tooltipDelay"
      >
        <!-- tooltip disabled at render-time when caller omits #tooltip -->
        <template v-if="hasTooltip" #title>
          <slot name="tooltip" :entry="entry" :index="index" />
        </template>

        <div
          class="ct-item"
          :class="{
            active: entry.hash === active,
            selected: sel.selectedHashes.value.has(entry.hash),
          }"
          @click="onClick($event, entry)"
          @contextmenu="onCtx($event, entry)"
          @mouseenter="emit('hover-item', entry, true)"
          @mouseleave="emit('hover-item', entry, false)"
        >
          <slot
            name="item"
            :entry="entry"
            :index="index"
            :active="entry.hash === active"
            :selected="sel.selectedHashes.value.has(entry.hash)"
          />
        </div>
      </a-tooltip>
      <!-- infinite-scroll sentinel + loading indicator -->
      <div ref="sentinelRef" class="ct-sentinel"></div>
      <div v-if="loadingMore" class="ct-more-loading"><a-spin size="small" /></div>
    </div>

    <div v-if="slots.footer" class="ct-footer"><slot name="footer" /></div>
  </div>
</template>

<style scoped>
.ct-list {
  display: flex;
  flex-direction: column;
  background: var(--color-surface);
  border: 1px solid var(--color-border);
  border-radius: var(--radius-md);
  overflow: hidden;
  min-height: 0;
  height: 100%;
}
.ct-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: var(--spacing-sm) var(--spacing-md);
  border-bottom: 1px solid var(--color-border);
  font-size: 12px;
  font-weight: 600;
  color: var(--color-text-secondary);
  flex-shrink: 0;
}
.ct-body {
  flex: 1;
  overflow-y: auto;
  padding: var(--spacing-xs);
}
.ct-sentinel { height: 1px; } /* invisible; observed by IntersectionObserver */
.ct-more-loading { display: flex; justify-content: center; padding: var(--spacing-sm); }
.ct-loading,
.ct-empty {
  flex: 1;
  display: flex;
  align-items: center;
  justify-content: center;
  color: var(--color-text-tertiary);
  font-size: 13px;
  min-height: 200px;
}
/* the row is just a clickable, themed wrapper — the caller fills its layout */
.ct-item {
  cursor: pointer;
  border-radius: var(--radius-sm);
  border-left: 2px solid transparent;
  transition: background var(--transition-fast);
  user-select: none;
}
.ct-item:hover { background: var(--color-hover); }
.ct-item.active {
  background: var(--color-primary-bg);
  border-left-color: var(--color-primary);
}
.ct-item.selected { background: rgba(125, 108, 248, 0.16); }
.ct-item.selected.active {
  background: var(--color-primary-bg);
  border-left-color: var(--color-primary);
}
.ct-footer {
  border-top: 1px solid var(--color-border);
  flex-shrink: 0;
}
</style>
