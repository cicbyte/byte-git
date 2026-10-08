<script setup>
/**
 * Create-branch modal — extracted from TimeTravelContextMenu.
 *
 * Prompts for a branch name with a default suggestion, then calls
 * CommitCreateBranch on the backend. Emits `done` on success so the
 * parent can refresh repo state.
 */
import { ref, watch } from 'vue'
import { message as AMessage } from 'ant-design-vue'
import { ForkOutlined } from '@ant-design/icons-vue'

const props = defineProps({
  open: { type: Boolean, default: false },
  hash: { type: String, default: '' },
})
const emit = defineEmits(['update:open', 'done'])

const wailsApp = window.go?.main?.ByteGitApp

const branchName = ref('')
const busy = ref(false)

// pre-fill with a sensible default each time the modal opens
watch(() => props.open, (v) => {
  if (v) {
    branchName.value = props.hash ? `from-${props.hash.slice(0, 7)}` : ''
    busy.value = false
  }
})

function close() { emit('update:open', false) }

async function submit() {
  const name = branchName.value.trim()
  if (!name) { AMessage.warning('请输入分支名'); return }
  busy.value = true
  try {
    await wailsApp.CommitCreateBranch(props.hash, name)
    AMessage.success(`已创建并切换到分支: ${name}`)
    emit('done')
    close()
  } catch (e) {
    AMessage.error('创建分支失败: ' + String(e?.message || e))
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
    :width="420"
    :body-style="{ padding: 0 }"
    destroy-on-close
  >
    <!-- Header -->
    <div class="cb-header">
      <div class="cb-icon-badge"><ForkOutlined /></div>
      <div>
        <div class="cb-title">从此提交创建分支</div>
        <div class="cb-subtitle">
          基于提交 <code>{{ hash.slice(0, 12) }}</code> 创建并切换到新分支
        </div>
      </div>
    </div>

    <!-- Body -->
    <div class="cb-body">
      <label class="cb-label">分支名</label>
      <a-input
        ref="inputRef"
        v-model:value="branchName"
        placeholder="输入新分支名"
        size="large"
        @press-enter="submit"
      >
        <template #prefix><ForkOutlined style="color: var(--color-text-tertiary)" /></template>
      </a-input>
    </div>

    <!-- Footer -->
    <div class="cb-footer">
      <a-button @click="close">取消</a-button>
      <a-button type="primary" :loading="busy" @click="submit">
        创建并切换
      </a-button>
    </div>
  </a-modal>
</template>

<style scoped>
.cb-header {
  display: flex; align-items: center; gap: 12px;
  padding: 20px 24px 16px;
  border-bottom: 1px solid var(--color-border-light);
}
.cb-icon-badge {
  width: 40px; height: 40px;
  border-radius: var(--radius-md);
  background: var(--color-primary-bg);
  color: var(--color-primary);
  display: flex; align-items: center; justify-content: center;
  font-size: 18px;
  flex-shrink: 0;
}
.cb-title { font-size: 15px; font-weight: 700; color: var(--color-text); }
.cb-subtitle { font-size: 12px; color: var(--color-text-tertiary); margin-top: 2px; }
.cb-subtitle code { font-family: 'Cascadia Code', monospace; font-weight: 600; color: var(--color-primary); }

.cb-body { padding: 16px 24px; }
.cb-label { display: block; font-size: 11px; font-weight: 600; color: var(--color-text-tertiary); text-transform: uppercase; letter-spacing: 0.05em; margin-bottom: 8px; }

.cb-footer {
  display: flex; justify-content: flex-end; gap: 8px;
  padding: 12px 24px;
  border-top: 1px solid var(--color-border-light);
}
</style>
