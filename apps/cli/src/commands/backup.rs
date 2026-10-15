//! backup：完整仓库备份管理（对齐上游：普通克隆 + 时间戳条目）。

use byte_git_core::backup;
use clap::{Args, Subcommand};

#[derive(Args, Debug)]
pub struct BackupCmd {
    #[command(subcommand)]
    pub cmd: BackupSub,
}

#[derive(Subcommand, Debug)]
pub enum BackupSub {
    /// 创建完整备份
    Create {
        /// 目标仓库路径
        #[arg(long, default_value = ".")]
        repo: String,
    },
    /// 列出所有备份
    Ls {
        /// 目标仓库路径
        #[arg(long, default_value = ".")]
        repo: String,
    },
    /// 删除指定备份
    Rm {
        /// 备份名
        name: String,
        /// 目标仓库路径
        #[arg(long, default_value = ".")]
        repo: String,
    },
    /// 列出所有备份（ls 别名）
    List {
        /// 目标仓库路径
        #[arg(long, default_value = ".")]
        repo: String,
    },
}

pub fn run(cmd: BackupCmd) -> anyhow::Result<()> {
    match cmd.cmd {
        BackupSub::Create { repo } => backup::create(&repo)?,
        BackupSub::Ls { repo } | BackupSub::List { repo } => backup::list(&repo)?,
        BackupSub::Rm { name, repo } => backup::remove(&repo, &name)?,
    }
    Ok(())
}
