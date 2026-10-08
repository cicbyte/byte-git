//! stash：带名称的 stash 管理。

use byte_git_core::stash;
use clap::{Args, Subcommand};
use std::path::Path;

#[derive(Args, Debug)]
pub struct StashCmd {
    #[command(subcommand)]
    pub cmd: StashSub,
}

#[derive(Subcommand, Debug)]
pub enum StashSub {
    /// 按名称保存 stash
    Save {
        /// stash 名称
        #[arg(long)]
        name: String,
    },
    /// 按名称弹出 stash
    Pop {
        /// stash 名称
        #[arg(long)]
        name: String,
    },
    /// 列出 stash（--search 过滤）
    List {
        /// 按关键词过滤（匹配消息或名称）
        #[arg(long)]
        search: Option<String>,
    },
    /// 查看 stash 内容
    Show {
        /// stash 引用（默认 stash@{0}）
        reference: Option<String>,
        /// 显示完整 diff
        #[arg(long)]
        patch: bool,
    },
    /// 删除 stash@{index}
    Drop { index: usize },
}

pub fn run(repo: &Path, cmd: StashCmd) -> anyhow::Result<()> {
    match cmd.cmd {
        StashSub::Save { name } => stash::save(repo, &name)?,
        StashSub::Pop { name } => stash::pop(repo, &name)?,
        StashSub::List { search } => {
            stash::list(repo, search.as_deref().unwrap_or(""))?;
        }
        StashSub::Show { reference, patch } => {
            stash::show(repo, reference.as_deref().unwrap_or(""), patch)?;
        }
        StashSub::Drop { index } => stash::drop(repo, index)?,
    }
    Ok(())
}
