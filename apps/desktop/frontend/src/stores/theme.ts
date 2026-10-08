import { defineStore } from 'pinia'
import { ref, computed, watch } from 'vue'

export const useThemeStore = defineStore('theme', () => {
  const currentTheme = ref('dark')
  const systemTheme = ref('dark')

  const isDark = computed(() => currentTheme.value === 'dark')

  function setTheme(theme: string) {
    currentTheme.value = theme
    document.documentElement.setAttribute('data-theme', theme)
    localStorage.setItem('byte-git-theme', theme)
  }

  function toggleTheme() {
    setTheme(isDark.value ? 'light' : 'dark')
  }

  function detectSystemTheme() {
    if (window.matchMedia('(prefers-color-scheme: dark)').matches) {
      systemTheme.value = 'dark'
    } else {
      systemTheme.value = 'light'
    }
  }

  function initializeTheme() {
    detectSystemTheme()
    const saved = localStorage.getItem('byte-git-theme')
    if (saved) {
      setTheme(saved)
    } else {
      setTheme(systemTheme.value)
    }

    window.matchMedia('(prefers-color-scheme: dark)').addEventListener('change', (e) => {
      systemTheme.value = e.matches ? 'dark' : 'light'
      const savedTheme = localStorage.getItem('byte-git-theme')
      if (!savedTheme) {
        setTheme(systemTheme.value)
      }
    })
  }

  return { currentTheme, systemTheme, isDark, setTheme, toggleTheme, initializeTheme }
})
