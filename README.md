# byte-git

> Git 增强工具集（CLI + 桌面端）：分支治理、命名 stash、多仓库开发统计、完整备份、模板管理，一条命令搞定。

**English**: [README_EN.md](README_EN.md)

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/Rust-1.85%2B-dea584?logo=rust)](https://www.rust-lang.org)
[![Release](https://github.com/cicbyte/byte-git/actions/workflows/release.yml/badge.svg)](https://github.com/cicbyte/byte-git/actions/workflows/release.yml)
[![Platforms](https://img.shields.io/badge/platform-Linux%20%7C%20Windows%20%7C%20macOS-lightgrey)](https://github.com/cicbyte/byte-git/releases)

<!-- screenshot: 在此处添加桌面端演示截图（GUI 主界面 / logview TUI） -->

## 目录

- [功能矩阵](#功能矩阵)
- [安装与构建](#安装与构建)
- [快速开始](#快速开始)
- [项目结构](#项目结构)
- [配置与数据](#配置与数据)
- [桌面端架构](#桌面端架构)
- [开源许可证](#开源许可证)

## 功能矩阵

| 命令 | 说明 |
|---|---|
| `version` | 版本信息 |
| `config` | 全局配置读写（YAML） |
| `branch` | 批量清理已合并分支、列出孤立分支 |
| `stash` | 带名称保存/弹出 stash，支持搜索 |
| `info` | 仓库概览 summary / 健康检查 health |
| `stats` | 多仓库提交统计、活跃时段、连续天数、语言分布 |
| `backup` | 完整仓库备份（git clone --mirror） |
| `license` | 47 份开源许可证模板创建与检测 |
| `ignore` | .gitignore 模板管理（幂等追加） |
| `clone` | 代理加速克隆（Lua 规则引擎，镜像/预设/-r 还原） |
| `download` | GitHub Release 资产下载（平台自动匹配/进度条） |
| `archive` | 仓库快照归档（tar.gz/zip/tar） |
| `ai` | AI 提交信息生成 / 历史批量改写（gen→run→apply） |
| `rewrite` | 提交元数据改写（作者/邮箱/日期/消息，规则+日期重分配） |
| `review` | AI 代码审查（外部 OCR 集成） |
| `secret` | 密钥泄露扫描（外部 gitleaks 集成，脱敏报告） |
| `hook` | Git 钩子管理（Lua/脚本，安装-执行-拦截） |
| `rebase` | Rebase 预览与 autosquash（fixup!/squash! 整理） |
| `undo` | 安全撤销（上次提交/rebase/merge，支持备份） |
| `logview` | Git 日志 TUI 浏览（列表/diff/搜索，ratatui） |

## 安装与构建

**预编译产物**（Linux / Windows / macOS amd64）：推送 `v*` tag 后由 [Release 流水线](https://github.com/cicbyte/byte-git/actions/workflows/release.yml)自动构建，从 [Releases](https://github.com/cicbyte/byte-git/releases) 下载对应平台压缩包解压即用。

**源码构建**：要求 Rust >= 1.85（2024 edition 兼容）、Git >= 2.0。

```bash
git clone https://github.com/cicbyte/byte-git
cd byte-git
cargo build --release -p byte-git-cli
# 产物：target/release/byte-git
```

**桌面端**（Tauri v2，需 Node >= 20）：

```bash
cd apps/desktop
npm --prefix frontend install
cargo tauri build          # 或 cargo tauri dev 开发模式
```

## 快速开始

```bash
byte-git version                     # 查看版本
byte-git info summary                # 仓库概览
byte-git info health                 # 健康检查
byte-git branch clean --dry-run      # 预览可清理的已合并分支
byte-git stash save --name wip       # 命名保存
byte-git stash list --search wip     # 搜索
byte-git stats --days 30             # 最近 30 天开发统计
byte-git backup create               # 镜像备份当前仓库
byte-git license create mit          # 创建 MIT LICENSE
byte-git ignore add rust node        # 追加 rust/node gitignore 模板
```

## 项目结构

```
byte-git/
├── apps/
│   ├── cli/                 # CLI（clap 命令树，二进制名 byte-git）
│   └── desktop/             # Tauri v2 桌面端
│       ├── frontend/        # Vue 3 + Vite + Pinia（含 Wails→Tauri 适配层）
│       └── src-tauri/       # Tauri 命令层（薄封装 crates/core）
├── crates/
│   └── core/                # 领域核心：git 操作 / 配置 / 备份 / 统计 / 模板
│       └── assets/          # 内嵌资产：47 份许可证 + gitignore 模板 + AI 提示词
├── docs/                    # 命令文档
├── scripts/                 # 发布脚本
└── rules/ prompts/          # Lua 代理规则 / 提示词源文件
```

## 配置与数据

```
~/.cicbyte/apps/byte-git/  # 应用根（旧 ~/.cicbyte/byte-git/ 首次运行自动迁移）
  config/config.yaml    # 全局配置（AI 模型、GitHub Token、功能开关）
  backups/              # 完整仓库备份
  prompts/              # AI 提示词模板（可自定义）
~/.cicbyte/data/        # 跨项目共享数据（cicbyte 工具族复用）
.git/byte-git/          # 项目级数据（hook 配置、改写 JSON、AI 缓存）
```

## 桌面端架构

前端通过 `window.go.main.ByteGitApp.Method(...)` 风格的绑定 API 调用后端，`frontend/src/wailsjs/`
提供适配层将其桥接到 Tauri：

- `go/main/ByteGitApp.ts` —— Proxy 桥接：方法名 camelCase → Tauri 命令 snake_case，参数按签名表编组
- `runtime/runtime.ts` —— `EventsOn/EventsOff/EventsEmit` → Tauri 事件

前端代码零改动运行于 Tauri。Rust 命令层已开放 **262 个命令**，组织为 15 个域模块
（commit / gitops / backup+log / info+stats / repo / rewrite / secret / license / system /
ignore / github / reference / live / terminal / wallpaper）：覆盖 git 操作、AI 提交与改写、密钥扫描、
备份归档、日志浏览、设置与模板管理，以及 GitHub REST 集成、实时文件监视（notify）、内嵌终端
（portable-pty ConPTY）、壁纸管理等扩展域——前端方法调用面全覆盖。

## 开源许可证

[MIT](LICENSE) © 2026 cicbyte
