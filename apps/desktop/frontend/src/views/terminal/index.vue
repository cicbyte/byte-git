<script setup>
import { ref, onMounted, onBeforeUnmount, onActivated, onDeactivated, nextTick, watch, computed, h } from 'vue'
import { message as AMessage } from 'ant-design-vue'
import {
  ReloadOutlined, ExclamationCircleOutlined,
  InfoCircleOutlined,
} from '@ant-design/icons-vue'
import { Terminal } from '@xterm/xterm'
import { FitAddon } from '@xterm/addon-fit'
import '@xterm/xterm/css/xterm.css'
import { EventsOn, EventsOff } from '../../../wailsjs/runtime/runtime'
import { useRepoStore } from '../../stores/repo'
import { useThemeStore } from '../../stores/theme'
import { usePageFooter } from '../../composables/usePageFooter'
import TerminalFooter from '../shared/TerminalFooter.vue'

/**
 * 内嵌终端 — xterm.js + 后端 PTY（Windows ConPTY / Unix pty）。
 *
 * shell 信息 + 连接状态 + 操作全部注入全局 footer，终端获得最大空间。
 * 被 <keep-alive> 缓存——切走时 PTY 保持存活，切回来恢复会话。
 */
// name must match keep-alive :include in MainContent
defineOptions({ name: 'TerminalView' })

const app = window.go?.main?.ByteGitApp
const repo = useRepoStore()
const theme = useThemeStore()
const { setFooter, clearFooter } = usePageFooter()

const hostRef = ref(null)
const shellInfo = ref({ path: '', isGitBash: false })
const status = ref('idle') // 'idle' | 'connecting' | 'connected' | 'exited'
const errorMsg = ref('')

let term = null
let fitAddon = null
let resizeObserver = null

// ---- xterm theme palettes (aligned with project's dark/light vars) ----
const darkTheme = {
  background: '#0f172a',           // project --color-background (dark)
  foreground: '#e2e8f0',           // --color-text
  cursor: '#bd93f9',              // --color-primary
  cursorAccent: '#0f172a',
  selectionBackground: 'rgba(189, 147, 249, 0.25)',
  black: '#1e293b',               // --color-surface
  red: '#ff5555',                 // --color-error
  green: '#50fa7b',               // --color-success
  yellow: '#ffb86c',              // --color-warning
  blue: '#8be9fd',                // --color-info
  magenta: '#ff79c6',             // --color-accent
  cyan: '#8be9fd',
  white: '#f1f5f9',
  brightBlack: '#64748b',         // --color-text-secondary
  brightRed: '#ff6e6e',
  brightGreen: '#69ff94',
  brightYellow: '#ffcb6b',
  brightBlue: '#a4e9fd',
  brightMagenta: '#ff92e0',
  brightCyan: '#a4e9fd',
  brightWhite: '#ffffff',
}
const lightTheme = {
  background: '#f8fafc',           // --color-background (light)
  foreground: '#0f172a',           // --color-text
  cursor: '#7c3aed',
  cursorAccent: '#f8fafc',
  selectionBackground: 'rgba(124, 58, 237, 0.15)',
  black: '#0f172a',
  red: '#dc2626',
  green: '#16a34a',
  yellow: '#d97706',
  blue: '#0e7490',
  magenta: '#db2777',
  cyan: '#0e7490',
  white: '#f8fafc',
  brightBlack: '#64748b',
  brightRed: '#dc2626',
  brightGreen: '#16a34a',
  brightYellow: '#d97706',
  brightBlue: '#0284c7',
  brightMagenta: '#db2777',
  brightCyan: '#0284c7',
  brightWhite: '#ffffff',
}

function currentTheme() {
  return theme.isDark ? darkTheme : lightTheme
}

// ---- base64 helpers (UTF-8 safe) ----
function b64encode(str) {
  const bytes = new TextEncoder().encode(str)
  let binary = ''
  bytes.forEach((b) => { binary += String.fromCharCode(b) })
  return btoa(binary)
}
function b64decode(b64) {
  const binary = atob(b64)
  const bytes = new Uint8Array(binary.length)
  for (let i = 0; i < binary.length; i++) bytes[i] = binary.charCodeAt(i)
  return new TextDecoder().decode(bytes)
}

