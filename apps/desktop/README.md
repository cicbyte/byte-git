# byte-git GUI

**[English](README_EN.md) | 中文**

> byte-git 的桌面应用 — 用可视化界面操作 Git 增强工具集的全部能力，无需记命令。

基于 Tauri v2（Rust 后端）+ Vue 3（前端）构建，编译为单一原生可执行文件，无 Electron 体积负担。

> 前端移植自 git-mate（Wails），通过 `frontend/src/wailsjs/` 适配层运行于 Tauri：方法调用经 Proxy 桥接转发为 `invoke()`，事件经 `EventsOn/EventsOff` 转发为 Tauri 事件。

![仪表盘](images/仪表盘.png)

## 界面预览

| | |
|---|---|
| ![Git Log](images/git%20log.png) | ![代码穿梭](images/代码穿梭.png) |
| **Git Log** — 提交树 / diff / 右键操作 | **代码穿梭** — 浏览任意 commit 源码 |
| ![仓库洞察](images/仓库洞察.png) | ![开发统计](images/开发统计.png) |
| **仓库洞察** — 活动分析 / 健康检查 | **开发统计** — 提交热力图 / 贡献排行 |
| ![Gitignore](images/gitignore.png) | ![许可证](images/许可证.png) |
| **Gitignore** — AI 整理 / 模板管理 | **许可证** — 创建 / 检测 / 审计 |
| ![Git 备忘录](images/git备忘录.png) | |
| **Git 备忘录** — 命令速查三栏浏览 | |

## 功能特性

- **四区布局** — 项目轨道 + 侧边栏 + 顶栏 + 内容区，多仓库一键切换
- **可视化 Git 操作** — 提交树浏览、side-by-side diff、右键操作（cherry-pick / revert / reset / 创建分支）
- **实时变更监听** — 文件系统级监听，顶栏徽章实时反映工作区改动数，抽屉式暂存 / 提交 / 推送
- **内嵌终端** — PTY 终端（Windows ConPTY / Unix pty），自动检测 git-bash，跟随项目切换
- **AI 辅助** — AI 生成 commit message、AI 改写历史、AI 代码审查（集成 OpenCodeReview）
- **GitHub 全集成** — PR / Issues / Actions / Releases / 代码搜索 / Trending，操作不离开应用
- **引用仓库浏览** — 浏览引用的代码仓库（文件树 / 搜索 / 代码预览）、Wiki 知识库
- **历史改写** — 提交元数据可视化编辑、日期重映射、AI 批量改写，预览影响后再应用
- **安全治理** — 全历史密钥泄露扫描（gitleaks）、.gitignore 管理、开源许可证管理
- **主题与响应式** — 浅色 / 深色主题，移动端自动折叠侧边栏

## 快速开始

### 环境要求

