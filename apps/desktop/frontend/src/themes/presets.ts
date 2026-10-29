/**
 * themes/presets.ts — Preset seeds + colour derivation algorithms.
 *
 * Design (2026-10 restyle, "Linear-style dark refinement"):
 *   - Each builtin preset defines ONLY two seed values: primary + bgBase
 *     (per light/dark). Everything else is derived.
 *   - DARK MODE: every preset shares one hand-tuned neutral near-black
 *     ramp (LINEAR_DARK — canvas #08090A / surface #0F1013 / elevated
 *     #17181C, 1px hairline #1C1D21). Only primary/accent vary per
 *     preset: one neutral system, brand hue varies (the Linear model).
 *     Presets carry desaturated primaries tuned for ≥4.5:1 on the ramp.
 *   - LIGHT MODE: unchanged — dracula uses hand-tuned NEUTRAL_LIGHT,
 *     the other 4 presets derive tinted bases via deriveBaseLayer().
 *   - Extension-layer tokens (status, shadow) are per-mode in SUPPLEMENT.
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
    // dark restyle: desaturated indigo-violet (Linear-grade, ≈6:1 on #08090A)
    primary: { light: '#BD93F9', dark: '#7D82E8' },
    accent:  { light: '#FF79C6', dark: '#93A5FF' },
    bgBase:  { light: '#f8fafc', dark: '#08090A' },
  },
  paper: {
    label: '纸墨',
    // 主色即墨：浅色墨黑 → 主按钮黑底白字；
    // 深色纸白 → 主按钮纸白底墨字。品牌色让位于文字本身。
    primary: { light: '#1b1c1f', dark: '#f2f2f0' },
    accent:  { light: '#B5432A', dark: '#D08168' },  // 印章朱：墨与朱的克制点缀
    bgBase:  { light: '#f1f1ee', dark: '#191a1c' },
  },
  emerald: {
    label: '翡翠绿',
    primary: { light: '#059669', dark: '#4ADE9C' },
    accent:  { light: '#0e7490', dark: '#6FD8E8' },  // teal/cyan contrast
    bgBase:  { light: '#f1f6f3', dark: '#08090A' },
  },
  violet: {
    label: '紫罗兰',
    primary: { light: '#7c3aed', dark: '#A390F5' },
    accent:  { light: '#db2777', dark: '#F0A6D8' },  // magenta contrast
    bgBase:  { light: '#f5f2f8', dark: '#08090A' },
  },
  amber: {
    label: '琥珀橙',
    primary: { light: '#d97706', dark: '#F5C451' },
    accent:  { light: '#dc2626', dark: '#FF8F6B' },  // coral contrast
    bgBase:  { light: '#faf6f0', dark: '#08090A' },
  },
  rose: {
    label: '玫瑰红',
    primary: { light: '#e11d48', dark: '#FF8A9B' },
    accent:  { light: '#7c3aed', dark: '#B4A8FF' },  // violet contrast
    bgBase:  { light: '#f8f2f4', dark: '#08090A' },
  },
}

/** Ordered list for preset pickers. */
export const PRESET_LIST: { key: BuiltinPreset; label: string }[] = (
  Object.keys(PRESETS) as BuiltinPreset[]
).map((key) => ({ key, label: PRESETS[key].label }))

/* ============================================================
 * 2. Hand-tuned base palettes.
 *    LINEAR_DARK — the shared neutral near-black ramp EVERY preset uses
 *    in dark mode (canvas/surface/elevated/input + text ramp + 1px
 *    hairlines). Secondary text #A9AAB2 ≈ 8:1, tertiary #8A8B93 ≈ 5:1
 *    on the canvas — both clear the 4.5:1 floor.
 *    NEUTRAL_LIGHT — dracula's light anchor (light mode unchanged).
 * ============================================================ */
