import { ref, computed, shallowRef, type Component } from 'vue'

// Module-level singleton state — shared across all components
const visible = ref(true)
const content = shallowRef<Component | null>(null)
const propsGetter = ref<(() => Record<string, any>) | null>(null)

/**
 * Global footer injection mechanism.
 *
 * Any view can call `setFooter(MyComponent, () => ({ ...props }))` to render a
 * custom component in the bottom global footer. Call `clearFooter()` on
 * unmount to restore the default status bar.
 */
export function usePageFooter() {
  function setFooter(component: Component, getProps: () => Record<string, any>) {
    content.value = component
    propsGetter.value = getProps
    visible.value = true
  }

  function setVisible(v: boolean) {
    visible.value = v
  }

  function clearFooter() {
    content.value = null
    propsGetter.value = null
  }

  function hideFooter() {
    visible.value = false
  }

  function showFooter() {
    visible.value = true
  }

  return {
    visible,
    content,
    props: computed(() => propsGetter.value?.() ?? {}),
    setFooter,
    setVisible,
    clearFooter,
    hideFooter,
    showFooter,
  }
}
