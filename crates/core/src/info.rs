//! 仓库洞察：summary（概览）/ health（健康检查）。
//!
//! 对应 git-mate `internal/info` 的 CLI 子集。

use crate::git;
use crate::util::{exec_capture, run_capture};
use anyhow::Result;
use std::path::Path;

/// 仓库概览：分支/远程/提交数/工作区状态/最近提交。
pub fn summary(repo: &Path) -> Result<()> {
    let branch = git::current_branch(repo)?;
    let remotes = git::remotes(repo)?;
    let commits = git::commit_count(repo)?;
    let status = git::status(repo)?;
    let root = git::repo_root(repo)?;
    let name = root
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();

    let last = run_capture(repo, ["log", "-1", "--format=%h %ad %s", "--date=short"]).ok();

    println!("仓库概览");
    println!("  路径:     {}", root.display());
    println!("  名称:     {name}");
    println!("  分支:     {branch}");
    println!("  提交数:   {commits}");
    println!("  状态:     {} (暂存 {} / 未暂存 {} / 未跟踪 {})",
        if status.dirty { "有变更" } else { "干净" },
        status.staged, status.unstaged, status.untracked);
    if remotes.is_empty() {
        println!("  远程:     (无)");
    } else {
        println!("  远程:");
        for (n, u) in &remotes {
            println!("    {n:<10} {u}");
        }
    }
    if let Some(l) = last {
        println!("  最近提交: {}", l.trim());
    }
    Ok(())
}

/// 健康检查：git 可用性/版本、大文件、待推送提交、未合并分支等。
pub fn health(repo: &Path) -> Result<()> {
    let mut issues = 0usize;

    // git 版本
    let ver = exec_capture("git", &["--version"]).unwrap_or_else(|_| "未知".into());
    println!("git 版本: {}", ver.trim());

    // 工作区
    let status = git::status(repo)?;
    if status.dirty {
        issues += 1;
        println!("⚠ 工作区有未提交变更 (暂存 {} / 未暂存 {} / 未跟踪 {})",
            status.staged, status.unstaged, status.untracked);
    } else {
        println!("✓ 工作区干净");
    }

    // 大文件（>5MB，Top 5）
    let ls = run_capture(repo, ["ls-files"]).unwrap_or_default();
    let mut large: Vec<(String, u64)> = Vec::new();
    for f in ls.lines() {
        if let Ok(m) = std::fs::metadata(repo.join(f)) {
            if m.len() > 5 * 1024 * 1024 {
                large.push((f.to_string(), m.len()));
            }
        }
    }
    large.sort_by(|a, b| b.1.cmp(&a.1));
    if !large.is_empty() {
        issues += 1;
        println!("⚠ 大于 5MB 的文件 ({}):", large.len());
        for (f, s) in large.iter().take(5) {
            println!("    {f} ({:.1} MB)", *s as f64 / 1024.0 / 1024.0);
        }
    } else {
        println!("✓ 无大于 5MB 的跟踪文件");
    }

    // 待推送（有上游时）
    let ahead = run_capture(repo, ["rev-list", "--count", "@{upstream}..HEAD"]).ok();
    if let Some(a) = ahead {
        let n: u64 = a.trim().parse().unwrap_or(0);
        if n > 0 {
            issues += 1;
            println!("⚠ 有 {n} 个提交未推送到上游");
        } else {
            println!("✓ 与上游同步");
        }
    }

    // gitignore 存在性
    if !repo.join(".gitignore").exists() {
        issues += 1;
        println!("⚠ 缺少 .gitignore（byte-git ignore add 可生成）");
    } else {
        println!("✓ 存在 .gitignore");
    }

    println!();
    if issues == 0 {
        println!("健康检查通过，未发现问题");
    } else {
        println!("发现 {issues} 项需要注意");
    }
    Ok(())
}
