/**
 * stores/theme.ts — Theme store (three-layer architecture).
 *
 * Ported from byte-stash (commit fae308a) and localised for byte-git.
 *
 * State layout:
 *   preset  : 'dracula' | 'emerald' | 'violet' | 'amber' | 'rose' | 'custom'
 *   mode    : 'auto' | 'light' | 'dark'
 *   custom  : { light: ThemeColors; dark: ThemeColors }   (used when preset='custom')
 *   regions : { light: RegionOverrides; dark: RegionOverrides }
 *
 * Persistence: pure localStorage key 'byte-git-theme' (versioned). Migrates
 * the legacy 'byte-git-theme' string ('dark'/'light') transparently.
 *
 * Public API (kept compatible with the legacy store so existing consumers
 * like Navbar.vue / DisplayForm.vue / terminal keep working):
 *   - currentTheme : 'dark' | 'light'           (legacy alias of resolvedMode)
 *   - systemTheme  : 'dark' | 'light'
 *   - isDark       : boolean
 *   - setTheme(theme)                            (legacy: theme='dark'/'light')
 *   - toggleTheme()
 *   - initializeTheme()
 *
 * New API (for AppearanceForm.vue):
 *   - preset, mode, custom, regions (state)
 *   - resolvedMode, palette, antdTheme (getters)
 *   - setPreset, setMode, setCustomToken, setRegionToken, apply (actions)
 */

import { defineStore } from 'pinia'
import { ref, computed, watch } from 'vue'
import {
  buildPresetColors,
  defaultCustom,
  recomputeDerived,
} from '@/themes/presets'
import { applyColorsToDOM, buildAntdToken } from '@/themes/token'
import type {
  PresetKey,
  Mode,
  ResolvedMode,
  ThemeColors,
  RegionOverrides,
  ThemeState,
} from '@/themes/types'

const THEME_KEY = 'byte-git-theme'
const THEME_STATE_VERSION = 1

/* ---------- helpers ---------- */

function resolveMode(mode: Mode): ResolvedMode {
  if (mode !== 'auto') return mode
  if (typeof window !== 'undefined' && window.matchMedia) {
    return window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light'
  }
  return 'dark'
}

function systemMode(): ResolvedMode {
  if (typeof window !== 'undefined' && window.matchMedia) {
    return window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light'
  }
  return 'dark'
}

/* ---------- store ---------- */

