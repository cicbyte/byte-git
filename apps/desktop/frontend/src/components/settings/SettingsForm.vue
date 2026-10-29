<script setup>
import { computed } from 'vue'
import { InfoCircleOutlined, SafetyOutlined, CodeOutlined } from '@ant-design/icons-vue'
import AiSparkIcon from '@/components/common/AiSparkIcon.vue'
import { useSettings, SETTING_KEYS } from '@/composables/useSettings'
import { useErrorLogStore } from '@/stores/errorLog'

const props = defineProps({
  activeTab: { type: String, default: 'ai' },
})

const { settings, loading, queueSave } = useSettings()
const errorLog = useErrorLogStore()

const langOptions = [
  { label: '中文', value: 'zh' },
  { label: 'English', value: 'en' },
]

// AI group
const onModelChange = (v) => queueSave(SETTING_KEYS.model, v)
const onBaseUrlChange = (v) => queueSave(SETTING_KEYS.baseUrl, v)
const onApiKeyChange = (v) => queueSave(SETTING_KEYS.apiKey, v)
const onLangChange = (v) => queueSave(SETTING_KEYS.lang, v)
const onMaxTokensChange = (v) => queueSave(SETTING_KEYS.maxTokens, v ?? 4000)

// General group
const onAutoBackupChange = (v) => queueSave(SETTING_KEYS.rewriteAutoBackup, v)

const appVersion = computed(() => (window.go && window.go.main && window.go.main.ByteGitApp ? 'GUI v0.1.0' : 'dev'))
</script>

<template>
  <div class="settings-form">
    <a-spin :spinning="loading">
      <!-- AI Settings -->
      <div v-show="activeTab === 'ai'" class="setting-group">
        <div class="group-title"><AiSparkIcon /> AI 设置</div>

        <div class="setting-row">
          <div class="row-label">
            <div class="row-title">模型</div>
            <div class="row-help">AI commit / rewrite 使用的模型名</div>
          </div>
          <a-input
            :value="settings.model"
            placeholder="如 gemma4:e4b"
            style="width: 280px"
            @change="(e) => onModelChange(e.target.value)"
          />
        </div>

        <div class="setting-row">
          <div class="row-label">
            <div class="row-title">API Base URL</div>
            <div class="row-help">OpenAI 兼容接口地址</div>
          </div>
          <a-input
            :value="settings.baseUrl"
            placeholder="http://localhost:11434/v1"
            style="width: 280px"
            @change="(e) => onBaseUrlChange(e.target.value)"
          />
        </div>

        <div class="setting-row">
          <div class="row-label">
            <div class="row-title">API Key</div>
            <div class="row-help">部分本地服务可留空</div>
          </div>
          <a-input-password
            :value="settings.apiKey"
            placeholder="sk-..."
            style="width: 280px"
            @change="(e) => onApiKeyChange(e.target.value)"
          />
        </div>

        <div class="setting-row">
          <div class="row-label">
            <div class="row-title">Commit 语言</div>
            <div class="row-help">生成提交信息的语言</div>
          </div>
          <a-select
            :value="settings.lang"
            :options="langOptions"
            style="width: 280px"
            @change="onLangChange"
          />
        </div>

        <div class="setting-row">
          <div class="row-label">
            <div class="row-title">Max Tokens</div>
            <div class="row-help">单次请求最大 token 数；提交信息提示词规则见「提示词」页签</div>
          </div>
          <a-input-number
            :value="settings.maxTokens"
            :min="100"
            :max="128000"
            :step="500"
            style="width: 280px"
            @change="onMaxTokensChange"
          />
        </div>
      </div>

      <!-- About -->
      <div v-show="activeTab === 'about'" class="setting-group">
        <div class="group-title"><InfoCircleOutlined /> 关于</div>

        <div class="setting-row">
          <div class="row-label"><div class="row-title">应用</div></div>
          <span class="about-value">byte-git {{ appVersion }}</span>
        </div>
        <div class="setting-row">
          <div class="row-label"><div class="row-title">配置文件</div></div>
          <span class="about-value mono" :title="settings.configPath">{{ settings.configPath || '—' }}</span>
        </div>
        <div class="setting-row">
          <div class="row-label">
            <div class="row-title">说明</div>
            <div class="row-help">所有配置实时写入 YAML，与 CLI（byte-git config）完全同步。</div>
          </div>
        </div>

        <!-- error log (debug aid — no DevTools needed) -->
        <div class="err-log-section">
          <div class="err-log-head">
            <span class="err-log-title">错误日志</span>
            <a-button size="small" type="text" @click="errorLog.clear()" v-if="errorLog.errors.length">清空</a-button>
          </div>
          <div v-if="errorLog.errors.length" class="err-log-list">
            <div v-for="(e, i) in errorLog.errors" :key="i" class="err-log-item">
              <span class="err-log-type" :class="'et-' + e.type">{{ e.type }}</span>
              <span class="err-log-msg">{{ e.message }}</span>
              <span v-if="e.source" class="err-log-src">{{ e.source }}:{{ e.line }}</span>
              <pre v-if="e.stack" class="err-log-stack">{{ e.stack }}</pre>
            </div>
          </div>
          <div v-else class="err-log-empty">暂无错误</div>
        </div>
      </div>

      <!-- General -->
      <div v-show="activeTab === 'general'" class="setting-group">
        <div class="group-title"><SafetyOutlined /> 常规</div>

        <div class="setting-row">
          <div class="row-label">
            <div class="row-title">改写前自动整库备份</div>
            <div class="row-help">改写历史 / AI改写 / 密钥抹除执行前，是否默认创建完整 git clone 备份。可在每次应用时临时勾选覆盖。</div>
          </div>
          <a-switch :checked="settings.rewriteAutoBackup" @change="onAutoBackupChange" />
        </div>
      </div>
    </a-spin>
  </div>
