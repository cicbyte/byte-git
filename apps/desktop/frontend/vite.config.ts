import {defineConfig} from 'vite'
import vue from '@vitejs/plugin-vue'

// https://vitejs.dev/config/
export default defineConfig({
  plugins: [vue()],

  // ---- Wails dev-mode requirements ----
  server: {
    // Vite must listen on all interfaces so the Wails webview process
    // (which loads assets over http) can reach the dev server.
    host: '0.0.0.0',
    port: 1430,
    strictPort: true,
    // HMR over websocket; the port must match what Wails auto-detects.
    hmr: {
      port: 1430,
    },
    // Windows / network drives sometimes miss native fs events — polling
    // guarantees file changes are picked up and HMR fires reliably.
    watch: {
      usePolling: true,
      interval: 100,
    },
    // 启动时预转译全部路由视图：懒加载路由首开不再付整棵模块树的
    // 冷转译成本（实测 ai-rewrite 页 99 个模块约 1.7s）。
    warmup: {
      clientFiles: ['./src/views/**/*.vue'],
    },
  },
})
