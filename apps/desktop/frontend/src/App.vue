<script setup>
import { computed, onMounted, onBeforeUnmount, ref } from 'vue'
import { theme } from 'ant-design-vue'
import zhCN from 'ant-design-vue/es/locale/zh_CN'
import dayjs from 'dayjs'
import 'dayjs/locale/zh-cn'
import { useThemeStore } from './stores/theme'
import { useLayoutStore } from './stores/layout'
import AppLayout from './components/layout/AppLayout.vue'
import CommandPalette from './components/common/CommandPalette.vue'

// Set dayjs locale so date pickers/formatters (e.g. RangePicker placeholders,
// "Today" preset labels) show Chinese text.
dayjs.locale('zh-cn')

const themeStore = useThemeStore()
const layoutStore = useLayoutStore()

// antd ConfigProvider theme — derived from the central palette so it follows
// preset / custom / mode changes automatically. Algorithm is filled here
// (token.ts intentionally stays free of antd imports).
const antTheme = computed(() => ({
  algorithm: themeStore.isDark ? theme.darkAlgorithm : theme.defaultAlgorithm,
  token: themeStore.antdToken.token,
}))

// ---- Command Palette (Ctrl+K) ----
const paletteOpen = ref(false)
function onGlobalKeydown(e) {
  if ((e.ctrlKey || e.metaKey) && e.key === 'k') {
    e.preventDefault()
    paletteOpen.value = !paletteOpen.value
  }
}

onMounted(() => {
  // themeStore.initializeTheme() is already called from main.ts BEFORE mount
  // (FOUC defence line #3) — no need to re-init here.
  window.addEventListener('resize', layoutStore.handleResize)
  window.addEventListener('keydown', onGlobalKeydown)
})
onBeforeUnmount(() => {
  window.removeEventListener('keydown', onGlobalKeydown)
})
</script>

<template>
  <a-config-provider :theme="antTheme" :locale="zhCN">
    <AppLayout />
    <CommandPalette v-model:open="paletteOpen" />
  </a-config-provider>
</template>

<style>
* {
  margin: 0;
  padding: 0;
  box-sizing: border-box;
}

html, body, #app {
  height: 100vh;
  width: 100vw;
  overflow: hidden;
  background-color: var(--color-background);
  color: var(--color-text);
  font-family: Inter, -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
  -webkit-font-smoothing: antialiased;
}

::-webkit-scrollbar {
  width: 8px;
  height: 8px;
}

::-webkit-scrollbar-track {
  background: transparent;
}

::-webkit-scrollbar-thumb {
  background: var(--color-border);
  border-radius: 4px;
  /* 2px 透明边 + content-box 裁切：同为 8px 轨道，视觉滑块更纤细 */
  border: 2px solid transparent;
  background-clip: content-box;
}

::-webkit-scrollbar-thumb:hover {
  background: var(--color-text-tertiary);
  border: 2px solid transparent;
  background-clip: content-box;
}

::selection {
  background: var(--color-primary);
  color: white;
}

:focus-visible {
  outline: 2px solid var(--color-primary);
  outline-offset: 2px;
}
</style>
