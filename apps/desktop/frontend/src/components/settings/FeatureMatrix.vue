<script setup>
/**
 * GUI ↔ CLI feature comparison matrix.
 * Shown in Settings → 关于. Hardcoded — update when new features are added.
 */
const features = [
  // group, feature, gui status, cli command, note
  { group: '浏览与变更', feature: 'Git Log 浏览器', gui: 'full', cli: 'log', note: 'GUI 三栏+hover+右键操作；CLI 为 TUI' },
  { group: '浏览与变更', feature: '工作区浏览', gui: 'full', cli: '—', note: 'GUI 独有（文件树+搜索+代码预览，纯只读，告别被迫开 IDE）' },
  { group: '浏览与变更', feature: '实时变更', gui: 'full', cli: '—', note: 'GUI 独有（文件系统监听 + 暂存/提交/推送）' },
  { group: '浏览与变更', feature: '代码穿梭', gui: 'full', cli: '—', note: 'GUI 独有（浏览任意 commit 的完整源码）' },
  { group: '浏览与变更', feature: '终端', gui: 'full', cli: '—', note: 'GUI 独有（内嵌 PTY，支持 git-bash）' },
  { group: '浏览与变更', feature: '仓库洞察', gui: 'full', cli: 'info', note: 'GUI 9 个 tab；CLI 9 个子命令' },
  { group: '引用管理', feature: '引用仓库', gui: 'full', cli: '—', note: 'GUI 独有（浏览引用代码/wiki/统计，文件搜索+终端联动）' },
  { group: '引用管理', feature: '引用管理', gui: 'full', cli: '—', note: 'GUI 独有（初始化/健康检查/Wiki 同步/远程配置）' },
  { group: '引用管理', feature: '功能开关', gui: 'full', cli: '—', note: 'GUI 独有（按需启用 GitHub/引用/密钥扫描/代码审查）' },
  { group: '历史改写', feature: '元数据改写', gui: 'full', cli: 'rewrite', note: 'GUI 可视化编辑 + 规则/逐日分配' },
  { group: '历史改写', feature: 'AI 改写', gui: 'full', cli: 'ai rewrite', note: 'GUI 任务驱动工作流' },
  { group: '历史改写', feature: '数据编辑器', gui: 'full', cli: '—', note: 'GUI 独有（表单/JSON 双模式）' },
  { group: '历史改写', feature: 'Commit 操作', gui: 'full', cli: '—', note: 'GUI 独有（cherry-pick/revert/reset/创建分支）' },
  { group: '规范治理', feature: 'Gitignore 管理', gui: 'full', cli: 'ignore', note: '' },
  { group: '规范治理', feature: '许可证管理', gui: 'full', cli: 'license', note: '' },
  { group: '规范治理', feature: '密钥扫描', gui: 'full', cli: 'secret', note: 'GUI 支持文件分组 + 一键抹除' },
  { group: '规范治理', feature: 'AI 代码审查', gui: 'full', cli: 'review', note: 'GUI 支持分步进度 + LLM 配置（12 家 Provider）' },
  { group: '数据管理', feature: '备份管理', gui: 'full', cli: 'backup', note: 'GUI 文件浏览器在线预览' },
  { group: '数据管理', feature: 'Git 管理', gui: 'full', cli: 'gitconfig', note: 'GUI 含分支清理/Stash/Rebase/Undo' },
  { group: '数据管理', feature: 'Stash 管理', gui: 'full', cli: 'stash', note: 'GUI 命名保存/弹出/删除/diff预览' },
  { group: '数据管理', feature: '开发统计', gui: 'full', cli: 'stats', note: 'GUI 热力图+按日钻取提交列表；CLI 文本表格' },
  { group: '工作流', feature: '命令片段库', gui: 'full', cli: '—', note: 'GUI 独有（Ctrl+K 全局搜索 + 模板变量 + 终端联动）' },
  { group: '工作流', feature: 'Git 备忘录', gui: 'full', cli: '—', note: 'GUI 独有（自动 clone 内容仓库）' },
  { group: '工作流', feature: 'Hook 管理', gui: 'full', cli: 'hook', note: 'GUI CodeMirror 编辑 Lua 脚本 + 测试运行；CLI 命令行' },
  { group: '工作流', feature: '代理规则', gui: 'full', cli: '—', note: 'GUI 独有（CodeMirror 编辑 rules.lua）' },
  { group: 'GitHub', feature: 'Pull Requests', gui: 'full', cli: '—', note: 'GUI 独有（列表/详情/diff/评论/review/创建/筛选搜索）' },
  { group: 'GitHub', feature: 'Issues', gui: 'full', cli: '—', note: 'GUI 独有（列表/创建/关闭/评论/标签/筛选搜索）' },
  { group: 'GitHub', feature: 'Actions / CI', gui: 'full', cli: '—', note: 'GUI 独有（运行列表/Jobs/日志/rerun/cancel）' },
  { group: 'GitHub', feature: 'Release 管理', gui: 'full', cli: 'download', note: 'GUI 创建/删除/上传资产；CLI 仅下载' },
  { group: 'GitHub', feature: '代码搜索', gui: 'full', cli: '—', note: 'GUI 独有（仓库内代码搜索 + 全 GitHub 仓库搜索，匹配片段高亮）' },
  { group: 'GitHub', feature: 'Trending', gui: 'full', cli: '—', note: 'GUI 独有（今日/本周/月热门仓库 + 编程语言/自然语言筛选 + star 增量）' },
  { group: '基础', feature: '代理克隆', gui: '—', cli: 'clone', note: 'CLI 独有（代理加速 + 预设）' },
  { group: '基础', feature: 'Release 下载', gui: 'full', cli: 'download', note: 'GUI 在 Navbar 下载栏' },
  { group: '基础', feature: '仓库打包', gui: '—', cli: 'archive', note: 'CLI 独有' },
  { group: '基础', feature: '分支清理', gui: 'partial', cli: 'branch', note: 'GUI 在 Git 管理页；CLI 更完整' },
  { group: '基础', feature: 'Rebase 辅助', gui: 'partial', cli: 'rebase', note: 'GUI 在 Git 管理页（预览）；CLI 有 auto-squash' },
  { group: '基础', feature: '安全撤销', gui: 'partial', cli: 'undo', note: 'GUI 在 Git 管理页；CLI 有 commit/rebase/merge undo' },
  { group: '基础', feature: '配置管理', gui: 'full', cli: 'config', note: 'GUI 表单；CLI 命令行（完全互通）' },
]

