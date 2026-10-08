//! stats：多仓库开发统计。

use byte_git_core::stats::{self, StatsOptions};
use clap::Args;

#[derive(Args, Debug)]
pub struct StatsCmd {
    /// 扫描目录（默认当前目录；若是仓库本身则直接统计）
    #[arg(long)]
    dir: Option<String>,
    /// 统计最近 N 天
    #[arg(long)]
    days: Option<i64>,
    /// 按作者过滤（默认当前 git 用户）
    #[arg(long)]
    author: Option<String>,
}

pub fn run(cmd: StatsCmd) -> anyhow::Result<()> {
    stats::stats(&StatsOptions {
        dir: cmd.dir.unwrap_or_else(|| ".".into()),
        days: cmd.days.unwrap_or(0),
        author: cmd.author.unwrap_or_default(),
    })
}
