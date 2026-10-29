/**
 * stores/wallpaper.ts — Wallpaper state machine.
 *
 * Architecture:
 *   - Wallpaper FILES are managed by the Go backend (WallpaperUpload/List/
 *     Delete + WallpaperDataDir) at ~/.cicbyte/apps/byte-git/wallpapers/.
 *   - Wallpaper PREFERENCES (enabled / activeName / blur / mask / panelAlpha)
 *     live in localStorage 'byte-git-wallpaper' (pure UI state).
 *   - Rendering uses TWO absolute-positioned layers in AppLayout.vue
 *     (.wp-layer image + .wp-mask color wash). Panels become translucent
 *     via derivation.css's color-mix(--bg-*-orig, transparent --wp-panel-alpha).
 *
 * Image source strategy:
 *   Wallpapers are converted to base64 data URLs on the Go side
 *   (WallpaperDataURL), which works universally in <img> / background-image
 *   without any CSP configuration.
 */

import { defineStore } from 'pinia'
import { ref, computed, watch } from 'vue'
import { useThemeStore } from './theme'

const LS_KEY = 'byte-git-wallpaper'
const LS_VERSION = 1

interface WallpaperState {
  version: number
  enabled: boolean
  activeName: string | null
  blur: number        // 0~40 px (wallpaper layer blur)
  mask: number        // 0~0.8 (overlay alpha)
  panelAlpha: number  // 0~0.7 (content/card bg alpha → --wp-panel-alpha)
  navAlpha: number    // 0~0.6 (rail/sider/navbar/footer alpha → --wp-panel-alpha-nav)
}

/* ---------- Wails binding access ---------- */
function app(): any | null {
  return (window as any).go?.main?.ByteGitApp ?? null
}

/* ---------- persistence ---------- */
function loadPrefs(): WallpaperState {
  const fallback: WallpaperState = {
    version: LS_VERSION,
    enabled: false,
    activeName: null,
    blur: 0,
    mask: 0.3,
    panelAlpha: 0.4,
    navAlpha: 0.24,
  }
  try {
    const raw = localStorage.getItem(LS_KEY)
    if (!raw) return fallback
    const parsed = JSON.parse(raw) as Partial<WallpaperState>
    return {
      version: LS_VERSION,
      enabled: typeof parsed.enabled === 'boolean' ? parsed.enabled : false,
      activeName: typeof parsed.activeName === 'string' ? parsed.activeName : null,
      blur: clamp(typeof parsed.blur === 'number' ? parsed.blur : 0, 0, 40),
      mask: clamp(typeof parsed.mask === 'number' ? parsed.mask : 0.3, 0, 0.8),
      panelAlpha: clamp(typeof parsed.panelAlpha === 'number' ? parsed.panelAlpha : 0.4, 0, 0.7),
      navAlpha: clamp(typeof parsed.navAlpha === 'number' ? parsed.navAlpha : 0.24, 0, 0.6),
    }
  } catch {
    return fallback
  }
}

function persist(state: WallpaperState) {
  try {
    localStorage.setItem(LS_KEY, JSON.stringify(state))
  } catch (e) {
    console.warn('[wallpaper] persist failed', e)
  }
}

function clamp(v: number, lo: number, hi: number): number {
  return Math.max(lo, Math.min(hi, v))
}

/* ---------- store ---------- */

