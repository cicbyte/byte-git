import { onUnmounted } from 'vue'
import { EventsOn } from '../../bridge/runtime/runtime'

/**
 * Subscribe to backend filesystem-watch events ("live:changed").
 *
 * The backend LiveStartWatch goroutine emits this (debounced) whenever the
 * working tree changes. Pass a callback to react — typically a re-fetch of
 * LiveStat + the currently selected file's diff.
 *
 * IMPORTANT: we deliberately do NOT call EventsOff on unmount. Wails v2's
 * EventsOff(eventName) removes ALL listeners for that event — including the
 * SCM store's own subscription that drives the Navbar change badge. Instead
 * we guard the callback with an `active` flag: after unmount the callback
 * becomes a no-op, but the event listener stays registered for other
 * subscribers (SCM store). This avoids wiping the global Navbar badge.
 */
export function useLiveStream(onChange: () => void) {
  const EVENT = 'live:changed'
  let active = true

  // register once; the guard prevents the callback from firing after the
  // owning component has unmounted
  EventsOn(EVENT, () => {
    if (active) onChange()
  })

  onUnmounted(() => {
    active = false
  })

  return {
    /** Manually trigger the onChange callback (bypasses the active guard). */
    trigger: onChange,
  }
}
