import { ref, computed, type Ref } from 'vue'

/**
 * Multi-selection state for a list of commits (or any items keyed by `hash`).
 *
 * Factory style — each call site gets its own independent selection state, so
 * MetaRewrite and AIRewrite (which may coexist) never share selection.
 *
 * Handles the standard click modifiers:
 *   - plain click : single-select active, clear bulk selection
 *   - ctrl/cmd    : toggle the clicked item
 *   - shift       : range select from the last anchor to the clicked item
 *
 * @param items       reactive list of entries; must expose a `hash` field
 * @param multiSelect when false, modifier keys are ignored (single-select only)
 */
export function useCommitSelection<T extends { hash: string }>(
  items: Ref<T[]>,
  multiSelect = true,
) {
  const selectedHashes = ref(new Set<string>())   // bulk-selected hashes
  const lastClickedHash = ref('')                  // anchor for shift-range

  const selectedItems = computed(() =>
    items.value.filter((e) => selectedHashes.value.has(e.hash)),
  )
  const selectedCount = computed(() => selectedHashes.value.size)

  function isInRange(hash: string) {
    if (!lastClickedHash.value) return false
    const idx = items.value.findIndex((e) => e.hash === hash)
    const anchor = items.value.findIndex((e) => e.hash === lastClickedHash.value)
    if (idx < 0 || anchor < 0) return false
    const [lo, hi] = idx < anchor ? [idx, anchor] : [anchor, idx]
    return lo <= idx && idx <= hi
  }

  /**
   * Click handler with modifier keys. Pass the native event so this can be
   * wired straight to `@click`. Returns nothing; mutates selection state.
   */
  function onItemClick(evt: MouseEvent, hash: string) {
    // prevent native text selection triggered by shift-click
    if (multiSelect && (evt.shiftKey || evt.ctrlKey || evt.metaKey)) evt.preventDefault()

    if (multiSelect && evt.shiftKey && lastClickedHash.value) {
      // range select: add everything from anchor to here
      const next = new Set(selectedHashes.value)
      const idx = items.value.findIndex((x) => x.hash === hash)
      const anchor = items.value.findIndex((x) => x.hash === lastClickedHash.value)
      if (idx >= 0 && anchor >= 0) {
        const [lo, hi] = idx < anchor ? [idx, anchor] : [anchor, idx]
        for (let i = lo; i <= hi; i++) next.add(items.value[i].hash)
      }
      selectedHashes.value = next
    } else if (multiSelect && (evt.ctrlKey || evt.metaKey)) {
      // toggle single
      const next = new Set(selectedHashes.value)
      if (next.has(hash)) next.delete(hash)
      else next.add(hash)
      selectedHashes.value = next
      lastClickedHash.value = hash
    } else {
      // plain click: single-select active, clear bulk
      selectedHashes.value = new Set()
      lastClickedHash.value = hash
    }
  }

  /**
   * Context-menu handler: ensures the right-clicked item is part of the
   * selection before emitting, matching common file-explorer behaviour.
   */
  function onContextMenu(evt: MouseEvent, hash: string) {
    evt.preventDefault()
    if (!selectedHashes.value.has(hash)) {
      selectedHashes.value = new Set([hash])
      lastClickedHash.value = hash
    }
  }

  function clear() {
    selectedHashes.value = new Set()
    lastClickedHash.value = ''
  }

  return {
    selectedHashes,
    lastClickedHash,
    selectedItems,
    selectedCount,
    onItemClick,
    onContextMenu,
    clear,
    isInRange,
  }
}
