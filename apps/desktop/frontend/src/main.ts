import { createApp } from 'vue'
import { createPinia } from 'pinia'
import Antd from 'ant-design-vue'
import App from './App.vue'
import router from './router'
import { installBridge } from '../bridge/go/main/ByteGitApp'
import 'ant-design-vue/dist/reset.css'
import './assets/styles/variables.css'

// Tauri 环境：把 invoke 桥接安装为 window.go.main.ByteGitApp（Wails 原生绑定优先）
installBridge()

const app = createApp(App)
app.use(createPinia())
app.use(router)
app.use(Antd)
app.mount('#app')

// Suppress the browser's native context menu app-wide. Custom right-click
// menus (CommitTree, AIRewrite, MetaRewrite …) listen on `contextmenu` and
// call preventDefault themselves; preventDefault here only blocks the
// browser default, event propagation continues, so those handlers still fire.
document.addEventListener('contextmenu', (e) => e.preventDefault())