</template>

<style scoped>
.settings-form {
  flex: 1;
  overflow-y: auto;
  padding: 20px 24px;
  background: var(--color-background);
}

.setting-group {
  margin-bottom: var(--spacing-md);
  border-radius: var(--radius-md);
  border: 1px solid var(--color-border);
  padding: 16px 18px;
  background: var(--color-surface);
  box-shadow: var(--shadow-xs);
}

.group-title {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 14px;
  font-weight: 600;
  color: var(--color-text);
  margin-bottom: 12px;
}

.setting-row {
  display: flex;
  justify-content: space-between;
  align-items: center;
  min-height: 36px;
  padding: 6px 0;
}

.row-label {
  display: flex;
  flex-direction: column;
}

.row-title {
  font-size: 14px;
  color: var(--color-text);
}

.row-help {
  font-size: 12px;
  color: var(--color-text-tertiary);
  margin-top: 4px;
}

.about-value {
  font-size: 13px;
  color: var(--color-text-secondary);
}

.about-value.mono {
  font-family: 'Cascadia Code', 'Fira Code', monospace;
  max-width: 60%;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

/* error log (debug aid) */
.err-log-section { margin-top: var(--spacing-md); border-top: 1px solid var(--color-border-light); padding-top: 12px; }
.err-log-head { display: flex; align-items: center; justify-content: space-between; margin-bottom: 8px; }
.err-log-title { font-size: 13px; font-weight: 600; color: var(--color-text); }
.err-log-list { display: flex; flex-direction: column; gap: 4px; max-height: 300px; overflow-y: auto; }
.err-log-item {
  padding: 6px 10px; border-radius: var(--radius-sm);
  background: var(--color-background); font-size: 12px;
  border-left: 3px solid var(--color-error);
}
.err-log-type {
  display: inline-block; font-size: 9px; font-weight: 700; padding: 1px 5px;
  border-radius: 3px; margin-right: 6px; text-transform: uppercase;
}
.et-error { color: var(--color-error); background: rgba(255,85,85,0.1); }
.et-promise { color: var(--color-warning); background: rgba(255,184,108,0.1); }
.err-log-msg { color: var(--color-text); word-break: break-word; }
.err-log-src { font-size: 10px; color: var(--color-text-tertiary); font-family: 'Cascadia Code', monospace; display: block; margin-top: 2px; }
.err-log-stack { font-size: 10px; color: var(--color-text-tertiary); font-family: 'Cascadia Code', monospace; white-space: pre-wrap; word-break: break-word; margin: 4px 0 0; max-height: 100px; overflow-y: auto; }
.err-log-empty { font-size: 12px; color: var(--color-text-tertiary); padding: 12px 0; }
</style>