// ---- terminal lifecycle ----
async function startTerminal() {
  if (!term || !hostRef.value) return
  status.value = 'connecting'
  errorMsg.value = ''

  try {
    shellInfo.value = await app.TerminalDetectShell()
  } catch (e) {
    errorMsg.value = String(e?.message || e)
    status.value = 'idle'
    return
  }

  const cwd = repo.path || ''
  const { cols, rows } = term
  try {
    await app.TerminalStart('', cwd, cols, rows)
    status.value = 'connected'
    term.focus()
  } catch (e) {
    errorMsg.value = String(e?.message || e)
    status.value = 'idle'
  }
}

async function restart() {
  if (app?.TerminalStop) {
    try { await app.TerminalStop() } catch {}
  }
  term?.clear()
  await startTerminal()
}

function clearScreen() {
  term?.clear()
  term?.focus()
}

// ---- xterm event handlers ----
function onDataHandler(data) {
  if (!app?.TerminalInput) return
  app.TerminalInput(b64encode(data)).catch(() => {})
}
function onResizeHandler({ cols, rows }) {
  if (!app?.TerminalResize) return
  app.TerminalResize(cols, rows).catch(() => {})
}
function onTermData(ev) {
  if (ev?.data && term) term.write(b64decode(ev.data))
}
function onTermExit() {
  status.value = 'exited'
}

onMounted(async () => {
  await nextTick()
  term = new Terminal({
    cursorBlink: true,
    cursorStyle: 'bar',
    fontSize: 13,
    fontFamily: "'JetBrains Mono', 'Cascadia Code', 'Fira Code', 'Consolas', monospace",
    theme: currentTheme(),
    allowProposedApi: true,
    scrollback: 5000,
    smoothScrollDuration: 80,
  })
  fitAddon = new FitAddon()
  term.loadAddon(fitAddon)
  term.open(hostRef.value)
  fitAddon.fit()

  term.onData(onDataHandler)
  term.onResize(onResizeHandler)

  EventsOn('term:data', onTermData)
  EventsOn('term:exit', onTermExit)

  resizeObserver = new ResizeObserver(() => {
    if (!fitAddon || !term) return
    try { fitAddon.fit() } catch {}
  })
  resizeObserver.observe(hostRef.value)

  await startTerminal()
})

// keep-alive: when the user navigates away, keep the PTY alive (don't stop it).
// Just clear the footer so other pages can use it.
onDeactivated(() => {
  clearFooter()
})

// keep-alive: when the user comes back, re-fit the terminal (it may have been
// hidden when the container resized) and re-inject the footer.
onActivated(() => {
  // re-inject footer
  setFooter(TerminalFooter, () => ({
    shellLabel: shellLabel.value,
    shellPath: shellInfo.value.path,
    repoName: repo.name,
    status: status.value,
    onClear: () => clearScreen(),
    onRestart: () => restart(),
  }))
  // refit after the DOM is visible again
  nextTick(() => {
    if (fitAddon) { try { fitAddon.fit() } catch {} }
    term?.focus()
  })
})

onBeforeUnmount(() => {
  // true unmount (app exit / navigation from cached) — stop the PTY for real
  EventsOff('term:data', onTermData)
  EventsOff('term:exit', onTermExit)
  if (resizeObserver) { resizeObserver.disconnect(); resizeObserver = null }
  if (app?.TerminalStop) { app.TerminalStop().catch(() => {}) }
  if (term) { term.dispose(); term = null }
  clearFooter()
})

// theme switching
watch(() => theme.isDark, () => {
  if (term) term.options.theme = currentTheme()
})

// repo switch — the view is keep-alive cached, so a project switch won't
// remount it. Restart the PTY so the shell's cwd follows the new repo.
watch(() => repo.repoEpoch, () => {
  if (status.value === 'connected' || status.value === 'exited') restart()
})

