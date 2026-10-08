<script setup>
import { ref, onMounted } from 'vue'
import { message as AMessage } from 'ant-design-vue'
import {
  CodeOutlined, FolderOpenOutlined, ReloadOutlined, CheckCircleOutlined,
} from '@ant-design/icons-vue'
import { useSettings, SETTING_KEYS } from '../../composables/useSettings'

const app = window.go?.main?.ByteGitApp
const { settings, queueSave } = useSettings()

const detectedShell = ref({ path: '', isGitBash: false })
const detecting = ref(false)

async function detectShell() {
  if (!app?.TerminalDetectShell) return
  detecting.value = true
  try {
    detectedShell.value = await app.TerminalDetectShell()
  } catch {
    detectedShell.value = { path: '', isGitBash: false }
  } finally {
    detecting.value = false
  }
}

onMounted(detectShell)

function useDetected() {
  if (detectedShell.value.path) {
    queueSave(SETTING_KEYS.termShellPath, detectedShell.value.path)
    AMessage.success(`已设为: ${detectedShell.value.path}`)
  }
}

const fontOptions = [
  { label: "JetBrains Mono", value: "'JetBrains Mono', 'Cascadia Code', monospace" },
  { label: "Cascadia Code", value: "'Cascadia Code', 'Fira Code', monospace" },
  { label: "Fira Code", value: "'Fira Code', 'Consolas', monospace" },
  { label: "Consolas", value: "'Consolas', monospace" },
  { label: "monospace", value: "monospace" },
]
// NOTE: font family is stored in settings but not yet wired to TerminalView;
// TerminalView currently hardcodes the font. This will be connected when
// config reading is added to TerminalView's onMounted.
const selectedFont = ref("'JetBrains Mono', 'Cascadia Code', monospace")

function onFontChange(v) {
  selectedFont.value = v
  // future: persist to config
}
</script>

<template>
  <div class="tf-form">
    <!-- Shell section -->
    <div class="setting-group">
      <div class="group-title"><CodeOutlined /> Shell 配置</div>

      <!-- detected status -->
      <div class="tf-detect-bar">
        <template v-if="detecting">
          <a-spin size="small" />
          <span class="tf-detect-text">正在检测…</span>
        </template>
        <template v-else-if="detectedShell.path">
          <CheckCircleOutlined class="tf-ok-icon" />
          <span class="tf-detect-text">
            检测到: <code class="tf-detect-path">{{ detectedShell.path }}</code>
            <span v-if="detectedShell.isGitBash" class="tf-git-badge">Git Bash</span>
          </span>
          <a-button size="small" type="link" @click="useDetected">使用此路径</a-button>
        </template>
        <template v-else>
          <span class="tf-detect-text muted">未检测到可用 Shell</span>
        </template>
        <a-button size="small" type="text" :loading="detecting" @click="detectShell" title="重新检测">
          <template #icon><ReloadOutlined /></template>
        </a-button>
      </div>

      <!-- shell path input -->
      <div class="setting-row">
        <div class="row-label">
          <div class="row-title">Shell 路径</div>
          <div class="row-help">留空 = 自动检测（优先 git-bash）；手动指定可覆盖（如 WSL 的 wsl.exe）</div>
        </div>
        <a-input
          :value="settings.termShellPath"
          placeholder="留空 = 自动检测"
          style="width: 340px"
          allow-clear
          @change="(e) => queueSave(SETTING_KEYS.termShellPath, e.target.value)"
        >
          <template #suffix>
            <a-tooltip title="选择文件">
              <button class="tf-file-btn" @click="$refs.fileInput.click()">
                <FolderOpenOutlined />
              </button>
            </a-tooltip>
          </template>
        </a-input>
        <input
          ref="fileInput"
          type="file"
          style="display:none"
          accept=".exe,.bat,.cmd,.sh,.bash"
          @change="(e) => {
            const f = e.target.files?.[0]
            if (f) queueSave(SETTING_KEYS.termShellPath, f.path)
            e.target.value = ''
          }"
        />
      </div>
    </div>

    <!-- Appearance section -->
    <div class="setting-group">
      <div class="group-title">外观</div>

      <div class="setting-row">
        <div class="row-label">
          <div class="row-title">字号</div>
          <div class="row-help">终端字体大小（8-32）</div>
        </div>
        <a-input-number
          :value="settings.termFontSize || 13"
          :min="8"
          :max="32"
          :step="1"
          style="width: 160px"
          @change="(v) => queueSave(SETTING_KEYS.termFontSize, v ?? 13)"
        />
      </div>

      <div class="setting-row">
        <div class="row-label">
          <div class="row-title">字体</div>
          <div class="row-help">等宽字体（需系统已安装）</div>
        </div>
        <a-select
          :value="selectedFont"
          :options="fontOptions"
          style="width: 240px"
          @change="onFontChange"
        />
      </div>
    </div>

    <!-- info section -->
    <div class="tf-info">
      终端使用 ConPTY（Windows）/ PTY（Unix），支持交互式 Shell。
      当前仓库路径会作为终端的默认工作目录。
    </div>
  </div>
</template>

<style scoped>
.tf-form { flex: 1; overflow-y: auto; padding: 20px 24px; background: var(--color-background); }

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

/* detected status bar */
.tf-detect-bar {
  display: flex; align-items: center; gap: 8px;
  padding: 8px 12px; margin-bottom: 12px;
  border-radius: var(--radius-sm);
  background: var(--color-background);
  border: 1px solid var(--color-border-light);
}
.tf-ok-icon { color: var(--color-success); flex-shrink: 0; }
.tf-detect-text { font-size: 12px; color: var(--color-text-secondary); }
.tf-detect-text.muted { color: var(--color-text-tertiary); }
.tf-detect-path { font-family: 'Cascadia Code', monospace; font-size: 11px; color: var(--color-primary); }
.tf-git-badge {
  font-size: 9px; padding: 1px 5px; border-radius: 3px;
  background: var(--color-info-bg); color: var(--color-info);
  margin-left: 4px; font-weight: 600;
}

.setting-row {
  display: flex; justify-content: space-between; align-items: center;
  min-height: 36px; padding: 6px 0;
}
.row-label { display: flex; flex-direction: column; }
.row-title { font-size: 14px; color: var(--color-text); }
.row-help { font-size: 12px; color: var(--color-text-tertiary); margin-top: 4px; }

/* file picker button (inside input suffix) */
.tf-file-btn {
  -webkit-appearance: none; appearance: none;
  border: none; background: transparent; cursor: pointer;
  color: var(--color-text-tertiary); font-size: 13px;
  padding: 2px; display: inline-flex; align-items: center;
}
.tf-file-btn:hover { color: var(--color-primary); }

.tf-info {
  font-size: 12px; color: var(--color-text-tertiary); line-height: 1.6;
  padding: 10px 14px; border-radius: var(--radius-sm);
  background: var(--color-info-bg); border-left: 3px solid var(--color-info);
}
</style>
