<script setup>
/**
 * Footer injected by LogView / TimeTravelView — shows the selected commit's
 * full info + quick actions: copy hash, cherry-pick, revert, soft/hard reset,
 * view in terminal. Each dangerous action has a confirmation modal with
 * optional full-backup checkbox (reuses the existing backup module).
 */
import {
  CopyOutlined, BranchesOutlined, UndoOutlined, CodeOutlined,
  VerticalLeftOutlined, VerticalRightOutlined,
} from '@ant-design/icons-vue'
import { ref } from 'vue'
import { message as AMessage, Modal } from 'ant-design-vue'

const props = defineProps({
  hash: { type: String, default: '' },
  author: { type: String, default: '' },
  date: { type: String, default: '' },
  subject: { type: String, default: '' },
  added: { type: Number, default: 0 },
  deleted: { type: Number, default: 0 },
  fileCount: { type: Number, default: 0 },
})
const emit = defineEmits(['copy-hash', 'view-terminal'])

const menuOpen = ref(false)
const app = window.go?.main?.ByteGitApp

// ---- preview state for reset modals ----
const previewLoading = ref(false)

function copyHash() {
  if (!props.hash) return
  navigator.clipboard.writeText(props.hash).then(() => AMessage.success('已复制 hash', 1.2))
}

function confirmCherryPick() {
  if (!props.hash) return
  Modal.confirm({
    title: 'Cherry-pick 此提交？',
    content: `将把 ${props.hash.slice(0, 7)} 的改动应用到当前分支。`,
    okText: '确认', cancelText: '取消',
    async onOk() {
      try { await app.CommitCherryPick(props.hash); AMessage.success('Cherry-pick 成功') }
      catch (e) { AMessage.error('失败: ' + String(e?.message || e)) }
    },
  })
}

function confirmRevert() {
  if (!props.hash) return
  Modal.confirm({
    title: 'Revert 此提交？',
    content: `将创建一个反向提交来撤销 ${props.hash.slice(0, 7)}。`,
    okText: '确认', cancelText: '取消',
    async onOk() {
      try { await app.CommitRevert(props.hash); AMessage.success('Revert 成功') }
      catch (e) { AMessage.error('失败: ' + String(e?.message || e)) }
    },
  })
}

// ---- reset with full-flow protection ----
async function confirmReset(mode) {
  if (!props.hash) return
  menuOpen.value = false
  previewLoading.value = true

  // fetch preview (commits that will be discarded + work-tree status)
  let preview = null
  try {
    preview = await app.CommitResetPreview(props.hash)
  } catch (e) {
    AMessage.error('预览失败: ' + String(e?.message || e))
    previewLoading.value = false
    return
  }
  previewLoading.value = false
  if (!preview) return

  const isHard = mode === 'hard'
  const danger = isHard && preview.count > 0
  const backupChecked = ref(danger) // default check backup for hard resets

  // build the confirmation modal content
  const commitList = preview.commits.map(
    (c) => `  ${c.hash}  ${c.subject}`,
  ).join('\n')

  let content = ''
  if (preview.count > 0) {
    content += `${isHard ? '⚠ 将永久丢弃' : '将取消'}以下 ${preview.count} 个提交：\n${commitList}\n\n`
  }
  if (!preview.workClean) {
    content += isHard
      ? '⚠ 工作区有未提交改动，硬回滚将全部丢失！\n\n'
      : '工作区有改动（软回滚会保留到暂存区）。\n\n'
  }
  if (isHard) {
    content += '如已推送到远程，需 force push，可能影响协作者。\n\n'
  }
  content += '可通过「备份管理」页面恢复。'

  Modal.confirm({
    title: isHard
      ? `🔴 危险：硬回滚到 ${props.hash.slice(0, 7)}？`
      : `🟡 软回滚到 ${props.hash.slice(0, 7)}？`,
    content: content,
    okText: isHard ? '我确认，硬回滚' : '确认软回滚',
    okType: isHard ? 'danger' : 'default',
    cancelText: '取消',
    async onOk() {
      // optional full backup before reset
      if (backupChecked.value) {
        try {
          await app.FullBackupCreate()
          AMessage.loading('正在创建整库备份…', 2)
        } catch {
          AMessage.warning('备份失败，是否继续？请在备份管理页检查。')
        }
      }
      try {
        await app.CommitReset(props.hash, mode)
        AMessage.success(isHard ? '已硬回滚' : '已软回滚')
        // reload the current page's data
        window.location.reload()
      } catch (e) {
        AMessage.error('reset 失败: ' + String(e?.message || e))
      }
    },
  })
}
</script>

