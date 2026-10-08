<script setup>
import { ref, computed, onMounted, watch } from 'vue'
import { message as AMessage } from 'ant-design-vue'
import {
  CheckCircleOutlined, CloseCircleOutlined, ReloadOutlined,
  ApiOutlined, ControlOutlined, ToolOutlined, CodeOutlined, SaveOutlined,
  FolderOpenOutlined,
} from '@ant-design/icons-vue'
import { useSettings, SETTING_KEYS, useFeatureFlags } from '../../composables/useSettings'
import { useOcrConfig } from '../../composables/useOcrConfig'
import CodeViewer from '../common/CodeViewer.vue'

const app = window.go?.main?.ByteGitApp
const { settings, queueSave } = useSettings()
const { reviewEnabled } = useFeatureFlags()

function onToggle(v) {
  settings.featuresReview = v
  queueSave(SETTING_KEYS.featuresReview, v)
}
const {
  cfg, path, exists, presets, load: loadOcrCfg, save: saveOcr,
  rawContent, rawDirty, rawLoading, loadRaw, saveRaw, saving,
} = useOcrConfig()

// ---- edit mode: structured form vs raw JSON ----
// 'form' = the friendly field-by-field editor; 'json' = CodeMirror on the
// whole config.json for advanced users. Switching is lazy — raw content is
// only fetched when the user first enters JSON mode (so we don't pull the
// unmasked secrets into memory until needed).
const editMode = ref('form')
const rawLoaded = ref(false)
async function switchMode(m) {
  editMode.value = m
  if (m === 'json' && !rawLoaded.value) {
    await loadRaw()
    rawLoaded.value = true
  }
}
function onRawEdit(v) {
  rawContent.value = v
  rawDirty.value = true
}
async function onSaveRaw() {
  await saveRaw()
  // structured form is now stale — force a reload so switching back is fresh
  rawLoaded.value = false
}

// ---- OCR exe detection (existing logic) ----
const ocrStatus = ref({ available: false, exePath: '', version: '', message: '' })
const checking = ref(false)
async function checkOcr() {
  if (!app?.ReviewCheck) return
  checking.value = true
  // invalidate cache before re-check so a settings change is picked up
  try { await app.SettingsSave({ 'ocr.exe_path': settings.ocrExePath || '' }) } catch {}
  try { ocrStatus.value = await app.ReviewCheck() } finally { checking.value = false }
}

// ---- provider model ----
// current provider: cfg.provider ("" = legacy llm / custom)
const providerOptions = computed(() => [
  { label: '自定义 / llm 直连', value: '' },
  ...presets.value.map((p) => ({ label: p.displayName, value: p.name })),
])
const currentPreset = computed(() => presets.value.find((p) => p.name === cfg.provider) || null)
// model options: preset models + provider-entry models (custom gateways may
// declare their own model list) + allow free-text custom input.
const modelOptions = computed(() => {
  const base = currentPreset.value?.models || providerEntry.value.models || []
  return base.map((m) => ({ label: m, value: m }))
})

// current values read from cfg (with provider-entry overrides folded in).
// Look in both `providers` (presets) and `custom_providers` (custom gateways)
// since the user may have configured a custom provider as the active one.
const providerEntry = computed(() => {
  if (!cfg.provider) return {}
  return cfg.providers?.[cfg.provider] || cfg.custom_providers?.[cfg.provider] || {}
})
const isCustomProvider = computed(() =>
  !cfg.provider || !!cfg.custom_providers?.[cfg.provider]
)
const apiKeyDisplay = computed(() => providerEntry.value.api_key || '')
const urlDisplay = computed(() => providerEntry.value.url || currentPreset.value?.baseUrl || '')
const modelDisplay = computed(() => providerEntry.value.model || cfg.model || '')

// language options for the OCR-output language (config.json), separate from
// byte-git's own commit language.
const langOptions = [
  { label: '跟随 OCR 默认', value: '' },
  { label: '中文', value: 'zh' },
  { label: 'English', value: 'en' },
]
const audienceOptions = [
  { label: 'agent (JSON)', value: 'agent' },
  { label: 'human (进度)', value: 'human' },
]

