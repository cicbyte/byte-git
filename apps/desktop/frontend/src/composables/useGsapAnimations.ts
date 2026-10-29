/**
 * Centralized GSAP animation helpers — a restrained, business-appropriate
 * motion layer shared across the app.
 *
 * Ported from byte-stash's useMotion.ts (commit fae308a), merged with the
 * pre-existing byte-git helpers. All existing function signatures are
 * preserved so MainContent.vue / App.vue keep working unchanged.
 *
 * Design language (deliberate):
 *   - short durations (0.25–0.4s) — never sluggish
 *   - power2/power3 eases for most things; a single gentle back.out for
 *     empty-state placeholders so they feel inviting without being bouncy
 *   - small displacement (8–14px, controllable via motion.offset) — subtle
 *
 * Accessibility: every helper checks prefers-reduced-motion AND the user's
 * motion.enabled preference (via motion store). When disabled, tweens are
 * replaced by instant gsap.set to the end state — content shows, no movement.
 *
 * The motion store (stores/motion.ts) pushes speed/bounce into
 * gsap.defaults(), so tweens that don't explicitly set duration/ease follow
 * the user's preferences automatically.
 *
 * Implements the gsap-core skill: camelCase props, autoAlpha over opacity,
 * transform aliases (x/y/scale) over translate(), clearProps where needed.
 */
import gsap from 'gsap'
import type { Directive, DirectiveBinding } from 'vue'
import { nextTick, watch } from 'vue'
import { useMotionStore } from '@/stores/motion'

/**
 * Common end-state for reveal animations. clearProps:"all" is the key
 * contract: it strips the inline transform/opacity GSAP wrote, handing
 * control back to CSS so :hover { transform } etc. keep working.
 */
const REVEAL_TO = { y: 0, autoAlpha: 1, clearProps: 'all' as const }

/** Resolve the motion store lazily. Safe to call outside a component. */
function motion() {
  try {
    return useMotionStore()
  } catch {
    // Pinia not yet initialised (e.g. called very early). Fall back to a
    // permissive shim — tweens still run, just with default settings.
    return null
  }
}

/** True when motion should be skipped entirely (system OR user opt-out). */
export function motionDisabled(): boolean {
  const m = motion()
  if (!m) {
    // Fallback: still honour the system flag.
    return (
      typeof window !== 'undefined' &&
      typeof window.matchMedia === 'function' &&
      window.matchMedia('(prefers-reduced-motion: reduce)').matches
    )
  }
  return m.motionDisabled()
}

/** @deprecated use motionDisabled() — kept for back-compat with old callers. */
export function prefersReducedMotion(): boolean {
  return motionDisabled()
}

function speedFactor(): number {
  return motion()?.speedFactor() ?? 1
}

function revealOffset(): number {
  return motion()?.revealOffset() ?? 8
}

// ---- page / view transitions (router) ----

/** Play the page-enter tween on a freshly-mounted view root element. */
export function enterPage(el: Element | HTMLElement, done?: () => void): void {
  if (motionDisabled()) {
    gsap.set(el, { autoAlpha: 1, y: 0 })
    done?.()
    return
  }
  gsap.fromTo(
    el,
    { autoAlpha: 0, y: 14 },
    {
      autoAlpha: 1,
      y: 0,
      duration: 0.35 * speedFactor(),
      ease: 'power3.out',
      onComplete: done,
      // clear transform + opacity so the page root does NOT keep a residual
      // `transform: translate(0,0)` — any non-none transform makes it the
      // containing block for position:fixed children (ContextMenus, tooltips,
      // antd portals), throwing off clientX/clientY positioning.
      clearProps: 'all',
    },
  )
}

/** Play the page-leave tween before a view is unmounted. */
export function leavePage(el: Element | HTMLElement, done?: () => void): void {
  if (motionDisabled()) {
    gsap.set(el, { autoAlpha: 0 })
    done?.()
    return
  }
  gsap.to(el, {
    autoAlpha: 0,
    y: -8,
    duration: 0.2 * speedFactor(),
    ease: 'power2.in',
    onComplete: done,
    clearProps: 'all',
  })
}

