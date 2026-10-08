//! branch：已合并分支清理、孤立分支发现。

use byte_git_core::branch::{self, CleanOptions};
use clap::{Args, Subcommand};
use std::path::Path;

#[derive(Args, Debug)]
pub struct BranchCmd {
    #[command(subcommand)]
    pub cmd: BranchSub,
}

#[derive(Subcommand, Debug)]
pub enum BranchSub {
    /// 清理已合并分支
    Clean {
        /// 相对哪个引用判断合并（默认 HEAD）
        #[arg(long)]
        merged_into: Option<String>,
        /// 仅预览，不实际删除
        #[arg(long)]
        dry_run: bool,
        /// 额外保护分支
        #[arg(long)]
        exclude: Vec<String>,
        /// 不保护 main/master/develop/dev
        #[arg(long)]
        no_protected: bool,
    },
    /// 列出无上游的孤立分支
    Orphans,
}

pub fn run(repo: &Path, cmd: BranchCmd) -> anyhow::Result<()> {
    match cmd.cmd {
        BranchSub::Clean {
            merged_into,
            dry_run,
            exclude,
            no_protected,
        } => {
            branch::clean(
                repo,
                &CleanOptions {
                    merged_into: merged_into.unwrap_or_default(),
                    dry_run,
                    exclude,
                    no_protected,
                },
            )?;
        }
        BranchSub::Orphans => {
            branch::list_orphans(repo)?;
        }
    }
    Ok(())
}