// ---- handlers ----
// When the active provider is a custom gateway (in custom_providers), writes
// go to custom_providers.<name>.<field>; otherwise providers.<name>.<field>.
const providerBucket = computed(() => {
  if (!cfg.provider) return ''
  return cfg.custom_providers?.[cfg.provider] ? 'custom_providers' : 'providers'
})
function onProviderChange(v) {
  saveOcr('provider', v)
  // when switching to a preset, seed model/url if empty so the form reflects it
  const p = presets.value.find((x) => x.name === v)
  if (p) {
    if (!cfg.providers?.[v]?.url) saveOcr(`providers.${v}.url`, p.baseUrl)
    if (!cfg.model && p.models?.length) saveOcr('model', p.models[0])
  }
}
function onApiKeyChange(e) {
  const v = e.target.value
  if (cfg.provider) saveOcr(`${providerBucket.value}.${cfg.provider}.api_key`, v)
}
function onUrlChange(e) {
  if (cfg.provider) saveOcr(`${providerBucket.value}.${cfg.provider}.url`, e.target.value)
}
function onModelChange(v) {
  saveOcr('model', v)
  if (cfg.provider) saveOcr(`${providerBucket.value}.${cfg.provider}.model`, v)
}
function onLanguageChange(v) { saveOcr('language', v) }

// byte-git-side review behavior defaults (config.yaml)
function onConcurrencyChange(v) { queueSave(SETTING_KEYS.ocrConcurrency, v ?? 0) }
function onMaxToolsChange(v) { queueSave(SETTING_KEYS.ocrMaxTools, v ?? 0) }
function onAudienceChange(v) { queueSave(SETTING_KEYS.ocrAudience, v) }
function onOcrLangChange(v) { queueSave(SETTING_KEYS.ocrLanguage, v) }

function onPathChange(v) {
  queueSave(SETTING_KEYS.ocrExePath, v)
  setTimeout(checkOcr, 600)
}

onMounted(() => {
  loadOcrCfg()
  checkOcr()
})

// re-check OCR when the exe path changes elsewhere
watch(() => settings.ocrExePath, () => { /* no-op; checkOcr triggered by onPathChange */ })
</script>

