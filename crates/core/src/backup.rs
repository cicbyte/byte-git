//! 完整仓库备份：`git clone --no-hardlinks` 到
//! `~/.cicbyte/apps/byte-git/backups/<repo-name>/<时间戳>-<repo-name>/`。
//!
//! 移植自 git-mate `internal/backup`（cef8f58）：普通克隆带工作区，
//! GUI 文件浏览器可直接浏览备份代码；目录名时间戳前缀、字典序即时间序。
//!
//! 兼容：早期镜像模型（`backups/<repo-name>/` 本身是 --mirror 裸仓库）
//! 在 list 时自动转换——从裸仓库克隆出标准条目后清理裸内部结构。

use crate::git;
use crate::util::{AppPaths, BACKUP_LEGACY_BARE_ENTRIES};
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// 结构化备份条目（对应 Go BackupEntry，GUI 用）。
#[derive(Debug, Clone, serde::Serialize)]
pub struct BackupEntry {
    /// 目录名 "20260624-150000-myrepo"
    pub name: String,
    /// 可读时间 "2026-06-24 15:00:00"
    pub timestamp: String,
    /// 备份目录绝对路径
    pub path: String,
    /// 递归总字节数（best-effort）
    pub size: i64,
}

/// 某仓库的备份根：AppDir/backups/<repoName>。
pub fn backups_repo_dir(repo: &Path) -> Result<PathBuf> {
    let name = git::repo_name(repo);
    Ok(AppPaths::new()?.backups_dir().join(name))
}

/// 目录名 "20260624-150000-myrepo" → "2026-06-24 15:00:00"；解析失败原样返回。
pub fn format_backup_name(name: &str) -> String {
    let parts: Vec<&str> = name.splitn(3, '-').collect();
    if parts.len() < 2 || parts[0].len() != 8 || parts[1].len() != 6 {
        return name.to_string();
    }
    let (d, t) = (parts[0], parts[1]);
    format!(
        "{}-{}-{} {}:{}:{}",
        &d[0..4], &d[4..6], &d[6..8], &t[0..2], &t[2..4], &t[4..6]
    )
}

/// 递归目录大小（best-effort，对应 Go dirSize）。
pub fn dir_size(path: &Path) -> i64 {
    fn walk(p: &Path, total: &mut i64) {
        if let Ok(rd) = std::fs::read_dir(p) {
            for e in rd.flatten() {
                let Ok(md) = e.metadata() else { continue };
                if md.is_dir() {
                    walk(&e.path(), total);
                } else {
                    *total += md.len() as i64;
                }
            }
        }
    }
    let mut t = 0i64;
    walk(path, &mut t);
    t
}

/// 克隆超时（大仓库慢网络可能很久，但不能永远挂着——对齐上游 30 分钟）。
const CLONE_TIMEOUT: Duration = Duration::from_secs(30 * 60);

/// 带超时与输出继承的 git 克隆（CLI 需要流式进度，对应 Go cmd.Stdout = os.Stdout）。
fn clone_with_timeout(src: &Path, dest: &Path) -> Result<()> {
    let mut child = std::process::Command::new("git")
        .arg("clone")
        .arg("--no-hardlinks")
        .arg(src)
        .arg(dest)
        .spawn()
        .with_context(|| format!("启动 git clone 失败: {} → {}", src.display(), dest.display()))?;
    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                if status.success() {
                    return Ok(());
                }
                anyhow::bail!("克隆失败: {status}");
            }
            Ok(None) => {
                if start.elapsed() > CLONE_TIMEOUT {
                    let _ = child.kill();
                    anyhow::bail!("克隆超时（超过 30 分钟）— 仓库过大或网络异常");
                }
                std::thread::sleep(Duration::from_millis(100));
            }
            Err(e) => anyhow::bail!("等待 git clone 失败: {e}"),
        }
    }
}

/// 旧模型检测：备份根本身是裸仓库（HEAD + objects + refs 并存）。
fn is_legacy_bare(dir: &Path) -> bool {
    dir.join("HEAD").is_file() && dir.join("objects").is_dir() && dir.join("refs").is_dir()
}

/// 旧镜像迁移：从裸仓库克隆出标准条目（保留全部历史），再清理裸内部结构。
/// 幂等、失败静默跳过（下次再试）。
pub fn migrate_legacy_mirror(repo_dir: &Path) {
    let Some(repo_name) = repo_dir.file_name().map(|n| n.to_string_lossy().into_owned()) else {
        return;
    };
    if !is_legacy_bare(repo_dir) {
        return;
    }
    // 以裸仓库 mtime 近似备份时间
    let ts = repo_dir
        .metadata()
        .and_then(|m| m.modified())
        .map(|t| {
            let dt: chrono::DateTime<chrono::Local> = t.into();
            dt.format("%Y%m%d-%H%M%S").to_string()
        })
        .unwrap_or_else(|_| chrono::Local::now().format("%Y%m%d-%H%M%S").to_string());
    let dest = repo_dir.join(format!("{ts}-{repo_name}"));
    eprintln!(
        "[byte-git] 检测到旧镜像备份，自动转换为标准条目: {}",
        dest.display()
    );
    if clone_with_timeout(repo_dir, &dest).is_err() {
        return; // 留待下次，不动原数据
    }
    for entry in BACKUP_LEGACY_BARE_ENTRIES {
        let p = repo_dir.join(entry);
        if p.is_dir() {
            let _ = std::fs::remove_dir_all(&p);
        } else {
            let _ = std::fs::remove_file(&p);
        }
    }
}

