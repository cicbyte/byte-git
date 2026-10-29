<script setup>
import { ref, watch, computed } from 'vue'
import { storeToRefs } from 'pinia'
import { Modal, message as AMessage } from 'ant-design-vue'
import {
  CheckCircleOutlined, PlusOutlined, MinusOutlined,
  UndoOutlined, ReloadOutlined, SaveOutlined, CloseOutlined,
  CloudUploadOutlined, DownOutlined,
} from '@ant-design/icons-vue'
import AiSparkIcon from '@/components/common/AiSparkIcon.vue'
import { useScmStore, COMMIT_TYPES } from '@/stores/scm'

/**
 * 全局源代码管理抽屉（参考 VSCode SCM 面板）。
 * 从 Navbar 的 SCM 按钮触发，v-model:open 双向绑定。
 *
 * 结构：
 *   ┌─ commit 输入区（类型模板 / textarea / AI / 提交按钮）
 *   ├─ Staged Changes（每行：文件 + 取消暂存图标）
 *   └─ Changes（每行：文件 + 暂存/放弃更改图标）
 *
 * 不含 diff 展示——纯操作面板，看 diff 走 LiveView。
 */
const props = defineProps({ open: Boolean })
const emit = defineEmits(['update:open'])

const scm = useScmStore()
const { status, loading, commitMsg, generating, committing, noVerify, tplScope,
  stagedCount, unstagedCount, untrackedCount, ahead, pushing, pushStatus } = storeToRefs(scm)

// 抽屉打开时刷新
watch(() => props.open, (v) => { if (v) scm.refresh() })

// ---- file path split: gray dir prefix + white basename (VSCode style) ----
function splitPath(p) {
  const idx = p.lastIndexOf('/')
  if (idx < 0) return { dir: '', name: p }
  return { dir: p.slice(0, idx + 1), name: p.slice(idx + 1) }
}

// ---- force push confirmation ----
function confirmForcePush() {
  Modal.confirm({
    title: '强制推送？',
    content: '将使用 --force-with-lease 强制推送。这会覆盖远端历史，可能导致其他协作者的本地分支分叉。如果远端有你未拉取的新提交，推送将被拒绝。',
    okText: '强制推送',
    okType: 'danger',
    cancelText: '取消',
    onOk() { scm.push(true) },
  })
}

// ---- button mode: "commit" when there are staged changes, "push" otherwise ----
// When staged files exist → commit mode (primary action).
// When no staged changes but ahead > 0 → push mode.
// Both zero → disabled (or guidance shown for no-remote / no-upstream cases).
const isPushMode = computed(() => stagedCount.value === 0 && ahead.value > 0)
// guidance: show a hint when the user has committed but can't push yet
const pushHint = computed(() => {
  const ps = pushStatus.value
  if (!ps) return ''
  if (stagedCount.value > 0 || ahead.value === 0) return ''
  if (!ps.hasRemote) {
    return '此仓库没有配置远端。推送需先添加 remote：git remote add origin <url>'
  }
  if (!ps.hasUpstream) {
    return `分支「${ps.branch || '当前'}」尚未关联远端。点击推送将自动设置上游到 ${ps.remote}。`
  }
  return ''
})

// status label → color class
function statusClass(s) {
  if (s === 'added') return 'st-A'
  if (s === 'deleted') return 'st-D'
  if (s === 'renamed' || s === 'copied') return 'st-R'
  if (s === 'untracked') return 'st-U'
  return 'st-M'
}
// short status letter for the row chip
function statusLetter(s) {
  if (s === 'added') return 'A'
  if (s === 'deleted') return 'D'
  if (s === 'renamed') return 'R'
  if (s === 'copied') return 'C'
  if (s === 'untracked') return 'U'
  return 'M'
}

// ---- per-row actions ----
function stageOne(f) { scm.stage([f.path]) }
function unstageOne(f) { scm.unstage([f.path]) }

function discardTrackedOne(f) {
  Modal.confirm({
    title: '放弃更改？',
    content: `「${f.path}」的工作区改动将被恢复到 HEAD，此操作不可撤销。`,
    okText: '放弃更改',
    okType: 'danger',
    cancelText: '取消',
    onOk: () => scm.discardTracked([f.path]),
  })
}

function discardUntrackedOne(f) {
  Modal.confirm({
    title: '永久删除文件？',
    content: `未跟踪文件「${f.path}」将被物理删除（git clean -f），此操作不可撤销，且无法从 git 恢复。`,
    okText: '永久删除',
    okType: 'danger',
    cancelText: '取消',
    onOk: () => scm.discardUntracked([f.path]),
  })
}

