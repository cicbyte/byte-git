//! info：仓库概览与健康检查。

use byte_git_core::info;
use clap::{Args, Subcommand};
use std::path::Path;

#[derive(Args, Debug)]
pub struct InfoCmd {
    #[command(subcommand)]
    pub cmd: InfoSub,
}

#[derive(Subcommand, Debug)]
pub enum InfoSub {
    /// 仓库概览（分支/远程/提交数/状态）
    Summary,
    /// 健康检查（大文件/未推送/缺 gitignore）
    Health,
}

pub fn run(repo: &Path, cmd: InfoCmd) -> anyhow::Result<()> {
    match cmd.cmd {
        InfoSub::Summary => info::summary(repo)?,
        InfoSub::Health => info::health(repo)?,
    }
    Ok(())
}
