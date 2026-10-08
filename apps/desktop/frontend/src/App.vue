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

const antTheme = computed(() => ({
  algorithm: themeStore.isDark ? theme.darkAlgorithm : theme.defaultAlgorithm,
  token: {
    colorPrimary: '#BD93F9',
    borderRadius: 8,
    colorBgContainer: themeStore.isDark ? '#1e293b' : '#ffffff',
    colorBgLayout: themeStore.isDark ? '#0f172a' : '#f8fafc',
    colorBorder: themeStore.isDark ? '#334155' : '#e2e8f0',
    colorText: themeStore.isDark ? '#f1f5f9' : '#0f172a',
    colorTextSecondary: themeStore.isDark ? '#94a3b8' : '#64748b',
    fontFamily: 'Inter, -apple-system, BlinkMacSystemFont, Segoe UI, Roboto, sans-serif',
  }
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
  themeStore.initializeTheme()
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
}

::-webkit-scrollbar-thumb:hover {
  background: var(--color-text-tertiary);
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
