<script setup>
import { onMounted, computed, ref, watch, nextTick } from 'vue'
import { useRoute } from 'vue-router'
import { CheckCircleOutlined, ExclamationCircleOutlined } from '@ant-design/icons-vue'
import { usePageFooter } from '@/composables/usePageFooter'
import { useToolStatus } from '@/composables/useToolStatus'
import { useSettings } from '@/composables/useSettings'
import { fadeIn, enterPage, leavePage } from '@/composables/useGsapAnimations'

const route = useRoute()
// Global footer (dynamically injectable per-page component)
const { visible, content, props } = usePageFooter()
// external CLI tool availability (gitleaks / ocr) — shared + cached
const { gitleaks, ocr, ensureLoaded } = useToolStatus()
// feature toggles (drives sidebar menu visibility) — must load at boot so
// the menu reflects the user's enabled/disabled features immediately
const { load: loadSettings } = useSettings()
// ref to the footer element so we can animate it on content/route change
const footerRef = ref(null)

// which tool is relevant to the current page? null = none → footer hides the
// tool entry and shows only the app label.
const routeToolMap = {
  '/secret': 'gitleaks',
  '/review': 'ocr',
}
const activeTool = computed(() => {
  const kind = routeToolMap[route.path]
  if (kind === 'gitleaks') return { kind: 'gitleaks', status: gitleaks.value, name: 'gitleaks' }
  if (kind === 'ocr') return { kind: 'ocr', status: ocr.value, name: 'ocr' }
  return null
})

// animate the footer when its injected component type changes or the route
// switches (not on every prop tick — only structural changes get animated,
// keeping frequent updates like commit-selection smooth without flicker).
watch([content, () => route.path], () => {
  nextTick(() => fadeIn(footerRef.value))
})

// repo init on boot (display + switch entry live in the Navbar now)
onMounted(async () => {
  // load feature toggles first so the sidebar renders with the correct
  // visibility on first paint (avoids flash of hidden-then-shown menus)
  loadSettings()
  await Promise.all([repo.detectCurrent(), repo.loadRecent()])
  ensureLoaded() // fire-and-forget; footer updates reactively when results land
})
</script>

<template>
  <div class="main-content">
    <!-- Scrollable page area -->
    <div class="content-wrapper">
      <router-view v-slot="{ Component }">
        <transition mode="out-in" :css="false" @enter="enterPage" @leave="leavePage">
          <keep-alive :include="['TerminalView']">
            <component :is="Component" />
          </keep-alive>
        </transition>
      </router-view>
    </div>

    <!-- Global Footer: injected component if set, otherwise a minimal status bar -->
    <div v-if="visible" class="global-footer" ref="footerRef">
      <component v-if="content" :is="content" v-bind="props" />
      <div v-else class="status-bar">
        <!-- external tool availability — only the tool relevant to this page -->
        <span
          v-if="activeTool"
          class="status-item tool"
          :class="{ ok: activeTool.status.available, bad: !activeTool.status.available }"
          :title="activeTool.status.exePath || activeTool.status.message"
        >
          <CheckCircleOutlined v-if="activeTool.status.available" />
          <ExclamationCircleOutlined v-else />
          <span class="tool-label">{{ activeTool.name }}</span>
        <span v-if="activeTool.status.version" class="tool-ver">{{ activeTool.status.version }}</span>
        <span v-if="activeTool.status.exePath" class="tool-path">· {{ activeTool.status.exePath }}</span>
        </span>
      </div>
    </div>
  </div>
</template>

<style scoped>
.main-content {
  flex: 1;
  display: flex;
  flex-direction: column;
  min-height: 0;
  /* Single translucent content background. Full-bleed pages set their own
     root background to transparent so this shows through (single layer);
     non-full-bleed pages sit on .content-wrapper which is transparent and
     inherits this. Either way there's exactly one translucent layer between
     the wallpaper and the page content. */
  background: var(--bg-content);
}

.content-wrapper {
  flex: 1;
  overflow-y: auto;
  padding: var(--spacing-lg);
  min-height: 0;
}

/* Global footer — pinned to bottom, aligns with the sidebar footer */
.global-footer {
  display: flex;
  align-items: center;
  padding: 0 var(--spacing-lg);
  height: var(--footer-height);
  border-top: 1px solid var(--color-border);
  background: var(--bg-footer);
  flex-shrink: 0;
  overflow: hidden;
}

/* Default status bar */
.status-bar {
  display: flex;
  align-items: center;
  gap: var(--spacing-md);
  width: 100%;
  font-size: 12px;
  color: var(--color-text-secondary);
  user-select: none;
}

.status-item {
  display: inline-flex;
  align-items: center;
  gap: 6px;
}

/* external-tool status entries in the footer */
.tool { gap: 4px; }
.tool.ok { color: var(--color-success); }
.tool.bad { color: var(--color-text-tertiary); }
.tool-label { font-weight: 500; color: var(--color-text); }
.tool-ver { color: var(--color-text-secondary); }
.tool-path {
  color: var(--color-text-tertiary);
  font-family: 'Cascadia Code', monospace;
  font-size: 11px;
  max-width: 260px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.status-value {
  color: var(--color-text);
  font-weight: 500;
}

.status-spacer {
  flex: 1;
}
</style>
