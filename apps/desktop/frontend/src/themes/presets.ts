/**
 * themes/presets.ts — Preset seeds + colour derivation algorithms.
 *
 * Ported from byte-stash (commit fae308a) and localised for byte-git.
 *
 * Design (mirrors byte-stash):
 *   - Each builtin preset defines ONLY two seed values: primary + bgBase
 *     (per light/dark). Everything else is derived.
 *   - The `dracula` preset is byte-git's anchor — it uses a hand-tuned
 *     neutral palette (NEUTRAL) rather than derivation, so existing users
 *     see zero visual change.
 *   - The other 4 presets (emerald/violet/amber/rose) use deriveBaseLayer()
 *     to compute harmonious neutrals from their tinted bgBase.
 *   - Extension-layer tokens (status, primaryHover/Active/Border, shadow)
 *     are added per-mode by SUPPLEMENT and never change with preset tone.
 */

import type {
  BuiltinPreset,
  EditableToken,
  RegionDef,
  RegionOverrides,
  ResolvedMode,
  ThemeColors,
} from './types'

/* ============================================================
 * 0. Pure colour helpers
 * ============================================================ */

/** Parse #abc / #abcdef / #abcdef12 (alpha optional) into [r,g,b,a]. */
export function hexToRgb(hex: string): { r: number; g: number; b: number; a: number } {
  let h = hex.replace('#', '').trim()
  if (h.length === 3) h = h.split('').map((c) => c + c).join('')
  if (h.length === 8) {
    return {
      r: parseInt(h.slice(0, 2), 16),
      g: parseInt(h.slice(2, 4), 16),
      b: parseInt(h.slice(4, 6), 16),
      a: parseInt(h.slice(6, 8), 16) / 255,
    }
  }
  return {
    r: parseInt(h.slice(0, 2), 16),
    g: parseInt(h.slice(2, 4), 16),
    b: parseInt(h.slice(4, 6), 16),
    a: 1,
  }
}

function toHex2(n: number): string {
  const v = Math.max(0, Math.min(255, Math.round(n)))
  return v.toString(16).padStart(2, '0')
}

function rgbToHex(r: number, g: number, b: number): string {
  return `#${toHex2(r)}${toHex2(g)}${toHex2(b)}`
}

/** Mix two hex colours. amt=0 → a, amt=1 → b. */
function mix(a: string, b: string, amt: number): string {
  const ca = hexToRgb(a)
  const cb = hexToRgb(b)
  return rgbToHex(
    ca.r + (cb.r - ca.r) * amt,
    ca.g + (cb.g - ca.g) * amt,
    ca.b + (cb.b - ca.b) * amt,
  )
}

/** Lighten a hex colour towards white by amt (0..1). */
function lighten(hex: string, amt: number): string {
  return mix(hex, '#ffffff', amt)
}

/** Darken a hex colour towards black by amt (0..1). */
function darken(hex: string, amt: number): string {
  return mix(hex, '#000000', amt)
}

/** Convert a hex colour + alpha into rgba() string. */
export function withAlpha(hex: string, alpha: number): string {
  const { r, g, b } = hexToRgb(hex)
  return `rgba(${r}, ${g}, ${b}, ${alpha})`
}

/* ============================================================
 * 1. Preset seeds
 * ============================================================ */

interface PresetSeed {
  label: string
  primary: Record<ResolvedMode, string>
  /** Accent — a contrasting hue used in primary→accent gradients. Each
      preset defines its own so gradients stay visible across presets. */
  accent: Record<ResolvedMode, string>
  bgBase: Record<ResolvedMode, string>
}

export const PRESETS: Record<BuiltinPreset, PresetSeed> = {
  dracula: {
    label: 'Dracula 紫',
    primary: { light: '#BD93F9', dark: '#BD93F9' },
    accent:  { light: '#FF79C6', dark: '#FF79C6' },  // signature pink
    bgBase:  { light: '#f8fafc', dark: '#0f172a' },
  },
  emerald: {
    label: '翡翠绿',
    primary: { light: '#059669', dark: '#34d399' },
    accent:  { light: '#0e7490', dark: '#22d3ee' },  // teal/cyan contrast
    bgBase:  { light: '#f1f6f3', dark: '#0a1612' },
  },
  violet: {
    label: '紫罗兰',
    primary: { light: '#7c3aed', dark: '#a78bfa' },
    accent:  { light: '#db2777', dark: '#f472b6' },  // magenta contrast
    bgBase:  { light: '#f5f2f8', dark: '#140f1d' },
  },
  amber: {
    label: '琥珀橙',
    primary: { light: '#d97706', dark: '#fbbf24' },
    accent:  { light: '#dc2626', dark: '#f87171' },  // red contrast
    bgBase:  { light: '#faf6f0', dark: '#1a140a' },
  },
  rose: {
    label: '玫瑰红',
    primary: { light: '#e11d48', dark: '#fb7185' },
    accent:  { light: '#7c3aed', dark: '#a78bfa' },  // violet contrast
    bgBase:  { light: '#f8f2f4', dark: '#1a0d11' },
  },
}

