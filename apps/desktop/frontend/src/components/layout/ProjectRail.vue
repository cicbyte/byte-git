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
  PushpinFilled, PushpinOutlined,
} from '@ant-design/icons-vue'
import { useRepoStore } from '../../stores/repo'
import ContextMenu from '../common/ContextMenu.vue'
import CtxItem from '../common/CtxItem.vue'

const route = useRoute()
const repo = useRepoStore()
const app = window.go?.main?.ByteGitApp

// 置顶分组渲染（置顶在前，各段带小标题）
const pinnedList = computed(() => repo.recent.filter((r) => r.pinned))
const recentList = computed(() => repo.recent.filter((r) => !r.pinned))

// ---- 搜索过滤（名称/路径子串，大小写不敏感）----
const query = ref('')
const match = (r) => {
  const q = query.value.trim().toLowerCase()
  if (!q) return true
  return r.name.toLowerCase().includes(q) || r.path.toLowerCase().includes(q)
}
const shownPinned = computed(() => pinnedList.value.filter(match))
const shownRecent = computed(() => recentList.value.filter(match))

// ---- 拖拽排序（分区内拖动；拖过项上实时换位，dragend 持久化）----
const drag = ref(null) // { path, pinned }
function onDragStart(e, r) {
  drag.value = { path: r.path, pinned: !!r.pinned }
  e.dataTransfer.effectAllowed = 'move'
  e.dataTransfer.setData('text/plain', r.path) // Firefox 兼容
}
function onDragOver(e, r) {
  if (!drag.value || !!r.pinned !== drag.value.pinned || r.path === drag.value.path) return
  e.preventDefault()
  e.dataTransfer.dropEffect = 'move'
  // 在展示分区里换位 → 同步映射回 store 的完整扁平列表
  const m = new Map(repo.recent.map((x) => [x.path, x]))
  const move = (list) => {
    const from = list.findIndex((p) => p === drag.value.path)
    const to = list.findIndex((p) => p === r.path)
    if (from < 0 || to < 0) return list
    const next = [...list]
    next.splice(to, 0, next.splice(from, 1)[0])
    return next
  }
  const pinnedPaths = move(pinnedList.value.map((x) => x.path))
  const recentPaths = move(recentList.value.map((x) => x.path))
  repo.recent = [...pinnedPaths, ...recentPaths].map((p) => m.get(p)).filter(Boolean)
}
async function onDragEnd() {
  if (!drag.value) return
  drag.value = null
  await repo.setOrder(pinnedList.value.map((x) => x.path), recentList.value.map((x) => x.path))
}

// ---- context menu ----
const ctxMenu = ref({ open: false, x: 0, y: 0, path: '', name: '', pinned: false })

function onItemContextmenu(evt, r) {
  ctxMenu.value = { open: true, x: evt.clientX, y: evt.clientY, path: r.path, name: r.name, pinned: !!r.pinned }
}

function ctxOpenExplorer() {
  const p = ctxMenu.value.path
  ctxMenu.value.open = false
  if (p && app?.OpenInExplorer) app.OpenInExplorer(p).catch(() => {})
}