export const useThemeStore = defineStore('theme', () => {
  /* ===== state ===== */
  const preset = ref<PresetKey>('dracula')
  const mode = ref<Mode>('dark')
  const custom = ref<{ light: ThemeColors; dark: ThemeColors }>(defaultCustom())
  const regions = ref<{ light: RegionOverrides; dark: RegionOverrides }>({ light: {}, dark: {} })

  /* ===== getters ===== */
  const resolvedMode = computed<ResolvedMode>(() => resolveMode(mode.value))
  const isDark = computed(() => resolvedMode.value === 'dark')

  /** Legacy alias — old code reads themeStore.currentTheme as 'dark'/'light'. */
  const currentTheme = computed<'dark' | 'light'>(() => resolvedMode.value)
  /** Legacy alias — mirrors the system mode (resolved). */
  const systemTheme = computed<'dark' | 'light'>(() => systemMode())

  /** The currently active palette (built from preset or custom). */
  const palette = computed<ThemeColors>(() => {
    const m = resolvedMode.value
    if (preset.value === 'custom') return custom.value[m]
    return buildPresetColors(preset.value, m)
  })

  /** antd ConfigProvider token (algorithm is filled by caller — needs antd import). */
  const antdToken = computed(() => buildAntdToken(palette.value, resolvedMode.value))

  /* ===== persistence ===== */

  function persist() {
    const state: ThemeState = {
      version: THEME_STATE_VERSION,
      preset: preset.value,
      mode: mode.value,
      custom: custom.value,
      regions: regions.value,
    }
    try {
      localStorage.setItem(THEME_KEY, JSON.stringify(state))
    } catch (e) {
      console.warn('[theme] persist failed', e)
    }
  }

  /**
   * Load + migrate. Accepts three input shapes:
   *   1. legacy string ('dark' / 'light') from older byte-git builds
   *   2. new ThemeState object with version
   *   3. anything else / broken JSON → reset to defaults
   */
  function loadAndMigrate(): void {
    let raw: string | null = null
    try {
      raw = localStorage.getItem(THEME_KEY)
    } catch {
      raw = null
    }
    if (!raw) {
      // Default: dracula / dark. Persist so the inline FOUC script can read it.
      persist()
      return
    }

    // Case 1: legacy plain string.
    if (raw === 'dark' || raw === 'light') {
      preset.value = 'dracula'
      mode.value = raw
      persist()
      return
    }

    // Case 2: JSON state.
    try {
      const parsed = JSON.parse(raw) as Partial<ThemeState>
      if (parsed && typeof parsed === 'object') {
        if (typeof parsed.preset === 'string') preset.value = parsed.preset as PresetKey
        if (typeof parsed.mode === 'string') mode.value = parsed.mode as Mode
        if (parsed.custom && parsed.custom.light && parsed.custom.dark) {
          custom.value = parsed.custom as { light: ThemeColors; dark: ThemeColors }
        }
        if (parsed.regions) {
          regions.value = parsed.regions as { light: RegionOverrides; dark: RegionOverrides }
        }
        // Re-persist to bump version if it was older.
        if (parsed.version !== THEME_STATE_VERSION) persist()
        return
      }
    } catch (e) {
      console.warn('[theme] state JSON broken, resetting', e)
    }

    // Case 3: fallback.
    preset.value = 'dracula'
    mode.value = 'dark'
    persist()
  }

  /* ===== apply ===== */

  function apply() {
    applyColorsToDOM(palette.value, resolvedMode.value, regions.value[resolvedMode.value])
  }

  /* ===== actions ===== */

  function setPreset(k: PresetKey) {
    preset.value = k
    // Switching to a builtin preset should show that preset's pure official
    // palette — clear any per-region overrides the user tuned while in custom
    // mode, otherwise the old region colours bleed through and the preset
    // looks half-applied. Custom mode keeps regions (they're the whole point).
    if (k !== 'custom') {
      regions.value = { light: {}, dark: {} }
    }
    apply()
    persist()
  }

  function setMode(m: Mode) {
    mode.value = m
    apply()
    persist()
  }

  /**
   * Legacy setTheme — accepts 'dark' or 'light'. Maps to mode directly so the
   * old DisplayForm.vue / Navbar.vue callers keep working.
   */
  function setTheme(theme: string) {
    if (theme === 'dark' || theme === 'light') {
      setMode(theme)
    }
  }

  function toggleTheme() {
    setMode(isDark.value ? 'light' : 'dark')
  }

  /**
   * Edit a custom-palette token. Recomputes derived fields when `primary`
   * changes. Pass liveApply=true to skip persist (dragging) — caller must
   * persist() on release.
   *
   * Key can be any ThemeColors field (seed tokens AND derived/status tokens)
   * so the UI can expose the full byte-stash-compatible editor.
   */
  function setCustomToken(
    m: ResolvedMode,
    key: keyof ThemeColors,
    val: string,
    liveApply = false,
  ) {
    custom.value[m] = { ...custom.value[m], [key]: val }
    // primary and accent are seeds — recompute their derived followers so
    // (e.g. dragging accent) keeps accentBg in sync.
    if (key === 'primary' || key === 'accent') {
      custom.value[m] = recomputeDerived(custom.value[m], m)
    }
    if (preset.value === 'custom' && m === resolvedMode.value) apply()
    if (!liveApply) persist()
  }

  /**
   * Batch-replace one mode's custom palette. Used by the visual editor's
   * "apply on commit" path and by JSON/import flows.
   */
  function setCustomColors(m: ResolvedMode, colors: ThemeColors) {
    custom.value = { ...custom.value, [m]: colors }
    if (preset.value === 'custom' && m === resolvedMode.value) apply()
    persist()
  }

  /** Read a region override value (returns undefined when not set). */
  function regionValue(
    m: ResolvedMode,
    region: keyof RegionOverrides,
    prop: 'bg' | 'text' | 'textSub',
  ): string | undefined {
    return regions.value[m]?.[region]?.[prop as 'bg' | 'text' | 'textSub']
  }

  /** Set a region token override (or undefined to clear). */
  function setRegionToken(
    m: ResolvedMode,
    region: keyof RegionOverrides,
    prop: 'bg' | 'text' | 'textSub',
    val: string | undefined,
  ) {
    const cur = regions.value[m][region] ?? {}
    regions.value = {
      ...regions.value,
      [m]: { ...regions.value[m], [region]: { ...cur, [prop]: val } },
    }
    if (m === resolvedMode.value) apply()
    persist()
  }

  /* ===== JSON export/import (full state round-trip) ===== */

  /** Serialise the full theme state (preset + mode + custom + regions). */
  function exportState(): ThemeState {
    return {
      version: THEME_STATE_VERSION,
      preset: preset.value,
      mode: mode.value,
      custom: custom.value,
      regions: regions.value,
    }
  }

  /**
   * Apply a JSON string (same shape as exportState). Returns {ok} or {error}.
   * Tolerant: accepts a bare {light, dark} custom-only object too.
   */
  function applyRawState(raw: string): { ok: boolean; error?: string } {
    try {
      const parsed = JSON.parse(raw)
      if (!parsed || typeof parsed !== 'object') {
        return { ok: false, error: 'JSON 不是对象' }
      }
      // Full state shape
      if (parsed.custom?.light && parsed.custom?.dark) {
        if (typeof parsed.preset === 'string') preset.value = parsed.preset as PresetKey
        if (typeof parsed.mode === 'string') mode.value = parsed.mode as Mode
        custom.value = parsed.custom as { light: ThemeColors; dark: ThemeColors }
        if (parsed.regions) regions.value = parsed.regions as { light: RegionOverrides; dark: RegionOverrides }
      } else if (parsed.light && parsed.dark) {
        // Bare custom-only shape
        custom.value = parsed as { light: ThemeColors; dark: ThemeColors }
        preset.value = 'custom'
      } else {
        return { ok: false, error: '缺少 custom.light / custom.dark 字段' }
      }
      apply()
      persist()
      return { ok: true }
    } catch (e: any) {
      return { ok: false, error: 'JSON 解析失败：' + (e?.message || String(e)) }
    }
  }

  /* ===== initialization ===== */

  let mediaListener: ((e: MediaQueryListEvent) => void) | null = null

  function initializeTheme() {
    loadAndMigrate()
    apply()

    // Register a prefers-color-scheme listener — only re-applies when in auto.
    if (typeof window !== 'undefined' && window.matchMedia) {
      const mq = window.matchMedia('(prefers-color-scheme: dark)')
      mediaListener = (e: MediaQueryListEvent) => {
        if (mode.value === 'auto') apply()
      }
      // addEventListener is the modern API; addListener is the deprecated fallback.
      if (typeof mq.addEventListener === 'function') {
        mq.addEventListener('change', mediaListener)
      } else if (typeof (mq as any).addListener === 'function') {
        ;(mq as any).addListener(mediaListener)
      }
    }
  }

  /* ===== reactive persist (belt-and-suspenders for direct state writes) ===== */
  // Note: explicit actions already persist. This watch is a safety net for
  // any future code that mutates state directly.
  watch([preset, mode], () => persist())

  return {
    // state
    preset,
    mode,
    custom,
    regions,
    // getters
    resolvedMode,
    isDark,
    currentTheme,
    systemTheme,
    palette,
    antdToken,
    // actions
    apply,
    setPreset,
    setMode,
    setTheme,
    toggleTheme,
    setCustomToken,
    setCustomColors,
    regionValue,
    setRegionToken,
    exportState,
    applyRawState,
    initializeTheme,
    persist,
    loadAndMigrate,
  }
})
