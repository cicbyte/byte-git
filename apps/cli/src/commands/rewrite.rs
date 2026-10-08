//! 提交元数据改写命令（export → 编辑 JSON → apply）。

use byte_git_core::rewrite as core_rewrite;
use clap::{Args, Subcommand};
use std::io::Write;
use std::path::Path;

#[derive(Args, Debug)]
pub struct RewriteCmd {
    #[command(subcommand)]
    pub cmd: RewriteSub,
}

#[derive(Subcommand, Debug)]
pub enum RewriteSub {
    /// 导出提交元数据到 JSON（rewrite 段可编辑）
    Export {
        /// 最近 N 条
        #[arg(long)]
        last: Option<usize>,
        /// 提交区间（A..B）
        #[arg(long)]
        range: Option<String>,
        /// 仅未推送提交
        #[arg(long)]
        unpushed: bool,
        /// 改写规则，可多次：author:旧=新 / email:=新 / date-shift:+2h
        #[arg(long = "rule")]
        rules: Vec<String>,
        /// 日期范围重分配：2024-01-01,2024-06-30
        #[arg(long)]
        date_range: Option<String>,
        /// spread（均匀）| random（随机）
        #[arg(long)]
        date_mode: Option<String>,
        /// 每天最大提交数（0 不限）
        #[arg(long)]
        daily_limit: Option<i64>,
        /// 活跃时段，如 9-22
        #[arg(long)]
        active_hours: Option<String>,
        /// 输出文件（默认 .git/byte-git/rewrite/rewrite.json）
        #[arg(short = 'o', long)]
        output: Option<String>,
    },
    /// 应用 JSON 中的改写（filter-branch）
    Apply {
        /// JSON 文件
        #[arg(short = 'f', long)]
        file: Option<String>,
        /// 目标分支（默认当前分支）
        #[arg(long)]
        branch: Option<String>,
        /// 仅预览级联影响
        #[arg(long)]
        dry_run: bool,
        /// 同步远程追踪 ref
        #[arg(long)]
        sync_refs: bool,
        /// 跳过确认（危险）
        #[arg(long)]
        yes: bool,
    },
}

pub fn run(repo: &Path, c: RewriteCmd) -> anyhow::Result<()> {
    match c.cmd {
        RewriteSub::Export { last, range, unpushed, rules, date_range, date_mode, daily_limit, active_hours, output } => {
            core_rewrite::gen(&core_rewrite::GenOptions {
                repo: repo.display().to_string(),
                range: range.unwrap_or_default(),
                last: last.unwrap_or(0),
                unpushed,
                output: output.unwrap_or_default(),
                rules,
                date_range: date_range.unwrap_or_default(),
                date_mode: date_mode.unwrap_or_default(),
                daily_limit: daily_limit.unwrap_or(0),
                active_hour: active_hours.unwrap_or_default(),
            })
        }
        RewriteSub::Apply { file, branch, dry_run, sync_refs, yes } => {
            let file = file.map(Ok).unwrap_or_else(|| core_rewrite::default_rewrite_file(repo).map(|p| p.display().to_string()))?;
            if !dry_run && !yes {
                println!("⚠ filter-branch 将改写提交历史（建议先 byte-git backup create 备份）");
                print!("确认继续? [y/N]: ");
                std::io::stdout().flush()?;
                let mut input = String::new();
                std::io::stdin().read_line(&mut input)?;
                if !input.trim().eq_ignore_ascii_case("y") {
                    println!("已取消");
                    return Ok(());
                }
            }
            core_rewrite::apply(&core_rewrite::ApplyOptions {
                repo: repo.display().to_string(),
                branch: branch.unwrap_or_default(),
                file,
                dry_run,
                sync_refs,
            })
        }
    }
}