// ---- list / card stagger entrance ----

/**
 * Stagger-in the children of `container` matching `selector`. Call after data
 * loads (nextTick) so the rows exist in the DOM. Default selector animates
 * direct children.
 */
export function staggerIn(
  container: Element | HTMLElement | null | undefined,
  selector = ':scope > *',
  opts: { each?: number; y?: number; duration?: number } = {},
): void {
  if (!container) return
  const each = opts.each ?? 0.04
  const y = opts.y ?? 10
  const duration = opts.duration ?? 0.3
  if (motionDisabled()) {
    gsap.set(container.querySelectorAll(selector), { autoAlpha: 1, y: 0 })
    return
  }
  gsap.from(container.querySelectorAll(selector), {
    autoAlpha: 0,
    y,
    duration,
    ease: 'power2.out',
    stagger: each,
    // clear ALL animated inline styles afterward so CSS hover/active take over
    // and nothing (opacity/visibility/transform) is left baked into the element.
    clearProps: 'all',
  })
}

// ---- drawer / overlay open-close ----

/** Open tween for a drawer panel (slide-in from the side + fade). */
export function drawerOpen(
  el: Element | HTMLElement,
  done?: () => void,
  fromX = 40,
): void {
  if (motionDisabled()) {
    gsap.set(el, { autoAlpha: 1, x: 0 })
    done?.()
    return
  }
  gsap.fromTo(
    el,
    { autoAlpha: 0, x: fromX },
    { autoAlpha: 1, x: 0, duration: 0.3, ease: 'power3.out', onComplete: done },
  )
}

/** Close tween for a drawer panel. */
export function drawerClose(
  el: Element | HTMLElement,
  done?: () => void,
  toX = 40,
): void {
  if (motionDisabled()) {
    gsap.set(el, { autoAlpha: 0 })
    done?.()
    return
  }
  gsap.to(el, {
    autoAlpha: 0,
    x: toX,
    duration: 0.25,
    ease: 'power3.inOut',
    onComplete: done,
  })
}

// ---- empty-state / placeholder entrance ----

/**
 * Gentle fade + scale-in for empty-state placeholders and loading spinners.
 * The only place we use a soft back.out so empty states feel inviting.
 */
export function fadeIn(el: Element | HTMLElement, done?: () => void): void {
  if (motionDisabled()) {
    gsap.set(el, { autoAlpha: 1, scale: 1 })
    done?.()
    return
  }
  gsap.fromTo(
    el,
    { autoAlpha: 0, scale: 0.96 },
    {
      autoAlpha: 1,
      scale: 1,
      duration: 0.35,
      ease: 'back.out(1.4)',
      onComplete: done,
      clearProps: 'all',
    },
  )
}

// ---- v-reveal: single-element entrance (byte-stash port) ----

/**
 * Directive: reveal a single element on mount. binding.value = delay (seconds).
 *
 * Usage:  <div v-reveal="0.1">...</div>
 */
export const vReveal: Directive = {
  mounted(el: HTMLElement, binding: DirectiveBinding) {
    if (motionDisabled()) {
      gsap.set(el, { autoAlpha: 1, y: 0 })
      return
    }
    gsap.fromTo(
      el,
      { y: revealOffset(), autoAlpha: 0 },
      { ...REVEAL_TO, delay: binding.value ?? 0 },
    )
  },
  unmounted(el: HTMLElement) {
    gsap.killTweensOf(el) // prevent tween leak on fast route switches
  },
}

/**
 * Directive: reveal a container's direct children with stagger on mount.
 * binding.value = stagger seconds (default 0.06). Honours motion.stagger
 * preference — disabled means all children appear together.
 *
 * Usage:  <ul v-reveal-group="0.08"><li>...</li>...</ul>
 */
