<script setup>
/**
 * AppearanceForm — 主题系统设置 UI。
 *
 * 三块能力：
 *   1. 主题模式（亮/暗/跟随系统）
 *   2. 预设选择（5 套内置预设 + 自定义）—— ThemePreviewCard 真实预览
 *   3. 自定义色板编辑（仅当 preset=custom）：
 *      - 可视化：6 字段组（主色/背景×4/文字×4/边框×2/主色状态×3/状态色×4）
 *      - 区域覆盖：6 区域 × 3 属性（背景/主文字/次文字）
 *      - 按底色自动协调（deriveBaseLayer 一键派生协调中性色）
 *      - 深色/浅色独立编辑
 *      - JSON 代码模式（整进整出）+ 文件导入导出
 *
 * 性能：拖动色板时 applyLive() 只更 CSS 变量（轻量），松开才 applyCustom()
 * 完整持久化。
 */
import { ref, computed, watch } from 'vue'
import {
  BulbOutlined, BulbFilled, DesktopOutlined,
  DownloadOutlined, UploadOutlined,
  CodeOutlined, CheckCircleOutlined, BgColorsOutlined,
  EditOutlined, ThunderboltOutlined,
} from '@ant-design/icons-vue'
import { message as AMessage } from 'ant-design-vue'
import { useThemeStore } from '@/stores/theme'
import {
  PRESET_LIST,
  REGION_DEFS,
  buildPresetColors,
  deriveBaseLayer,
} from '@/themes/presets'
import { applyColorsToDOM } from '@/themes/token'
import ThemePreviewCard from './ThemePreviewCard.vue'

const theme = useThemeStore()

/* ---- mode ---- */
const modeValue = computed({
  get: () => theme.mode,
  set: (v) => theme.setMode(v),
})

/* ---- preset selection ---- */
function onSelectPreset(key) {
  theme.setPreset(key)
}

/* ===== 自定义色板编辑（本地副本，深/浅各一套） ===== */
const editTab = ref(theme.resolvedMode)  // 'dark' | 'light'
const editDark = ref({ ...theme.custom.dark })
const editLight = ref({ ...theme.custom.light })

const editColors = computed(() => (editTab.value === 'dark' ? editDark.value : editLight.value))

/** 即时视觉：拖动只更新 CSS 变量（轻量高频；不触发持久化） */
function applyLive() {
  applyColorsToDOM(editColors.value, editTab.value, theme.regions[editTab.value])
}
/** 完整应用：持久化两边 */
function applyCustom() {
  theme.setCustomColors('light', { ...editLight.value })
  theme.setCustomColors('dark', { ...editDark.value })
}

// 进入自定义时，用 store.custom 初始化本地副本（首次即创建）
watch(
  () => theme.preset,
  (p) => {
    if (p !== 'custom') return
    editDark.value = { ...theme.custom.dark }
    editLight.value = { ...theme.custom.light }
    editTab.value = theme.resolvedMode
  },
  { immediate: true },
)

function setEditColor(field, value) {
  const target = editColors.value
  target[field] = value
}
function onColorInput(field, value) {
  setEditColor(field, value)
  applyLive()
}
function onColorCommit() {
  applyCustom()
}
function onHexCommit(field, value) {
  setEditColor(field, value)
  applyCustom()
}

function resetVariant() {
  editDark.value = buildPresetColors('dracula', 'dark')
  editLight.value = buildPresetColors('dracula', 'light')
  applyCustom()
  AMessage.success('已重置为默认配色')
}

/** 基于当前编辑变体的页面底色自动派生协调色板（保留主色 + 状态色扩展） */
function onAutoFit() {
  const m = editTab.value
  const cur = { ...editColors.value }
  const next = { ...cur, ...deriveBaseLayer(cur.bgBase, m) }
  // deriveBaseLayer 不覆盖 bgBase（保留用户选的种子），但派生其余中性色
  if (m === 'dark') editDark.value = next
  else editLight.value = next
  theme.setCustomColors(m, next)
  AMessage.success('已按底色自动协调')
}

