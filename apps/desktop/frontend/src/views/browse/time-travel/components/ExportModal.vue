<script setup>
/**
 * Export modal for the Time-Travel view.
 *
 * Lets the user choose:
 *   - Export mode: archive (zip / tar.gz / tar) or directory (no compression)
 *   - Format (when archive mode is selected)
 *
 * Calls ArchiveRun (archive) or ArchiveExportToDir (directory) on the backend,
 * then opens the file manager to reveal the result.
 */
import { ref, watch } from 'vue'
import { message as AMessage } from 'ant-design-vue'
import {
  FileZipOutlined, FolderOpenOutlined, CheckCircleOutlined,
  FileOutlined,
} from '@ant-design/icons-vue'

const props = defineProps({
  open: { type: Boolean, default: false },
  hash: { type: String, default: '' },
})
const emit = defineEmits(['update:open'])

const wailsApp = window.go?.main?.ByteGitApp

const mode = ref('archive')     // 'archive' | 'dir'
const format = ref('zip')       // 'zip' | 'tar.gz' | 'tar'
const busy = ref(false)

const formats = [
  { value: 'zip',   label: 'ZIP',   desc: '通用压缩格式' },
  { value: 'tar.gz', label: 'TAR.GZ', desc: 'Linux 友好' },
  { value: 'tar',   label: 'TAR',   desc: '无压缩归档' },
]

// reset to defaults each time the modal opens
watch(() => props.open, (v) => {
  if (v) { mode.value = 'archive'; format.value = 'zip'; busy.value = false }
})

function close() { emit('update:open', false) }

async function doExport() {
  const h = props.hash
  if (!h) return
  busy.value = true
  try {
    if (mode.value === 'archive') {
      const res = await wailsApp.ArchiveRun({ ref: h, mode: 'standard', format: format.value, output: '' })
      AMessage.success(`已导出 ${res.fileCount || ''} 个文件 (${(res.size / 1024).toFixed(0)} KB)`)
      if (wailsApp.OpenInExplorer) wailsApp.OpenInExplorer(res.outputPath)
    } else {
      const dir = await wailsApp.ArchivePickDir()
      if (!dir) { busy.value = false; return }
      const res = await wailsApp.ArchiveExportToDir(h, dir)
      AMessage.success(`已导出 ${res.fileCount} 个文件到 ${dir}`)
      if (wailsApp.OpenInExplorer) wailsApp.OpenInExplorer(dir)
    }
    close()
  } catch (e) {
    AMessage.error('导出失败: ' + String(e?.message || e))
  } finally {
    busy.value = false
  }
}
</script>

<template>
  <a-modal
    :open="open"
    @update:open="(v) => emit('update:open', v)"
    :footer="null"
    :width="460"
    :body-style="{ padding: 0 }"
    destroy-on-close
    class="export-modal-wrap"
  >
    <!-- Header -->
    <div class="em-header">
      <div class="em-header-left">
        <div class="em-icon-badge"><FileZipOutlined /></div>
        <div>
          <div class="em-title">导出提交源码</div>
          <div class="em-subtitle">
            <code>{{ hash.slice(0, 12) }}</code>
            <span class="em-dot"></span>
            完整文件树快照
          </div>
        </div>
      </div>
    </div>

    <!-- Body -->
    <div class="em-body">
      <!-- Mode cards -->
      <div class="em-cards">
        <div
          class="em-card"
          :class="{ active: mode === 'archive' }"
          @click="mode = 'archive'"
        >
          <FileZipOutlined class="em-card-icon" />
          <div class="em-card-text">
            <div class="em-card-name">压缩包</div>
            <div class="em-card-desc">保存到下载目录</div>
          </div>
          <CheckCircleOutlined v-if="mode === 'archive'" class="em-card-check" />
        </div>
        <div
          class="em-card"
          :class="{ active: mode === 'dir' }"
          @click="mode = 'dir'"
        >
          <FolderOpenOutlined class="em-card-icon" />
          <div class="em-card-text">
            <div class="em-card-name">文件夹</div>
            <div class="em-card-desc">解压到指定目录</div>
          </div>
          <CheckCircleOutlined v-if="mode === 'dir'" class="em-card-check" />
        </div>
      </div>

      <!-- Format selector (archive mode only) -->
      <transition name="em-slide">
        <div v-if="mode === 'archive'" class="em-formats">
          <div class="em-formats-label">格式</div>
          <div class="em-format-list">
            <div
              v-for="f in formats"
              :key="f.value"
              class="em-format"
              :class="{ active: format === f.value }"
              @click="format = f.value"
            >
              <div class="em-format-name">{{ f.label }}</div>
              <div class="em-format-desc">{{ f.desc }}</div>
            </div>
          </div>
        </div>
      </transition>

      <!-- Info hint -->
      <div class="em-hint">
        <CheckCircleOutlined />
        <span v-if="mode === 'archive'">压缩包将保存到下载目录，导出后自动在文件管理器中定位</span>
        <span v-else>将弹出目录选择器，把此提交的完整源码解压到选定目录</span>
      </div>
    </div>

    <!-- Footer -->
    <div class="em-footer">
      <a-button @click="close">取消</a-button>
      <a-button type="primary" :loading="busy" @click="doExport">
        <template #icon><FileOutlined /></template>
        {{ busy ? '导出中…' : '导出' }}
      </a-button>
    </div>
  </a-modal>