// ---- template CRUD (shared with LiveView's modal; minimal inline version) ----
const tplModalOpen = ref(false)
</script>

<template>
  <a-drawer
    :open="open"
    @update:open="(v) => emit('update:open', v)"
    title="源代码管理"
    placement="right"
    :width="420"
    :body-style="{ padding: 0, display: 'flex', flexDirection: 'column', height: '100%' }"
    :header-style="{ flexShrink: 0 }"
  >
    <!-- commit input region -->
    <div class="commit-region">
      <!-- type quick buttons + scope -->
      <div class="tpl-toolbar" :class="{ disabled: generating }">
        <div class="tpl-types">
          <button
            v-for="t in COMMIT_TYPES" :key="t.type"
            class="tpl-type-btn" :style="{ '--tc': t.color }"
            :title="t.desc"
            :disabled="generating"
            @click="scm.applyType(t.type)"
          >{{ t.label }}</button>
        </div>
        <input
          class="tpl-scope-input"
          v-model="tplScope"
          placeholder="scope (可选)"
          :disabled="generating"
        />
      </div>

      <div class="msg-input-wrap">
        <textarea
          class="msg-input"
          v-model="commitMsg"
          :readonly="generating"
          :placeholder="generating ? 'AI 生成中…' : '输入 commit message（⌘/Ctrl+Enter 提交）'"
          rows="3"
          @keydown.ctrl.enter.prevent="scm.commit()"
          @keydown.meta.enter.prevent="scm.commit()"
        ></textarea>
        <!-- suffix actions inside the input area -->
        <div class="msg-suffix">
          <button
            class="msg-suffix-btn ai"
            :class="{ loading: generating }"
            :disabled="generating"
            title="AI 生成"
            @click="scm.aiGenerate()"
          >
            <AiSparkIcon />
          </button>
          <button
            v-if="commitMsg"
            class="msg-suffix-btn"
            :disabled="generating"
            title="清空"
            @click="scm.reset()"
          >
            <CloseOutlined />
          </button>
        </div>
      </div>

      <!-- push guidance: shown when committed but can't push normally -->
      <div v-if="pushHint" class="push-hint">{{ pushHint }}</div>

      <!-- smart action button: commit → push (auto-switches based on state) -->
      <!-- commit mode: single button -->
      <a-button
        v-if="!isPushMode"
        type="primary"
        block
        :loading="committing"
        :disabled="generating || !commitMsg.trim() || !stagedCount"
        @click="scm.commit()"
        class="commit-btn"
      >
        <template #icon><CheckCircleOutlined /></template>
        提交{{ stagedCount ? ` (${stagedCount} 个文件)` : '' }}
      </a-button>
      <!-- push mode: main push button + separate dropdown caret (split button) -->
      <div v-else class="push-split">
        <a-button
          type="primary"
          :loading="pushing"
          @click="scm.push(false)"
          class="push-main"
        >
          <template #icon><CloudUploadOutlined /></template>
          推送{{ ahead ? ` (${ahead} 个提交)` : '' }}
        </a-button>
        <a-dropdown placement="topRight" :trigger="['click']">
          <a-button type="primary" class="push-caret-btn" :disabled="pushing">
            <DownOutlined />
          </a-button>
          <template #overlay>
            <a-menu>
              <a-menu-item @click="scm.push(false)">
                <CloudUploadOutlined /> 推送 ({{ ahead }} 个提交)
              </a-menu-item>
              <a-menu-divider />
              <a-menu-item danger @click="confirmForcePush">
                <CloudUploadOutlined /> 强制推送 (--force-with-lease)
              </a-menu-item>
            </a-menu>
          </template>
        </a-dropdown>
      </div>
    </div>

    <!-- file list region -->
    <div class="files-region">
      <div class="region-toolbar">
        <span class="region-title">{{ stagedCount + unstagedCount + untrackedCount }} 个更改</span>
        <a-button size="small" type="text" :loading="loading" @click="scm.refresh()" title="刷新">
          <template #icon><ReloadOutlined /></template>
        </a-button>
      </div>

      <div class="files-scroll">
        <!-- Staged Changes -->
        <div v-if="stagedCount" class="group">
          <div class="group-head">
            <span>已暂存 · Staged ({{ stagedCount }})</span>
            <button class="grp-act" title="取消全部暂存" @click="scm.unstageAll()">
              <MinusOutlined />
            </button>
          </div>
          <div
            v-for="f in status.staged" :key="'s-' + f.path"
            class="file-row"
          >
            <span class="status-chip" :class="statusClass(f.status)">{{ statusLetter(f.status) }}</span>
            <span class="fname">
              <span class="fname-dir">{{ splitPath(f.path).dir }}</span>{{ splitPath(f.path).name }}
            </span>
            <span class="row-acts">
              <button class="row-act" title="取消暂存" @click="unstageOne(f)">
                <MinusOutlined />
              </button>
            </span>
          </div>
        </div>

        <!-- Changes (unstaged) -->
        <div v-if="unstagedCount" class="group">
          <div class="group-head">
            <span>更改 · Changes ({{ unstagedCount }})</span>
            <button class="grp-act" title="全部暂存" @click="scm.stageAll()">
              <PlusOutlined />
            </button>
          </div>
          <div
            v-for="f in status.unstaged" :key="'u-' + f.path"
            class="file-row"
          >
            <span class="status-chip" :class="statusClass(f.status)">{{ statusLetter(f.status) }}</span>
            <span class="fname">
              <span class="fname-dir">{{ splitPath(f.path).dir }}</span>{{ splitPath(f.path).name }}
            </span>
            <span class="row-acts">
              <button class="row-act" title="暂存" @click="stageOne(f)">
                <PlusOutlined />
              </button>
              <button class="row-act danger" title="放弃更改" @click="discardTrackedOne(f)">
                <UndoOutlined />
              </button>
            </span>
          </div>
        </div>

        <!-- Untracked -->
        <div v-if="untrackedCount" class="group">
          <div class="group-head">
            <span>未跟踪 · Untracked ({{ untrackedCount }})</span>
          </div>
          <div
            v-for="f in status.untracked" :key="'t-' + f.path"
            class="file-row"
          >
            <span class="status-chip st-U">U</span>
            <span class="fname">
              <span class="fname-dir">{{ splitPath(f.path).dir }}</span>{{ splitPath(f.path).name }}
            </span>
            <span class="row-acts">
              <button class="row-act" title="暂存（git add）" @click="stageOne(f)">
                <PlusOutlined />
              </button>
              <button class="row-act danger" title="删除未跟踪文件" @click="discardUntrackedOne(f)">
                <UndoOutlined />
              </button>
            </span>
          </div>
        </div>

        <!-- empty state -->
        <div v-if="!stagedCount && !unstagedCount && !untrackedCount && !loading" class="empty">
          <CheckCircleOutlined style="font-size:28px;opacity:0.3" />
          <div style="margin-top:8px">工作区干净，无待提交的更改</div>
        </div>
      </div>
    </div>
  </a-drawer>
