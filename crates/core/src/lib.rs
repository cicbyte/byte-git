//! byte-git 领域层：桌面端（Tauri）与 CLI 共用的纯 Rust 实现。
//!
//! 不依赖任何 GUI 框架。应用根目录默认 `~/.cicbyte/byte-git`
//! （`BYTE_GIT_HOME` 环境变量可覆盖），领域模块围绕 git 仓库操作组织：
//! 配置、分支清理、stash、统计、备份、许可证与 gitignore 模板等。
//!
//! 功能规格移植自 git-mate（Go），行为语义保持一致：
//! - 全局配置：`~/.cicbyte/byte-git/config/config.yaml`
//! - 完整备份：`~/.cicbyte/byte-git/backups/<repo-name>/`
//! - 项目级数据：`.git/byte-git/`

pub mod backup;
pub mod branch;
pub mod config;
pub mod git;
pub mod ignore;
pub mod info;
pub mod license;
pub mod stash;
pub mod stats;
pub mod util;
