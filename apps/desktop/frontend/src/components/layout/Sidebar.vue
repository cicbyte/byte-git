<script setup>
import { ref, computed, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { useLayoutStore } from '../../stores/layout'
import { useThemeStore } from '../../stores/theme'
import { useRepoStore } from '../../stores/repo'
import {
  DashboardOutlined,
  ApartmentOutlined,
  FileTextOutlined,
  FolderOpenOutlined,
  StopOutlined,
  EditOutlined,
  FileSearchOutlined,
  SafetyCertificateOutlined,
  DatabaseOutlined,
  FieldTimeOutlined,
  BarChartOutlined,
  SettingOutlined,
  EyeOutlined,
  HistoryOutlined,
  CaretRightFilled,
  LockOutlined,
  CloudServerOutlined,
  TableOutlined,
  CodeOutlined,
  GithubOutlined,
  QuestionCircleOutlined,
  ReadOutlined,
  SnippetsOutlined,
  InboxOutlined,
  ToolOutlined,
  BranchesOutlined,
} from '@ant-design/icons-vue'
import AiSparkIcon from '../common/AiSparkIcon.vue'
import logoUrl from '../../assets/logo.png'
import { useFeatureFlags } from '../../composables/useSettings'

const route = useRoute()
const router = useRouter()
const layout = useLayoutStore()
const theme = useThemeStore()
const repo = useRepoStore()
const { githubEnabled, referenceEnabled, secretEnabled, reviewEnabled } = useFeatureFlags()

// 5-group categorization. Each group: { key, label, icon, children:[{key,label,icon}] }
const menuGroups = [
  {
    key: 'overview', label: '总览', icon: DashboardOutlined,
    children: [
      { key: '/', icon: DashboardOutlined, label: '仪表盘' },
      { key: '/info', icon: ApartmentOutlined, label: '仓库洞察' },
      { key: '/stats', icon: BarChartOutlined, label: '开发统计' },
    ],
  },
  {
    key: 'browse', label: '浏览与变更', icon: EyeOutlined,
    children: [
      { key: '/workspace', icon: FolderOpenOutlined, label: '工作区' },
      { key: '/log', icon: FileTextOutlined, label: 'Git Log' },
      { key: '/live', icon: EyeOutlined, label: '实时变更' },
      { key: '/time-travel', icon: FieldTimeOutlined, label: '代码穿梭' },
    ],
  },
  {
    key: 'rewrite', label: '历史改写', icon: HistoryOutlined,
    children: [
      { key: '/rewrite', icon: EditOutlined, label: '改写' },
      { key: '/ai-rewrite', icon: AiSparkIcon, label: 'AI改写' },
      { key: '/json-editor', icon: TableOutlined, label: '数据编辑器' },
    ],
  },
  {
    key: 'govern', label: '规范治理', icon: LockOutlined,
    children: [
      { key: '/ignore', icon: StopOutlined, label: 'Gitignore' },
      { key: '/license', icon: SafetyCertificateOutlined, label: '许可证' },
      { key: '/secret', icon: SafetyCertificateOutlined, label: '密钥扫描' },
      { key: '/review', icon: FileSearchOutlined, label: '代码审查' },
    ],
  },
  {
    key: 'data', label: '数据', icon: DatabaseOutlined,
    children: [
      { key: '/backup', icon: DatabaseOutlined, label: '备份管理' },
      { key: '/git-manage', icon: CloudServerOutlined, label: 'Git 管理' },
      { key: '/stash', icon: InboxOutlined, label: 'Stash 管理' },
    ],
  },
  {
    key: 'terminal', label: '终端', icon: CodeOutlined,
    children: [
      { key: '/terminal', icon: CodeOutlined, label: '终端' },
    ],
  },
  {
    key: 'github', label: 'GitHub', icon: GithubOutlined,
    children: [
      { key: '/github', icon: GithubOutlined, label: 'Pull Requests' },
      { key: '/github-issues', icon: GithubOutlined, label: 'Issues' },
      { key: '/github-actions', icon: GithubOutlined, label: 'Actions' },
      { key: '/github-releases', icon: GithubOutlined, label: 'Releases' },
      { key: '/github-search', icon: GithubOutlined, label: '搜索' },
      { key: '/github-trending', icon: GithubOutlined, label: 'Trending' },
    ],
  },
  {
    key: 'reference', label: '引用', icon: ApartmentOutlined,
    children: [
      { key: '/reference', icon: ApartmentOutlined, label: '引用仓库' },
      { key: '/reference-ops', icon: ToolOutlined, label: '引用管理' },
    ],
  },
  {
    key: 'help', label: '帮助', icon: QuestionCircleOutlined,
    children: [
      { key: '/snippets', icon: SnippetsOutlined, label: '命令片段' },
      { key: '/cheat', icon: ReadOutlined, label: 'Git 备忘录' },
    ],
  },
]

// which group the current route belongs to
// Feature-flag-aware menu: filters out whole groups (github/reference) and
// individual children (secret/review inside govern) based on user toggles.
// The raw menuGroups array stays intact for debugging; only the visible slice
// changes. Empty groups (all children filtered out) are dropped entirely.
const visibleMenuGroups = computed(() => {
  return menuGroups
    .map((g) => {
      // filter individual children by feature flag
      let children = g.children
      if (g.key === 'govern') {
        children = children.filter((c) => {
          if (c.key === '/secret') return secretEnabled.value
          if (c.key === '/review') return reviewEnabled.value
          return true
        })
      }
      return { ...g, children }
    })
    .filter((g) => {
      // drop whole groups whose feature is disabled, or that have no children
      if (g.key === 'github' && !githubEnabled.value) return false
      if (g.key === 'reference' && !referenceEnabled.value) return false
      return g.children.length > 0
    })
})

function groupOf(path) {
  for (const g of visibleMenuGroups.value) {
    if (g.children.some((c) => c.key === path)) return g.key
  }
  return null
}

// expanded state — current route's group is always expanded; others collapse
// by default but the user can expand/collapse them. Stored as a Set of keys.
const expanded = ref(new Set([groupOf(route.path)]))
// when collapsed (icon-only mode) we track the hovered/focused group for a flyout
const collapsedHover = ref(null)

watch(
  () => route.path,
  (p) => {
    const g = groupOf(p)
    if (g) {
      // ensure the active group is expanded (don't force-collapse others)
      expanded.value.add(g)
      // trigger reactivity on the Set
      expanded.value = new Set(expanded.value)
    }
  },
  { immediate: true },
)

// when feature flags load (async from config.yaml), a previously-hidden group
// may become visible — re-expand the current route's group so the user lands
// in the right context instead of seeing no active group.
watch(visibleMenuGroups, () => {
  const g = groupOf(route.path)
  if (g && !expanded.value.has(g)) {
    expanded.value.add(g)
    expanded.value = new Set(expanded.value)
  }
})

function toggleGroup(key) {
  const next = new Set(expanded.value)
  if (next.has(key)) next.delete(key)
  else next.add(key)
  expanded.value = next
}

function isExpanded(key) {
  return expanded.value.has(key)
}

function navigate(key) {
  router.push(key)
}

const collapsedWidth = 60
const expandedWidth = 200
</script>

<template>
  <div
    class="sidebar"
    :class="{ collapsed: layout.sidebarCollapsed }"
    :style="{ width: layout.sidebarCollapsed ? collapsedWidth + 'px' : expandedWidth + 'px' }"
  >
    <!-- Logo -->
    <div class="sidebar-logo" @click="router.push('/')">
      <img :src="logoUrl" alt="byte-git" class="logo-img" />
      <span v-if="!layout.sidebarCollapsed" class="logo-text">
        <span class="logo-git">byte</span><span class="logo-mate">-git</span>
      </span>
      <div class="logo-actions" v-if="!layout.sidebarCollapsed">
        <button class="logo-btn" @click.stop="theme.toggleTheme()" :title="theme.isDark ? '切换亮色' : '切换暗色'">
          <svg v-if="theme.isDark" viewBox="0 0 24 24" width="14" height="14" fill="currentColor"><path d="M12 3a9 9 0 109 9c0-.46-.04-.92-.1-1.36a5.389 5.389 0 01-4.4 2.26 5.403 5.403 0 01-3.14-9.8c-.44-.06-.9-.1-1.36-.1z"/></svg>
          <svg v-else viewBox="0 0 24 24" width="14" height="14" fill="currentColor"><path d="M6.76 4.84l-1.8-1.79-1.41 1.41 1.79 1.79 1.42-1.41zM4 10.5H1v2h3v-2zm9-9.95h-2V3.5h2V.55zm7.45 3.91l-1.41-1.41-1.79 1.79 1.41 1.41 1.79-1.79zm-3.21 13.7l1.79 1.8 1.41-1.41-1.8-1.79-1.4 1.4zM20 10.5v2h3v-2h-3zm-8-5c-3.31 0-6 2.69-6 6s2.69 6 6 6 6-2.69 6-6-2.69-6-6-6zm-1 16.95h2V19.5h-2v2.95zm-7.45-3.91l1.41 1.41 1.79-1.8-1.41-1.41-1.79 1.8z"/></svg>
        </button>
        <button class="logo-btn" @click.stop="navigate('/settings')" :class="{ active: route.path === '/settings' }" title="设置">
          <SettingOutlined />
        </button>
      </div>
      <!-- collapsed: show icons stacked below logo -->
      <div class="logo-actions-collapsed" v-if="layout.sidebarCollapsed">
        <button class="logo-btn" @click.stop="theme.toggleTheme()" :title="theme.isDark ? '切换亮色' : '切换暗色'">
          <svg v-if="theme.isDark" viewBox="0 0 24 24" width="14" height="14" fill="currentColor"><path d="M12 3a9 9 0 109 9c0-.46-.04-.92-.1-1.36a5.389 5.389 0 01-4.4 2.26 5.403 5.403 0 01-3.14-9.8c-.44-.06-.9-.1-1.36-.1z"/></svg>
          <svg v-else viewBox="0 0 24 24" width="14" height="14" fill="currentColor"><path d="M6.76 4.84l-1.8-1.79-1.41 1.41 1.79 1.79 1.42-1.41zM4 10.5H1v2h3v-2zm9-9.95h-2V3.5h2V.55zm7.45 3.91l-1.41-1.41-1.79 1.79 1.41 1.41 1.79-1.79zm-3.21 13.7l1.79 1.8 1.41-1.41-1.8-1.79-1.4 1.4zM20 10.5v2h3v-2h-3zm-8-5c-3.31 0-6 2.69-6 6s2.69 6 6 6 6-2.69 6-6-2.69-6-6-6zm-1 16.95h2V19.5h-2v2.95zm-7.45-3.91l1.41 1.41 1.79-1.8-1.41-1.41-1.79 1.8z"/></svg>
        </button>
        <button class="logo-btn" @click.stop="navigate('/settings')" :class="{ active: route.path === '/settings' }" title="设置">
          <SettingOutlined />
        </button>
      </div>
    </div>

    <!-- Navigation -->
    <nav class="sidebar-nav">
      <template v-for="group in visibleMenuGroups" :key="group.key">
        <!-- Collapsed mode: flyout (hover to show children in a popover) -->
        <div
          v-if="layout.sidebarCollapsed"
          class="nav-group-collapsed"
          @mouseenter="collapsedHover = group.key"
          @mouseleave="collapsedHover = null"
        >
          <div
            class="nav-item group-head"
            :class="{ 'has-active': groupOf(route.path) === group.key }"
            @click="toggleGroup(group.key)"
          >
            <component :is="group.icon" class="nav-icon" />
          </div>
          <transition name="flyout">
            <div v-if="collapsedHover === group.key" class="flyout">
              <div class="flyout-title">{{ group.label }}</div>
              <div
                v-for="child in group.children"
                :key="child.key"
                class="nav-item flyout-item"
                :class="{ active: route.path === child.key }"
                @click="navigate(child.key)"
              >
                <component :is="child.icon" class="nav-icon" />
                <span class="nav-label">{{ child.label }}</span>
              </div>
            </div>
          </transition>
        </div>

        <!-- Expanded mode: inline collapsible group -->
        <div v-else class="nav-group">
          <div
            class="nav-group-head"
            :class="{ 'has-active': groupOf(route.path) === group.key }"
            @click="toggleGroup(group.key)"
          >
            <component :is="group.icon" class="group-head-icon" />
            <span class="group-head-label">{{ group.label }}</span>
            <CaretRightFilled class="group-caret" :class="{ open: isExpanded(group.key) }" />
          </div>
          <transition name="expand">
            <div v-show="isExpanded(group.key)" class="nav-children">
              <div
                v-for="child in group.children"
                :key="child.key"
                class="nav-item child"
                :class="{ active: route.path === child.key }"
                @click="navigate(child.key)"
              >
                <component :is="child.icon" class="nav-icon" />
                <span class="nav-label">{{ child.label }}</span>
              </div>
            </div>
          </transition>
        </div>
      </template>
    </nav>

    <!-- Bottom: current repo info (mirrors global footer right side) -->
    <div class="sidebar-bottom">
      <div v-if="repo.name" class="repo-display" :title="repo.path">
        <BranchesOutlined class="repo-icon" />
        <span v-if="!layout.sidebarCollapsed" class="repo-info">
          <span class="repo-name">{{ repo.name }}</span>
          <span v-if="repo.branch" class="repo-branch">{{ repo.branch }}</span>
        </span>
      </div>
      <div v-else class="repo-display">
        <BranchesOutlined class="repo-icon muted" />
        <span v-if="!layout.sidebarCollapsed" class="repo-name muted">未选择仓库</span>
      </div>
    </div>
  </div>
</template>

<style scoped>
.sidebar {
  display: flex;
  flex-direction: column;
  height: 100vh;
  background-color: var(--color-surface);
  border-right: 1px solid var(--color-border);
  transition: width var(--transition-normal);
  overflow: clip;
  flex-shrink: 0;
}

.sidebar-logo {
  display: flex;
  align-items: center;
  gap: var(--spacing-sm);
  padding: var(--spacing-md);
  height: var(--navbar-height);
  cursor: pointer;
  flex-shrink: 0;
  border-bottom: 1px solid var(--color-border);
}

.logo-actions {
  margin-left: auto;
  display: flex;
  gap: 2px;
}
.logo-actions-collapsed {
  display: flex;
  flex-direction: column;
  gap: 2px;
  margin-top: 4px;
}
.logo-btn {
  -webkit-appearance: none; appearance: none;
  border: none; background: transparent; cursor: pointer;
  color: var(--color-text-tertiary); font-size: 14px;
  width: 24px; height: 24px;
  display: flex; align-items: center; justify-content: center;
  border-radius: var(--radius-xs);
  transition: all var(--transition-fast);
  --wails-draggable: no-drag;
}
.logo-btn:hover { color: var(--color-primary); background: var(--color-hover); }
.logo-btn.active { color: var(--color-primary); }

.logo-img {
  width: 28px;
  height: 28px;
  border-radius: 7px;
  flex-shrink: 0;
  object-fit: contain;
  /* subtle shadow so it reads on light & dark surfaces */
  filter: drop-shadow(0 1px 2px rgba(0, 0, 0, 0.25));
}
.logo-text { font-size: 16px; font-weight: 700; white-space: nowrap; }
.logo-git { color: var(--color-primary); }
.logo-mate { color: var(--color-text-secondary); }

.sidebar-nav {
  flex: 1;
  min-height: 0; /* allow flex to shrink below content size so footer stays visible */
  overflow-y: auto;
  padding: var(--spacing-sm);
}
.sidebar-nav::-webkit-scrollbar { width: 5px; }
.sidebar-nav::-webkit-scrollbar-thumb { background: var(--color-border); border-radius: 3px; }

/* ---- group head (expanded mode) ---- */
.nav-group { margin-bottom: 2px; }
.nav-group-head {
  display: flex;
  align-items: center;
  gap: var(--spacing-sm);
  padding: 7px 10px;
  border-radius: var(--radius-md);
  cursor: pointer;
  color: var(--color-text-tertiary);
  font-size: 11px;
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: 0.05em;
  transition: all var(--transition-fast);
  white-space: nowrap;
  user-select: none;
}
.nav-group-head:hover { color: var(--color-text-secondary); background: var(--color-hover); }
.nav-group-head.has-active { color: var(--color-primary); }
.group-head-icon { font-size: 14px; flex-shrink: 0; }
.group-head-label { flex: 1; }
.group-caret {
  font-size: 9px;
  transition: transform var(--transition-fast);
  opacity: 0.7;
}
.group-caret.open { transform: rotate(90deg); }

/* ---- children ---- */
.nav-children {
  display: flex;
  flex-direction: column;
  gap: 1px;
  padding-left: 14px;
  overflow: hidden;
}

.nav-item {
  display: flex;
  align-items: center;
  gap: var(--spacing-sm);
  padding: 8px 12px;
  border-radius: var(--radius-md);
  cursor: pointer;
  transition: all var(--transition-fast);
  color: var(--color-text-secondary);
  white-space: nowrap;
}
.nav-item:hover { background: var(--color-hover); color: var(--color-primary); }
.nav-item.active { background: var(--color-primary-bg); color: var(--color-primary); font-weight: 500; }

.nav-item.child { padding: 7px 12px; }
.nav-item.child::before {
  content: '';
  width: 3px; height: 3px; border-radius: 50%;
  background: var(--color-border);
  flex-shrink: 0;
  margin-left: -7px;
}
.nav-item.child.active::before { background: var(--color-primary); }

.nav-icon { font-size: 16px; flex-shrink: 0; }
.nav-label { font-size: 13px; }

/* ---- collapsed mode ---- */
.nav-group-collapsed { position: relative; }
.nav-group-collapsed .nav-item.group-head {
  justify-content: center;
  padding: 10px;
  margin-bottom: 2px;
}
.nav-group-collapsed .nav-item.group-head.has-active {
  background: var(--color-primary-bg);
  color: var(--color-primary);
}

.flyout {
  position: absolute;
  left: calc(100% + 8px);
  top: 0;
  min-width: 160px;
  background: var(--color-background);
  border: 1px solid var(--color-border);
  border-radius: var(--radius-md);
  box-shadow: var(--shadow-lg);
  padding: 6px;
  z-index: 1100;
}
.flyout-title {
  font-size: 10px;
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: 0.05em;
  color: var(--color-text-tertiary);
  padding: 4px 10px;
}
.flyout-item { padding: 7px 10px; }
.flyout-item::before { display: none; }

/* flyout transition */
.flyout-enter-active, .flyout-leave-active { transition: opacity var(--transition-fast), transform var(--transition-fast); }
.flyout-enter-from, .flyout-leave-to { opacity: 0; transform: translateX(-4px); }

/* expand transition */
.expand-enter-active, .expand-leave-active { transition: all var(--transition-fast); }
.expand-enter-from, .expand-leave-to { opacity: 0; max-height: 0; }

.sidebar-bottom {
  min-height: var(--footer-height);
  display: flex;
  align-items: center;
  padding: 6px var(--spacing-md);
  border-top: 1px solid var(--color-border);
  flex-shrink: 0;
  overflow: visible;
}
.repo-display {
  display: flex;
  align-items: center;
  gap: 6px;
  min-width: 0;
}
.repo-icon { color: var(--color-primary); font-size: 13px; flex-shrink: 0; }
.repo-icon.muted { color: var(--color-text-tertiary); }
.repo-info { display: inline-flex; align-items: center; gap: 6px; min-width: 0; }
.repo-name {
  font-size: 12px;
  font-weight: 600;
  line-height: 1.2;
  color: var(--color-text);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.repo-name.muted { font-weight: 400; color: var(--color-text-tertiary); }
.repo-branch {
  font-size: 10px;
  line-height: 1.2;
  color: var(--color-text-tertiary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

/* collapsed: center the repo icon */
.collapsed .sidebar-bottom { justify-content: center; }
.collapsed .sidebar-logo { justify-content: center; padding: var(--spacing-md) 0; flex-direction: column; gap: 4px; }
.collapsed .nav-item { justify-content: center; padding: 10px; }
</style>
