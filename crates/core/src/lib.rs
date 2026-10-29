//! byte-git 领域层：桌面端（Tauri）与 CLI 共用的纯 Rust 实现。
//!
//! 不依赖任何 GUI 框架。应用根目录默认 `~/.cicbyte/apps/byte-git`
//! （`BYTE_GIT_HOME` 环境变量可覆盖），领域模块围绕 git 仓库操作组织：
//! 配置、分支清理、stash、统计、备份、许可证与 gitignore 模板等。
//!
//! 行为语义约定：
//! - 全局配置：`~/.cicbyte/apps/byte-git/config/config.yaml`
//! - 完整备份：`~/.cicbyte/apps/byte-git/backups/<repo-name>/`
//! - 项目级数据：`.git/byte-git/`

pub mod ai;
pub mod aicommit;
pub mod airewrite;
pub mod archive;
pub mod backup;
pub mod branch;
pub mod clone;
pub mod config;
pub mod download;
pub mod git;
pub mod gitleaks;
pub mod hook;
pub mod ignore;
pub mod info;
pub mod license;
pub mod logview;
pub mod ocr;
pub mod rebase;
pub mod rewrite;
pub mod stash;
pub mod stats;
pub mod undo;
pub mod util;
