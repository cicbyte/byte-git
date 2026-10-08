<script setup>
/**
 * Right-click context menu for the Time-Travel view.
 *
 * Owns the context-menu chrome AND the action handlers. Pure side effects
 * (clipboard, AMessage, the reset-confirm Modal, the actual reset/branch
 * calls) live here; only the steps that touch parent state are emitted:
 *   - `select`       — parent sets activeCommit (cherry-pick / revert /
 *                      create-branch need the row active)
 *   - `goto-log`     — parent navigates to /log
 *   - `reload`       — parent reloads the commit list (after a reset)
 *   - `branch-created` — parent calls repo.detectCurrent()
 *
 * Position/open/hash are two-way bound via props + `update:open`, driven by
 * the parent's ctxMenu state (set from CommitTree's @contextmenu event).
 */
import { ref } from 'vue'
import { message as AMessage } from 'ant-design-vue'
import {
  CopyOutlined, UndoOutlined, FileTextOutlined, HistoryOutlined,
  BranchesOutlined, ForkOutlined, VerticalLeftOutlined, VerticalRightOutlined,
  DownloadOutlined,
} from '@ant-design/icons-vue'
import ContextMenu from '../../../../components/common/ContextMenu.vue'
import CtxItem from '../../../../components/common/CtxItem.vue'
import ExportModal from './ExportModal.vue'
import CreateBranchModal from './CreateBranchModal.vue'

const props = defineProps({
  open: { type: Boolean, default: false },
  x: { type: Number, default: 0 },
  y: { type: Number, default: 0 },
  hash: { type: String, default: '' },
  commits: { type: Array, default: () => [] },
})

const emit = defineEmits([
  'update:open',
  'select',
  'goto-log',
  'reload',
  'branch-created',
])

const wailsApp = window.go?.main?.ByteGitApp

function close() { emit('update:open', false) }

function copyHash() {
  const h = props.hash
  close()
  if (h) { navigator.clipboard.writeText(h); AMessage.success('已复制', 1.2) }
}

function copyMessage() {
  const h = props.hash
  close()
  const c = props.commits.find((x) => x.hash === h)
  if (c?.subject) { navigator.clipboard.writeText(c.subject); AMessage.success('已复制 message', 1.2) }
}

function gotoLogView() {
  close()
  emit('goto-log')
}

function cherryPick() {
  const h = props.hash; close()
  if (!h) return
  AMessage.info({ content: `Cherry-pick ${h.slice(0,7)} — 请在 footer 操作区确认`, duration: 3 })
  emit('select', h)
}

function revert() {
  const h = props.hash; close()
  if (!h) return
  AMessage.info({ content: `Revert ${h.slice(0,7)} — 请在 footer 操作区确认`, duration: 3 })
  emit('select', h)
}

// ---- create branch ----

const branchOpen = ref(false)
const branchHash = ref('')

function createBranch() {
  const h = props.hash; close()
  if (!h) return
  emit('select', h)
  branchHash.value = h
  branchOpen.value = true
}

function softReset() { const h = props.hash; close(); if (h) { emit('select', h); doReset(h, 'soft') } }
function hardReset() { const h = props.hash; close(); if (h) { emit('select', h); doReset(h, 'hard') } }

// ---- export ----

const exportOpen = ref(false)
const exportHash = ref('')

function openExport() {
  const h = props.hash; close()
  if (!h) return
  exportHash.value = h
  exportOpen.value = true
}

// shared reset logic (used by both soft/hard reset menu items)
async function doReset(hash, mode) {
  if (!wailsApp?.CommitResetPreview) return
  let preview = null
  try { preview = await wailsApp.CommitResetPreview(hash) }
  catch (e) { AMessage.error('预览失败: ' + String(e?.message || e)); return }
  if (!preview) return
  const isHard = mode === 'hard'
  const commitList = preview.commits.map((c) => `  ${c.hash}  ${c.subject}`).join('\n')
  let content = ''
  if (preview.count > 0) {
    content += `${isHard ? '⚠ 将永久丢弃' : '将取消'}以下 ${preview.count} 个提交：\n${commitList}\n\n`
  }
  if (!preview.workClean) {
    content += isHard ? '⚠ 工作区有未提交改动，硬回滚将全部丢失！\n\n' : '工作区有改动（软回滚会保留到暂存区）。\n\n'
  }
  if (isHard) content += '如已推送到远程，需 force push。\n\n'
  content += '可通过「备份管理」页面恢复。'
  AMessage.destroy()
  const { Modal } = await import('ant-design-vue')
  Modal.confirm({
    title: isHard ? `🔴 危险：硬回滚到 ${hash.slice(0, 7)}？` : `🟡 软回滚到 ${hash.slice(0, 7)}？`,
    content, okText: isHard ? '我确认，硬回滚' : '确认软回滚',
    okType: isHard ? 'danger' : 'default', cancelText: '取消',
    async onOk() {
      try { await wailsApp.FullBackupCreate(); AMessage.loading('正在创建整库备份…', 2) } catch {}
      try {
        await wailsApp.CommitReset(hash, mode)
        AMessage.success(isHard ? '已硬回滚' : '已软回滚')
        emit('reload')
      } catch (e) { AMessage.error('reset 失败: ' + String(e?.message || e)) }
    },
  })
}
</script>

<template>
  <ContextMenu
    :open="open"
    @update:open="$emit('update:open', $event)"
    :x="x"
    :y="y"
    :title="hash ? hash.slice(0, 7) : ''"
  >
    <CtxItem @click="copyHash"><CopyOutlined /> 复制 Hash</CtxItem>
    <CtxItem @click="copyMessage"><FileTextOutlined /> 复制 Message</CtxItem>
    <CtxItem @click="gotoLogView"><HistoryOutlined /> 在 Git Log 中查看</CtxItem>
    <div class="ctx-sep"></div>
    <CtxItem @click="cherryPick"><BranchesOutlined /> Cherry-pick</CtxItem>
    <CtxItem @click="revert"><UndoOutlined /> Revert 撤销</CtxItem>
    <CtxItem @click="createBranch"><ForkOutlined /> 从此提交创建分支…</CtxItem>
    <div class="ctx-sep"></div>
    <CtxItem @click="openExport"><DownloadOutlined /> 导出此提交源码…</CtxItem>
    <div class="ctx-sep"></div>
    <CtxItem @click="softReset"><VerticalLeftOutlined /> 软回滚（保留改动）</CtxItem>
    <CtxItem danger @click="hardReset"><VerticalRightOutlined /> 硬回滚（丢弃提交）</CtxItem>
  </ContextMenu>

  <ExportModal v-model:open="exportOpen" :hash="exportHash" />
  <CreateBranchModal v-model:open="branchOpen" :hash="branchHash" @done="emit('branch-created')" />
</template>
