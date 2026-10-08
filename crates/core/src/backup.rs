//! 完整仓库备份：`git clone --mirror` 到 `~/.cicbyte/byte-git/backups/<repo-name>/`。
//!
//! 移植自 git-mate `internal/backup`。备份不占用项目空间，可整目录拷回恢复。

use crate::git;
use crate::util::{exec_capture, AppPaths};
use anyhow::{bail, Context, Result};
use std::path::{Path, PathBuf};

/// 创建（或更新）备份。返回备份目录。
pub fn create(repo: &str, update: bool) -> Result<PathBuf> {
    let root = git::repo_root(Path::new(repo))?;
    let name = git::repo_name(Path::new(repo));
    let paths = AppPaths::new()?;
    let dest = paths.backups_dir().join(&name);

    if dest.exists() {
        if !update {
            bail!("备份已存在: {}（--update 可增量更新）", dest.display());
        }
        // 更新已有镜像备份
        exec_capture("git", &["-C", &dest.display().to_string(), "remote", "update", "--prune"])
            .context("更新备份失败")?;
        println!("已更新备份: {}", dest.display());
        return Ok(dest);
    }

    AppPaths::ensure_dir(&paths.backups_dir())?;
    exec_capture(
        "git",
        &["clone", "--mirror", &root.display().to_string(), &dest.display().to_string()],
    )
    .context("创建镜像备份失败")?;

    // 记录来源路径，供恢复使用
    std::fs::write(dest.join("byte-git-source-path"), root.display().to_string())
        .context("写入来源记录失败")?;
    println!("已创建备份: {}", dest.display());
    Ok(dest)
}

/// 列出所有备份。
pub fn list() -> Result<Vec<String>> {
    let paths = AppPaths::new()?;
    let dir = paths.backups_dir();
    let mut names = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for e in entries.flatten() {
            if e.path().is_dir() && e.path().join("config").exists() {
                names.push(e.file_name().to_string_lossy().into_owned());
            }
        }
    }
    names.sort();
    if names.is_empty() {
        println!("暂无备份（backups 目录: {}）", dir.display());
    } else {
        println!("备份 ({}):", names.len());
        for n in &names {
            println!("  {n}");
        }
    }
    Ok(names)
}

/// 删除备份。
pub fn remove(name: &str) -> Result<()> {
    let paths = AppPaths::new()?;
    let dest = paths.backups_dir().join(name);
    if !dest.exists() {
        bail!("备份不存在: {}", dest.display());
    }
    std::fs::remove_dir_all(&dest).with_context(|| format!("删除备份失败: {}", dest.display()))?;
    println!("已删除备份: {name}");
    Ok(())
}