/* ===== 区域覆盖 ===== */
const REGION_PROPS = [
  { prop: 'bg', label: '背景' },
  { prop: 'text', label: '主文字' },
  { prop: 'textSub', label: '次文字' },
]
function regionVal(key, prop) {
  return theme.regionValue(editTab.value, key, prop)
}
function onRegionInput(key, prop, val) {
  theme.setRegionToken(editTab.value, key, prop, val)
}

/* ===== 字段组 ===== */
const TOKEN_GROUPS = [
  {
    label: '主色', fields: [
      { key: 'primary', label: '主题色' },
      { key: 'accent', label: '强调色' },
    ],
  },
  {
    label: '背景', fields: [
      { key: 'bgBase', label: '页面底色' },
      { key: 'bgSurface', label: '容器底色' },
      { key: 'bgInput', label: '输入框背景' },
      { key: 'bgElevated', label: '浮层背景' },
    ],
  },
  {
    label: '文字', fields: [
      { key: 'textPrimary', label: '主文字' },
      { key: 'textSecondary', label: '次要文字' },
      { key: 'textTertiary', label: '弱化文字' },
      { key: 'textDisabled', label: '禁用文字' },
    ],
  },
  {
    label: '边框', fields: [
      { key: 'borderBase', label: '主边框' },
      { key: 'borderLight', label: '弱边框' },
    ],
  },
  {
    label: '主色状态', fields: [
      { key: 'primaryHover', label: '主色 · 悬停' },
      { key: 'primaryActive', label: '主色 · 按下' },
      { key: 'primaryBorder', label: '主色 · 边框' },
      { key: 'primaryBg', label: '主色 · 底色' },
    ],
  },
  {
    label: '状态色', fields: [
      { key: 'success', label: '成功' },
      { key: 'warning', label: '警告' },
      { key: 'danger', label: '危险' },
      { key: 'info', label: '信息' },
    ],
  },
]

/* ===== JSON 代码编辑 + 导入导出 ===== */
const editorMode = ref('visual')  // 'visual' | 'json'
const jsonText = ref('')

function serializeTheme() {
  return JSON.stringify(theme.exportState(), null, 2)
}
function enterJsonMode() {
  jsonText.value = serializeTheme()
  editorMode.value = 'json'
}
function applyJson() {
  const r = theme.applyRawState(jsonText.value)
  if (r.ok) {
    editDark.value = { ...theme.custom.dark }
    editLight.value = { ...theme.custom.light }
    AMessage.success('已应用 JSON 主题')
  } else {
    AMessage.error(r.error)
  }
}
function exportToFile() {
  const blob = new Blob([serializeTheme()], { type: 'application/json' })
  const url = URL.createObjectURL(blob)
  const a = document.createElement('a')
  a.href = url
  a.download = `byte-git-theme-${Date.now()}.json`
  a.click()
  URL.revokeObjectURL(url)
  AMessage.success('主题已导出')
}
function onImportFile(e) {
  const file = e.target.files?.[0]
  if (!file) return
  const reader = new FileReader()
  reader.onload = () => {
    const r = theme.applyRawState(reader.result)
    if (r.ok) {
      editDark.value = { ...theme.custom.dark }
      editLight.value = { ...theme.custom.light }
      AMessage.success('主题已导入')
    } else {
      AMessage.error(r.error)
    }
  }
  reader.readAsText(file)
  e.target.value = ''
}
</script>

