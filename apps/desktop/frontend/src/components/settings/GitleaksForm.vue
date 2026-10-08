<script setup>
import { ref, onMounted } from 'vue'
import {
  CheckCircleOutlined, CloseCircleOutlined, ReloadOutlined,
  FolderOpenOutlined, SafetyCertificateOutlined,
} from '@ant-design/icons-vue'
import { useSettings, SETTING_KEYS, useFeatureFlags } from '../../composables/useSettings'
import { useToolStatus } from '../../composables/useToolStatus'

const app = window.go?.main?.ByteGitApp
const { settings, queueSave } = useSettings()
const { secretEnabled } = useFeatureFlags()
const { gitleaks: gitleaksStatus, ensureLoaded, refresh } = useToolStatus()

function onToggle(v) {
  settings.featuresSecret = v
  queueSave(SETTING_KEYS.featuresSecret, v)
}

const checking = ref(false)

async function checkGitleaks() {
  checking.value = true
  // invalidate cache before re-check so a path change is picked up
  try { await app.SettingsSave({ 'gitleaks.exe_path': settings.gitleaksExePath || '' }) } catch {}
  await refresh()
  checking.value = false
}

onMounted(() => { ensureLoaded() })

function onPathChange(v) {
  queueSave(SETTING_KEYS.gitleaksExePath, v)
  setTimeout(checkGitleaks, 600)
}
</script>

<template>
  <div class="settings-form">
    <!-- feature toggle -->
    <div class="setting-group feature-toggle">
      <div class="ft-row">
        <div class="ft-label">
          <div class="ft-title">启用密钥扫描</div>
          <div class="ft-desc">开启后在侧边栏「规范治理」下显示密钥扫描入口</div>
        </div>
        <a-switch :checked="secretEnabled" @change="onToggle" />
      </div>
    </div>

    <div :class="{ 'ft-disabled': !secretEnabled }">
    <div class="setting-group">
      <div class="group-title"><SafetyCertificateOutlined /> gitleaks 集成</div>

      <!-- Status -->
      <div class="gl-status">
        <template v-if="gitleaksStatus.available">
          <CheckCircleOutlined class="ok" />
          <span>已就绪 · v{{ gitleaksStatus.version }}</span>
          <code class="gl-path">{{ gitleaksStatus.exePath }}</code>
        </template>
        <template v-else>
          <CloseCircleOutlined class="err" />
          <span>未检测到</span>
          <span v-if="gitleaksStatus.message" class="gl-err-msg">{{ gitleaksStatus.message }}</span>
        </template>
        <a-button size="small" type="text" :loading="checking" @click="checkGitleaks" title="重新检测">
          <template #icon><ReloadOutlined /></template>
        </a-button>
      </div>

      <!-- Custom exe path with file picker -->
      <div class="setting-row">
        <div class="row-label">
          <div class="row-title">可执行文件路径</div>
          <div class="row-help">留空 = 自动检测 PATH 中的 gitleaks；优先级：此路径 > PATH</div>
        </div>
        <a-input
          :value="settings.gitleaksExePath"
          placeholder="留空 = 自动检测"
          style="width: 340px"
          allow-clear
          @change="(e) => onPathChange(e.target.value)"
        >
          <template #suffix>
            <a-tooltip title="选择文件">
              <button class="tf-file-btn" @click="$refs.glFileInput.click()">
                <FolderOpenOutlined />
              </button>
            </a-tooltip>
          </template>
        </a-input>
        <input
          ref="glFileInput"
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

      <!-- Install guide -->
      <a-alert
        v-if="!gitleaksStatus.available"
        type="info"
        show-icon
        message="安装 gitleaks"
        style="margin-top: 12px"
      >
        <template #description>
          <div>方式一：通过 Go 工具链安装（需本机有 Go）:</div>
          <pre class="install-cmd">go install github.com/zricethezav/gitleaks/v8@latest</pre>
          <div>方式二：从 GitHub Releases 下载预编译二进制:</div>
          <div><a href="https://github.com/gitleaks/gitleaks/releases" target="_blank">https://github.com/gitleaks/gitleaks/releases</a></div>
          <div style="margin-top:6px">安装后确认 <code>gitleaks version</code> 可执行，或在此页指定 gitleaks 可执行文件的完整路径。</div>
        </template>
      </a-alert>
    </div>
    </div><!-- /ft-disabled wrapper -->
  </div>
</template>

<style scoped>
.settings-form { flex: 1; overflow-y: auto; padding: 20px 24px; background: var(--color-background); }

/* feature toggle */
.feature-toggle { border-left: 3px solid var(--color-primary); }
.ft-row { display: flex; align-items: center; justify-content: space-between; gap: 14px; }
.ft-label { flex: 1; }
.ft-title { font-size: 14px; font-weight: 600; color: var(--color-text); }
.ft-desc { font-size: 12px; color: var(--color-text-tertiary); margin-top: 3px; }
.ft-disabled { opacity: 0.4; pointer-events: none; }
.settings-form { flex: 1; overflow-y: auto; padding: 20px 24px; background: var(--color-background); }
.setting-group { border-radius: var(--radius-md); border: 1px solid var(--color-border); padding: 16px 18px; background: var(--color-surface); box-shadow: var(--shadow-xs); }
.group-title { display: flex; align-items: center; gap: 8px; font-size: 14px; font-weight: 600; color: var(--color-text); margin-bottom: 12px; }

.gl-status { display: flex; align-items: center; gap: 8px; margin-bottom: 12px; font-size: 13px; flex-wrap: wrap; }
.gl-status .ok { color: var(--color-success); }
.gl-status .err { color: var(--color-error); }
.gl-path { font-family: 'Cascadia Code', monospace; font-size: 11px; color: var(--color-text-secondary); background: var(--color-background); padding: 1px 6px; border-radius: var(--radius-xs); }
.gl-err-msg { font-size: 11px; color: var(--color-text-tertiary); }

.setting-row {
  display: flex; justify-content: space-between; align-items: center;
  min-height: 36px; padding: 6px 0;
}
.row-label { display: flex; flex-direction: column; }
.row-title { font-size: 14px; color: var(--color-text); }
.row-help { font-size: 12px; color: var(--color-text-tertiary); margin-top: 4px; }

.tf-file-btn {
  -webkit-appearance: none; appearance: none;
  border: none; background: transparent; cursor: pointer;
  color: var(--color-text-tertiary); font-size: 13px;
  padding: 2px; display: inline-flex; align-items: center;
}
.tf-file-btn:hover { color: var(--color-primary); }

.install-cmd { background: var(--color-background); padding: var(--spacing-sm); border-radius: var(--radius-xs); font-family: 'Cascadia Code', monospace; font-size: 12px; margin: 6px 0; }
</style>