- [Rust](https://rustup.rs/) >= 1.85（含 MSVC 工具链 / WebView2，Windows）
- [Node.js](https://nodejs.org/) >= 20
- [Tauri CLI](https://tauri.app/) v2：`cargo install tauri-cli --version "^2"`
- Git >= 2.0

### 开发模式

```bash
cd apps/desktop
npm --prefix frontend install   # 安装前端依赖（首次）
cargo tauri dev                 # 启动开发服务器，前端热重载
```

### 构建产物

```bash
cd apps/desktop
cargo tauri build               # 产物输出到 src-tauri/target/release/bundle/
```

构建结果为单一原生可执行文件（`ByteGit.exe`），前端 dist 由 Tauri 嵌入资源。

## 技术栈

| 层级 | 技术 |
|---|---|
| 桌面框架 | [Tauri](https://tauri.app/) v2 — Rust 后端 + Webview 前端 |
| 前端框架 | Vue 3.5 + TypeScript 5.7 |
| 构建工具 | Vite 6 |
| UI 组件库 | Ant Design Vue 4.2 |
| 状态管理 | Pinia 3 |
| 路由 | Vue Router 4（hash 模式） |
| 动画 | GSAP 3 |
| 终端 | xterm.js 6 + addon-fit |
| 代码编辑器 | CodeMirror 6（Go / JS / JSON / Markdown / Python / YAML） |
| Markdown | md-editor-v3 + marked |
| 后端语言 | Rust（领域逻辑在 `crates/core`，命令层在 `src-tauri`） |
| 前后端桥接 | `@tauri-apps/api` invoke + Wails 兼容适配层 |

## 界面布局

```
┌─────────┬──────────────┬───────────────────────────────────┐
│ 项目轨道 │   侧边栏      │            顶栏                    │
│         │              │  面包屑 │ 实时变更│终端│SCM│下载│窗口 │
│ 项目 A  │ ▸ 总览        ├───────────────────────────────────┤
│ 项目 B  │ ▸ 代码浏览    │                                   │
│ 项目 C  │ ▸ 历史改写    │                                   │
│         │ ▸ 规范治理    │            内 容 区                │
│ + 添加  │ ▸ 数据        │                                   │
│         │ ▸ GitHub      │                                   │
│         │ ▸ ...         │                                   │
└─────────┴──────────────┴───────────────────────────────────┘
```

- **项目轨道**（最左侧）：持久列出最近打开的仓库，单击切换，彩色头像基于名称哈希自动着色
- **侧边栏**：9 分组分类（总览 / 代码浏览 / 历史改写 / 规范治理 / 数据 / 终端 / GitHub / 引用 / 帮助），可折叠为图标模式
- **顶栏**：面包屑导航、项目轨道 / 侧边栏切换、快捷入口、全局 SCM 徽章、Release 下载、窗口控制

## 功能模块

| 模块 | 说明 |
|---|---|
| **仪表盘** | 仓库概览、今日提交 / 变更 / 待推送 / 文件数统计卡片、语言分布、快捷操作 |
| **工作区浏览** | 只读浏览当前仓库代码（文件树 + 搜索 + 语法高亮预览） |
| **Git Log** | 三栏：提交树 / 变更文件树 / side-by-side diff，hover 卡片预览、右键操作 |
| **实时变更** | 文件系统监听实时刷新工作区改动，内置暂存 / 提交 / 推送 / AI 生成 message |
| **代码穿梭** | 浏览任意 commit 的完整源码（git show），左提交树 / 中文件树 / 右代码预览 |
| **终端** | 内嵌 PTY 终端，自动检测 git-bash，切换项目时自动跟随 |
| **改写** | 提交元数据可视化编辑（作者 / 邮箱 / 日期 / message），逐日分配、规则批处理 |
| **AI 改写** | 任务驱动改写：规则匹配 → AI 生成 → 审核采用 → 批量应用 |
| **数据编辑器** | 表单 / JSON 双模式编辑改写数据 |
| **代码审查** | AI 审查工作区 / 提交范围 / 单提交，分步进度、文件分组、diff 对比建议 |
| **密钥扫描** | 全历史密钥泄露扫描（gitleaks），按文件分组定位，一键抹除 |
| **Gitignore** | AI 整理、本地 / GitHub 模板、检测、模板管理 |
| **许可证** | 创建、检测、依赖许可证审计 |
| **备份管理** | 完整 clone 备份，文件浏览器在线预览 |
| **Git 管理** | 批量分支清理、stash 管理、rebase 预览、安全撤销 |
| **Git 备忘录** | 三栏 Git 命令速查（首次自动 clone 内容仓库） |
| **命令片段** | 个人 Git 命令库（Ctrl+K 全局搜索 + 模板变量 + 终端联动） |
| **GitHub** | PR / Issues / Actions / Releases / 代码搜索 / Trending 全流程管理 |
| **引用仓库** | 浏览引用的代码仓库、Wiki 知识库、代码统计 |
| **开发统计** | 提交热力图（按日钻取）、连续天数、贡献排行 |
| **全局 SCM** | 顶栏徽章实时反映变更数（全局文件监听），抽屉式暂存 / 提交 / 推送 |

## 项目结构

```
apps/desktop/
├── main.go                 # Wails 应用入口，窗口配置（1200×800，无边框）
├── app.go                  # ByteGitApp 实例，生命周期管理，PTY / watcher
├── internal/               # Go 后端绑定模块（~15 个包，~280 个 Wails 方法）
├── wails.json              # Wails 构建配置
├── frontend/               # Vue 3 前端
│   ├── src/
│   │   ├── views/          # 31 个视图页面
│   │   ├── components/     # 64 个组件（layout / common / info / settings ...）
│   │   ├── stores/         # Pinia 状态（repo / scm / layout / theme / snippets ...）
│   │   ├── composables/    # 组合式函数
│   │   ├── router/         # 路由配置（hash 模式）
│   │   └── main.ts         # 前端入口
│   ├── vite.config.ts      # Vite + Wails dev 适配
│   └── package.json
└── bin/                    # 构建产物输出
```

## 配置

GUI 与 CLI 共享同一套配置，存储位置：

| 路径 | 内容 |
|---|---|
| `~/.cicbyte/byte-git/config.yaml` | 全局配置（AI 模型、代理、Token） |
| `~/.cicbyte/byte-git/backups/` | 完整仓库备份 |
| `~/.cicbyte/byte-git/prompts/` | AI 提示词模板 |
| `~/.opencodereview/config.json` | 代码审查 LLM 配置（12 家预设 Provider + 自定义网关） |
| `.git/byte-git/` | 项目级数据（hook 配置、改写 JSON、AI 缓存） |

功能开关（设置页）：按需启用 GitHub、引用仓库、密钥扫描、代码审查，关闭后侧边栏菜单自动隐藏空分组。

## 开源许可证

[MIT](../LICENSE) © 2025 cicbyte
