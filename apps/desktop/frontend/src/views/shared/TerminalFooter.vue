<script setup>
/**
 * Footer injected by TerminalView into the global footer slot.
 * Shows shell identity + connection status + clear/restart actions.
 */
import { CodeOutlined, ReloadOutlined, ClearOutlined } from '@ant-design/icons-vue'

defineProps({
  shellLabel: { type: String, default: '' },
  shellPath: { type: String, default: '' },
  repoName: { type: String, default: '' },
  status: { type: String, default: 'idle' },
})
const emit = defineEmits(['clear', 'restart'])

const statusLabel = {
  idle: '未连接',
  connecting: '连接中',
  connected: '已连接',
  exited: '已断开',
}
</script>

<template>
  <div class="tf-bar">
    <CodeOutlined class="tf-icon" />
    <span class="tf-shell" :title="shellPath">{{ shellLabel }}</span>
    <span v-if="repoName" class="tf-repo">{{ repoName }}</span>
    <span class="tf-status" :class="'st-' + status">
      <span class="tf-dot"></span>
      {{ statusLabel[status] }}
    </span>
    <div class="tf-spacer" />
    <button class="tf-btn" :disabled="status !== 'connected'" @click="emit('clear')" title="清屏">
      <ClearOutlined />
    </button>
    <button class="tf-btn" @click="emit('restart')" title="重启终端">
      <ReloadOutlined />
    </button>
  </div>
</template>

<style scoped>
.tf-bar {
  display: flex; align-items: center; gap: 8px;
  width: 100%; font-size: 12px; color: var(--color-text-secondary);
  user-select: none;
}
.tf-icon { font-size: 13px; color: var(--color-text-tertiary); flex-shrink: 0; }
.tf-shell {
  font-family: 'JetBrains Mono', 'Cascadia Code', monospace;
  font-size: 11px; font-weight: 500; color: var(--color-text); flex-shrink: 0;
}
.tf-repo {
  font-size: 11px; color: var(--color-text-tertiary);
  padding: 1px 6px; border-radius: var(--radius-xs);
  background: var(--color-surface-raised); flex-shrink: 0;
  max-width: 120px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
}

.tf-status { display: flex; align-items: center; gap: 5px; font-size: 11px; color: var(--color-text-tertiary); }
.tf-dot { width: 7px; height: 7px; border-radius: 50%; background: var(--color-text-tertiary); transition: background var(--transition-fast); }
.tf-status.st-connected { color: var(--color-success); }
.tf-status.st-connected .tf-dot { background: var(--color-success); }
.tf-status.st-connecting .tf-dot { background: var(--color-warning); animation: pulse 1.2s infinite; }
.tf-status.st-exited { color: var(--color-error); }
.tf-status.st-exited .tf-dot { background: var(--color-error); }
@keyframes pulse { 0%, 100% { opacity: 1; } 50% { opacity: 0.3; } }

.tf-spacer { flex: 1; }
.tf-btn {
  display: inline-flex; align-items: center; justify-content: center;
  width: 26px; height: 26px; border: none; border-radius: var(--radius-xs);
  background: transparent; cursor: pointer; color: var(--color-text-tertiary);
  font-size: 12px; transition: all var(--transition-fast);
}
.tf-btn:hover { background: var(--color-hover); color: var(--color-text); }
.tf-btn:disabled { opacity: 0.35; cursor: not-allowed; }
</style>