function ctxTogglePin() {
  const { path, pinned } = ctxMenu.value
  ctxMenu.value.open = false
  repo.setPinned(path, !pinned)
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

async function addProject() {
  const dir = await repo.pickFolder()
  if (dir) await repo.switchTo(dir)
}
</script>

<template>
  <div class="project-rail">
    <!-- header -->
    <div class="rail-head">项目</div>

    <!-- 搜索过滤（名称/路径） -->
    <div class="rail-search-wrap">
      <input
        v-model="query"
        class="rail-search"
        type="text"
        placeholder="搜索项目…"
        spellcheck="false"
      />
      <button v-if="query" class="rail-search-clear" title="清空" @click="query = ''">×</button>
    </div>

    <!-- project list：置顶区（若有）+ 项目区，均带小标题；支持分区内拖拽排序 -->
    <div class="rail-items">
      <template v-if="shownPinned.length">
        <div class="rail-section-title">置顶</div>
        <div
          v-for="r in shownPinned"
          :key="r.path"
          class="rail-item pinned"
          :class="{ active: repo.path === r.path, dragging: drag?.path === r.path }"
          draggable="true"
          :title="r.path"
          @click="repo.switchTo(r.path)"
          @contextmenu.prevent="onItemContextmenu($event, r)"
          @dragstart="onDragStart($event, r)"
          @dragover="onDragOver($event, r)"
          @dragend="onDragEnd"
        >
          <div class="rail-avatar">
            {{ initials(r.name) }}
          </div>
          <div class="rail-info">
            <div class="rail-name">{{ r.name }}</div>
          </div>
          <PushpinFilled class="rail-pin" />
        </div>
      </template>

      <div v-if="shownRecent.length" class="rail-section-title">项目</div>
      <div
        v-for="r in shownRecent"
        :key="r.path"
        class="rail-item"
        :class="{ active: repo.path === r.path, dragging: drag?.path === r.path }"
        draggable="true"
        :title="r.path"
        @click="repo.switchTo(r.path)"
        @contextmenu.prevent="onItemContextmenu($event, r)"
        @dragstart="onDragStart($event, r)"
        @dragover="onDragOver($event, r)"
        @dragend="onDragEnd"
      >
        <div class="rail-avatar">
          {{ initials(r.name) }}
        </div>
        <div class="rail-info">
          <div class="rail-name">{{ r.name }}</div>
        </div>
      </div>
      <div v-if="!repo.recent.length" class="rail-empty">
        暂无项目
      </div>
      <div v-else-if="!shownPinned.length && !shownRecent.length" class="rail-empty">
        无匹配项目
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
      <CtxItem @click="ctxTogglePin">
        <PushpinOutlined v-if="!ctxMenu.pinned" /> <PushpinFilled v-else />
        {{ ctxMenu.pinned ? '取消置顶' : '置顶' }}
      </CtxItem>
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
  position: relative;
  z-index: 2;  /* sit above wallpaper layer (.wp-layer z:0, .wp-mask z:1) */
  width: 160px;
  flex-shrink: 0;
  display: flex;
  flex-direction: column;
  background: var(--bg-rail);
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

/* 搜索框：紧凑，主题变量驱动 */
.rail-search-wrap {
  position: relative;
  padding: 6px 6px 2px;
  flex-shrink: 0;
}
.rail-search {
  width: 100%;
  height: 26px;
  padding: 0 8px;
  font-size: 12px;
  color: var(--color-text);
  background: var(--color-bg-2, var(--bg-input, transparent));
  border: 1px solid var(--color-border-light);
  border-radius: var(--radius-sm);
  outline: none;
  transition: border-color var(--transition-fast);
}
.rail-search::placeholder { color: var(--color-text-tertiary); }
.rail-search:focus { border-color: var(--color-primary); }
.rail-search-clear {
  position: absolute;
  right: 12px;
  top: 50%;
  transform: translateY(-50%);
  border: none;
  background: transparent;
  color: var(--color-text-tertiary);
  font-size: 13px;
  cursor: pointer;
  line-height: 1;
  padding: 2px;
}
.rail-search-clear:hover { color: var(--color-text); }

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

/* 分组小标题（置顶/最近） */
.rail-section-title {
  padding: 10px 6px 3px;
  font-size: 10px;
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: 0.05em;
  color: var(--color-text-tertiary);
  user-select: none;
  white-space: nowrap;
}
.rail-section-title:first-child { padding-top: 4px; }

/* 置顶项：主色钉标 + 微主色底，与普通项区分 */
.rail-item.pinned .rail-avatar {
  background: var(--color-primary-bg);
  border-color: var(--color-primary);
  color: var(--color-primary);
}
.rail-item.pinned .rail-name { color: var(--color-primary); }
.rail-pin {
  font-size: 11px;
  color: var(--color-primary);
  flex-shrink: 0;
  opacity: 0.85;
}

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
/* 拖拽中的源项半透明；光标维持 pointer（点击主导），
   拖拽过程的光标由浏览器按 dropEffect 自动切换 */
.rail-item.dragging { opacity: 0.4; }

.rail-avatar {
  width: 24px;
  height: 24px;
  border-radius: 6px;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 12px;
  font-weight: 600;
  /* 透明自绘风格：透明背景 + 细描边 + 次要文字色 */
  background: transparent;
  border: 1px solid var(--color-border);
  color: var(--color-text-secondary);
  flex-shrink: 0;
  transition: all var(--transition-fast);
}
.rail-item:hover .rail-avatar {
  border-color: var(--color-primary);
  color: var(--color-primary);
}
.rail-item.active .rail-avatar {
  /* active 态：填充轻微主色底 + 主色描边 + 主色文字 */
  background: var(--color-primary-bg);
  border-color: var(--color-primary);
  color: var(--color-primary);
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
