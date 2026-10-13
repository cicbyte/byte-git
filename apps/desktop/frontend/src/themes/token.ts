/**
 * themes/token.ts — CSS variable injection + antd token generation.
 *
 * applyColorsToDOM() is the single bridge between ThemeColors (JS) and the
 * CSS variable system (derivation.css). It writes three families of vars:
 *
 *   1. base bg → --bg-*-orig  (consumed by derivation.css color-mix)
 *   2. base non-bg → direct   (--color-primary, --text-*, --border-*)
 *   3. region overrides       (--bg-<region>-orig + --text-<region>(-sub))
 *
 * Plus it sets <html data-theme="light|dark"> — the ONLY switch we flip.
 * No class swapping, no stylesheet swapping. Every change is just CSS vars.
 */

import type { ThemeColors, RegionOverrides, ResolvedMode } from './types'
import { REGION_DEFS, defaultRegions } from './presets'

/** ThemeColors bg-typed keys that map to --bg-*-orig vars. */
const BASE_BG = {
  bgBase: '--bg-base-orig',
  bgSurface: '--bg-surface-orig',
  bgElevated: '--bg-elevated-orig',
  bgInput: '--bg-input-orig',
} as const

/** ThemeColors non-bg keys that map directly to --* CSS vars. */
const BASE_DIRECT = {
  primary: '--color-primary',
  primaryBg: '--color-primary-bg',
  colorHover: '--color-hover',
  textPrimary: '--text-primary',
  textSecondary: '--text-secondary',
  textTertiary: '--text-tertiary',
  // Borders inject into the *-orig vars so derivation.css's color-mix
  // (wallpaper-aware) derivation still applies. Injecting directly into
  // --border-base would overwrite the color-mix with an opaque colour and
  // block the wallpaper in multi-column pages that use --color-border as
  // the 1px gap background.
  borderBase: '--border-base-orig',
  borderLight: '--border-light-orig',
} as const

/** Extension-layer keys → CSS vars. */
const EXT_CSS_VARS = {
  textDisabled: '--text-disabled',
  primaryHover: '--color-primary-hover',
  primaryActive: '--color-primary-active',
  primaryBorder: '--color-primary-border',
  success: '--color-success',
  warning: '--color-warning',
  danger: '--color-error',
  info: '--color-info',
  successBg: '--color-success-bg',
  warningBg: '--color-warning-bg',
  dangerBg: '--color-error-bg',
  infoBg: '--color-info-bg',
  shadowMd: '--shadow-md',
  accent: '--color-accent',
  accentBg: '--color-accent-bg',
  colorActive: '--color-active',
} as const

/**
 * Inject a ThemeColors palette (+ optional region overrides) into :root and
 * set the data-theme attribute.
 *
 * NOTE: does NOT touch wallpaper vars (--wp-panel-alpha etc.) — those are
 * owned by the wallpaper store.
 */
export function applyColorsToDOM(
  colors: ThemeColors,
  appearance: ResolvedMode,
  regions?: RegionOverrides,
): void {
  const root = document.documentElement

  // 1. base bg → -orig vars (derivation.css derives --bg-* from these)
  for (const key in BASE_BG) {
    root.style.setProperty(BASE_BG[key as keyof typeof BASE_BG], colors[key as keyof ThemeColors] ?? '')
  }

  // 2. base non-bg → direct vars
  for (const key in BASE_DIRECT) {
    root.style.setProperty(BASE_DIRECT[key as keyof typeof BASE_DIRECT], colors[key as keyof ThemeColors] ?? '')
  }

  // 3. extension layer
  for (const key in EXT_CSS_VARS) {
    root.style.setProperty(EXT_CSS_VARS[key as keyof typeof EXT_CSS_VARS], colors[key as keyof ThemeColors] ?? '')
  }

  // 4. region semantics — override (if any) or default backref.
  const defs = defaultRegions(colors)
  const regionOver = regions ?? {}
  REGION_DEFS.forEach((d) => {
    const ov = regionOver[d.key]
    const bgOverride = ov?.bg
    const textOverride = ov?.text
    const textSubOverride = ov?.textSub
    // bg: write override or default — both go to the -orig var so the
    // wallpaper color-mix picks them up.
    root.style.setProperty(`${d.vars.bg}`, bgOverride ?? defs[d.key].bg ?? '')
    // text: write override, or REMOVE so derivation.css `var()` backref wins.
    if (textOverride != null) {
      root.style.setProperty(d.vars.text, textOverride)
    } else {
      root.style.removeProperty(d.vars.text)
    }
    if (textSubOverride != null) {
      root.style.setProperty(d.vars.textSub, textSubOverride)
    } else {
      root.style.removeProperty(d.vars.textSub)
    }
  })

  // 5. data-theme switch (the only attribute flip in the whole system)
  root.setAttribute('data-theme', appearance)
}

/**
 * Build an antd ConfigProvider token object from a ThemeColors.
 *
 * antd v4 token names differ from our CSS var names — this bridges them.
 * Imperative APIs (Modal.confirm / message.success) don't read this; those
 * need a separate CSS-var override (added later in global.css).
 */
export function buildAntdToken(colors: ThemeColors, mode: ResolvedMode) {
  return {
    algorithm: undefined, // set by caller — needs theme.darkAlgorithm / defaultAlgorithm
    token: {
      colorPrimary: colors.primary,
      colorInfo: colors.info,
      colorSuccess: colors.success,
      colorWarning: colors.warning,
      colorError: colors.danger,
      colorLink: colors.primary,

      colorBgContainer: colors.bgSurface,
      colorBgElevated: colors.bgElevated,
      colorBgLayout: colors.bgBase,
      colorBgSpotlight: colors.bgElevated,

      colorText: colors.textPrimary,
      colorTextSecondary: colors.textSecondary,
      colorTextTertiary: colors.textTertiary,
      colorTextDisabled: colors.textDisabled,

      colorBorder: colors.borderBase,
      colorBorderSecondary: colors.borderLight,

      colorBgTextHover: colors.colorHover,
      colorBgTextActive: colors.colorActive,

      borderRadius: 8,
      fontFamily:
        'Inter, -apple-system, BlinkMacSystemFont, Segoe UI, Roboto, sans-serif',
    },
    // Hint callers so they know which mode this token was built for.
    _resolvedMode: mode,
  } as const
}