/** Ordered list for preset pickers. */
export const PRESET_LIST: { key: BuiltinPreset; label: string }[] = (
  Object.keys(PRESETS) as BuiltinPreset[]
).map((key) => ({ key, label: PRESETS[key].label }))

/* ============================================================
 * 2. Hand-tuned neutral palette for the dracula anchor preset.
 *    Using these directly guarantees zero visual drift for existing users.
 * ============================================================ */
const NEUTRAL: Record<ResolvedMode, {
  bgBase: string
  bgSurface: string
  bgElevated: string
  bgInput: string
  textPrimary: string
  textSecondary: string
  textTertiary: string
  textDisabled: string
  borderBase: string
  borderLight: string
}> = {
  dark: {
    bgBase: '#0f172a',
    bgSurface: '#1e293b',
    bgElevated: '#3d4d63',
    bgInput: '#1e293b',
    textPrimary: '#f1f5f9',
    textSecondary: '#94a3b8',
    textTertiary: '#64748b',
    textDisabled: '#475569',
    borderBase: '#3d4d63',
    borderLight: '#2d3e50',
  },
  light: {
    bgBase: '#ffffff',
    bgSurface: '#f8fafc',
    bgElevated: '#f1f5f9',
    bgInput: '#f8fafc',
    textPrimary: '#0f172a',
    textSecondary: '#64748b',
    textTertiary: '#94a3b8',
    textDisabled: '#cbd5e1',
    borderBase: '#e2e8f0',
    borderLight: '#f1f5f9',
  },
}

/* ============================================================
 * 3. Extension-layer supplement (fixed per mode, applied to every preset).
 *    Status colours, disabled text, shadows — must stay readable across
 *    all presets, so they're mode-only.
 *
 *    NOTE: accent / accentBg / colorActive are NOT here — they're derived
 *    from the preset's primary/accent seeds in buildPresetColors(), so they
 *    follow the preset instead of being locked to the dracula anchor.
 * ============================================================ */
const SUPPLEMENT: Record<ResolvedMode, Omit<
  ThemeColors,
  | 'primary' | 'primaryBg' | 'colorHover' | 'colorActive'
  | 'bgBase' | 'bgSurface' | 'bgInput' | 'bgElevated'
  | 'textPrimary' | 'textSecondary' | 'textTertiary'
  | 'borderBase' | 'borderLight'
  | 'accent' | 'accentBg'
>> = {
  dark: {
    textDisabled: '#475569',
    primaryHover: '',
    primaryActive: '',
    primaryBorder: '',
    success: '#50FA7B', successBg: 'rgba(80, 250, 123, 0.18)',
    warning: '#FFB86C', warningBg: 'rgba(255, 184, 108, 0.18)',
    danger: '#FF5555', dangerBg: 'rgba(255, 85, 85, 0.18)',
    info: '#8BE9FD', infoBg: 'rgba(139, 233, 253, 0.18)',
    shadowMd: '0 4px 6px rgba(0,0,0,0.25), 0 2px 4px rgba(0,0,0,0.2)',
  },
  light: {
    textDisabled: '#cbd5e1',
    primaryHover: '',
    primaryActive: '',
    primaryBorder: '',
    success: '#16a34a', successBg: 'rgba(22, 163, 74, 0.12)',
    warning: '#d97706', warningBg: 'rgba(217, 119, 6, 0.12)',
    danger: '#dc2626', dangerBg: 'rgba(220, 38, 38, 0.12)',
    info: '#0e7490', infoBg: 'rgba(14, 116, 144, 0.12)',
    shadowMd: '0 4px 6px rgba(0,0,0,0.07), 0 2px 4px rgba(0,0,0,0.06)',
  },
}

/* ============================================================
 * 4. Derivation algorithms
 * ============================================================ */