// derived state
const statusLabel = {
  idle: '未连接',
  connecting: '连接中',
  connected: '已连接',
  exited: '已断开',
}
const shellLabel = computed(() => {
  if (!shellInfo.value.path) return '检测中…'
  return shellInfo.value.isGitBash ? 'Git Bash' : shellInfo.value.path.split(/[/\\]/).pop()
})

// inject shell info + status + actions into the global footer
watch(
  () => [status.value, shellInfo.value, repo.name],
  () => {
    setFooter(TerminalFooter, () => ({
      shellLabel: shellLabel.value,
      shellPath: shellInfo.value.path,
      repoName: repo.name,
      status: status.value,
      onClear: () => clearScreen(),
      onRestart: () => restart(),
    }))
  },
  { immediate: true },
)
</script>

<template>
  <div class="term-view" :class="{ 'is-dark': theme.isDark }">
    <!-- terminal host -->
    <div class="term-host-wrap">
      <!-- error overlay -->
      <div v-if="errorMsg && status !== 'connected'" class="term-error-overlay">
        <ExclamationCircleOutlined class="teo-icon" />
        <div class="teo-title">无法启动终端</div>
        <div class="teo-msg">{{ errorMsg }}</div>
        <div class="teo-hint">
          <InfoCircleOutlined /> 请检查 Shell 路径是否正确
         （设置 → 终端），或确认 git-bash 已安装。
        </div>
        <a-button size="small" type="primary" @click="restart">
          <template #icon><ReloadOutlined /></template>重试
        </a-button>
      </div>

      <!-- xterm mounts here -->
      <div class="term-host" ref="hostRef"></div>
    </div>
  </div>
</template>

<style scoped>
.term-view {
  width: 100%; height: 100%;
  background: #0f172a;
  min-height: 0;
}
.term-view:not(.is-dark) { background: #f8fafc; }

/* ---- terminal host ---- */
.term-host-wrap { flex: 1; min-height: 0; position: relative; overflow: hidden; }
.term-host { width: 100%; height: 100%; }
/* force all xterm internal layers transparent so the terminal theme bg
   (set via Terminal options) fills uniformly — without this, .xterm-viewport
   / .xterm-screen can show a default dark strip at the bottom when rows
   don't perfectly divide the container height. */
.term-host :deep(.xterm) { height: 100%; padding: 4px 8px; }
.term-host :deep(.xterm .xterm-viewport),
.term-host :deep(.xterm .xterm-screen) { background-color: transparent !important; }
.term-host :deep(.xterm-viewport) {
  overflow-y: auto;
  scrollbar-width: thin;
}
.term-host :deep(.xterm-viewport::-webkit-scrollbar) { width: 6px; }
.term-host :deep(.xterm-viewport::-webkit-scrollbar-thumb) {
  background: rgba(148, 163, 184, 0.3); border-radius: 3px;
}
.term-host :deep(.xterm-viewport::-webkit-scrollbar-track) { background: transparent; }

/* ---- error overlay ---- */
.term-error-overlay {
  position: absolute; inset: 0; z-index: 10;
  display: flex; flex-direction: column; align-items: center; justify-content: center;
  gap: 10px; padding: 40px;
  background: rgba(15, 23, 42, 0.92);
  backdrop-filter: blur(4px);
}
.term-view:not(.is-dark) .term-error-overlay {
  background: rgba(248, 250, 252, 0.95);
}
.teo-icon { font-size: 40px; color: #ff6e6e; opacity: 0.7; }
.term-view:not(.is-dark) .teo-icon { color: var(--color-error); }
.teo-title { font-size: 15px; font-weight: 600; color: #e2e8f0; }
.term-view:not(.is-dark) .teo-title { color: var(--color-text); }
.teo-msg {
  font-size: 12px; color: #94a3b8; max-width: 400px; text-align: center;
  word-break: break-word; line-height: 1.6;
}
.term-view:not(.is-dark) .teo-msg { color: var(--color-text-secondary); }
.teo-hint {
  font-size: 11px; color: #64748b; display: flex; align-items: center; gap: 4px;
  margin-bottom: 4px;
}
.term-view:not(.is-dark) .teo-hint { color: var(--color-text-tertiary); }
</style>
