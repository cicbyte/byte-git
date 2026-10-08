//! GitHub Release 资产下载命令。

use byte_git_core::download as core_dl;
use clap::{Args, Subcommand};
use std::io::Write;

#[derive(Args, Debug)]
pub struct DownloadCmd {
    #[command(subcommand)]
    pub cmd: Option<DownloadSub>,
}

#[derive(Subcommand, Debug)]
pub enum DownloadSub {
    /// 下载仓库 Release（owner/repo 或完整 URL）
    Get {
        /// owner/repo 或 https://github.com/owner/repo
        repo: String,

        /// 直接下载最新版本（自动匹配平台）
        #[arg(short = 'l', long)]
        latest: bool,

        /// 指定 release tag（默认最新）
        #[arg(short = 't', long)]
        tag: Option<String>,

        /// 下载目录
        #[arg(short = 'd', long)]
        dest: Option<String>,

        /// 资产名过滤（包含匹配，可多次）
        #[arg(short = 'a', long)]
        asset: Vec<String>,
    },
    /// 直接下载指定 URL 文件
    Url {
        /// 下载 URL
        #[arg(short = 'u', long)]
        url: String,

        /// 保存路径（默认当前目录，文件名取 URL 末段）
        #[arg(short = 's', long)]
        save_path: Option<String>,
    },
}

pub fn run(c: DownloadCmd) -> anyhow::Result<()> {
    match c.cmd {
        Some(DownloadSub::Get { repo, latest, tag, dest, asset }) => {
            get(repo, latest, tag, dest, asset)
        }
        Some(DownloadSub::Url { url, save_path }) => {
            let save = save_path.unwrap_or_else(|| {
                url.rsplit('/').next().unwrap_or("download.bin").to_string()
            });
            println!("下载到: {save}");
            core_dl::download_file(&url, std::path::Path::new(&save), 0, |done, total| {
                if total > 0 && done % (1024 * 512) < 65536 {
                    print!("\r  {}/{}", core_dl::format_size(done), core_dl::format_size(total));
                    let _ = std::io::stdout().flush();
                }
            })?;
            println!("\n下载完成");
            Ok(())
        }
        None => {
            anyhow::bail!("用法: byte-git download get <owner/repo> 或 download url -u <URL>（-h 查看详情）")
        }
    }
}

fn get(repo: String, latest: bool, tag: Option<String>, dest: Option<String>, asset_filters: Vec<String>) -> anyhow::Result<()> {
    let (owner, name) = core_dl::parse_repo_input(&repo)?;
    let releases = core_dl::fetch_releases(&owner, &name)?;

    // 选择 release
    let release = if let Some(ref t) = tag {
        releases
            .iter()
            .find(|r| r.tag_name == *t)
            .ok_or_else(|| anyhow::anyhow!("未找到 release: {t}"))?
            .clone()
    } else {
        releases[0].clone()
    };

    println!("版本: {} ({})", release.tag_name, release.name);

    // 选择资产
    let mut assets = release.assets.clone();
    if !asset_filters.is_empty() {
        assets.retain(|a| asset_filters.iter().any(|f| a.name.contains(f.as_str())));
        if assets.is_empty() {
            anyhow::bail!("没有匹配过滤条件的资产（--asset {filters}）", filters = asset_filters.join(","));
        }
    } else if latest && tag.is_none() {
        // --latest：自动平台匹配，无匹配则全量
        let matched = core_dl::auto_select_assets(&assets);
        if !matched.is_empty() {
            assets = matched;
        }
    }

    println!("资产:");
    for a in &assets {
        println!("  {} ({})", a.name, core_dl::format_size(a.size));
    }

    let dest_dir = match dest {
        Some(d) => std::path::PathBuf::from(d),
        None => core_dl::default_download_dir(&owner, &name, &release.tag_name)?,
    };
    std::fs::create_dir_all(&dest_dir)?;

    core_dl::download_files(&assets, &dest_dir)
}
