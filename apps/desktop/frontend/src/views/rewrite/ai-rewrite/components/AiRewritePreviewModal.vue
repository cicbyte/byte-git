<script setup>
/**
 * AI 改写预览弹窗 — 从 ai-rewrite/index.vue 拆出。
 * 展示级联影响提示 + 改写明细列表，并提供整库备份勾选与确认应用入口。
 */
import { ref, watch } from 'vue'
import { message as AMessage } from 'ant-design-vue'

const props = defineProps({
  open: Boolean,
  /** 后端预览返回的结构：{ totalAffected, extraAffected, entries[] } */
  preview: { type: Object, default: null },
  /** 后端应用方法所需入参（repoArg + tasks），由父组件提供 */
  repoArg: { type: [Object, Array, String], default: null },
  /** 应用时传入的任务数组 */
  tasks: { type: Array, default: () => [] },
})

const emit = defineEmits(['update:open', 'applied'])

const app = window.go?.main?.ByteGitApp

// 整库备份默认值（首次打开时从设置读取一次即可）
const autoBackup = ref(false)
const applying = ref(false)

async function loadAutoBackupDefault() {
  try { autoBackup.value = await app.RewriteAutoBackupDefault() } catch { /* ignore */ }
}
watch(() => props.open, (v) => { if (v) loadAutoBackupDefault() }, { immediate: true })

async function confirmApply() {
  applying.value = true
  try {
    await app.RewriteApply(props.repoArg, props.tasks, autoBackup.value)
    AMessage.success('改写完成' + (autoBackup.value ? '（已创建整库备份）' : ''))
    emit('update:open', false)
    emit('applied')
  } catch (e) {
    AMessage.error('改写失败: ' + String(e?.message || e))
  } finally {
    applying.value = false
  }
}
</script>

<template>
  <a-modal
    :open="open"
    @update:open="(v) => emit('update:open', v)"
    title="改写预览"
    width="640px"
    :footer="null"
  >
    <div v-if="preview" class="pv">
      <a-alert
        v-if="preview.extraAffected > 0"
        type="warning"
        show-icon
        :message="`将级联改写 ${preview.totalAffected} 条提交（含 ${preview.extraAffected} 条后续）`"
        style="margin-bottom:12px"
      />
      <a-alert
        v-else
        type="info"
        show-icon
        :message="`将改写 ${preview.totalAffected} 条提交`"
        style="margin-bottom:12px"
      />
      <div class="pv-list">
        <div v-for="e in preview.entries" :key="e.hash" class="pv-row">
          <span class="pv-hash">{{ e.hash.slice(0, 8) }}</span>
          <span class="pv-orig">{{ e.original }}</span>
          <span class="pv-arrow">→</span>
          <span class="pv-new">{{ e.suggested }}</span>
        </div>
      </div>
      <div class="pv-foot">
        <a-checkbox v-model:checked="autoBackup">应用前整库备份</a-checkbox>
        <span class="pv-note">改写历史不可逆，建议勾选备份</span>
        <div class="pv-spacer"></div>
        <a-button type="primary" :loading="applying" @click="confirmApply">确认应用改写</a-button>
      </div>
    </div>
  </a-modal>
</template>

<style scoped>
.pv-list { max-height: 300px; overflow-y: auto; display: flex; flex-direction: column; gap: 4px; margin-bottom: 12px; }
.pv-row { display: flex; align-items: center; gap: 8px; font-size: 12px; padding: 6px 8px; background: var(--color-background); border-radius: var(--radius-sm); }
.pv-hash { font-family: monospace; color: var(--color-hash); flex-shrink: 0; }
.pv-orig { color: var(--color-text-tertiary); text-decoration: line-through; flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.pv-arrow { color: var(--color-text-tertiary); }
.pv-new { color: var(--color-success); flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.pv-foot { display: flex; align-items: center; gap: 8px; }
.pv-note { font-size: 12px; color: var(--color-text-tertiary); }
.pv-spacer { flex: 1; }
</style>
