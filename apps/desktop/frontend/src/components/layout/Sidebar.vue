<script setup>
import { computed } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { useLayoutStore } from '../../stores/layout'
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
  EyeOutlined,
  HistoryOutlined,
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
    </div>

    <!-- Navigation：平铺模式（参考 byte-writer）——全部菜单项常显直达，
         小标题仅作视觉分隔不折叠；分组折叠只增加点击成本且收起时当前
         路由高亮不可见。折叠态为纯图标列（小标题隐藏，tooltip 补全名）。 -->
    <nav class="sidebar-nav">
      <template v-for="group in visibleMenuGroups" :key="group.key">
        <!-- 单子项组：提升为独立项（略重，无小标题） -->
        <template v-if="group.children.length === 1">
          <div
            v-for="child in group.children"
            :key="child.key"
            class="nav-item solo"
            :class="{ active: route.path === child.key }"
            :title="layout.sidebarCollapsed ? child.label : undefined"
            @click="navigate(child.key)"
          >
            <component :is="child.icon" class="nav-icon" />
            <span v-if="!layout.sidebarCollapsed" class="nav-label">{{ child.label }}</span>
          </div>
        </template>

        <!-- 多子项组：小标题分隔 + 子项常显 -->
        <template v-else>
          <div v-if="!layout.sidebarCollapsed" class="nav-group">
            <div class="nav-label-title">{{ group.label }}</div>
            <div
              v-for="child in group.children"
              :key="child.key"
              class="nav-item"
              :class="{ active: route.path === child.key }"
              @click="navigate(child.key)"
            >
              <component :is="child.icon" class="nav-icon" />
              <span class="nav-label">{{ child.label }}</span>
            </div>
          </div>
          <!-- 折叠态：子项并入图标列 -->
          <template v-else>
            <div
              v-for="child in group.children"
              :key="child.key"
              class="nav-item icon-only"
              :class="{ active: route.path === child.key }"
              :title="child.label"
              @click="navigate(child.key)"
            >
              <component :is="child.icon" class="nav-icon" />
            </div>
          </template>
        </template>
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
  position: relative;
  z-index: 2;  /* sit above wallpaper layer (.wp-layer z:0, .wp-mask z:1) */
  display: flex;
  flex-direction: column;
  height: 100vh;
  background-color: var(--bg-sider);
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

/* ---- 分组小标题：仅视觉分隔，不可点击（平铺模式） ---- */
.nav-group { margin-bottom: 2px; }
.nav-label-title {
  padding: 14px 10px 4px;
  font-size: 11px;
  font-weight: 600;
  color: var(--color-text-tertiary);
  letter-spacing: 0.05em;
  text-transform: uppercase;
  user-select: none;
  white-space: nowrap;
}
.nav-group:first-of-type .nav-label-title { padding-top: 6px; }

/* ---- 菜单项：30px 行高胶囊，低透明度层次 ---- */
.nav-item {
  display: flex;
  align-items: center;
  gap: var(--spacing-sm);
  height: 30px;
  padding: 0 10px;
  margin-bottom: 2px;
  border-radius: var(--radius-md);
  cursor: pointer;
  transition: background-color var(--transition-fast), color var(--transition-fast);
  color: var(--color-text-secondary);
  white-space: nowrap;
  user-select: none;
}
.nav-item:hover { background: var(--color-hover); color: var(--color-text); }
.nav-item.active { background: var(--color-primary-bg); color: var(--color-primary); font-weight: 550; }

/* 单子项组提升的独立项（比普通菜单项略重） */
.nav-item.solo { padding: 0 12px; font-weight: 500; }

/* 折叠态图标列 */
.nav-item.icon-only { justify-content: center; padding: 0; height: 32px; }

.nav-icon { font-size: 15px; color: var(--color-text-tertiary); flex-shrink: 0; transition: color var(--transition-fast); }
.nav-item:hover .nav-icon,
.nav-item.active .nav-icon { color: inherit; }
.nav-label { font-size: 13px; }

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
