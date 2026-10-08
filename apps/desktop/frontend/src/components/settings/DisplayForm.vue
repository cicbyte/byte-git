<script setup>
/**
 * 显示设置 — 外观偏好与布局开关。
 *
 * 目前涵盖:
 *   - 主题模式(亮/暗/跟随系统) — 实时切换,不持久化(已有 Sidebar 底部按钮)
 *   - 项目轨道默认显示 — 持久化到 layout store + localStorage
 *   - 侧边栏默认折叠 — 持久化到 layout store + localStorage
 *
 * 未来可扩展:紧凑模式、动画开关、字号全局等。
 */
import { ref, watch } from 'vue'
import { message as AMessage } from 'ant-design-vue'
import {
  EyeOutlined, BulbOutlined, BulbFilled, AppstoreOutlined,
  MenuFoldOutlined, CheckCircleOutlined,
} from '@ant-design/icons-vue'
import { useLayoutStore } from '../../stores/layout'
import { useThemeStore } from '../../stores/theme'

const layout = useLayoutStore()
const theme = useThemeStore()

// ---- theme (read-only mirror of Sidebar bottom toggle) ----
const themeMode = ref(theme.isDark ? 'dark' : 'light')

function onThemeChange(v) {
  themeMode.value = v
  if (v === 'dark' && !theme.isDark) theme.toggleTheme()
  if (v === 'light' && theme.isDark) theme.toggleTheme()
}

// ---- project rail default visibility ----
// persisted to localStorage so it survives app restart
const RAIL_KEY = 'byte-git.projectRailDefault'
const railDefault = ref(localStorage.getItem(RAIL_KEY) !== 'false') // default true

function onRailDefaultChange(v) {
  railDefault.value = v
  localStorage.setItem(RAIL_KEY, String(v))
  // apply immediately
  layout.projectRailVisible = v
  AMessage.success(v ? '项目轨道已设为默认显示' : '项目轨道已设为默认隐藏')
}

// ---- sidebar default collapsed ----
const SIDEBAR_KEY = 'byte-git.sidebarDefaultCollapsed'
const sidebarDefault = ref(localStorage.getItem(SIDEBAR_KEY) === 'true') // default false

function onSidebarDefaultChange(v) {
  sidebarDefault.value = v
  localStorage.setItem(SIDEBAR_KEY, String(v))
  layout.sidebarCollapsed = v
  AMessage.success(v ? '侧边栏已设为默认折叠' : '侧边栏已设为默认展开')
}
</script>

<template>
  <div class="settings-form">
    <!-- 主题 -->
    <div class="setting-group">
      <div class="group-title"><BulbOutlined /> 外观主题</div>
      <div class="setting-row">
        <div class="row-label">
          <div class="row-title">主题模式</div>
          <div class="row-help">亮色适合白天，暗色适合夜间。也可通过左下角灯泡图标快速切换。</div>
        </div>
        <a-radio-group :value="themeMode" @change="(e) => onThemeChange(e.target.value)">
          <a-radio-button value="light"><BulbOutlined /> 亮色</a-radio-button>
          <a-radio-button value="dark"><BulbFilled /> 暗色</a-radio-button>
        </a-radio-group>
      </div>
    </div>

    <!-- 布局 -->
    <div class="setting-group">
      <div class="group-title"><AppstoreOutlined /> 布局</div>

      <div class="setting-row">
        <div class="row-label">
          <div class="row-title">项目轨道默认显示</div>
          <div class="row-help">启动时是否显示最左侧的项目切换栏。随时可通过导航栏按钮临时切换。</div>
        </div>
        <a-switch :checked="railDefault" @change="onRailDefaultChange" />
      </div>

      <div class="setting-row">
        <div class="row-label">
          <div class="row-title">侧边栏默认折叠</div>
          <div class="row-help">启动时侧边导航栏是否折叠为图标模式。</div>
        </div>
        <a-switch :checked="sidebarDefault" @change="onSidebarDefaultChange" />
      </div>
    </div>

    <!-- 提示 -->
    <div class="setting-group">
      <div class="group-title"><EyeOutlined /> 说明</div>
      <div class="display-hint">
        <p><CheckCircleOutlined class="hint-icon" /> 主题和布局设置即时生效，无需重启。</p>
        <p><CheckCircleOutlined class="hint-icon" /> 项目轨道可随时通过导航栏按钮显示/隐藏。</p>
      </div>
    </div>
  </div>
</template>

<style scoped>
.settings-form { flex: 1; overflow-y: auto; padding: 20px 24px; background: var(--color-background); }
.setting-group {
  margin-bottom: var(--spacing-md);
  border-radius: var(--radius-md);
  border: 1px solid var(--color-border);
  padding: 16px 18px;
  background: var(--color-surface);
  box-shadow: var(--shadow-xs);
}
.group-title {
  display: flex; align-items: center; gap: 8px;
  font-size: 14px; font-weight: 600; color: var(--color-text);
  margin-bottom: 12px;
}
.setting-row {
  display: flex; justify-content: space-between; align-items: center;
  min-height: 36px; padding: 6px 0;
}
.row-label { display: flex; flex-direction: column; }
.row-title { font-size: 14px; color: var(--color-text); }
.row-help { font-size: 12px; color: var(--color-text-tertiary); margin-top: 4px; }

.display-hint { font-size: 12px; color: var(--color-text-tertiary); line-height: 2; }
.display-hint p { margin: 0; display: flex; align-items: center; gap: 6px; }
.hint-icon { color: var(--color-success); font-size: 13px; }
</style>