/** Derive primaryHover (lighten 12%) / primaryActive (darken 12%) / border (alpha 40%). */
export function derivePrimaryStates(primary: string): {
  primaryHover: string
  primaryActive: string
  primaryBorder: string
} {
  return {
    primaryHover: lighten(primary, 0.12),
    primaryActive: darken(primary, 0.12),
    primaryBorder: withAlpha(primary, 0.4),
  }
}

/** Derive primaryBg (primary @ 10-15% alpha, mode-tuned). */
export function derivePrimaryBg(primary: string, mode: ResolvedMode): string {
  return withAlpha(primary, mode === 'dark' ? 0.15 : 0.1)
}

/** Derive colorHover (white@5% in dark / black@4% in light). */
export function deriveHover(primary: string, mode: ResolvedMode): string {
  // byte-stash uses white/black wash; byte-git keeps the existing primary-tinted
  // hover for the dracula anchor feel, falling back to neutral wash for others.
  return withAlpha(primary, mode === 'dark' ? 0.08 : 0.12)
}

/** Derive colorActive (slightly stronger than hover, same primary tint). */
export function deriveActive(primary: string, mode: ResolvedMode): string {
  return withAlpha(primary, mode === 'dark' ? 0.12 : 0.18)
}

/** Derive accentBg (accent @ low alpha) — follows the preset's accent seed. */
export function deriveAccentBg(accent: string, mode: ResolvedMode): string {
  return withAlpha(accent, mode === 'dark' ? 0.18 : 0.1)
}

/**
 * Derive the base layer (bg family + text family + border family) for the
 * 4 non-dracula presets from a tinted bgBase. Returns harmonious neutrals
 * by walking bgBase towards white/black.
 */
export function deriveBaseLayer(
  bgBase: string,
  mode: ResolvedMode,
): Pick<
  ThemeColors,
  'bgBase' | 'bgSurface' | 'bgElevated' | 'bgInput' | 'textPrimary' | 'textSecondary' | 'textTertiary' | 'borderBase' | 'borderLight'
> {
  if (mode === 'dark') {
    return {
      bgBase,
      bgSurface: lighten(bgBase, 0.06),
      bgElevated: lighten(bgBase, 0.14),
      bgInput: lighten(bgBase, 0.04),
      textPrimary: lighten(bgBase, 0.92),
      textSecondary: lighten(bgBase, 0.62),
      textTertiary: lighten(bgBase, 0.46),
      borderBase: lighten(bgBase, 0.22),
      borderLight: lighten(bgBase, 0.10),
    }
  }
  // light
  return {
    bgBase,
    bgSurface: darken(bgBase, 0.025),
    bgElevated: darken(bgBase, 0.05),
    bgInput: darken(bgBase, 0.02),
    textPrimary: darken(bgBase, 0.95),
    textSecondary: darken(bgBase, 0.55),
    textTertiary: darken(bgBase, 0.40),
    borderBase: darken(bgBase, 0.12),
    borderLight: darken(bgBase, 0.05),
  }
}

/** Build the full ThemeColors object for a builtin preset. */
export function buildPresetColors(preset: BuiltinPreset, mode: ResolvedMode): ThemeColors {
  const seed = PRESETS[preset]
  const primary = seed.primary[mode]
  const accent = seed.accent[mode]
  const supplement = SUPPLEMENT[mode]

  // dracula uses hand-tuned neutrals; others derive from bgBase.
  const base =
    preset === 'dracula'
      ? NEUTRAL[mode]
      : deriveBaseLayer(seed.bgBase[mode], mode)

  // For dracula we keep the user's existing bgBase seed even if NEUTRAL
  // disagrees (they're identical for dracula, but this keeps things honest).
  base.bgBase = seed.bgBase[mode]

  const states = derivePrimaryStates(primary)
  const primaryBg = derivePrimaryBg(primary, mode)
  const colorHover = deriveHover(primary, mode)
  const colorActive = deriveActive(primary, mode)
  const accentBg = deriveAccentBg(accent, mode)

  return {
    primary,
    primaryBg,
    colorHover,
    bgBase: base.bgBase,
    bgSurface: base.bgSurface,
    bgInput: base.bgInput,
    bgElevated: base.bgElevated,
    textPrimary: base.textPrimary,
    textSecondary: base.textSecondary,
    textTertiary: base.textTertiary,
    borderBase: base.borderBase,
    borderLight: base.borderLight,
    textDisabled: supplement.textDisabled,
    primaryHover: states.primaryHover,
    primaryActive: states.primaryActive,
    primaryBorder: states.primaryBorder,
    success: supplement.success,
    warning: supplement.warning,
    danger: supplement.danger,
    info: supplement.info,
    successBg: supplement.successBg,
    warningBg: supplement.warningBg,
    dangerBg: supplement.dangerBg,
    infoBg: supplement.infoBg,
    shadowMd: supplement.shadowMd,
    // accent family — derived from preset seeds, NOT locked to dracula
    accent,
    accentBg,
    colorActive,
  }
}