// group order
const groups = [...new Set(features.map((f) => f.group))]

const statusLabel = { full: '✅ 完整', partial: '🟡 部分', '—': '❌ 无' }
const statusClass = { full: 'st-full', partial: 'st-partial', '—': 'st-none' }
</script>

<template>
  <div class="fm-wrap">
    <div class="fm-legend">
      <span class="fm-leg-item"><span class="fm-dot st-full"></span> 完整支持</span>
      <span class="fm-leg-item"><span class="fm-dot st-partial"></span> 部分支持</span>
      <span class="fm-leg-item"><span class="fm-dot st-none"></span> 不支持</span>
    </div>

    <!-- single table so all columns share the same width across groups -->
    <table class="fm-table">
      <colgroup>
        <col class="fm-col-feature" />
        <col class="fm-col-gui" />
        <col class="fm-col-cli" />
        <col class="fm-col-note" />
      </colgroup>
      <thead>
        <tr>
          <th class="fm-th">功能</th>
          <th class="fm-th">GUI</th>
          <th class="fm-th">CLI 命令</th>
          <th class="fm-th">说明</th>
        </tr>
      </thead>
      <tbody>
        <template v-for="g in groups" :key="g">
          <tr class="fm-group-row">
            <td colspan="4">{{ g }}</td>
          </tr>
          <tr v-for="f in features.filter((x) => x.group === g)" :key="f.feature" class="fm-data-row">
            <td class="fm-td-feature">{{ f.feature }}</td>
            <td class="fm-td-status">
              <span class="fm-status" :class="statusClass[f.gui]">{{ statusLabel[f.gui] }}</span>
            </td>
            <td class="fm-td-cli">
              <code v-if="f.cli !== '—'">byte-git {{ f.cli }}</code>
              <span v-else class="fm-dash">—</span>
            </td>
            <td class="fm-td-note">{{ f.note }}</td>
          </tr>
        </template>
      </tbody>
    </table>
  </div>
</template>

<style scoped>
.fm-wrap { margin-top: var(--spacing-lg); }

.fm-legend { display: flex; gap: 16px; margin-bottom: 12px; }
.fm-leg-item { display: flex; align-items: center; gap: 6px; font-size: 12px; color: var(--color-text-secondary); }
.fm-dot { width: 8px; height: 8px; border-radius: 50%; }
.fm-dot.st-full { background: var(--color-success); }
.fm-dot.st-partial { background: var(--color-warning); }
.fm-dot.st-none { background: var(--color-text-tertiary); }

/* single table with fixed colgroup widths so every row aligns */
.fm-table { width: 100%; border-collapse: collapse; table-layout: fixed; }
.fm-col-feature { width: 28%; }
.fm-col-gui { width: 14%; }
.fm-col-cli { width: 20%; }
.fm-col-note { width: 38%; }

.fm-th {
  text-align: left; font-size: 11px; font-weight: 600;
  color: var(--color-text-tertiary); padding: 6px 10px;
  text-transform: uppercase; letter-spacing: 0.03em;
  border-bottom: 1px solid var(--color-border);
}

/* group separator row */
.fm-group-row td {
  font-size: 12px; font-weight: 600; color: var(--color-primary);
  padding: 10px 10px 4px;
  border-bottom: 1px solid var(--color-border-light);
  background: var(--color-surface);
}

.fm-data-row { transition: background var(--transition-fast); }
.fm-data-row:hover { background: var(--color-hover); }

.fm-td-feature { font-size: 13px; color: var(--color-text); padding: 6px 10px; }
.fm-td-status { padding: 6px 10px; white-space: nowrap; }
.fm-status { font-size: 11px; font-weight: 500; }
.fm-status.st-full { color: var(--color-success); }
.fm-status.st-partial { color: var(--color-warning); }
.fm-status.st-none { color: var(--color-text-tertiary); }

.fm-td-cli { padding: 6px 10px; }
.fm-td-cli code {
  font-family: 'Cascadia Code', monospace; font-size: 11px;
  color: var(--color-primary); background: var(--color-primary-bg);
  padding: 1px 6px; border-radius: 3px;
}
.fm-dash { color: var(--color-text-tertiary); }

.fm-td-note { font-size: 11px; color: var(--color-text-tertiary); padding: 6px 10px; line-height: 1.5; word-break: break-word; }
</style>
