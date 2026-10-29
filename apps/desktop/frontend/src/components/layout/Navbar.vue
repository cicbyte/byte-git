<script setup>
import { computed, onMounted, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { MinusOutlined, BorderOutlined, CloseOutlined, MenuFoldOutlined, MenuUnfoldOutlined, BranchesOutlined, EyeOutlined, CodeOutlined, AppstoreOutlined, SettingOutlined } from '@ant-design/icons-vue'
import { useLayoutStore } from '@/stores/layout'
import { useRepoStore } from '@/stores/repo'
import { useScmStore } from '@/stores/scm'
import { useThemeStore } from '@/stores/theme'
import { storeToRefs } from 'pinia'
import DownloadBar from './DownloadBar.vue'
import RepoModal from './RepoModal.vue'
import ScmDrawer from '@/components/scm/ScmDrawer.vue'

const route = useRoute()
const router = useRouter()
const layout = useLayoutStore()
const repo = useRepoStore()
const scm = useScmStore()
const theme = useThemeStore()
const { totalChanges } = storeToRefs(scm)
const app = window.go?.main?.ByteGitApp

// repo switcher modal (shared entry with the footer)
const repoModalOpen = ref(false)
// SCM drawer open state (local to Navbar)
const scmOpen = ref(false)

onMounted(async () => {
  await repo.detectCurrent()
  // load the recent-repos list so the picker can offer quick re-open entries
  await repo.loadRecent()
  // initial SCM count for the badge
  scm.refresh()
  // start the global fs watcher so the badge auto-updates on working-tree
  // changes, independent of whether LiveView is currently mounted.
  scm.startGlobalWatch()
  // No repo detected (typical when launched as a packaged .exe whose CWD is
  // not a git repo) → proactively pop the repo picker so the user is guided
  // to open one instead of staring at an empty dashboard.
  if (!repo.path) {
    repoModalOpen.value = true
  }
})

// keep SCM count fresh after repo switches and restart the watcher on the new path
watch(() => repo.repoEpoch, () => {
  scm.refresh()
  scm.startGlobalWatch()
})
// refresh badge count when the drawer closes (something may have changed inside)
watch(scmOpen, (v) => { if (!v) scm.refresh() })
// listen for programmatic open requests (e.g. dashboard quick action)
window.addEventListener('open-scm', () => { scmOpen.value = true })

// Menu hierarchy mirror of Sidebar.vue. Used to render a breadcrumb that
// reflects the actual menu path: GroupLabel > ChildLabel.
// Settings & home are top-level (no group) so they show as a single item.
const menuGroups = [
  { label: '总览', children: [{ key: '/', label: '仪表盘' }, { key: '/stats', label: '开发统计' }] },
  { label: '浏览与变更', children: [{ key: '/log', label: 'Git Log' }, { key: '/live', label: '实时变更' }, { key: '/time-travel', label: '代码穿梭' }, { key: '/info', label: '仓库洞察' }] },
  { label: '历史改写', children: [{ key: '/rewrite', label: '改写' }, { key: '/ai-rewrite', label: 'AI改写' }, { key: '/json-editor', label: '数据编辑器' }] },
  { label: '规范治理', children: [{ key: '/ignore', label: 'Gitignore' }, { key: '/license', label: '许可证' }, { key: '/secret', label: '密钥扫描' }, { key: '/review', label: '代码审查' }] },
  { label: '数据', children: [{ key: '/backup', label: '备份管理' }, { key: '/git-manage', label: 'Git 管理' }, { key: '/stash', label: 'Stash 管理' }] },
  { label: '终端', children: [{ key: '/terminal', label: '终端' }] },
  { label: 'GitHub', children: [{ key: '/github', label: 'Pull Requests' }, { key: '/github-issues', label: 'Issues' }, { key: '/github-actions', label: 'Actions' }, { key: '/github-releases', label: 'Releases' }, { key: '/github-search', label: '搜索' }, { key: '/github-trending', label: 'Trending' }] },
  { label: '帮助', children: [{ key: '/snippets', label: '命令片段' }, { key: '/cheat', label: 'Git 备忘录' }] },
]

// Build the breadcrumb items for the current route: [group, leaf]
const breadcrumbItems = computed(() => {
  const path = route.path
  for (const g of menuGroups) {
    const child = g.children.find((c) => c.key === path)
    if (child) {
      return [{ label: g.label }, { label: child.label }]
    }
  }
  // top-level routes not nested in a menu group (e.g. /settings)
  return [{ label: route.meta?.title || 'byte-git' }]
})

function minimize() { app?.WindowMinimize() }
function maximize() { app?.WindowMaximize() }
function close() { app?.WindowClose() }
</script>

<template>
  <!-- data-tauri-drag-region：Tauri 原生拖拽（Wails 的 --wails-draggable CSS 不生效）。
       属性按 mousedown 目标生效，容器标注可拖，按钮/面包屑等子元素自动不受影响。 -->
  <div class="navbar" data-tauri-drag-region>
    <div class="navbar-left" data-tauri-drag-region>
      <!-- ProjectRail toggle (show/hide the project switcher rail) -->
      <button
        v-show="!layout.isMobile"
        class="sidebar-toggle"
        :class="{ dimmed: !layout.projectRailVisible }"
        :title="layout.projectRailVisible ? '隐藏项目栏' : '显示项目栏'"
        @click="layout.toggleProjectRail()"
      >
        <AppstoreOutlined />
      </button>

      <!-- Sidebar collapse toggle (sits left of the breadcrumb) -->
      <button
        v-show="!layout.isMobile"
        class="sidebar-toggle"
        :title="layout.sidebarCollapsed ? '展开侧边栏' : '折叠侧边栏'"
        @click="layout.toggleSidebar()"
      >
        <MenuUnfoldOutlined v-if="layout.sidebarCollapsed" />
        <MenuFoldOutlined v-else />
      </button>

      <!-- Breadcrumb: reflects the actual menu path (Group > Page) -->
      <a-breadcrumb class="breadcrumb">
        <a-breadcrumb-item v-for="(item, idx) in breadcrumbItems" :key="idx">
          {{ item.label }}
        </a-breadcrumb-item>
      </a-breadcrumb>
    </div>

    <div class="navbar-center" data-tauri-drag-region>
      <!-- 仓库切换已移至项目轨道 -->
    </div>

    <!-- Repo switcher modal (shared entry with the footer) -->
    <RepoModal v-model:open="repoModalOpen" />

    <!-- Global SCM drawer -->
    <ScmDrawer v-model:open="scmOpen" />

    <div class="navbar-right" data-tauri-drag-region>
      <!-- Live changes (实时变更) — high-frequency entry, pinned next to SCM -->
      <button
        class="scm-btn"
        :class="{ active: route.path === '/live' }"
        title="实时变更"
        @click="router.push('/live')"
      >
        <EyeOutlined />
      </button>
      <!-- Terminal quick entry -->
      <button
        class="scm-btn"
        :class="{ active: route.path === '/terminal' }"
        title="终端"
        @click="router.push('/terminal')"
      >
        <CodeOutlined />
      </button>
      <!-- Global SCM (source control) drawer trigger — shows change count badge -->
      <a-badge :count="totalChanges" :overflow-count="99" :offset="[-2, 4]" size="small">
        <button class="scm-btn" :class="{ active: scmOpen }" title="源代码管理" @click="scmOpen = true">
          <BranchesOutlined />
        </button>
      </a-badge>
      <!-- Global download (GitHub Release), independent of current repo -->
      <DownloadBar />
      <!-- 主题切换 -->
      <button
        class="scm-btn"
        :title="theme.isDark ? '切换亮色' : '切换暗色'"
        @click="theme.toggleTheme()"
      >
        <svg v-if="theme.isDark" viewBox="0 0 24 24" width="14" height="14" fill="currentColor"><path d="M12 3a9 9 0 109 9c0-.46-.04-.92-.1-1.36a5.389 5.389 0 01-4.4 2.26 5.403 5.403 0 01-3.14-9.8c-.44-.06-.9-.1-1.36-.1z"/></svg>
        <svg v-else viewBox="0 0 24 24" width="14" height="14" fill="currentColor"><path d="M6.76 4.84l-1.8-1.79-1.41 1.41 1.79 1.79 1.42-1.41zM4 10.5H1v2h3v-2zm9-9.95h-2V3.5h2V.55zm7.45 3.91l-1.41-1.41-1.79 1.79 1.41 1.41 1.79-1.79zm-3.21 13.7l1.79 1.8 1.41-1.41-1.8-1.79-1.4 1.4zM20 10.5v2h3v-2h-3zm-8-5c-3.31 0-6 2.69-6 6s2.69 6 6 6 6-2.69 6-6-2.69-6-6-6zm-1 16.95h2V19.5h-2v2.95zm-7.45-3.91l1.41 1.41 1.79-1.8-1.41-1.41-1.79 1.8z"/></svg>
      </button>
      <!-- 设置 -->
      <button
        class="scm-btn"
        :class="{ active: route.path === '/settings' }"
        title="设置"
        @click="router.push('/settings')"
      >
        <SettingOutlined />
      </button>
      <button class="win-btn" title="最小化" @click="minimize">
        <MinusOutlined />
      </button>
      <button class="win-btn" title="最大化" @click="maximize">
        <BorderOutlined />
      </button>
      <button class="win-btn win-btn-close" title="关闭" @click="close">
        <CloseOutlined />
      </button>
    </div>
  </div>
</template>

<style scoped>
.navbar {
  display: flex;
  align-items: center;
  height: var(--navbar-height);
  padding: 0 var(--spacing-md);
  border-bottom: 1px solid var(--color-border);
  background: var(--bg-navbar);
  flex-shrink: 0;
  /* Whole navbar is a drag region; only specific interactive elements opt out below */
  --wails-draggable: drag;
}

.navbar-left,
.navbar-center,
.navbar-right {
  /* keep draggable so the empty space within them still drags the window */
}

.navbar-left {
  /* 唯一伸展段：面包屑可用全部富余空间（窄到极限才回落省略号） */
  flex: 1 1 auto;
  display: flex;
  align-items: center;
  gap: var(--spacing-sm);
  min-width: 0;
}

.navbar-center {
  /* 空占位（上游预留搜索框）——不再吃 2/4 宽度，避免面包屑被挤省略 */
  flex: 0 0 0;
  max-width: none;
}

.navbar-right {
  flex: 0 0 auto;
  display: flex;
  justify-content: flex-end;
  gap: 2px;
}

/* Sidebar collapse toggle button */
.sidebar-toggle {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 28px;
  height: 28px;
  border: none;
  background: transparent;
  color: var(--color-text-secondary);
  cursor: pointer;
  border-radius: var(--radius-xs);
  transition: all var(--transition-fast);
  flex-shrink: 0;
  --wails-draggable: no-drag;
}
.sidebar-toggle.dimmed { opacity: 0.4; }

.sidebar-toggle:hover {
  background: var(--color-hover);
  color: var(--color-primary);
}

/* Breadcrumb：窄窗单行显示——不换行，末项溢出省略（防导航栏被撑高重叠） */
.breadcrumb {
  font-size: 13px;
  min-width: 0;
  overflow: hidden;
  flex-shrink: 1;
  --wails-draggable: no-drag;
}
.breadcrumb :deep(ol) {
  flex-wrap: nowrap;
  overflow: hidden;
  white-space: nowrap;
}
.breadcrumb :deep(.ant-breadcrumb-link) {
  white-space: nowrap;
}
.breadcrumb :deep(li:last-child .ant-breadcrumb-link) {
  overflow: hidden;
  text-overflow: ellipsis;
}

/* SCM (source control) trigger button */
.scm-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 30px;
  height: 28px;
  border: none;
  background: transparent;
  color: var(--color-text-secondary);
  cursor: pointer;
  border-radius: var(--radius-xs);
  transition: all var(--transition-fast);
  --wails-draggable: no-drag;
  margin-right: 4px;
}
.scm-btn:hover { background: var(--color-hover); color: var(--color-primary); }
.scm-btn.active { color: var(--color-primary); background: var(--color-primary-bg); }

/* Window control buttons */
.win-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 36px;
  height: 28px;
  border: none;
  background: transparent;
  color: var(--color-text-secondary);
  cursor: pointer;
  border-radius: var(--radius-xs);
  transition: all var(--transition-fast);
  --wails-draggable: no-drag;
}

.win-btn:hover {
  background: var(--color-hover);
  color: var(--color-text);
}

.win-btn-close:hover {
  background: var(--color-error);
  color: white;
}
</style>
