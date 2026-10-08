import { defineStore } from 'pinia'
import { ref, computed } from 'vue'

export const useLayoutStore = defineStore('layout', () => {
  // read persisted display preferences on first load (set by DisplayForm)
  const sidebarCollapsed = ref(localStorage.getItem('byte-git.sidebarDefaultCollapsed') === 'true')
  const projectRailVisible = ref(localStorage.getItem('byte-git.projectRailDefault') !== 'false')
  const windowWidth = ref(window.innerWidth)

  const isMobile = computed(() => windowWidth.value < 768)
  const isTablet = computed(() => windowWidth.value >= 768 && windowWidth.value < 1024)

  function toggleSidebar() {
    sidebarCollapsed.value = !sidebarCollapsed.value
  }

  function toggleProjectRail() {
    projectRailVisible.value = !projectRailVisible.value
  }

  function handleResize() {
    windowWidth.value = window.innerWidth
    if (isMobile.value) {
      sidebarCollapsed.value = true
    }
  }

  return { sidebarCollapsed, projectRailVisible, windowWidth, isMobile, isTablet, toggleSidebar, toggleProjectRail, handleResize }
})
