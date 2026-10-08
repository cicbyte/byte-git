//! license：开源许可证创建与检测。

use byte_git_core::license;
use clap::{Args, Subcommand};
use std::path::Path;

#[derive(Args, Debug)]
pub struct LicenseCmd {
    #[command(subcommand)]
    pub cmd: LicenseSub,
}

#[derive(Subcommand, Debug)]
pub enum LicenseSub {
    /// 列出可用许可证模板
    List,
    /// 创建 LICENSE 文件
    Create {
        /// 许可证名（license list 查看）
        name: String,
        /// 版权作者
        #[arg(long)]
        author: Option<String>,
        /// 版权年份（默认当前年）
        #[arg(long)]
        year: Option<i32>,
    },
    /// 检测现有 LICENSE 类型
    Detect,
}

pub fn run(repo: &Path, cmd: LicenseCmd) -> anyhow::Result<()> {
    match cmd.cmd {
        LicenseSub::List => {
            for n in license::list() {
                println!("  {n}");
            }
        }
        LicenseSub::Create { name, author, year } => {
            license::create(repo, &name, author.as_deref().unwrap_or(""), year)?;
        }
        LicenseSub::Detect => {
            license::detect(repo)?;
        }
    }
    Ok(())
}