<template>
  <div class="settings-form">
    <!-- 主题模式 -->
    <div class="setting-group">
      <div class="group-title"><BulbOutlined /> 主题模式</div>
      <div class="setting-row">
        <div class="row-label">
          <div class="row-title">明暗模式</div>
          <div class="row-help">跟随系统会按操作系统的明暗设置自动切换。</div>
        </div>
        <a-radio-group v-model:value="modeValue">
          <a-radio-button value="light"><BulbOutlined /> 亮色</a-radio-button>
          <a-radio-button value="dark"><BulbFilled /> 暗色</a-radio-button>
          <a-radio-button value="auto"><DesktopOutlined /> 跟随系统</a-radio-button>
        </a-radio-group>
      </div>
    </div>

    <!-- 预设选择 -->
    <div class="setting-group">
      <div class="group-title"><BgColorsOutlined /> 预设主题</div>
      <div class="theme-grid">
        <ThemePreviewCard
          v-for="p in PRESET_LIST"
          :key="p.key"
          :preset="p.key"
          :label="p.label"
          :is-active="theme.preset === p.key"
          @select="onSelectPreset"
        />
        <!-- 自定义卡片 -->
        <div
          class="custom-preview-card"
          :class="{ active: theme.preset === 'custom' }"
          :style="{
            background: theme.custom[theme.resolvedMode].bgSurface,
            borderColor: theme.preset === 'custom'
              ? theme.custom[theme.resolvedMode].primary
              : theme.custom[theme.resolvedMode].borderBase,
          }"
          @click="onSelectPreset('custom')"
        >
          <div class="cpc-swatch" :style="{ background: theme.custom[theme.resolvedMode].primary }" />
          <div class="cpc-body">
            <div class="cpc-name" :style="{ color: theme.custom[theme.resolvedMode].textPrimary }">自定义</div>
            <div class="cpc-lines">
              <div class="cpc-line w70" :style="{ background: theme.custom[theme.resolvedMode].textTertiary }" />
              <div class="cpc-line w50" :style="{ background: theme.custom[theme.resolvedMode].textDisabled }" />
              <div class="cpc-line w60" :style="{ background: theme.custom[theme.resolvedMode].primaryBg }" />
            </div>
            <div class="cpc-dots">
              <span class="cpc-dot" :style="{ background: theme.custom[theme.resolvedMode].success }" />
              <span class="cpc-dot" :style="{ background: theme.custom[theme.resolvedMode].warning }" />
              <span class="cpc-dot" :style="{ background: theme.custom[theme.resolvedMode].danger }" />
            </div>
          </div>
          <EditOutlined v-if="theme.preset === 'custom'" class="cpc-edit" :style="{ color: theme.custom[theme.resolvedMode].primary }" />
        </div>
      </div>
      <div class="row-help" style="margin-top: 10px;">
        Dracula 紫为默认配色（与历史版本一致），其余 4 套由种子色派生协调中性色。
      </div>
    </div>

    <!-- 自定义编辑（选中自定义时展开） -->
    <div v-if="theme.preset === 'custom'" class="setting-group custom-editor">
      <div class="editor-head">
        <span class="editor-title"><EditOutlined /> 自定义主题配色</span>
        <div class="head-actions">
          <a-button size="small" @click="exportToFile"><DownloadOutlined /> 导出</a-button>
          <label class="ant-btn ant-btn-sm import-btn">
            <UploadOutlined /> 导入
            <input class="import-input" type="file" accept="application/json" @change="onImportFile" />
          </label>
          <div class="mode-tabs">
            <button class="mode-tab" :class="{ active: editorMode === 'visual' }" @click="editorMode = 'visual'">可视化</button>
            <button class="mode-tab" :class="{ active: editorMode === 'json' }" @click="enterJsonMode"><CodeOutlined /> JSON</button>
          </div>
        </div>
      </div>

      <!-- 可视化编辑 -->
      <template v-if="editorMode === 'visual'">
        <div class="visual-subhead">
          <div class="editor-tabs">
            <button class="editor-tab" :class="{ active: editTab === 'dark' }" @click="editTab = 'dark'">深色</button>
            <button class="editor-tab" :class="{ active: editTab === 'light' }" @click="editTab = 'light'">浅色</button>
          </div>
          <span class="editor-hint">改动即时生效</span>
          <div class="subhead-actions">
            <a-button size="small" @click="onAutoFit"><ThunderboltOutlined /> 按底色自动协调</a-button>
            <a-button size="small" @click="resetVariant">重置</a-button>
          </div>
        </div>

        <div class="editor-body">
          <div v-for="group in TOKEN_GROUPS" :key="group.label" class="color-group">
            <div class="color-group-label">{{ group.label }}</div>
            <div class="color-fields">
              <div class="color-row" v-for="f in group.fields" :key="f.key">
                <input
                  type="color"
                  :value="editColors[f.key] || '#000000'"
                  class="color-picker"
                  @input="(e) => onColorInput(f.key, e.target.value)"
                  @change="onColorCommit"
                />
                <span class="color-label">{{ f.label }}</span>
                <input
                  type="text"
                  :value="editColors[f.key] || ''"
                  class="color-hex"
                  @change="(e) => onHexCommit(f.key, e.target.value)"
                />
              </div>
            </div>
          </div>

          <!-- 区域覆盖 -->
          <div class="color-group">
            <div class="color-group-label">区域覆盖（独立于预设，跨预设保留）</div>
            <div v-for="reg in REGION_DEFS" :key="reg.key" class="region-block">
              <div class="region-label">{{ reg.label }}</div>
              <div class="color-fields">
                <div class="color-row" v-for="rp in REGION_PROPS" :key="rp.prop">
                  <input
                    type="color"
                    :value="regionVal(reg.key, rp.prop) || '#000000'"
                    class="color-picker"
                    @input="(e) => onRegionInput(reg.key, rp.prop, e.target.value)"
                  />
                  <span class="color-label">{{ rp.label }}</span>
                  <input type="text" :value="regionVal(reg.key, rp.prop) || ''" class="color-hex" disabled />
                </div>
              </div>
            </div>
          </div>
        </div>
      </template>

      <!-- JSON 代码编辑 -->
      <template v-else>
        <div class="json-wrap">
          <a-textarea
            :value="jsonText"
            @input="(e) => jsonText = e.target.value"
            :rows="16"
            class="json-area"
          />
          <div class="json-footer">
            <a-button size="small" @click="enterJsonMode">刷新</a-button>
            <a-button size="small" type="primary" @click="applyJson">应用</a-button>
          </div>
        </div>
      </template>
    </div>

    <!-- 说明 -->
    <div class="setting-group">
      <div class="group-title"><CheckCircleOutlined /> 说明</div>
      <div class="display-hint">
        <p><CheckCircleOutlined class="hint-icon" /> 切换预设即时生效，无需重启。</p>
        <p><CheckCircleOutlined class="hint-icon" /> 修改主题色会自动重算 hover/active/border 派生项。</p>
        <p><CheckCircleOutlined class="hint-icon" /> 「按底色自动协调」基于页面底色派生协调的中性色（文字/边框/容器底色）。</p>
        <p><CheckCircleOutlined class="hint-icon" /> 区域覆盖独立于预设，可针对特定 UI 区域（如顶栏/侧栏）单独调色。</p>
        <p><CheckCircleOutlined class="hint-icon" /> 设置保存在 localStorage，仅当前浏览器/桌面端有效。</p>
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