<template>
  <div class="settings-form">
    <!-- feature toggle -->
    <div class="setting-group feature-toggle">
      <div class="ft-row">
        <div class="ft-label">
          <div class="ft-title">启用代码审查</div>
          <div class="ft-desc">开启后在侧边栏「规范治理」下显示代码审查入口</div>
        </div>
        <a-switch :checked="reviewEnabled" @change="onToggle" />
      </div>
    </div>

    <div :class="{ 'ft-disabled': !reviewEnabled }">
    <a-spin :spinning="false">
      <!-- edit-mode switch -->
      <div class="mode-bar">
        <a-radio-group :value="editMode" size="small" @change="(e) => switchMode(e.target.value)">
          <a-radio-button value="form"><ApiOutlined /> 表单</a-radio-button>
          <a-radio-button value="json"><CodeOutlined /> JSON</a-radio-button>
        </a-radio-group>
        <span class="mode-tip" v-if="editMode === 'json'">直接编辑 ~/.opencodereview/config.json（高级）</span>
      </div>

      <!-- ============ JSON editor mode ============ -->
      <div v-if="editMode === 'json'" class="setting-group json-group">
        <div class="json-toolbar">
          <span class="json-path" :title="path">{{ path || '—' }}</span>
          <a-tag v-if="rawDirty" color="orange">未保存</a-tag>
          <a-button size="small" type="text" :loading="rawLoading" @click="loadRaw" title="重新加载">
            <template #icon><ReloadOutlined /></template>
          </a-button>
          <a-button size="small" type="primary" :loading="saving" :disabled="!rawDirty" @click="onSaveRaw">
            <template #icon><SaveOutlined /></template>保存
          </a-button>
        </div>
        <div class="json-editor">
          <a-spin v-if="rawLoading" class="json-spin" />
          <CodeViewer
            v-else
            :code="rawContent"
            filename="config.json"
            editable
            @update:modelValue="onRawEdit"
          />
        </div>
        <div class="json-hint">
          ⚠ 此模式直接编辑整个文件，保存后将覆盖所有字段（含表单模式已配置的内容）。
          JSON 格式错误会被拒绝；OCR 运行时也会校验 schema。
        </div>
      </div>

      <!-- ============ 区块 A: LLM 配置 (OCR config.json) ============ -->
      <div v-show="editMode === 'form'" class="setting-group">
        <div class="group-title"><ApiOutlined /> LLM 配置
          <span class="group-sub">写入 ~/.opencodereview/config.json · 与 ocr config 互通</span>
        </div>

        <!-- provider select -->
        <div class="setting-row">
          <div class="row-label">
            <div class="row-title">Provider</div>
            <div class="row-help">选择 LLM 服务商；选「自定义」可用 llm 直连或 custom gateway</div>
          </div>
          <a-select
            :value="cfg.provider || ''"
            :options="providerOptions"
            placeholder="选择服务商"
            style="width: 280px"
            @change="onProviderChange"
          />
        </div>

        <!-- preset provider fields -->
        <template v-if="cfg.provider">
          <div class="setting-row">
            <div class="row-label">
              <div class="row-title">API Key</div>
              <div class="row-help">{{ currentPreset?.displayName || cfg.provider }} 的密钥</div>
            </div>
            <a-input-password
              :value="apiKeyDisplay"
              placeholder="sk-..."
              style="width: 340px"
              @change="onApiKeyChange"
            />
          </div>

          <div class="setting-row">
            <div class="row-label">
              <div class="row-title">API URL</div>
              <div class="row-help">默认填入预设地址，可改为代理/私有部署</div>
            </div>
            <a-input
              :value="urlDisplay"
              placeholder="https://..."
              style="width: 340px"
              @change="onUrlChange"
            />
          </div>

          <div class="setting-row">
            <div class="row-label">
              <div class="row-title">模型</div>
              <div class="row-help">预设模型可直接选，也可手动输入自定义模型名</div>
            </div>
            <a-auto-complete
              :value="modelDisplay"
              :options="modelOptions"
              placeholder="选择或输入模型"
              style="width: 280px"
              :filter-option="(input, opt) => opt.label.toLowerCase().includes(input.toLowerCase())"
              allow-clear
              @change="onModelChange"
            />
          </div>
        </template>

        <!-- legacy llm direct mode (provider == "") -->
        <template v-else>
          <a-alert type="info" show-icon style="margin: 8px 0">
            <template #message>未选择 Provider — 使用 llm 直连或环境变量</template>
            <template #description>
              可在下方配置 llm 直连（url/token/model），或留空让 OCR 从环境变量 (OCR_LLM_*) / Claude Code 配置读取。
              建议选择上方的 Provider 获得最佳体验。
            </template>
          </a-alert>
          <div class="setting-row">
            <div class="row-label"><div class="row-title">LLM URL</div></div>
            <a-input :value="cfg.llm?.url || ''" placeholder="https://api.openai.com/v1" style="width: 340px" @change="(e) => saveOcr('llm.url', e.target.value)" />
          </div>
          <div class="setting-row">
            <div class="row-label"><div class="row-title">Auth Token</div></div>
            <a-input-password :value="cfg.llm?.auth_token || ''" placeholder="token" style="width: 340px" @change="(e) => saveOcr('llm.auth_token', e.target.value)" />
          </div>
          <div class="setting-row">
            <div class="row-label"><div class="row-title">模型</div></div>
            <a-input :value="cfg.llm?.model || cfg.model || ''" placeholder="模型名" style="width: 280px" @change="(e) => saveOcr('llm.model', e.target.value)" />
          </div>
        </template>

        <!-- output language (config.json) -->
        <div class="setting-row">
          <div class="row-label">
            <div class="row-title">审查输出语言</div>
            <div class="row-help">AI 审查意见的语言</div>
          </div>
          <a-select :value="cfg.language || ''" :options="langOptions" style="width: 200px" @change="onLanguageChange" />
        </div>

        <!-- config path / status -->
        <div class="cfg-path-row">
          <span class="cfg-path-label">配置文件:</span>
          <code class="cfg-path" :title="path">{{ path || '—' }}</code>
          <a-tag v-if="exists" color="green" style="margin-left: 8px">已存在</a-tag>
          <a-tag v-else color="default" style="margin-left: 8px">未创建（保存后自动生成）</a-tag>
        </div>
      </div>

      <!-- ============ 区块 B: 审查参数默认值 (byte-git config.yaml) ============ -->
      <div v-show="editMode === 'form'" class="setting-group">
        <div class="group-title"><ControlOutlined /> 审查参数默认值
          <span class="group-sub">写入 byte-git config.yaml · 审查发起时可临时覆盖</span>
        </div>

        <div class="setting-row">
          <div class="row-label">
            <div class="row-title">并发数</div>
            <div class="row-help">同时审查的文件数（0 = OCR 默认 8）</div>
          </div>
          <a-input-number :value="settings.ocrConcurrency" :min="0" :max="32" :step="1" style="width: 160px" @change="onConcurrencyChange" />
        </div>

        <div class="setting-row">
          <div class="row-label">
            <div class="row-title">工具调用上限</div>
            <div class="row-help">Agent 单次审查的工具调用次数上限（0 = OCR 默认，最小 10）</div>
          </div>
          <a-input-number :value="settings.ocrMaxTools" :min="0" :max="200" :step="5" style="width: 160px" @change="onMaxToolsChange" />
        </div>

        <div class="setting-row">
          <div class="row-label">
            <div class="row-title">Audience 模式</div>
            <div class="row-help">agent=纯 JSON 输出；human=带进度行（可改善进度感知）</div>
          </div>
          <a-select :value="settings.ocrAudience || 'agent'" :options="audienceOptions" style="width: 200px" @change="onAudienceChange" />
        </div>

        <div class="setting-row">
          <div class="row-label">
            <div class="row-title">输出语言（覆盖）</div>
            <div class="row-help">留空则用上方 config.json 的 language</div>
          </div>
          <a-select :value="settings.ocrLanguage" :options="langOptions" style="width: 200px" @change="onOcrLangChange" />
        </div>
      </div>

      <!-- ============ 区块 C: 可执行文件 ============ -->
      <div v-show="editMode === 'form'" class="setting-group">
        <div class="group-title"><ToolOutlined /> 可执行文件</div>

        <div class="oc-status">
          <template v-if="ocrStatus.available">
            <CheckCircleOutlined class="ok" />
            <span>已就绪 · v{{ ocrStatus.version }}</span>
            <code class="oc-path">{{ ocrStatus.exePath }}</code>
          </template>
          <template v-else>
            <CloseCircleOutlined class="err" />
            <span>未检测到</span>
          </template>
          <a-button size="small" type="text" :loading="checking" @click="checkOcr" title="重新检测">
            <template #icon><ReloadOutlined /></template>
          </a-button>
        </div>

        <div class="setting-row">
          <div class="row-label">
            <div class="row-title">OCR 可执行文件路径</div>
            <div class="row-help">留空 = 自动检测 PATH 中的 ocr / opencodereview</div>
          </div>
          <a-input
            :value="settings.ocrExePath"
            placeholder="留空=自动检测"
            style="width: 340px"
            allow-clear
            @change="(e) => onPathChange(e.target.value)"
          >
            <template #suffix>
              <a-tooltip title="选择文件">
                <button class="ocr-file-btn" @click="$refs.ocrFileInput.click()">
                  <FolderOpenOutlined />
                </button>
              </a-tooltip>
            </template>
          </a-input>
          <input
            ref="ocrFileInput"
            type="file"
            style="display:none"
            accept=".exe,.bat,.cmd"
            @change="(e) => {
              const f = e.target.files?.[0]
              if (f) onPathChange(f.path)
              e.target.value = ''
            }"
          />
        </div>

        <a-alert v-if="!ocrStatus.available" type="info" show-icon style="margin-top: 12px" message="安装 OpenCodeReview">
          <template #description>
            <div>通过 npm 全局安装:</div>
            <pre class="install-cmd">npm i -g @alibaba-group/open-code-review</pre>
            <div>安装后确认 <code>ocr --version</code> 可执行，或在此指定完整路径。</div>
          </template>
        </a-alert>
      </div>
    </a-spin>
    </div><!-- /ft-disabled wrapper -->
  </div>