</template>

<style scoped>
/* commit region — pinned top, shrink-wrapped */
.commit-region {
  flex-shrink: 0;
  padding: 12px 16px;
  border-bottom: 1px solid var(--color-border);
  background: var(--color-surface);
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.tpl-toolbar.disabled { opacity: 0.5; pointer-events: none; }
.tpl-types { display: flex; flex-wrap: wrap; gap: 4px; margin-bottom: 6px; }
.tpl-type-btn {
  padding: 2px 8px; font-size: 11px; font-family: 'Cascadia Code', monospace; font-weight: 600;
  color: var(--tc); background: color-mix(in srgb, var(--tc) 10%, transparent);
  border: 1px solid color-mix(in srgb, var(--tc) 30%, transparent); border-radius: var(--radius-xs);
  cursor: pointer; transition: all var(--transition-fast);
}
.tpl-type-btn:hover { background: color-mix(in srgb, var(--tc) 20%, transparent); }
.tpl-type-btn:disabled { cursor: not-allowed; opacity: 0.5; }
.tpl-scope-input {
  width: 100%; padding: 3px 8px; font-size: 12px;
  border: 1px solid var(--color-border); border-radius: var(--radius-xs);
  background: var(--color-background); color: var(--color-text); outline: none;
}
.tpl-scope-input:focus { border-color: var(--color-primary); }

.msg-input-wrap { position: relative; }
.msg-input {
  width: 100%; resize: vertical; padding: 8px 64px 8px 8px;
  border: 1px solid var(--color-border); border-radius: var(--radius-sm);
  background: var(--color-background); color: var(--color-text);
  font-family: 'Cascadia Code', 'Fira Code', monospace; font-size: 13px; line-height: 1.5;
  outline: none; min-height: 60px;
}
.msg-input:focus { border-color: var(--color-primary); }
.msg-input:read-only { opacity: 0.7; }

/* suffix action overlay inside the textarea (top-right) */
.msg-suffix {
  position: absolute;
  top: 6px;
  right: 6px;
  display: flex;
  align-items: center;
  gap: 2px;
  z-index: 1;
}
.msg-suffix-btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 24px;
  height: 24px;
  border: none;
  border-radius: var(--radius-xs);
  background: transparent;
  color: var(--color-text-tertiary);
  cursor: pointer;
  font-size: 14px;
  transition: all var(--transition-fast);
}
.msg-suffix-btn:hover { background: var(--color-hover); color: var(--color-text); }
.msg-suffix-btn:disabled { cursor: not-allowed; opacity: 0.4; }
.msg-suffix-btn.ai { color: var(--color-primary); font-size: 15px; }
.msg-suffix-btn.ai:hover { background: var(--color-primary-bg); }
.msg-suffix-btn.ai.loading { animation: ai-spin 1s linear infinite; }
@keyframes ai-spin { to { transform: rotate(360deg); } }

