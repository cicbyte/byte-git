/**
 * stores/motion.ts — Animation preference store.
 *
 * Ported from byte-stash's useMotion.ts preference layer (commit fae308a).
 *
 * Design:
 *   - 6 boolean/enum preferences persisted to localStorage 'byte-git-motion'.
 *   - applyPrefs() pushes duration + ease into gsap.defaults(), so EVERY
 *     tween that doesn't explicitly override duration/ease follows the
 *     user's speed/bounce choices — zero per-tween wiring.
 *   - motionDisabled() = (!enabled) OR prefers-reduced-motion. The system
 *     flag always wins (accessibility > user preference).
 *
 * The actual animation helpers (enterPage / staggerIn / v-reveal / countUp
 * etc.) live in composables/useGsapAnimations.ts and consume this store
 * via motionDisabled() + speedFactor() + revealOffset().
 */

import { defineStore } from 'pinia'
import { ref, watch } from 'vue'
import gsap from 'gsap'

const LS_KEY = 'byte-git-motion'
const LS_VERSION = 1

export type MotionSpeed = 'fast' | 'normal' | 'slow'
export type MotionOffset = 'none' | 'light' | 'strong'

export interface MotionPrefs {
  enabled: boolean
  bounce: boolean
  speed: MotionSpeed
  offset: MotionOffset
  stagger: boolean
  countUp: boolean
}

const SPEED_FACTOR: Record<MotionSpeed, number> = {
  fast: 0.7,
  normal: 1,
  slow: 1.35,
}
const OFFSET_PX: Record<MotionOffset, number> = {
  none: 0,
  light: 8,
  strong: 14,
}

function defaultPrefs(): MotionPrefs {
  return {
    enabled: true,
    bounce: true,
    speed: 'normal',
    offset: 'light',
    stagger: true,
    countUp: true,
  }
}

function loadPrefs(): MotionPrefs {
  try {
    const raw = localStorage.getItem(LS_KEY)
    if (!raw) return defaultPrefs()
    const parsed = JSON.parse(raw) as Partial<MotionPrefs & { version?: number }>
    return {
      enabled: typeof parsed.enabled === 'boolean' ? parsed.enabled : true,
      bounce: typeof parsed.bounce === 'boolean' ? parsed.bounce : true,
      speed: ['fast', 'normal', 'slow'].includes(parsed.speed as string) ? parsed.speed as MotionSpeed : 'normal',
      offset: ['none', 'light', 'strong'].includes(parsed.offset as string) ? parsed.offset as MotionOffset : 'light',
      stagger: typeof parsed.stagger === 'boolean' ? parsed.stagger : true,
      countUp: typeof parsed.countUp === 'boolean' ? parsed.countUp : true,
    }
  } catch {
    return defaultPrefs()
  }
}

export const useMotionStore = defineStore('motion', () => {
  const prefs = ref<MotionPrefs>(loadPrefs())

  /** Push speed/bounce into gsap.defaults so all tweens follow. */
  function applyPrefs() {
    gsap.defaults({
      duration: 0.42 * SPEED_FACTOR[prefs.value.speed],
      ease: prefs.value.bounce ? 'back.out(1.4)' : 'power2.out',
    })
  }

  function persist() {
    try {
      localStorage.setItem(LS_KEY, JSON.stringify({ version: LS_VERSION, ...prefs.value }))
    } catch (e) {
      console.warn('[motion] persist failed', e)
    }
  }

  /* ---- setters (each applies + persists) ---- */
  function setEnabled(v: boolean) { prefs.value.enabled = v; applyPrefs(); persist() }
  function setBounce(v: boolean) { prefs.value.bounce = v; applyPrefs(); persist() }
  function setSpeed(v: MotionSpeed) { prefs.value.speed = v; applyPrefs(); persist() }
  function setOffset(v: MotionOffset) { prefs.value.offset = v; persist() }
  function setStagger(v: boolean) { prefs.value.stagger = v; persist() }
  function setCountUp(v: boolean) { prefs.value.countUp = v; persist() }

  /* ---- query helpers (consumed by useGsapAnimations) ---- */

  /** System-level reduced-motion flag, read live on every call. */
  function prefersReduced(): boolean {
    return (
      typeof window !== 'undefined' &&
      typeof window.matchMedia === 'function' &&
      window.matchMedia('(prefers-reduced-motion: reduce)').matches
    )
  }

  /** True when all motion should be skipped (system OR user opt-out). */
  function motionDisabled(): boolean {
    return !prefs.value.enabled || prefersReduced()
  }

  function speedFactor(): number {
    return SPEED_FACTOR[prefs.value.speed]
  }

  function revealOffset(): number {
    return OFFSET_PX[prefs.value.offset]
  }

  function staggerEnabled(): boolean {
    return prefs.value.stagger
  }

  function countUpEnabled(): boolean {
    return prefs.value.countUp
  }

  // Apply on store creation so gsap.defaults is set before any tween runs.
  applyPrefs()

  // Re-apply if any pref mutates externally (defensive — setters already do it).
  watch(prefs, () => applyPrefs(), { deep: true })

  return {
    prefs,
    // setters
    setEnabled,
    setBounce,
    setSpeed,
    setOffset,
    setStagger,
    setCountUp,
    applyPrefs,
    persist,
    // query helpers
    prefersReduced,
    motionDisabled,
    speedFactor,
    revealOffset,
    staggerEnabled,
    countUpEnabled,
  }
})
