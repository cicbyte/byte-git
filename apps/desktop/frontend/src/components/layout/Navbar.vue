<script setup>
import { computed, onMounted, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { MinusOutlined, BorderOutlined, CloseOutlined, MenuFoldOutlined, MenuUnfoldOutlined, BranchesOutlined, EyeOutlined, CodeOutlined, AppstoreOutlined } from '@ant-design/icons-vue'
import { useLayoutStore } from '../../stores/layout'
import { useRepoStore } from '../../stores/repo'
import { useScmStore } from '../../stores/scm'
import { storeToRefs } from 'pinia'
import DownloadBar from './DownloadBar.vue'
import RepoModal from './RepoModal.vue'
import ScmDrawer from '../scm/ScmDrawer.vue'

const route = useRoute()
const router = useRouter()
const layout = useLayoutStore()
const repo = useRepoStore()
const scm = useScmStore()
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
  <div class="navbar">
    <div class="navbar-left">
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

    <div class="navbar-center">
      <!-- 仓库切换已移至项目轨道 -->
    </div>

    <!-- Repo switcher modal (shared entry with the footer) -->
    <RepoModal v-model:open="repoModalOpen" />

    <!-- Global SCM drawer -->
    <ScmDrawer v-model:open="scmOpen" />

    <div class="navbar-right">
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
  background: var(--color-surface);
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
  flex: 1;
  display: flex;
  align-items: center;
  gap: var(--spacing-sm);
  min-width: 0;
}

.navbar-center {
  flex: 2;
  display: flex;
  justify-content: center;
  max-width: 400px;
}

.navbar-right {
  flex: 1;
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

/* Breadcrumb */
.breadcrumb {
  font-size: 13px;
  min-width: 0;
  --wails-draggable: no-drag;
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
