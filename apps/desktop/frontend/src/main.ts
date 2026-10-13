import { createApp } from 'vue'
import { createPinia } from 'pinia'
import Antd from 'ant-design-vue'
import App from './App.vue'
import router from './router'
import { installBridge } from '../bridge/go/main/ByteGitApp'
import 'ant-design-vue/dist/reset.css'

// CSS load order matters: derivation.css (theme vars) before variables.css
// (layout/spacing) so consumers see both. FOUC defence line #2 lives in
// derivation.css :root; line #1 is the inline index.html script; line #3
// is themeStore.initializeTheme() below, called BEFORE mount.
import './themes/derivation.css'
import './assets/styles/variables.css'

// Tauri 环境：把 invoke 桥接安装为 window.go.main.ByteGitApp（Wails 原生绑定优先）
installBridge()

const app = createApp(App)
const pinia = createPinia()
app.use(pinia)
app.use(router)
app.use(Antd)

// Globally register the motion directives so any component can use
// v-reveal / v-reveal-group without per-file imports.
import { vReveal, vRevealGroup, vStagger } from './composables/useGsapAnimations'
app.directive('reveal', vReveal)
app.directive('reveal-group', vRevealGroup)
app.directive('stagger', vStagger)

// FOUC defence line #3 — apply theme synchronously BEFORE mount so the
// first Vue render uses the correct palette (inline index.html script gave
// us a one-frame Dracula fallback; this corrects it to the real preset).
import { useThemeStore } from './stores/theme'
useThemeStore().initializeTheme()

app.mount('#app')

// Suppress the browser's native context menu app-wide. Custom right-click
// menus (CommitTree, AIRewrite, MetaRewrite …) listen on `contextmenu` and
// call preventDefault themselves; preventDefault here only blocks the
// browser default, event propagation continues, so those handlers still fire.
document.addEventListener('contextmenu', (e) => e.preventDefault())