.commit-actions { display: flex; align-items: center; gap: 8px; }
.push-hint {
  margin-top: 4px; padding: 6px 10px; font-size: 11px; line-height: 1.5;
  color: var(--color-text-secondary); background: var(--color-warning-bg);
  border-radius: var(--radius-xs); border-left: 2px solid var(--color-warning);
}
.commit-btn { margin-top: 2px; width: 100%; }
/* push mode: main button + caret in a flex row (split button) */
.push-split { margin-top: 2px; display: flex; width: 100%; }
.push-main { flex: 1; }
.push-caret-btn { flex-shrink: 0; width: 32px; padding: 0; }

/* file list region — fills remaining height */
.files-region {
  flex: 1; display: flex; flex-direction: column; min-height: 0;
  background: var(--color-surface);
}
.region-toolbar {
  display: flex; align-items: center; justify-content: space-between;
  padding: 6px 16px; flex-shrink: 0;
  border-bottom: 1px solid var(--color-border-light);
}
.region-title { font-size: 11px; color: var(--color-text-tertiary); font-weight: 600; text-transform: uppercase; letter-spacing: 0.5px; }

.files-scroll { flex: 1; overflow-y: auto; padding: 4px 0; }

.group { margin-bottom: 4px; }
.group-head {
  display: flex; align-items: center; justify-content: space-between;
  padding: 6px 16px 2px;
  font-size: 11px; font-weight: 600; color: var(--color-text-secondary);
}
.grp-act {
  border: none; background: transparent; cursor: pointer;
  color: var(--color-text-tertiary); padding: 2px 4px; border-radius: var(--radius-xs);
  font-size: 11px; display: inline-flex; align-items: center;
  transition: all var(--transition-fast);
}
.grp-act:hover { color: var(--color-primary); background: var(--color-hover); }

.file-row {
  display: flex; align-items: center; gap: 8px;
  padding: 3px 16px; font-size: 12px;
  transition: background var(--transition-fast);
}
.file-row:hover { background: var(--color-hover); }

.status-chip {
  font-size: 9px; font-weight: 700; width: 14px; text-align: center;
  flex-shrink: 0; font-family: 'Cascadia Code', monospace;
}
.st-A { color: var(--color-success); }
.st-M { color: var(--color-warning); }
.st-D { color: var(--color-error); }
.st-R { color: var(--color-info); }
.st-U { color: var(--color-text-tertiary); }

.fname {
  flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
  color: var(--color-text);
}
.fname-dir { color: var(--color-text-tertiary); }

.row-acts {
  display: flex; gap: 2px; flex-shrink: 0;
  opacity: 0; transition: opacity var(--transition-fast);
}
.file-row:hover .row-acts { opacity: 1; }
.row-act {
  border: none; background: transparent; cursor: pointer;
  color: var(--color-text-tertiary); padding: 3px 4px; border-radius: var(--radius-xs);
  font-size: 11px; display: inline-flex; align-items: center;
  transition: all var(--transition-fast);
}
.row-act:hover { color: var(--color-primary); background: var(--color-surface-raised); }
.row-act.danger:hover { color: var(--color-error); }

.empty {
  display: flex; flex-direction: column; align-items: center; justify-content: center;
  padding: 60px 0; color: var(--color-text-tertiary); font-size: 13px;
}
</style>
