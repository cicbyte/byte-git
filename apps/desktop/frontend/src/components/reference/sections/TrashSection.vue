<script setup>
/**
 * 回收站 — 被删除的 wiki 文件可从 Git 历史恢复。
 *
 * 从 WikiSection 拆出，作为引用管理的独立 tab。
 */
import { ref, onMounted } from 'vue'
import { message, Modal } from 'ant-design-vue'
import { ReloadOutlined, RollbackOutlined, DeleteOutlined } from '@ant-design/icons-vue'

const app = window.go?.main?.ByteGitApp

const trash = ref([])
const loading = ref(false)

async function loadTrash() {
  if (!app?.ReferenceWikiTrash) return
  loading.value = true
  try {
    trash.value = (await app.ReferenceWikiTrash()) || []
  } catch (e) {
    message.error('读取回收站失败: ' + String(e?.message || e))
    trash.value = []
  } finally {
    loading.value = false
  }
}

function restore(entry) {
  Modal.confirm({
    title: '恢复此文件?',
    content: `将恢复 ${entry.path}（删除于 ${entry.date}）`,
    okText: '恢复', cancelText: '取消',
    async onOk() {
      try {
        await app.ReferenceWikiRestore(entry.path)
        message.success('已恢复: ' + entry.path)
        await loadTrash()
      } catch (e) {
        message.error('恢复失败: ' + String(e?.message || e))
      }
    },
  })
}

onMounted(() => loadTrash())
</script>

<template>
  <div class="section">
    <div class="sec-card">
      <div class="card-head">
        <div class="card-title">
          回收站
          <span class="count-badge" v-if="trash.length">{{ trash.length }}</span>
        </div>
        <div class="card-desc">被删除的 wiki 文件可从 Git 历史恢复。点击恢复按钮将文件还原到 wiki 目录。</div>
      </div>
      <div class="card-body">
        <!-- toolbar -->
        <div class="card-toolbar">
          <a-button size="small" :loading="loading" @click="loadTrash">
            <template #icon><ReloadOutlined /></template>刷新
          </a-button>
        </div>

        <!-- loading -->
        <div v-if="loading && !trash.length" class="empty-state">
          <a-spin />
        </div>

        <!-- trash list -->
        <div v-else-if="trash.length" class="trash-list">
          <div v-for="(t, i) in trash" :key="i" class="trash-row">
            <div class="tr-icon"><DeleteOutlined /></div>
            <div class="tr-body">
              <div class="tr-path" :title="t.path">{{ t.path }}</div>
              <div class="tr-meta">
                <span>{{ t.date }}</span>
                <span class="tr-msg" :title="t.message">{{ t.message }}</span>
              </div>
            </div>
            <a-button size="small" type="text" @click="restore(t)" title="恢复此文件">
              <template #icon><RollbackOutlined /></template>
            </a-button>
          </div>
        </div>

        <!-- empty -->
        <div v-else class="empty-state">
          <DeleteOutlined style="font-size: 36px; opacity: 0.25" />
          <div style="margin-top: 10px; font-size: 13px; color: var(--color-text-tertiary)">回收站为空</div>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
@import './section.css';

.count-badge {
  display: inline-flex; align-items: center; justify-content: center;
  min-width: 20px; height: 20px; padding: 0 6px;
  font-size: 11px; font-weight: 600;
  background: var(--color-primary-bg);
  color: var(--color-primary);
  border-radius: 999px;
  margin-left: 6px;
}

.card-toolbar { margin-bottom: 12px; }

.trash-list { display: flex; flex-direction: column; }

.trash-row {
  display: flex; align-items: center; gap: 10px;
  padding: 10px 12px;
  border-bottom: 1px solid var(--color-border-light);
  transition: background var(--transition-fast);
}
.trash-row:last-child { border-bottom: none; }
.trash-row:hover { background: var(--color-hover); }

.tr-icon {
  font-size: 14px; color: var(--color-text-tertiary);
  flex-shrink: 0; width: 28px; height: 28px;
  display: flex; align-items: center; justify-content: center;
  border-radius: var(--radius-sm);
  background: var(--color-surface-raised);
}

.tr-body { flex: 1; min-width: 0; }
.tr-path {
  font-size: 12px; color: var(--color-text);
  font-family: 'Cascadia Code', monospace;
  overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
}
.tr-meta {
  display: flex; gap: 10px; margin-top: 2px;
  font-size: 10px; color: var(--color-text-tertiary);
}
.tr-msg {
  overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
  flex: 1; min-width: 0;
}

.empty-state {
  display: flex; flex-direction: column; align-items: center; justify-content: center;
  padding: 40px 20px;
  color: var(--color-text-tertiary);
}
</style>