/// 创建备份（GUI/自动备份用，对应 Go CreateBackupFull）：时间戳命名 + 普通克隆。
pub fn create_full(repo: &str) -> Result<BackupEntry> {
    let root = git::repo_root(Path::new(repo))?;
    let repo_name = git::repo_name(&root);
    let backup_dir = backups_repo_dir(&root)?;
    migrate_legacy_mirror(&backup_dir);
    std::fs::create_dir_all(&backup_dir).context("创建备份目录失败")?;

    let ts = chrono::Local::now().format("%Y%m%d-%H%M%S");
    let backup_name = format!("{ts}-{repo_name}");
    let dest = backup_dir.join(&backup_name);

    clone_with_timeout(&root, &dest).context("创建备份失败")?;
    Ok(BackupEntry {
        timestamp: format_backup_name(&backup_name),
        path: dest.display().to_string(),
        size: dir_size(&dest),
        name: backup_name,
    })
}

/// 创建备份（CLI create）：带进度输出。
pub fn create(repo: &str) -> Result<()> {
    let root = git::repo_root(Path::new(repo))?;
    let repo_name = git::repo_name(&root);
    let backup_dir = backups_repo_dir(&root)?;
    migrate_legacy_mirror(&backup_dir);
    std::fs::create_dir_all(&backup_dir).context("创建备份目录失败")?;

    let ts = chrono::Local::now().format("%Y%m%d-%H%M%S");
    let backup_name = format!("{ts}-{repo_name}");
    let dest = backup_dir.join(&backup_name);

    println!("正在克隆 {} 到 {}", root.display(), dest.display());
    clone_with_timeout(&root, &dest).context("创建备份失败")?;
    println!("\n备份完成: {}", dest.display());
    Ok(())
}

/// 列出备份（CLI ls）：按仓库分组、新在前。
pub fn list(repo: &str) -> Result<()> {
    let root = git::repo_root(Path::new(repo))?;
    let repo_name = git::repo_name(&root);
    let backup_dir = backups_repo_dir(&root)?;
    migrate_legacy_mirror(&backup_dir);

    let Ok(entries) = std::fs::read_dir(&backup_dir) else {
        println!("没有备份");
        return Ok(());
    };
    let mut names: Vec<String> = entries
        .flatten()
        .filter(|e| e.path().is_dir())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    if names.is_empty() {
        println!("没有备份");
        return Ok(());
    }
    names.sort();
    names.reverse();
    println!("仓库 {repo_name} 的备份 ({} 个):\n", names.len());
    for n in &names {
        let ts = format_backup_name(n);
        if ts != *n {
            println!("  {n}  {ts}");
        } else {
            println!("  {n}");
        }
    }
    println!("\n备份目录: {}", backup_dir.display());
    Ok(())
}

/// 删除备份（CLI rm）。
pub fn remove(repo: &str, name: &str) -> Result<()> {
    let root = git::repo_root(Path::new(repo))?;
    let backup_dir = backups_repo_dir(&root)?;
    migrate_legacy_mirror(&backup_dir);
    let target = backup_dir.join(name);
    if !target.exists() {
        anyhow::bail!("备份不存在: {name}");
    }
    std::fs::remove_dir_all(&target).with_context(|| format!("删除失败: {name}"))?;
    println!("已删除备份: {name}");
    Ok(())
}

/// 结构化列表（GUI/嵌入复用，新在前；空时不报错）。
pub fn list_backups(repo: &str) -> Result<Vec<BackupEntry>> {
    let root = git::repo_root(Path::new(repo))?;
    let backup_dir = backups_repo_dir(&root)?;
    migrate_legacy_mirror(&backup_dir);
    let mut result = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&backup_dir) {
        for e in entries.flatten() {
            if !e.path().is_dir() {
                continue;
            }
            let name = e.file_name().to_string_lossy().into_owned();
            let full = backup_dir.join(&name);
            result.push(BackupEntry {
                name: name.clone(),
                timestamp: format_backup_name(&name),
                path: full.display().to_string(),
                size: dir_size(&full),
            });
        }
    }
    result.sort_by(|a, b| b.name.cmp(&a.name));
    Ok(result)
}
