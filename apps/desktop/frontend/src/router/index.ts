import { createRouter, createWebHashHistory } from 'vue-router'
import HomeView from '../views/overview/home/index.vue'

// projectScoped: true (default) — the page operates on the current git repo;
//                 ProjectRail stays visible.
//               false — the page is global/personal (cheat sheet, snippets,
//                 settings, trending, search); ProjectRail slides out.
const routes = [
  { path: '/', name: 'home', component: HomeView, meta: { title: '仪表盘' } },
  { path: '/info', name: 'info', component: () => import('../views/overview/info/index.vue'), meta: { title: '仓库洞察' } },
  { path: '/log', name: 'log', component: () => import('../views/browse/log/index.vue'), meta: { title: 'Git Log' } },
  { path: '/live', name: 'live', component: () => import('../views/browse/live/index.vue'), meta: { title: '实时变更' } },
  { path: '/workspace', name: 'workspace', component: () => import('../views/browse/workspace/index.vue'), meta: { title: '工作区' } },
  { path: '/time-travel', name: 'time-travel', component: () => import('../views/browse/time-travel/index.vue'), meta: { title: '代码穿梭' } },
  { path: '/terminal', name: 'terminal', component: () => import('../views/terminal/index.vue'), meta: { title: '终端' } },
  // /commit 已合并入 /live（实时提交台），保留重定向以兼容旧入口
  { path: '/commit', redirect: '/live' },
  { path: '/json-editor', name: 'json-editor', component: () => import('../views/rewrite/data-editor/index.vue'), meta: { title: '数据编辑器' } },
  { path: '/ignore', name: 'ignore', component: () => import('../views/governance/ignore/index.vue'), meta: { title: 'Gitignore' } },
  { path: '/license', name: 'license', component: () => import('../views/governance/license/index.vue'), meta: { title: '许可证' } },
  { path: '/rewrite', name: 'rewrite', component: () => import('../views/rewrite/meta-rewrite/index.vue'), meta: { title: '改写' } },
  { path: '/ai-rewrite', name: 'ai-rewrite', component: () => import('../views/rewrite/ai-rewrite/index.vue'), meta: { title: 'AI改写' } },
  { path: '/review', name: 'review', component: () => import('../views/governance/review/index.vue'), meta: { title: '代码审查' } },
  { path: '/secret', name: 'secret', component: () => import('../views/governance/secret/index.vue'), meta: { title: '密钥扫描' } },
  { path: '/backup', name: 'backup', component: () => import('../views/data/backup/index.vue'), meta: { title: '备份管理' } },
  { path: '/git-manage', name: 'git-manage', component: () => import('../views/data/git-manage/index.vue'), meta: { title: 'Git 管理' } },
  { path: '/stash', name: 'stash', component: () => import('../views/data/stash/index.vue'), meta: { title: 'Stash 管理' } },
  // --- non-project-scoped pages (ProjectRail hidden) ---
  { path: '/cheat', name: 'cheat', component: () => import('../views/help/cheat/index.vue'), meta: { title: 'Git 备忘录', projectScoped: false } },
  { path: '/github', name: 'github', component: () => import('../views/github/pr/index.vue'), meta: { title: 'Pull Requests' } },
  { path: '/github-issues', name: 'github-issues', component: () => import('../views/github/issues/index.vue'), meta: { title: 'Issues' } },
  { path: '/github-actions', name: 'github-actions', component: () => import('../views/github/actions/index.vue'), meta: { title: 'Actions' } },
  { path: '/github-releases', name: 'github-releases', component: () => import('../views/github/releases/index.vue'), meta: { title: 'Releases' } },
  { path: '/github-search', name: 'github-search', component: () => import('../views/github/search/index.vue'), meta: { title: '搜索', projectScoped: false } },
  { path: '/github-trending', name: 'github-trending', component: () => import('../views/github/trending/index.vue'), meta: { title: 'Trending', projectScoped: false } },
  { path: '/snippets', name: 'snippets', component: () => import('../views/help/snippets/index.vue'), meta: { title: '命令片段', projectScoped: false } },
  { path: '/reference', name: 'reference', component: () => import('../views/reference/repos/index.vue'), meta: { title: '引用仓库' } },
  { path: '/reference-ops', name: 'reference-ops', component: () => import('../views/reference/ops/index.vue'), meta: { title: '引用管理', projectScoped: false } },
  { path: '/stats', name: 'stats', component: () => import('../views/overview/stats/index.vue'), meta: { title: '开发统计' } },
  { path: '/settings', name: 'settings', component: () => import('../views/settings/index.vue'), meta: { title: '设置', projectScoped: false } },
]

const router = createRouter({
  history: createWebHashHistory(),
  routes,
})

export default router