interface BaseLayer {
  bgBase: string
  bgSurface: string
  bgElevated: string
  bgInput: string
  textPrimary: string
  textSecondary: string
  textTertiary: string
  /** optional — buildPresetColors takes textDisabled from SUPPLEMENT */
  textDisabled?: string
  borderBase: string
  borderLight: string
}

const LINEAR_DARK: BaseLayer = {
  bgBase: '#08090A',
  bgSurface: '#0F1013',
  bgElevated: '#17181C',
  bgInput: '#0D0E11',
  textPrimary: '#E6E6EA',
  textSecondary: '#A9AAB2',
  textTertiary: '#8A8B93',
  textDisabled: '#5C5D66',
  borderBase: '#1C1D21',
  borderLight: '#17181C',
}

const NEUTRAL_LIGHT: BaseLayer = {
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
}

/* 纸墨（手工精调双模式）：浅色=暖灰画布 + 白纸面 +
 * 墨色文字；深色=暖炭夜纸（带暖灰相、不死黑）。它的个性就是材质本身
 * （纸与墨），因此深色不走 Linear 共享中性阶，两种模式都整套装载。
 * textDisabled 按同族灰阶补齐（暖灰相，不用全局冷灰）。 */
const PAPER_PALETTE: Record<ResolvedMode, BaseLayer> = {
  light: {
    bgBase: '#f1f1ee',
    bgSurface: '#ffffff',
    bgElevated: '#f8f8f7',
    bgInput: '#f3f3f1',
    textPrimary: '#1c1d21',
    textSecondary: '#5c6167',
    textTertiary: '#97999d',
    textDisabled: '#c6c5c1',
    borderBase: '#dcdcd9',
    borderLight: '#e9e9e7',
  },
  dark: {
    bgBase: '#191a1c',
    bgSurface: '#232527',
    bgElevated: '#282a2c',
    bgInput: '#2d2f31',
    textPrimary: '#ececee',
    textSecondary: '#a3a6aa',
    textTertiary: '#74777c',
    textDisabled: '#5b5e63',
    borderBase: '#3a3c3f',
    borderLight: '#2d2f31',
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
    textDisabled: '#5C5D66',
    primaryHover: '',
    primaryActive: '',
    primaryBorder: '',
    // Raycast-family status set: saturated enough to read as status,
    // quiet enough for the neutral near-black ramp
    success: '#59D499', successBg: 'rgba(89, 212, 153, 0.14)',
    warning: '#FFC531', warningBg: 'rgba(255, 197, 49, 0.14)',
    danger: '#FF6363', dangerBg: 'rgba(255, 99, 99, 0.14)',
    info: '#52A9FF', infoBg: 'rgba(82, 169, 255, 0.14)',
    // Linear-style elevation: one soft drop + a 1px light rim
    shadowMd: '0 4px 12px rgba(0, 0, 0, 0.45), 0 0 0 1px rgba(255, 255, 255, 0.04)',
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

/** Derive colorHover — neutral lift wash (Linear-style: lighten, not tint). */
export function deriveHover(_primary: string, mode: ResolvedMode): string {
  return mode === 'dark' ? 'rgba(255, 255, 255, 0.05)' : 'rgba(0, 0, 0, 0.04)'
}

/** Derive colorActive (slightly stronger neutral lift than hover). */
export function deriveActive(_primary: string, mode: ResolvedMode): string {
  return mode === 'dark' ? 'rgba(255, 255, 255, 0.08)' : 'rgba(0, 0, 0, 0.06)'
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

  // Base layer: 纸墨 carries both modes hand-tuned (its identity IS the
  // paper/ink material); dark mode otherwise shares the Linear neutral
  // ramp; light mode = dracula anchor / derived tints.
  const base: BaseLayer =
    preset === 'paper'
      ? PAPER_PALETTE[mode]
      : mode === 'dark'
        ? LINEAR_DARK
        : preset === 'dracula'
          ? NEUTRAL_LIGHT
          : deriveBaseLayer(seed.bgBase[mode], mode)

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
    textDisabled: base.textDisabled ?? supplement.textDisabled,
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
