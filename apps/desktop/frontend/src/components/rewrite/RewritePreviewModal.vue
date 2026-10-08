<script setup>
/**
 * 改写预览弹窗 — 从 RewriteDataEditor 拆出。
 * 展示 dry-run 预览结果（受影响提交、字段差异、推送警告、级联数量），
 * 并提供「确认应用改写」入口。
 */
import { ref, watch } from 'vue'
import { message as AMessage } from 'ant-design-vue'

const props = defineProps({
  open: { type: Boolean, default: false },
  // 改写数据行（MetaEntryDTO[]），用于发起预览/应用请求
  rows: { type: Array, default: () => [] },
  repoArg: { type: String, default: '' },
})
const emit = defineEmits(['update:open', 'applied'])

const app = window.go?.main?.ByteGitApp

const preview = ref(null)
const previewing = ref(false)
const applying = ref(false)
const autoBackup = ref(false)

async function loadAutoBackupDefault() {
  try { autoBackup.value = await app.RewriteAutoBackupDefault() } catch { /* ignore */ }
}

// 弹窗首次打开时加载预览数据 + 默认备份开关
watch(() => props.open, async (visible) => {
  if (!visible) return
  await loadAutoBackupDefault()
  previewing.value = true
  try {
    preview.value = await app.MetaRewritePreview(props.repoArg, props.rows)
  } catch (e) {
    AMessage.error('预览失败: ' + String(e?.message || e))
    preview.value = null
  } finally {
    previewing.value = false
  }
})

function close() { emit('update:open', false) }

async function confirmApply() {
  applying.value = true
  try {
    await app.MetaRewriteApply(props.repoArg, props.rows, false, autoBackup.value)
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
    title="改写预览（dry-run）"
    width="720px"
    :footer="null"
    destroy-on-close
    @update:open="(v) => emit('update:open', v)"
  >
    <a-spin :spinning="previewing">
      <div v-if="preview && preview.changed" class="rde-pv">
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
        <div class="rde-pv-list">
          <div v-for="e in preview.changed" :key="e.hash" class="rde-pv-row">
            <span class="rde-pv-hash">{{ (e.hash || '').slice(0, 8) }}<span v-if="e.pushed" class="rde-pv-push">推送</span></span>
            <div class="rde-pv-diffs">
              <div v-for="(d, i) in e.diffs" :key="i" class="rde-pv-diff">
                <span class="rde-pv-k">{{ d.field }}</span>
                <span class="rde-pv-old">{{ d.old || '—' }}</span>
                <span class="rde-pv-arrow">→</span>
                <span class="rde-pv-new">{{ d.new || '—' }}</span>
              </div>
            </div>
          </div>
        </div>
        <!-- 底部操作栏 -->
        <div class="rde-pv-foot">
          <a-checkbox v-model:checked="autoBackup">应用前整库备份</a-checkbox>
          <span class="rde-pv-note">改写历史不可逆，建议勾选备份</span>
          <span class="rde-pv-spacer"></span>
          <a-button @click="close">取消</a-button>
          <a-button type="primary" :loading="applying" @click="confirmApply">确认应用改写</a-button>
        </div>
      </div>
      <div v-else-if="!previewing" class="rde-pv-empty">无可预览的改写</div>
    </a-spin>
  </a-modal>
</template>

<style scoped>
.rde-pv-list { max-height: 50vh; overflow-y: auto; display: flex; flex-direction: column; gap: 6px; }
.rde-pv-row { display: flex; gap: 10px; padding: 8px 10px; background: var(--color-background); border-radius: var(--radius-sm); font-size: 12px; }
.rde-pv-hash { font-family: 'Cascadia Code', monospace; color: var(--color-hash); flex-shrink: 0; min-width: 90px; display: flex; align-items: flex-start; gap: 4px; padding-top: 1px; }
.rde-pv-push { font-size: 9px; color: var(--color-text-tertiary); background: var(--color-surface-raised); padding: 0 4px; border-radius: 4px; }
.rde-pv-diffs { flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 3px; }
.rde-pv-diff { display: flex; align-items: baseline; gap: 6px; word-break: break-all; line-height: 1.5; }
.rde-pv-k { font-size: 10px; font-weight: 600; color: var(--color-text-tertiary); flex-shrink: 0; min-width: 56px; }
.rde-pv-old { color: var(--color-text-tertiary); text-decoration: line-through; white-space: pre-wrap; }
.rde-pv-arrow { color: var(--color-primary); flex-shrink: 0; font-size: 11px; }
.rde-pv-new { color: var(--color-success); white-space: pre-wrap; }
.rde-pv-foot { display: flex; align-items: center; gap: 10px; margin-top: 14px; padding-top: 12px; border-top: 1px solid var(--color-border-light); }
.rde-pv-note { font-size: 11px; color: var(--color-text-tertiary); }
.rde-pv-spacer { flex: 1; }
.rde-pv-empty { text-align: center; color: var(--color-text-tertiary); padding: 40px 0; }
</style>
