//! GitHub Release 资产下载：列表检索 + 平台自动匹配 + 进度条下载。
//!
//! CLI 行为：--latest 自动匹配 / --tag 指定版本 / 默认列出并交互选择。

use crate::config::Settings;
use crate::util::AppPaths;
use anyhow::{bail, Context, Result};
use serde::Deserialize;
use std::io::Read;
use std::path::Path;
use std::time::Duration;

#[derive(Debug, Clone, Deserialize, serde::Serialize)]
pub struct Asset {
    pub name: String,
    pub browser_download_url: String,
    pub size: i64,
}

#[derive(Debug, Clone, Deserialize, serde::Serialize)]
pub struct Release {
    pub tag_name: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub prerelease: bool,
    #[serde(default)]
    pub assets: Vec<Asset>,
}

/// 解析 owner/repo 或完整 GitHub URL。
pub fn parse_repo_input(input: &str) -> Result<(String, String)> {
    let input = input.trim().trim_end_matches('/');
    for prefix in ["https://github.com/", "http://github.com/"] {
        if let Some(after) = input.strip_prefix(prefix) {
            if let Some((o, r)) = after.split_once('/') {
                return Ok((o.to_string(), r.to_string()));
            }
        }
    }
    if let Some((o, r)) = input.split_once('/') {
        if !o.is_empty() && !r.is_empty() {
            return Ok((o.to_string(), r.to_string()));
        }
    }
    bail!("无法解析仓库 '{input}'，请使用 owner/repo 格式")
}

fn api_client() -> Result<reqwest::blocking::Client> {
    reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(15))
        .user_agent("byte-git")
        .build()
        .context("构建 HTTP 客户端失败")
}

/// 获取仓库 release 列表（最多 20 条，自动附加 token）。
pub fn fetch_releases(owner: &str, repo: &str) -> Result<Vec<Release>> {
    let url = format!("https://api.github.com/repos/{owner}/{repo}/releases?per_page=20");
    let mut req = api_client()?
        .get(&url)
        .header("Accept", "application/vnd.github+json");
    if let Ok(s) = Settings::load() {
        if !s.github.token.is_empty() {
            req = req.header("Authorization", format!("Bearer {}", s.github.token));
        }
    }
    let resp = req.send().context("请求 GitHub API 失败")?;
    match resp.status().as_u16() {
        404 => bail!("仓库 {owner}/{repo} 不存在或没有 Release"),
        403 => bail!("GitHub API 速率限制，请稍后重试或设置 github.token"),
        200 => {}
        code => {
            let body = resp.text().unwrap_or_default();
            bail!("GitHub API 错误 ({code}): {body}");
        }
    }
    let releases: Vec<Release> = resp.json().context("解析响应失败")?;
    if releases.is_empty() {
        bail!("仓库 {owner}/{repo} 没有 Release");
    }
    Ok(releases)
}

/// 默认下载目录：~/.cicbyte/apps/byte-git/downloads/<owner>/<repo>/<tag>/。
pub fn default_download_dir(owner: &str, repo: &str, tag: &str) -> Result<std::path::PathBuf> {
    Ok(AppPaths::new()?.downloads_dir().join(owner).join(repo).join(tag))
}

/// 下载文件（带进度条回调）。
pub fn download_file<F>(url: &str, dest: &Path, expected_size: i64, mut on_progress: F) -> Result<()>
where
    F: FnMut(i64, i64),
{
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(600))
        .user_agent("byte-git")
        .build()
        .context("构建下载客户端失败")?;
    let mut resp = client.get(url).send().context("下载请求失败")?;
    if !resp.status().is_success() {
        bail!("下载失败 ({})", resp.status());
    }
    let total = expected_size.max(0);
    if let Some(parent) = dest.parent() {
        AppPaths::ensure_dir(parent)?;
    }
    let mut file = std::fs::File::create(dest)
        .with_context(|| format!("创建文件失败: {}", dest.display()))?;
    let mut buf = [0u8; 64 * 1024];
    let mut downloaded: i64 = 0;
    let mut last_reported: i64 = -100;
    loop {
        let n = resp.read(&mut buf).context("读取下载流失败")?;
        if n == 0 {
            break;
        }
        std::io::Write::write_all(&mut file, &buf[..n])?;
        downloaded += n as i64;
        // 每 1% 或每 64KB 上报
        if total > 0 {
            let pct = downloaded * 100 / total;
            if pct != last_reported {
                on_progress(downloaded, total);
                last_reported = pct;
            }
        } else if downloaded - (last_reported.max(0) * 0) > 0 && downloaded % (1024 * 1024) < (64 * 1024) as i64 {
            on_progress(downloaded, 0);
        }
    }
    on_progress(downloaded, if total > 0 { total } else { downloaded });
    Ok(())
}