/** Default custom palette = dracula preset (so toggling custom is invisible). */
export function defaultCustom(): { light: ThemeColors; dark: ThemeColors } {
  return {
    light: buildPresetColors('dracula', 'light'),
    dark: buildPresetColors('dracula', 'dark'),
  }
}

/**
 * Recompute derived tokens when the user edits a seed colour (e.g. primary).
 * Returns a NEW ThemeColors with derived fields updated; non-derived fields
 * are preserved.
 */
export function recomputeDerived(colors: ThemeColors, mode: ResolvedMode): ThemeColors {
  const states = derivePrimaryStates(colors.primary)
  return {
    ...colors,
    primaryHover: states.primaryHover,
    primaryActive: states.primaryActive,
    primaryBorder: states.primaryBorder,
    primaryBg: derivePrimaryBg(colors.primary, mode),
    colorHover: deriveHover(colors.primary, mode),
    colorActive: deriveActive(colors.primary, mode),
    accentBg: deriveAccentBg(colors.accent, mode),
  }
}

/* ============================================================
 * 5. Region definitions + defaults
 * ============================================================ */

export const REGION_DEFS: RegionDef[] = [
  { key: 'rail',    label: '项目轨道', vars: { bg: '--bg-rail-orig',    text: '--text-rail',    textSub: '--text-rail-sub' } },
  { key: 'sider',   label: '菜单栏',   vars: { bg: '--bg-sider-orig',   text: '--text-sider',   textSub: '--text-sider-sub' } },
  { key: 'navbar',  label: '顶栏',     vars: { bg: '--bg-navbar-orig',  text: '--text-navbar',  textSub: '--text-navbar-sub' } },
  { key: 'content', label: '内容区',   vars: { bg: '--bg-content-orig', text: '--text-content', textSub: '--text-content-sub' } },
  { key: 'footer',  label: '页脚',     vars: { bg: '--bg-footer-orig',  text: '--text-footer',  textSub: '--text-footer-sub' } },
  { key: 'card',    label: '卡片',     vars: { bg: '--bg-card-orig',    text: '--text-card',    textSub: '--text-card-sub' } },
]

/** Default region values derived from a ThemeColors (no user overrides). */
export function defaultRegions(c: ThemeColors): Required<RegionOverrides> {
  return {
    rail:    { bg: c.bgSurface, text: c.textPrimary, textSub: c.textSecondary },
    sider:   { bg: c.bgSurface, text: c.textPrimary, textSub: c.textSecondary },
    navbar:  { bg: c.bgSurface, text: c.textPrimary, textSub: c.textSecondary },
    content: { bg: c.bgBase,    text: c.textPrimary, textSub: c.textSecondary },
    footer:  { bg: c.bgSurface, text: c.textPrimary, textSub: c.textSecondary },
    card:    { bg: c.bgSurface, text: c.textPrimary, textSub: c.textSecondary },
  }
}

/* ============================================================
 * 6. Editable-token picker UI descriptor
 * ============================================================ */

export const EDITABLE_TOKENS: EditableToken[] = [
  { key: 'primary',     label: '主题色',     group: 'primary' },
  { key: 'accent',      label: '强调色',     group: 'primary' },
  { key: 'bgBase',      label: '页面底色',   group: 'background' },
  { key: 'bgSurface',   label: '容器底色',   group: 'background' },
  { key: 'bgElevated',  label: '浮层底色',   group: 'background' },
  { key: 'bgInput',     label: '输入框底色', group: 'background' },
  { key: 'textPrimary', label: '主文字',     group: 'text' },
  { key: 'textSecondary', label: '次要文字', group: 'text' },
  { key: 'textTertiary',  label: '弱化文字', group: 'text' },
  { key: 'borderBase',  label: '主边框',     group: 'border' },
  { key: 'borderLight', label: '弱边框',     group: 'border' },
]