/* theme grid */
.theme-grid { display: flex; flex-wrap: wrap; gap: 12px; }

/* 自定义卡片（对齐 ThemePreviewCard 尺寸） */
.custom-preview-card {
  position: relative;
  display: flex;
  gap: 8px;
  padding: 10px;
  width: 140px;
  height: 90px;
  border: 1px solid;
  border-radius: var(--radius-md);
  cursor: pointer;
  transition: transform var(--transition-fast);
  overflow: hidden;
}
.custom-preview-card:hover { transform: translateY(-1px); }
.custom-preview-card.active { box-shadow: 0 0 0 2px var(--color-primary-bg); }
.cpc-swatch { width: 4px; border-radius: 2px; flex-shrink: 0; }
.cpc-body { display: flex; flex-direction: column; gap: 4px; flex: 1; min-width: 0; }
.cpc-name { font-size: 12px; font-weight: 600; }
.cpc-lines { display: flex; flex-direction: column; gap: 3px; margin-top: 2px; }
.cpc-line { height: 4px; border-radius: 2px; }
.cpc-line.w70 { width: 70%; }
.cpc-line.w50 { width: 50%; }
.cpc-line.w60 { width: 60%; }
.cpc-dots { display: flex; gap: 4px; margin-top: auto; }
.cpc-dot { width: 8px; height: 8px; border-radius: 50%; }
.cpc-edit { position: absolute; right: 6px; bottom: 4px; font-size: 14px; }

