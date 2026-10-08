<script setup>
/**
 * Archive pack modal — extracted from BackupView.
 *
 * Lets the user configure and run a git archive (standard or full mode,
 * zip/tar.gz/tar format, optional path filter). Emits `done` when the
 * archive is created so the parent can refresh its list.
 */
import { ref, watch } from 'vue'
import { message as AMessage } from 'ant-design-vue'
import { FileZipOutlined } from '@ant-design/icons-vue'

const props = defineProps({
  open: { type: Boolean, default: false },
})
const emit = defineEmits(['update:open', 'done'])

const app = window.go?.main?.ByteGitApp

const archForm = ref({ ref: 'HEAD', format: 'tar.gz', mode: 'standard', paths: '' })
const archiving = ref(false)

// reset form each time the modal opens
watch(() => props.open, (v) => {
  if (v) { archForm.value = { ref: 'HEAD', format: 'tar.gz', mode: 'standard', paths: '' }; archiving.value = false }
})

async function runArchive() {
  archiving.value = true
  try {
    await app.ArchiveRun({
      ref: archForm.value.ref || 'HEAD',
      paths: archForm.value.paths ? archForm.value.paths.split(',').map(s => s.trim()).filter(Boolean) : [],
      format: archForm.value.format,
      output: '',
      mode: archForm.value.mode,
    })
    AMessage.success('打包完成')
    emit('update:open', false)
    emit('done')
  } catch (e) {
    AMessage.error('打包失败: ' + String(e?.message || e))
  } finally {
    archiving.value = false
  }
}
</script>

<template>
  <a-modal
    :open="open"
    @update:open="(v) => emit('update:open', v)"
    title="打包压缩仓库"
    :footer="null"
    width="560px"
    destroy-on-close
  >
    <div class="ar-form">
      <div class="ar-row">
        <span class="ar-label">模式</span>
        <a-radio-group v-model:value="archForm.mode" size="small">
          <a-radio-button value="standard">标准（仅文件树）</a-radio-button>
          <a-radio-button value="full">全量（含 .git）</a-radio-button>
        </a-radio-group>
      </div>
      <div class="ar-row">
        <span class="ar-label">Ref</span>
        <a-input v-model:value="archForm.ref" placeholder="HEAD / v1.0.0 / 分支名" size="small" style="width:220px" />
      </div>
      <div class="ar-row">
        <span class="ar-label">格式</span>
        <a-select v-model:value="archForm.format" size="small" style="width:120px" :options="[
          { value: 'tar.gz', label: 'tar.gz' },
          { value: 'zip', label: 'zip' },
          { value: 'tar', label: 'tar' },
        ]" />
      </div>
      <div class="ar-row">
        <span class="ar-label">子目录</span>
        <a-input v-model:value="archForm.paths" placeholder="留空=全部，如 src/,docs/" size="small" style="width:220px" />
        <span class="ar-hint">逗号分隔</span>
      </div>
      <div class="ar-actions">
        <a-button type="primary" :loading="archiving" @click="runArchive">
          <template #icon><FileZipOutlined /></template>{{ archiving ? '打包中...' : '开始打包' }}
        </a-button>
      </div>
    </div>
  </a-modal>
</template>

<style scoped>
.ar-form { display: flex; flex-direction: column; gap: 14px; padding: 4px 0; }
.ar-row { display: flex; align-items: center; gap: 10px; }
.ar-label { width: 50px; font-size: 12px; color: var(--color-text-tertiary); flex-shrink: 0; }
.ar-hint { font-size: 11px; color: var(--color-text-tertiary); }
.ar-actions { margin-top: 4px; }
</style>
