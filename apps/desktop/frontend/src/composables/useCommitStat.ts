/**
 * Lazy-load + cache CommitStat per hash, with hover debounce.
 *
 * Two access patterns share one cache:
 *   - `fetch(hash)`  — immediate, for the *active* commit (file-tree column)
 *   - `onHover/onLeave` — debounced, for the *hovered* commit (tooltip card)
 *
 * Used by TimeTravelView (code-shuttle) and reusable by any future view that
 * browses commits and needs per-commit change stats. Replaces the ad-hoc
 * statCache / hoverTimers / onCommitHover logic that previously lived inline.
 */
import { ref, onBeforeUnmount } from 'vue'

/** One row of `git show --stat` — shape produced by the Go backend. */
export interface CommitStatEntry {
  file: string
  status: string // 'A' | 'M' | 'D' | ...
  added: number
  deleted: number
}

export function useCommitStat(
  fetcher: (hash: string) => Promise<CommitStatEntry[]>,
  opts: { delay?: number } = {},
) {
  const delay = opts.delay ?? 300
  const cache = ref(new Map<string, CommitStatEntry[]>())
  // pending hover timers + in-flight immediate fetches, keyed by hash
  const hoverTimers = new Map<string, ReturnType<typeof setTimeout>>()
  const pending = new Set<string>()

  /** Read a cached stat (empty array if not yet loaded). */
  function stat(hash: string): CommitStatEntry[] {
    return cache.value.get(hash) || []
  }
  /** Total added lines across all files in the cached stat. */
  function added(hash: string): number {
    return stat(hash).reduce((s, e) => s + e.added, 0)
  }
  /** Total deleted lines across all files in the cached stat. */
  function deleted(hash: string): number {
    return stat(hash).reduce((s, e) => s + e.deleted, 0)
  }
  function has(hash: string): boolean {
    return cache.value.has(hash)
  }

  /** Mark the cache as changed so Vue notices (Map mutation isn't tracked). */
  function touch() {
    cache.value = new Map(cache.value)
  }

  /** Immediately fetch + cache (de-duplicated). For the active commit. */
  async function fetch(hash: string) {
    if (!hash || cache.value.has(hash) || pending.has(hash)) return
    pending.add(hash)
    try {
      const data = (await fetcher(hash)) || []
      cache.value.set(hash, data)
      touch()
    } catch {
      // leave uncached; a later hover/activation can retry
    } finally {
      pending.delete(hash)
    }
  }

  /** Debounced fetch — call on mouseenter. Skips if already cached. */
  function onHover(hash: string) {
    if (!hash || cache.value.has(hash)) return
    if (hoverTimers.has(hash)) clearTimeout(hoverTimers.get(hash))
    hoverTimers.set(
      hash,
      setTimeout(() => {
        hoverTimers.delete(hash)
        fetch(hash)
      }, delay),
    )
  }

  /** Cancel a pending debounced fetch — call on mouseleave. */
  function onLeave(hash: string) {
    const t = hoverTimers.get(hash)
    if (t) {
      clearTimeout(t)
      hoverTimers.delete(hash)
    }
  }

  /** Drop everything (e.g. on repo switch). */
  function clear() {
    hoverTimers.forEach((t) => clearTimeout(t))
    hoverTimers.clear()
    pending.clear()
    cache.value = new Map()
  }

  onBeforeUnmount(clear)

  return { cache, stat, added, deleted, has, fetch, onHover, onLeave, clear }
}