<template>
  <div class="lf-bar">
    <template v-if="hash">
      <span class="lf-hash" :title="hash" @click="copyHash">{{ hash.slice(0, 7) }}</span>
      <span class="lf-author">{{ author }}</span>
      <span class="lf-stat" v-if="fileCount">
        <span class="lf-add">+{{ added }}</span>
        <span class="lf-del">-{{ deleted }}</span>
        <span class="lf-files">{{ fileCount }} 文件</span>
      </span>
      <span class="lf-time">{{ date }}</span>
      <span class="lf-subject" :title="subject">{{ subject }}</span>

      <!-- quick actions -->
      <div class="lf-actions">
        <button class="lf-btn" @click="copyHash" title="复制 Hash"><CopyOutlined /></button>
        <button class="lf-btn" @click="confirmCherryPick" title="Cherry-pick"><BranchesOutlined /></button>
        <button class="lf-btn" @click="confirmRevert" title="Revert 撤销"><UndoOutlined /></button>
        <a-dropdown :trigger="['click']" v-model:open="menuOpen" placement="topRight">
          <button class="lf-btn" title="回滚操作"><VerticalRightOutlined /></button>
          <template #overlay>
            <a-menu>
              <a-menu-item @click="confirmReset('soft')">
                <VerticalLeftOutlined /> 软回滚（保留改动到暂存区）
              </a-menu-item>
              <a-menu-divider />
              <a-menu-item danger @click="confirmReset('hard')">
                <VerticalRightOutlined /> 硬回滚（丢弃后续所有提交）
              </a-menu-item>
            </a-menu>
          </template>
        </a-dropdown>
        <button class="lf-btn" @click="emit('view-terminal', hash)" title="在终端查看"><CodeOutlined /></button>
      </div>
    </template>
    <span v-else class="lf-empty">选择一个提交查看详情</span>
    <a-spin v-if="previewLoading" size="small" class="lf-preview-spin" />
  </div>
</template>

<style scoped>
.lf-bar {
  display: flex; align-items: center; gap: var(--spacing-sm);
  width: 100%; font-size: 12px; color: var(--color-text-secondary);
  user-select: none; overflow: hidden;
}
.lf-hash {
  font-family: 'Cascadia Code', monospace;
  font-size: 11px; color: var(--color-hash);
  flex-shrink: 0; font-weight: 600; cursor: pointer;
}
.lf-hash:hover { text-decoration: underline; }
.lf-author { flex-shrink: 0; color: var(--color-text); font-weight: 500; }
.lf-stat { display: inline-flex; gap: 6px; flex-shrink: 0; font-family: 'Cascadia Code', monospace; }
.lf-add { color: var(--color-success); }
.lf-del { color: var(--color-error); }
.lf-files { color: var(--color-text-tertiary); }
.lf-time { flex-shrink: 0; color: var(--color-text-tertiary); font-size: 11px; }
.lf-subject {
  flex: 1; min-width: 0; color: var(--color-text);
  overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
}
.lf-actions { display: flex; gap: 2px; flex-shrink: 0; margin-left: 4px; }
.lf-btn {
  -webkit-appearance: none; appearance: none;
  display: inline-flex; align-items: center; justify-content: center;
  flex-shrink: 0; width: 24px; height: 24px; min-width: 24px; max-width: 24px;
  padding: 0; border: none; border-radius: var(--radius-xs);
  background: transparent; color: var(--color-text-tertiary);
  cursor: pointer; font-size: 12px; line-height: 1; box-sizing: border-box;
  transition: all var(--transition-fast);
}
.lf-btn:hover { background: var(--color-hover); color: var(--color-text); }
.lf-empty { color: var(--color-text-tertiary); }
.lf-preview-spin { margin-left: 4px; }
</style>