/* ===== 自定义编辑面板 ===== */
.custom-editor { display: flex; flex-direction: column; }
.editor-head { display: flex; align-items: center; gap: 12px; margin-bottom: 16px; flex-wrap: wrap; }
.editor-title { display: flex; align-items: center; gap: 8px; font-size: 14px; font-weight: 600; color: var(--color-text); }
.head-actions { display: flex; align-items: center; gap: 8px; margin-left: auto; }
.import-btn { position: relative; overflow: hidden; cursor: pointer; display: inline-flex; align-items: center; gap: 6px; }
.import-input { position: absolute; inset: 0; opacity: 0; cursor: pointer; }
.mode-tabs { display: flex; gap: 2px; }
.mode-tab {
  display: inline-flex; align-items: center; gap: 4px;
  padding: 4px 12px; border: 1px solid var(--color-border); border-radius: var(--radius-sm);
  background: transparent; color: var(--color-text-tertiary); font-size: 12px; cursor: pointer;
  transition: all var(--transition-fast);
}
.mode-tab.active { background: var(--color-primary); color: #fff; border-color: var(--color-primary); }

.visual-subhead { display: flex; align-items: center; gap: 12px; margin-bottom: 16px; flex-wrap: wrap; }
.editor-tabs { display: flex; gap: 2px; }
.editor-tab {
  padding: 4px 12px; border: 1px solid var(--color-border); border-radius: var(--radius-sm);
  background: transparent; color: var(--color-text-tertiary); font-size: 12px; cursor: pointer;
  transition: all var(--transition-fast);
}
.editor-tab.active { background: var(--color-primary); color: #fff; border-color: var(--color-primary); }
.editor-hint { font-size: 11px; color: var(--color-text-tertiary); }
.subhead-actions { display: flex; gap: 8px; margin-left: auto; }

/* Editor body: groups stack VERTICALLY in their declared order
   (主色 → 背景 → 文字 → 边框 → 主色状态 → 状态色 → 区域覆盖).
   Within each group, fields flow into 2 columns on wide screens. */
.editor-body { display: flex; flex-direction: column; gap: 16px; }
.color-group {
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding-bottom: 12px;
  border-bottom: 1px dashed var(--color-border-light);
}
.color-group:last-child { border-bottom: none; padding-bottom: 0; }
/* field rows inside a group form a responsive grid (1-col narrow, 2-col wide) */
.color-fields {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(220px, 1fr));
  gap: 8px 16px;
}
.color-group-label { font-size: 11px; font-weight: 600; color: var(--color-text-tertiary); text-transform: uppercase; letter-spacing: 0.05em; }
.color-row { display: flex; align-items: center; gap: 10px; }
.color-picker { width: 28px; height: 24px; border: 1px solid var(--color-border); border-radius: var(--radius-xs); cursor: pointer; background: none; padding: 2px; flex-shrink: 0; }
.color-picker::-webkit-color-swatch-wrapper { padding: 0; }
.color-picker::-webkit-color-swatch { border: none; border-radius: 2px; }
.color-label { flex: 1; font-size: 12px; color: var(--color-text-secondary); }
.color-hex { width: 90px; border: 1px solid var(--color-border-light); border-radius: var(--radius-xs); padding: 3px 6px; font-size: 11px; font-family: 'Cascadia Code', monospace; background: var(--color-background); color: var(--color-text-secondary); }
.color-hex:focus { outline: none; border-color: var(--color-primary); }
.color-hex:disabled { opacity: 0.6; }

.region-block { padding: 8px 0 0; border-top: 1px dashed var(--color-border-light); }
.region-block:first-of-type { border-top: none; padding-top: 0; }
.region-label { font-size: 12px; font-weight: 600; color: var(--color-text-secondary); margin-bottom: 4px; }

.json-wrap { display: flex; flex-direction: column; gap: 12px; }
.json-area { font-family: 'Cascadia Code', monospace !important; font-size: 12px !important; }
.json-footer { display: flex; gap: 8px; justify-content: flex-end; }

.display-hint { font-size: 12px; color: var(--color-text-tertiary); line-height: 2; }
.display-hint p { margin: 0; display: flex; align-items: center; gap: 6px; }
.hint-icon { color: var(--color-success); font-size: 13px; }
</style>
