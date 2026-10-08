# gui — 桌面应用

byte-git 的图形桌面应用，基于 Tauri v2（Rust 后端）+ Vue 3 构建，可视化操作 byte-git 的核心功能。

> **注意**：GUI 是**独立的可执行文件**（`ByteGit`），不是 `byte-git` CLI 的子命令。需要单独构建，详见[打包指南](build-guide.md)。

## 启动

```bash
# 如果已构建可执行文件（apps/desktop/src-tauri/target/release/ByteGit.exe）
./apps/desktop/src-tauri/target/release/ByteGit.exe

# 开发模式（热重载）
cd apps/desktop
cargo tauri dev
```

## 功能模块

GUI 包含以下主要模块：

| 模块 | 功能 |
|---|---|
| **仪表盘** | 仓库概览、最近活动、快速操作 |
| **Git Log** | 可视化提交历史浏览 |
| **实时变更** | 文件系统监听、实时 diff 查看、提交台 |
| **代码穿梭** | 历史版本对比与浏览 |
| **仓库洞察** | 健康检查、活动分析、热点文件、作者贡献 |
| **提交热力图** | 按年的每日提交热力图、贡献排行 |
| **AI 改写** | 可视化改写提交元数据、AI 辅助 |
| **代码审查** | AI 审查改动（集成 OpenCodeReview） |
| **密钥扫描** | 扫描密钥泄露（集成 gitleaks） |
| **Gitignore** | 可视化 .gitignore 管理 |
| **许可证** | 创建 / 检测 LICENSE |
| **备份管理** | 完整仓库备份 |
| **Git 管理** | Git 配置管理 |
| **源代码管理** | 全局暂存 / 提交 / SCM 抽屉（徽章自动刷新） |

## 技术栈

- **后端**：Rust + Tauri v2（领域逻辑在 `crates/core`，命令层在 `apps/desktop/src-tauri/`）
- **前端**：Vue 3 + Vite + Ant Design Vue + Pinia（`apps/desktop/frontend/`）
- **桥接**：`frontend/wailsjs/` 适配层（Wails 绑定调用 → Tauri invoke，事件 → Tauri 事件）
- **构建**：`cargo tauri build`（前端编译后由 Tauri 嵌入资源）

## 应用特性

- **无边框窗口**，自定义标题栏
- **明暗主题切换**，跟随系统或手动设置
- **应用图标**已嵌入（复用 byte-writer 图标，`apps/desktop/src-tauri/icons/`）
- **单文件分发**：前端内嵌，零外部依赖

## 构建

```bash
cd apps/desktop
npm --prefix frontend install   # 首次：安装前端依赖
cargo tauri build               # 生产构建
# 产物：src-tauri/target/release/bundle/
```