</template>

<style scoped>
/* ---- Header ---- */
.em-header {
  padding: 20px 24px 16px;
  border-bottom: 1px solid var(--color-border-light);
}
.em-header-left { display: flex; align-items: center; gap: 12px; }
.em-icon-badge {
  width: 40px; height: 40px;
  border-radius: var(--radius-md);
  background: var(--color-primary-bg);
  color: var(--color-primary);
  display: flex; align-items: center; justify-content: center;
  font-size: 18px;
  flex-shrink: 0;
}
.em-title { font-size: 15px; font-weight: 700; color: var(--color-text); line-height: 1.3; }
.em-subtitle {
  display: flex; align-items: center; gap: 6px;
  font-size: 12px; color: var(--color-text-tertiary); margin-top: 2px;
}
.em-subtitle code { font-family: 'Cascadia Code', monospace; font-weight: 600; color: var(--color-primary); }
.em-dot { width: 3px; height: 3px; border-radius: 50%; background: var(--color-text-tertiary); }

/* ---- Body ---- */
.em-body { padding: 16px 24px; }

/* ---- Mode cards ---- */
.em-cards { display: flex; gap: 10px; }
.em-card {
  flex: 1;
  display: flex; align-items: center; gap: 10px;
  padding: 12px 14px;
  border: 1.5px solid var(--color-border);
  border-radius: var(--radius-md);
  cursor: pointer;
  transition: all 0.2s ease;
  background: var(--color-surface);
  position: relative;
}
.em-card:hover { border-color: var(--color-primary); background: var(--color-hover); }
.em-card.active {
  border-color: var(--color-primary);
  background: var(--color-primary-bg);
  box-shadow: 0 0 0 1px var(--color-primary) inset;
}
.em-card-icon { font-size: 20px; color: var(--color-text-tertiary); flex-shrink: 0; transition: color 0.2s; }
.em-card.active .em-card-icon { color: var(--color-primary); }
.em-card-text { flex: 1; min-width: 0; }
.em-card-name { font-size: 13px; font-weight: 600; color: var(--color-text); }
.em-card-desc { font-size: 11px; color: var(--color-text-tertiary); margin-top: 1px; }
.em-card-check { font-size: 16px; color: var(--color-primary); flex-shrink: 0; }

/* ---- Format selector ---- */
.em-formats { margin-top: 16px; }
.em-formats-label {
  font-size: 11px; font-weight: 600;
  color: var(--color-text-tertiary);
  text-transform: uppercase; letter-spacing: 0.05em;
  margin-bottom: 8px;
}
.em-format-list { display: flex; gap: 8px; }
.em-format {
  flex: 1;
  padding: 8px 10px;
  border: 1.5px solid var(--color-border);
  border-radius: var(--radius-sm);
  cursor: pointer;
  text-align: center;
  transition: all 0.2s ease;
  background: var(--color-surface);
}
.em-format:hover { border-color: var(--color-primary); }
.em-format.active {
  border-color: var(--color-primary);
  background: var(--color-primary-bg);
}
.em-format-name { font-size: 12px; font-weight: 600; color: var(--color-text); font-family: 'Cascadia Code', monospace; }
.em-format-desc { font-size: 10px; color: var(--color-text-tertiary); margin-top: 2px; }

/* ---- Hint ---- */
.em-hint {
  display: flex; align-items: flex-start; gap: 6px;
  margin-top: 14px;
  padding: 8px 10px;
  font-size: 11px;
  color: var(--color-text-tertiary);
  background: var(--color-surface-raised);
  border-radius: var(--radius-sm);
  line-height: 1.5;
}
.em-hint :deep(.anticon) { color: var(--color-primary); margin-top: 1px; flex-shrink: 0; }

/* ---- Footer ---- */
.em-footer {
  display: flex; justify-content: flex-end; gap: 8px;
  padding: 12px 24px;
  border-top: 1px solid var(--color-border-light);
}

/* ---- Transition ---- */
.em-slide-enter-active, .em-slide-leave-active { transition: all 0.2s ease; overflow: hidden; }
.em-slide-enter-from, .em-slide-leave-to { opacity: 0; max-height: 0; margin-top: 0; }
.em-slide-enter-to, .em-slide-leave-from { opacity: 1; max-height: 120px; }
</style>
