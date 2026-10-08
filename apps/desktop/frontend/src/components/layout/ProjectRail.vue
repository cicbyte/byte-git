<script setup>
/**
 * 项目轨道 — 最左侧窄栏，持久列出所有最近打开的仓库。
 *
 * 仿 reference GUI 的 ProjectRail：每个项目是一张紧凑卡片（头像+名称+
 * 分支），当前项目高亮。一次点击切换，右键操作，底部添加。
 *
 * 两态宽度：
 *   默认 160px — 显示头像+名称+分支（信息充分）
 *   折叠 0px   — 用户手动隐藏（Navbar 按钮 / 路由感知自动隐藏）
 *
 * 数据层零改动：复用 stores/repo.ts 的 recent[] / switchTo() / pickFolder()。
 */
import { computed, ref } from 'vue'
import { useRoute } from 'vue-router'
import { Modal } from 'ant-design-vue'
import {
  FolderOpenOutlined, PlusOutlined, DeleteOutlined,
} from '@ant-design/icons-vue'
import { useRepoStore } from '../../stores/repo'
import ContextMenu from '../common/ContextMenu.vue'
import CtxItem from '../common/CtxItem.vue'

const route = useRoute()
const repo = useRepoStore()
const app = window.go?.main?.ByteGitApp

// ---- context menu ----
const ctxMenu = ref({ open: false, x: 0, y: 0, path: '', name: '' })

function onItemContextmenu(evt, r) {
  ctxMenu.value = { open: true, x: evt.clientX, y: evt.clientY, path: r.path, name: r.name }
}

function ctxOpenExplorer() {
  const p = ctxMenu.value.path
  ctxMenu.value.open = false
  if (p && app?.OpenInExplorer) app.OpenInExplorer(p).catch(() => {})
}

function ctxRemove() {
  const p = ctxMenu.value.path
  const n = ctxMenu.value.name
  ctxMenu.value.open = false
  Modal.confirm({
    title: `从最近列表移除「${n}」？`,
    content: '仅从最近打开列表移除，不删除仓库文件。',
    okText: '移除', okType: 'danger', cancelText: '取消',
    onOk: () => repo.removeRecent(p),
  })
}

// project initial letter for the icon avatar
function initials(name) {
  if (!name) return '?'
  return name.trim()[0].toUpperCase() || '?'
}

// distinct background colors per project (deterministic from name hash)
const palette = [
  '#7d6cf8', '#ff79c6', '#8be9fd', '#50fa7b',
  '#f1fa8c', '#ffb86c', '#bd93f9', '#ff7733',
]
function avatarColor(name) {
  let hash = 0
  for (let i = 0; i < name.length; i++) hash = name.charCodeAt(i) + ((hash << 5) - hash)
  return palette[Math.abs(hash) % palette.length]
}

async function addProject() {
  const dir = await repo.pickFolder()
  if (dir) await repo.switchTo(dir)
}
</script>

<template>
  <div class="project-rail">
    <!-- header -->
    <div class="rail-head">项目</div>

    <!-- project list -->
    <div class="rail-items">
      <div
        v-for="r in repo.recent"
        :key="r.path"
        class="rail-item"
        :class="{ active: repo.path === r.path }"
        :title="r.path"
        @click="repo.switchTo(r.path)"
        @contextmenu.prevent="onItemContextmenu($event, r)"
      >
        <div class="rail-avatar" :style="{ background: avatarColor(r.name) }">
          {{ initials(r.name) }}
        </div>
        <div class="rail-info">
          <div class="rail-name">{{ r.name }}</div>
        </div>
      </div>
      <div v-if="!repo.recent.length" class="rail-empty">
        暂无项目
      </div>
    </div>

    <!-- add button -->
    <div class="rail-bottom">
      <button class="rail-add" @click="addProject">
        <PlusOutlined /> <span>添加项目</span>
      </button>
    </div>

    <!-- context menu -->
    <ContextMenu v-model:open="ctxMenu.open" :x="ctxMenu.x" :y="ctxMenu.y" :title="ctxMenu.name">
      <CtxItem @click="ctxOpenExplorer">
        <FolderOpenOutlined /> 在文件管理器中打开
      </CtxItem>
      <div class="ctx-sep"></div>
      <CtxItem danger @click="ctxRemove">
        <DeleteOutlined /> 从列表移除
      </CtxItem>
    </ContextMenu>
  </div>
</template>

<style scoped>
.project-rail {
  width: 160px;
  flex-shrink: 0;
  display: flex;
  flex-direction: column;
  background: var(--color-surface);
  border-right: 1px solid var(--color-border);
  overflow: hidden;
  transition: width var(--transition-normal);
}

.rail-head {
  display: flex;
  align-items: center;
  padding: 0 12px;
  height: var(--navbar-height); /* align with Navbar top bar */
  font-size: 10px;
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: 0.05em;
  color: var(--color-text-tertiary);
  border-bottom: 1px solid var(--color-border-light);
  flex-shrink: 0;
}

.rail-items {
  flex: 1;
  overflow-y: auto;
  padding: 6px;
  display: flex;
  flex-direction: column;
  gap: 2px;
}
.rail-items::-webkit-scrollbar { width: 4px; }
.rail-items::-webkit-scrollbar-thumb { background: var(--color-border); border-radius: 2px; }

.rail-item {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 7px 8px;
  border-radius: var(--radius-sm);
  cursor: pointer;
  transition: background var(--transition-fast);
  border: 1px solid transparent;
}
.rail-item:hover { background: var(--color-hover); }
.rail-item.active {
  background: var(--color-primary-bg);
  border-color: var(--color-border);
}

.rail-avatar {
  width: 24px;
  height: 24px;
  border-radius: 5px;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 12px;
  font-weight: 700;
  color: #fff;
  flex-shrink: 0;
}
.rail-item.active .rail-avatar {
  box-shadow: 0 0 0 2px var(--color-primary);
}

.rail-info {
  flex: 1;
  min-width: 0;
}
.rail-name {
  font-size: 12px;
  font-weight: 500;
  color: var(--color-text);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.rail-item.active .rail-name {
  color: var(--color-primary);
}

.rail-empty {
  padding: 20px 10px;
  font-size: 11px;
  color: var(--color-text-tertiary);
  text-align: center;
}

.rail-bottom {
  padding: 0 8px;
  height: var(--footer-height); /* align with global footer */
  display: flex;
  align-items: center;
  border-top: 1px solid var(--color-border-light);
  flex-shrink: 0;
}
.rail-add {
  width: 100%;
  padding: 7px 10px;
  border-radius: var(--radius-sm);
  border: 1px dashed var(--color-border);
  background: transparent;
  cursor: pointer;
  color: var(--color-text-tertiary);
  font-size: 12px;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 6px;
  transition: all var(--transition-fast);
}
.rail-add:hover {
  color: var(--color-primary);
  border-color: var(--color-primary);
  background: var(--color-primary-bg);
}
</style>