</template>

<style scoped>
.settings-form { flex: 1; overflow-y: auto; padding: 20px 24px; background: var(--color-background); }

/* feature toggle */
.feature-toggle { border-left: 3px solid var(--color-primary); margin-bottom: var(--spacing-md); }
.ft-row { display: flex; align-items: center; justify-content: space-between; gap: 14px; }
.ft-label { flex: 1; }
.ft-title { font-size: 14px; font-weight: 600; color: var(--color-text); }
.ft-desc { font-size: 12px; color: var(--color-text-tertiary); margin-top: 3px; }
.ft-disabled { opacity: 0.4; pointer-events: none; }

/* edit-mode switch bar */
.mode-bar { display: flex; align-items: center; gap: var(--spacing-sm); margin-bottom: var(--spacing-md); }
.mode-tip { font-size: 12px; color: var(--color-text-tertiary); }

/* JSON editor group */
.json-group { padding: 12px; }
.json-toolbar { display: flex; align-items: center; gap: 8px; margin-bottom: 8px; }
.json-path { font-family: 'Cascadia Code', monospace; font-size: 11px; color: var(--color-text-secondary); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; flex: 1; }
.json-editor { height: 460px; border: 1px solid var(--color-border); border-radius: var(--radius-sm); overflow: hidden; position: relative; }
.json-spin { position: absolute; inset: 0; display: flex; align-items: center; justify-content: center; }
.json-hint { font-size: 11px; color: var(--color-text-tertiary); margin-top: 8px; line-height: 1.6; }
.setting-group { margin-bottom: var(--spacing-md); border-radius: var(--radius-md); border: 1px solid var(--color-border); padding: 16px 18px; background: var(--color-surface); box-shadow: var(--shadow-xs); }
.group-title { display: flex; align-items: center; gap: 8px; font-size: 14px; font-weight: 600; color: var(--color-text); margin-bottom: 12px; }
.group-sub { font-size: 11px; font-weight: 400; color: var(--color-text-tertiary); margin-left: auto; }

