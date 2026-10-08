//! 安全撤销命令：上次提交 / rebase / merge。

use byte_git_core::undo as core_undo;
use clap::{Args, Subcommand};
use std::path::Path;

#[derive(Args, Debug)]
pub struct UndoCmd {
    #[command(subcommand)]
    pub cmd: UndoSub,
}

#[derive(Subcommand, Debug)]
pub enum UndoSub {
    /// 撤销上一次提交（默认丢弃改动）
    Commit {
        /// 保留改动在工作区（soft reset）
        #[arg(long)]
        keep_changes: bool,
    },
    /// 撤销进行中的 rebase
    Rebase {
        /// abort 前备份 HEAD 到 refs/byte-git-undo/rebase-backup
        #[arg(long)]
        backup: bool,
        /// 仅执行 git rebase --abort
        #[arg(long)]
        abort: bool,
    },
    /// 撤销进行中的 merge
    Merge {
        /// 仅执行 git merge --abort
        #[arg(long)]
        abort: bool,
    },
}

pub fn run(repo: &Path, c: UndoCmd) -> anyhow::Result<()> {
    match c.cmd {
        UndoSub::Commit { keep_changes } => {
            core_undo::last_commit(repo, &core_undo::LastCommitOptions { keep_changes })
        }
        UndoSub::Rebase { backup, abort } => {
            core_undo::rebase(repo, &core_undo::RebaseOptions { backup, abort })
        }
        UndoSub::Merge { abort } => {
            core_undo::merge(repo, &core_undo::MergeOptions { abort })
        }
    }
}