/// 平台匹配（文件名按 -_. 分词精确匹配；复合关键词如 x86_64 会被分词拆开，
/// 因此对含分隔符的关键词额外做子串匹配）。
pub fn auto_select_assets(assets: &[Asset]) -> Vec<Asset> {
    let os_kws: &[&str] = if cfg!(windows) {
        &["windows", "win"]
    } else if cfg!(target_os = "macos") {
        &["darwin", "macos", "mac", "osx"]
    } else {
        &["linux"]
    };
    let arch_kws: &[&str] = if cfg!(target_arch = "aarch64") {
        &["arm64", "aarch64"]
    } else {
        &["amd64", "x86_64", "x64"]
    };
    assets
        .iter()
        .filter(|a| name_match(&a.name, os_kws) && name_match(&a.name, arch_kws))
        .cloned()
        .collect()
}

fn name_match(name: &str, keywords: &[&str]) -> bool {
    let lower = name.to_lowercase();
    keywords.iter().any(|k| {
        if k.contains(['-', '_', '.']) {
            // 复合词（x86_64 等）：分词会拆散，用子串匹配
            lower.contains(k)
        } else {
            // 简单词：分词精确匹配（避免 "win" 误中 "darwin" 这类子串误报）
            lower
                .split(|c| c == '-' || c == '_' || c == '.')
                .any(|t| t == *k)
        }
    })
}

pub fn format_size(b: i64) -> String {
    const MB: i64 = 1024 * 1024;
    const KB: i64 = 1024;
    match b {
        x if x >= MB => format!("{:.1} MB", x as f64 / MB as f64),
        x if x >= KB => format!("{:.1} KB", x as f64 / KB as f64),
        x => format!("{x} B"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn asset(name: &str) -> Asset {
        Asset { name: name.into(), browser_download_url: String::new(), size: 0 }
    }

    #[test]
    fn parse_repo_variants() {
        assert_eq!(parse_repo_input("a/b").unwrap(), ("a".into(), "b".into()));
        assert_eq!(
            parse_repo_input("https://github.com/a/b/").unwrap(),
            ("a".into(), "b".into())
        );
        assert!(parse_repo_input("nonsense").is_err());
    }

    #[test]
    fn platform_select_on_windows_x64() {
        if !cfg!(windows) || cfg!(target_arch = "aarch64") {
            return; // 平台相关断言仅在本机构上验证
        }
        let assets = vec![
            asset("bcode-aarch64-apple-darwin.tar.gz"),
            asset("bcode-x86_64-apple-darwin.tar.gz"),
            asset("bcode-x86_64-pc-windows-msvc.zip"),
            asset("bcode-x86_64-unknown-linux-gnu.tar.gz"),
        ];
        let picked: Vec<String> = auto_select_assets(&assets)
            .iter()
            .map(|a| a.name.clone())
            .collect();
        assert_eq!(picked, vec!["bcode-x86_64-pc-windows-msvc.zip".to_string()]);
    }
}

/// 下载一组资产到目录（带控制台进度条）。
pub fn download_files(assets: &[Asset], dest_dir: &Path) -> Result<()> {
    for (i, a) in assets.iter().enumerate() {
        let dest = dest_dir.join(&a.name);
        println!("\n[{}/{}] {} ({})", i + 1, assets.len(), a.name, format_size(a.size));
        let mut last_pct = -1i64;
        download_file(&a.browser_download_url, &dest, a.size, |done, total| {
            if total <= 0 {
                return;
            }
            let pct = done * 100 / total;
            if pct != last_pct {
                last_pct = pct;
                let width = 30usize;
                let pos = (width as i64 * pct / 100) as usize;
                let bar = "█".repeat(pos) + &"░".repeat(width - pos);
                print!("\r  {bar} {pct}% {}/{}", format_size(done), format_size(total));
                use std::io::Write;
                let _ = std::io::stdout().flush();
            }
        })?;
        println!("\r  {} 100% {}                    ", "█".repeat(30), format_size(a.size));
    }
    println!("\n下载完成: {}", dest_dir.display());
    Ok(())
}
