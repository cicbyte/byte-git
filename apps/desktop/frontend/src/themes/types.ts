/**
 * themes/types.ts — Type definitions for the three-layer theme system.
 *
 * Ported from byte-stash (commit fae308a) and localised for byte-git.
 * See themes/derivation.css for the layering rationale.
 */

/** Resolved light/dark mode (auto is resolved before reaching here). */
export type ResolvedMode = 'light' | 'dark'

/** User-facing mode preference. `auto` follows the OS. */
export type Mode = 'auto' | ResolvedMode

/** Alias used by some downstream consumers (kept for parity). */
export type Appearance = ResolvedMode

/** Built-in preset keys. `dracula` is byte-git's anchor preset. */
export type BuiltinPreset = 'dracula' | 'emerald' | 'violet' | 'amber' | 'rose'

/** Any preset key, including the user-editable custom one. */
export type PresetKey = BuiltinPreset | 'custom'

/**
 * ThemeColors — the base palette (Layer 1).
 *
 * 26 tokens split into two groups:
 *   - Base layer (12): primary + bg family + text family + border family.
 *     Editable by users (custom preset). Derived tokens auto-recompute.
 *   - Extension layer (14): status colors, primary states, disabled text,
 *     shadow. Fixed per light/dark; never overridden by presets.
 *
 * NOTE: byte-git business-fixed semantics (heatmap --heat-*, Dracula syntax
 * colors) are deliberately NOT part of ThemeColors — they live as static
 * CSS vars in derivation.css and only follow light/dark.
 */
export interface ThemeColors {
  /* ===== Base layer (12, ax-port) ===== */
  primary: string
  primaryBg: string
  colorHover: string
  bgBase: string
  bgSurface: string
  bgInput: string
  bgElevated: string
  textPrimary: string
  textSecondary: string
  textTertiary: string
  borderBase: string
  borderLight: string

  /* ===== Extension layer (14, byte-git business deps) ===== */
  textDisabled: string
  primaryHover: string
  primaryActive: string
  primaryBorder: string
  success: string
  warning: string
  danger: string
  info: string
  successBg: string
  warningBg: string
  dangerBg: string
  infoBg: string
  shadowMd: string

  /* ===== byte-git specifics (kept in the same object for uniform apply) ===== */
  /** Pink accent — part of the Dracula identity, follows preset primary tone. */
  accent: string
  accentBg: string
  /** Alias of colorActive — kept to avoid renaming legacy consumers. */
  colorActive: string
}

/** Keys of ThemeColors that are user-editable in the custom preset UI. */
export type EditableTokenKey =
  | 'primary'
  | 'bgBase'
  | 'bgSurface'
  | 'bgInput'
  | 'bgElevated'
  | 'textPrimary'
  | 'textSecondary'
  | 'textTertiary'
  | 'borderBase'
  | 'borderLight'
  | 'accent'

/** A UI region (maps 1:1 to byte-git layout areas). */
export interface RegionDef {
  key: 'rail' | 'sider' | 'navbar' | 'content' | 'footer' | 'card'
  label: string
  /** CSS var names this region owns (bg orig + text + text-sub). */
  vars: { bg: string; text: string; textSub: string }
}

/** Region overrides — values are CSS color strings or undefined (use default). */
export type RegionOverrides = Partial<{
  [K in RegionDef['key']]: { bg?: string; text?: string; textSub?: string }
}>

/** A token descriptor for the editable-token picker UI. */
export interface EditableToken {
  key: EditableTokenKey
  label: string
  group: 'primary' | 'background' | 'text' | 'border' | 'accent'
}

/** Persisted theme state shape (localStorage). */
export interface ThemeState {
  version: number
  preset: PresetKey
  mode: Mode
  custom: { light: ThemeColors; dark: ThemeColors }
  regions: { light: RegionOverrides; dark: RegionOverrides }
}
