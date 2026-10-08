<script setup>
/**
 * Meta 改写预览弹窗 — 从 meta-rewrite/index.vue 拆出。
 * 展示已推送告警 + 级联影响提示 + 字段级改动明细，
 * 并提供整库备份勾选与确认应用入口。
 *
 * 应用成功后 emit 'applied'，由父级负责清空选择并重新加载提交树
 * （历史已变更，内存数据失效）。
 */
import { ref, watch } from 'vue'
import { message as AMessage } from 'ant-design-vue'

const props = defineProps({
  open: Boolean,
  /** 后端预览返回结构：{ pushedChanged, extraAffected, totalAffected, changed[] } */
  preview: { type: Object, default: null },
  /** 仓库参数（repo.repoArg()） */
  repoArg: { type: [Object, Array, String], default: null },
  /** 应用时传入的 entries 数组 */
  entries: { type: Array, default: () => [] },
})

const emit = defineEmits(['update:open', 'applied'])

const app = window.go?.main?.ByteGitApp

// 整库备份默认值（首次打开时从设置读取）
const autoBackup = ref(false)
const applying = ref(false)

async function loadAutoBackupDefault() {
  try { autoBackup.value = await app.RewriteAutoBackupDefault() } catch { /* ignore */ }
}
watch(() => props.open, (v) => { if (v) loadAutoBackupDefault() }, { immediate: true })

async function confirmApply() {
  applying.value = true
  try {
    // MetaRewriteApply(repoArg, entries, false, autoBackup) — 第三个参数恒为 false
    await app.MetaRewriteApply(props.repoArg, props.entries, false, autoBackup.value)
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
    width="680px"
    :footer="null"
  >
    <div v-if="preview" class="pv">
      <a-alert
        v-if="preview.pushedChanged > 0"
        type="error"
        show-icon
        :message="`警告: ${preview.pushedChanged} 条已推送的提交将被改写！`"
        description="改写已推送提交会导致远程历史分叉，后续需 force push，可能影响协作者。"
        style="margin-bottom:12px"
      />
      <a-alert
        v-if="preview.extraAffected > 0"
        type="warning"
        show-icon
        :message="`将级联改写 ${preview.totalAffected} 条（含 ${preview.extraAffected} 条未直接修改的后续提交）`"
        style="margin-bottom:12px"
      />
      <a-alert
        v-if="preview.extraAffected === 0 && preview.pushedChanged === 0"
        type="info"
        show-icon
        :message="`将改写 ${preview.totalAffected} 条提交`"
        style="margin-bottom:12px"
      />
      <div class="pv-list">
        <div v-for="e in preview.changed" :key="e.hash" class="pv-row">
          <span class="pv-hash">{{ (e.hash||'').slice(0,8) }}<span v-if="e.pushed" class="pv-push">📤</span></span>
          <div class="pv-diffs">
            <div v-for="(d, i) in e.diffs" :key="i" class="pv-diff">
              <span class="pv-k">{{ d.field }}</span>
              <span class="pv-old">{{ d.old || '—' }}</span>
              <span class="pv-arrow">→</span>
              <span class="pv-new">{{ d.new || '—' }}</span>
            </div>
          </div>
        </div>
      </div>
      <div class="pv-foot">
        <a-checkbox v-model:checked="autoBackup">应用前整库备份</a-checkbox>
        <span class="pv-note">改写历史不可逆，建议勾选备份</span>
        <div class="pv-spacer"></div>
        <a-button type="primary" :loading="applying" @click="confirmApply">确认应用</a-button>
      </div>
    </div>
  </a-modal>
</template>

<style scoped>
/* preview modal */
.pv-list { max-height: 320px; overflow-y: auto; display: flex; flex-direction: column; gap: 6px; margin-bottom: var(--spacing-md); }
.pv-row { display: flex; gap: var(--spacing-sm); padding: 6px 8px; background: var(--color-background); border-radius: var(--radius-sm); font-size: 12px; }
.pv-hash { font-family: monospace; color: var(--color-hash); flex-shrink: 0; min-width: 80px; }
.pv-push { font-size: 10px; }
.pv-diffs { flex: 1; min-width: 0; }
.pv-diff { display: flex; align-items: baseline; gap: 6px; margin-bottom: 3px; word-break: break-all; line-height: 1.5; }
.pv-k { font-size: 10px; font-weight: 600; color: var(--color-text-tertiary); flex-shrink: 0; min-width: 56px; }
.pv-arrow { color: var(--color-primary); flex-shrink: 0; font-size: 11px; }
.pv-old { color: var(--color-text-tertiary); text-decoration: line-through; }
.pv-new { color: var(--color-success); }
.pv-foot { display: flex; align-items: center; gap: var(--spacing-sm); }
.pv-note { font-size: 12px; color: var(--color-text-tertiary); }
.pv-spacer { flex: 1; }
</style>