.setting-row { display: flex; justify-content: space-between; align-items: center; min-height: 36px; padding: 6px 0; }
.row-label { display: flex; flex-direction: column; }
.row-title { font-size: 14px; color: var(--color-text); }
.row-help { font-size: 12px; color: var(--color-text-tertiary); margin-top: 4px; }

.oc-status { display: flex; align-items: center; gap: 8px; margin-bottom: 12px; font-size: 13px; }
.oc-status .ok { color: var(--color-success); }
.oc-status .err { color: var(--color-error); }
.oc-path { font-family: 'Cascadia Code', monospace; font-size: 11px; color: var(--color-text-secondary); background: var(--color-background); padding: 1px 6px; border-radius: var(--radius-xs); }

.cfg-path-row { display: flex; align-items: center; gap: 8px; margin-top: 12px; padding-top: 10px; border-top: 1px solid var(--color-border-light); font-size: 12px; }
.cfg-path-label { color: var(--color-text-tertiary); flex-shrink: 0; }
.cfg-path { font-family: 'Cascadia Code', monospace; font-size: 11px; color: var(--color-text-secondary); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }

.install-cmd { background: var(--color-background); padding: var(--spacing-sm); border-radius: var(--radius-xs); font-family: 'Cascadia Code', monospace; font-size: 12px; margin: 6px 0; }
.ocr-file-btn {
  -webkit-appearance: none; appearance: none;
  border: none; background: transparent; cursor: pointer;
  color: var(--color-text-tertiary); font-size: 13px;
  padding: 2px; display: inline-flex; align-items: center;
}
.ocr-file-btn:hover { color: var(--color-primary); }
</style>