export const vRevealGroup: Directive = {
  mounted(el: HTMLElement, binding: DirectiveBinding) {
    const targets = el.children
    if (!targets.length) return
    if (motionDisabled()) {
      gsap.set(targets, { autoAlpha: 1, y: 0 })
      return
    }
    const m = motion()
    const staggerOn = m?.staggerEnabled?.() ?? true
    const stagger = staggerOn ? (binding.value ?? 0.06) : 0
    gsap.fromTo(
      targets,
      { y: revealOffset(), autoAlpha: 0 },
      { ...REVEAL_TO, stagger },
    )
  },
  unmounted(el: HTMLElement) {
    gsap.killTweensOf(el.children)
  },
}

// ---- Vue directive: v-stagger (legacy, kept for back-compat) ----

/**
 * Directive that staggers-in the element's children when it (or a watched
 * dependency) changes.
 *
 * Usage:
 *   <div v-stagger="items.length"> rows... </div>
 *
 * The bound value is used as a dependency key — when it changes (e.g. items
 * load), the stagger re-runs. Pass a static truthy value to run once on mount.
 */
export const vStagger: Directive = {
  mounted(el: HTMLElement, binding: DirectiveBinding) {
    // wait a tick so v-for children render before we measure/animate them
    requestAnimationFrame(() => staggerIn(el, binding.arg ? `[data-stagger]` : ':scope > *'))
  },
  updated(el: HTMLElement, binding: DirectiveBinding) {
    if (binding.value === binding.oldValue) return
    requestAnimationFrame(() => staggerIn(el, binding.arg ? `[data-stagger]` : ':scope > *'))
  },
}

// ---- reveal on change (detail-pane switch, no unmount) ----

/**
 * Reveal-on-change: watch a dependency (e.g. selected id) and re-fade the
 * element returned by getEl(). Critical for detail panes that swap content
 * WITHOUT unmounting heavy children (CodeMirror, xterm, md-editor).
 *
 * Usage:
 *   const paneRef = ref<HTMLElement>()
 *   revealOnChange(() => paneRef.value, () => selected.value?.id)
 */
export function revealOnChange(
  getEl: () => Element | HTMLElement | null | undefined,
  dep: () => unknown,
): void {
  watch(dep, () => {
    nextTick(() => {
      const el = getEl()
      if (!el) return
      if (motionDisabled()) {
        gsap.set(el, { autoAlpha: 1, y: 0 })
        return
      }
      gsap.fromTo(el, { y: revealOffset(), autoAlpha: 0 }, {
        y: 0,
        autoAlpha: 1,
        duration: 0.34 * speedFactor(),
        clearProps: 'all',
      })
    })
  })
}

// ---- count up (dashboard stats) ----

/**
 * Animate a number from 0 to `to`, writing it into el.textContent.
 * Pre-zeros the text to avoid the "flash the real value, then jump back
 * to 0" flicker when data loads slightly after mount.
 *
 * Usage:
 *   <span ref="countRef"></span>
 *   countUp(countRef.value!, 1234)
 */
export function countUp(
  el: Element | HTMLElement,
  to: number,
  formatter: (n: number) => string = (n) => String(Math.round(n)),
): void {
  const m = motion()
  const enabled = m?.countUpEnabled?.() ?? true
  if (motionDisabled() || !enabled) {
    el.textContent = formatter(to)
    return
  }
  el.textContent = formatter(0) // pre-zero to avoid value-then-0 flicker
  const obj = { v: 0 }
  gsap.to(obj, {
    v: to,
    duration: 0.8 * speedFactor(),
    ease: 'power2.out', // fixed — count-up never bounces
    onUpdate: () => {
      el.textContent = formatter(obj.v)
    },
  })
}

/** Convenience composable returning the helpers (and the directives). */
export function useGsapAnimations() {
  return {
    enterPage,
    leavePage,
    staggerIn,
    drawerOpen,
    drawerClose,
    fadeIn,
    countUp,
    revealOnChange,
    motionDisabled,
    prefersReducedMotion,
  }
}
