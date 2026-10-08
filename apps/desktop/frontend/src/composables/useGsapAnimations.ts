/**
 * Centralized GSAP animation helpers — a restrained, business-appropriate
 * motion layer shared across the app.
 *
 * Design language (deliberate):
 *   - short durations (0.25–0.4s) — never sluggish
 *   - power2/power3 eases for most things; a single gentle back.out for
 *     empty-state placeholders so they feel inviting without being bouncy
 *   - small位移 (8–14px) — subtle, not theatrical
 *
 * Accessibility: every helper checks `prefers-reduced-motion` (per GSAP's
 * matchMedia guidance). When reduced motion is requested, tweens are replaced
 * by instant `gsap.set` to the end state — content shows with no movement.
 *
 * Implements the gsap-core skill: camelCase props, autoAlpha over opacity,
 * transform aliases (x/y/scale) over translate(), clearProps where needed.
 */
import gsap from 'gsap'
import type { Directive, DirectiveBinding } from 'vue'

/** True when the user has asked the OS to minimize motion. */
export function prefersReducedMotion(): boolean {
  return (
    typeof window !== 'undefined' &&
    typeof window.matchMedia === 'function' &&
    window.matchMedia('(prefers-reduced-motion: reduce)').matches
  )
}

// ---- page / view transitions (router) ----

/** Play the page-enter tween on a freshly-mounted view root element. */
export function enterPage(el: Element | HTMLElement, done?: () => void): void {
  if (prefersReducedMotion()) {
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
      duration: 0.35,
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
  if (prefersReducedMotion()) {
    gsap.set(el, { autoAlpha: 0 })
    done?.()
    return
  }
  gsap.to(el, {
    autoAlpha: 0,
    y: -8,
    duration: 0.2,
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
  if (prefersReducedMotion()) {
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
  if (prefersReducedMotion()) {
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
  if (prefersReducedMotion()) {
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
  if (prefersReducedMotion()) {
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

// ---- Vue directive: v-stagger ----

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

/** Convenience composable returning the helpers (and the directive). */
export function useGsapAnimations() {
  return {
    enterPage,
    leavePage,
    staggerIn,
    drawerOpen,
    drawerClose,
    fadeIn,
    prefersReducedMotion,
  }
}