export const useWallpaperStore = defineStore('wallpaper', () => {
  const theme = useThemeStore()

  /* ===== state ===== */
  const enabled = ref<boolean>(false)
  const activeName = ref<string | null>(null)
  const blur = ref<number>(0)
  const mask = ref<number>(0.3)
  const panelAlpha = ref<number>(0.4)
  const navAlpha = ref<number>(0.24)

  /** Wall-paper library filenames (from Go WallpaperList). */
  const list = ref<string[]>([])
  /** Base64 data URL cache for the active wallpaper (avoids re-fetching). */
  const activeDataUrl = ref<string>('')

  /* ===== getters ===== */

  /** True when there's a usable wallpaper to show. */
  const hasWallpaper = computed(() => !!activeDataUrl.value)

  /** The URL to bind on .wp-layer's background-image. */
  const url = computed(() => activeDataUrl.value)

  /** Mask colour — black wash in dark mode, white wash in light mode. */
  const maskColor = computed(() => {
    const rgb = theme.resolvedMode === 'dark' ? '0,0,0' : '255,255,255'
    return `rgba(${rgb}, ${mask.value})`
  })

  /* ===== apply ===== */

  /**
   * Inject wallpaper CSS variables into :root + set data-wallpaper.
   * When off, clears --wp-panel-alpha to 0% so derivation.css's color-mix
   * makes every panel fully opaque again.
   *
   * Two alphas are injected:
   *   --wp-panel-alpha       — for content/card (user-controlled, up to 70%)
   *   --wp-panel-alpha-nav   — for rail/sider/navbar/footer (navigation regions
   *                            with dense text on narrow strips). Capped at a
   *                            fraction of panel-alpha so nav text stays legible
   *                            even with a busy wallpaper behind it.
   */
  function apply() {
    const root = document.documentElement
    const on = enabled.value && !!activeDataUrl.value
    if (on) {
      root.style.setProperty('--wp-blur', `${blur.value}px`)
      root.style.setProperty('--wp-panel-alpha', `${Math.round(panelAlpha.value * 100)}%`)
      // Nav regions use the user-controlled navAlpha (independent slider),
      // capped at 0.6 to keep navigation text readable even at the max.
      root.style.setProperty('--wp-panel-alpha-nav', `${Math.round(Math.min(navAlpha.value, 0.6) * 100)}%`)
      root.setAttribute('data-wallpaper', 'on')
    } else {
      root.style.setProperty('--wp-blur', '0px')
      root.style.setProperty('--wp-panel-alpha', '0%')
      root.style.setProperty('--wp-panel-alpha-nav', '0%')
      root.setAttribute('data-wallpaper', 'off')
    }
  }

  /* ===== setters ===== */

  function setEnabled(v: boolean) {
    enabled.value = v
  }

  function setActive(name: string | null) {
    activeName.value = name
    if (name) enabled.value = true
    refreshActiveDataUrl()
  }

  function setBlur(v: number) { blur.value = clamp(v, 0, 40) }
  function setMask(v: number) { mask.value = clamp(v, 0, 0.8) }
  function setPanelAlpha(v: number) { panelAlpha.value = clamp(v, 0, 0.7) }
  function setNavAlpha(v: number) { navAlpha.value = clamp(v, 0, 0.6) }

  function clear() {
    activeName.value = null
    activeDataUrl.value = ''
    apply()
  }

  /* ===== backend ops ===== */

  /** Refresh the wallpaper library list from the Go backend. */
  async function refreshList() {
    const a = app()
    if (!a?.WallpaperList) {
      list.value = []
      return
    }
    try {
      list.value = (await a.WallpaperList()) as string[]
    } catch (e) {
      console.warn('[wallpaper] refreshList failed', e)
      list.value = []
    }
  }

  /**
   * Fetch the active wallpaper as a base64 data URL and re-apply.
   * Called whenever activeName changes (or on init if one is set).
   */
  async function refreshActiveDataUrl() {
    const name = activeName.value
    if (!name) {
      activeDataUrl.value = ''
      apply()
      return
    }
    const a = app()
    if (!a?.WallpaperDataURL) {
      activeDataUrl.value = ''
      apply()
      return
    }
    try {
      activeDataUrl.value = (await a.WallpaperDataURL(name)) as string
    } catch (e) {
      console.warn('[wallpaper] load active data url failed', e)
      activeDataUrl.value = ''
    }
    apply()
  }

  /* ===== init ===== */

  async function init() {
    // 1. load prefs
    const s = loadPrefs()
    enabled.value = s.enabled
    activeName.value = s.activeName
    blur.value = s.blur
    mask.value = s.mask
    panelAlpha.value = s.panelAlpha

    // 2. fetch library list
    await refreshList()

    // 3. validate activeName — clear if the file was deleted externally
    if (activeName.value && !list.value.includes(activeName.value)) {
      activeName.value = null
    }

    // 4. fetch active image data + apply
    await refreshActiveDataUrl()
  }

  /* ===== reactive persistence (debounced) ===== */
  let persistTimer: ReturnType<typeof setTimeout> | null = null
  watch([enabled, activeName, blur, mask, panelAlpha, navAlpha], () => {
    apply()  // immediate visual feedback while dragging sliders
    if (persistTimer) clearTimeout(persistTimer)
    persistTimer = setTimeout(() => {
      persist({
        version: LS_VERSION,
        enabled: enabled.value,
        activeName: activeName.value,
        blur: blur.value,
        mask: mask.value,
        panelAlpha: panelAlpha.value,
        navAlpha: navAlpha.value,
      })
    }, 300)
  })

  return {
    // state
    enabled,
    activeName,
    blur,
    mask,
    panelAlpha,
    navAlpha,
    list,
    activeDataUrl,
    // getters
    hasWallpaper,
    url,
    maskColor,
    // actions
    apply,
    setEnabled,
    setActive,
    setBlur,
    setMask,
    setPanelAlpha,
    setNavAlpha,
    clear,
    refreshList,
    refreshActiveDataUrl,
    init,
  }
})
