//! backup：完整仓库备份管理。

use byte_git_core::backup;
use clap::{Args, Subcommand};

#[derive(Args, Debug)]
pub struct BackupCmd {
    #[command(subcommand)]
    pub cmd: BackupSub,
}

#[derive(Subcommand, Debug)]
pub enum BackupSub {
    /// 创建镜像备份（--update 更新已有备份）
    Create {
        /// 仓库路径（默认当前目录）
        repo: Option<String>,
        /// 更新已有备份而非报错
        #[arg(long)]
        update: bool,
    },
    /// 列出全部备份
    List,
    /// 删除指定备份
    Remove { name: String },
}

pub fn run(cmd: BackupCmd) -> anyhow::Result<()> {
    match cmd.cmd {
        BackupSub::Create { repo, update } => {
            backup::create(repo.as_deref().unwrap_or("."), update)?;
        }
        BackupSub::List => {
            backup::list()?;
        }
        BackupSub::Remove { name } => {
            backup::remove(&name)?;
        }
    }
    Ok(())
}
