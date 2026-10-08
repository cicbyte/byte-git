//! 仓库快照归档命令。

use byte_git_core::archive as core_archive;
use clap::{Args, Subcommand};
use std::path::Path;

#[derive(Args, Debug)]
pub struct ArchiveCmd {
    #[command(subcommand)]
    pub cmd: ArchiveSub,
}

#[derive(Subcommand, Debug)]
pub enum ArchiveSub {
    /// 创建归档（默认 HEAD / tar.gz）
    Create {
        /// 归档的 ref（分支/tag/commit）
        #[arg(long, default_value = "HEAD")]
        r#ref: String,
        /// 格式：tar.gz / zip / tar
        #[arg(long, default_value = "tar.gz")]
        format: String,
        /// 输出文件（默认 downloads/archive/<repo>/<时间戳>-<ref>.<ext>）
        #[arg(short = 'o', long)]
        output: Option<String>,
        /// 路径过滤（仅归档指定路径）
        #[arg(value_name = "PATH")]
        paths: Vec<String>,
    },
    /// 列出已有归档
    List,
}

pub fn run(repo: &Path, c: ArchiveCmd) -> anyhow::Result<()> {
    match c.cmd {
        ArchiveSub::Create { r#ref, format, output, paths } => {
            let result = core_archive::archive(&core_archive::Options {
                repo: repo.display().to_string(),
                ref_: r#ref,
                paths,
                format,
                output: output.unwrap_or_default(),
            })?;
            println!("归档完成: {} ({})", result.output_path, byte_git_core::download::format_size(result.size as i64));
            Ok(())
        }
        ArchiveSub::List => {
            let items = core_archive::list(repo)?;
            if items.is_empty() {
                println!("暂无归档（archive create 创建）");
                return Ok(());
            }
            println!("归档列表（{}）：", items.len());
            for i in items {
                println!("  {i}");
            }
            Ok(())
        }
    }
}
