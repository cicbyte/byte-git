//! Rebase 预览与 auto-squash 命令。

use byte_git_core::rebase as core_rebase;
use clap::{Args, Subcommand};
use std::path::Path;

#[derive(Args, Debug)]
pub struct RebaseCmd {
    #[command(subcommand)]
    pub cmd: RebaseSub,
}

#[derive(Subcommand, Debug)]
pub enum RebaseSub {
    /// 预览将被 rebase 的提交清单
    Preview {
        /// 目标分支（默认上游）
        #[arg(long)]
        branch: Option<String>,
        /// 最近 N 条（HEAD~N）
        #[arg(long)]
        count: Option<usize>,
    },
    /// fixup!/squash! 自动整理并执行 autosquash rebase
    Squash {
        /// 目标分支（默认上游）
        #[arg(long)]
        branch: Option<String>,
        /// 最近 N 条（HEAD~N）
        #[arg(long)]
        count: Option<usize>,
        /// 仅预览重排，不执行
        #[arg(long)]
        dry_run: bool,
    },
}

pub fn run(repo: &Path, c: RebaseCmd) -> anyhow::Result<()> {
    match c.cmd {
        RebaseSub::Preview { branch, count } => {
            core_rebase::dry_run(repo, &core_rebase::TargetOptions {
                branch: branch.unwrap_or_default(),
                count: count.unwrap_or(0),
            })
        }
        RebaseSub::Squash { branch, count, dry_run } => {
            core_rebase::auto_squash(repo, &core_rebase::AutoSquashOptions {
                branch: branch.unwrap_or_default(),
                count: count.unwrap_or(0),
                dry_run,
            })
        }
    }
}
