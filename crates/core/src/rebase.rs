//! Rebase 预览与 auto-squash。
//!

use crate::util::run_capture;
use anyhow::Result;
use std::path::Path;

#[derive(Debug, Clone, serde::Serialize)]
pub struct CommitEntry {
    pub action: String,
    pub hash: String,
    pub msg: String,
}

#[derive(Debug, Clone, Default)]
pub struct TargetOptions {
    pub branch: String,
    pub count: usize,
}

/// 解析 rebase 目标：--count N → HEAD~N；--branch → 分支；否则上游。
fn resolve_target(repo: &Path, opts: &TargetOptions) -> Result<String> {
    if opts.count > 0 {
        return Ok(format!("HEAD~{}", opts.count));
    }
    if !opts.branch.is_empty() {
        return Ok(opts.branch.clone());
    }
    default_upstream(repo).ok_or_else(|| anyhow::anyhow!("请指定 --branch 或 --count，或设置上游分支"))
}

fn default_upstream(repo: &Path) -> Option<String> {
    run_capture(repo, ["rev-parse", "--abbrev-ref", "--symbolic-full-name", "@{u}"])
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn commits_between(repo: &Path, target: &str) -> Result<Vec<CommitEntry>> {
    let out = run_capture(repo, ["log", "--oneline", &format!("{target}..HEAD")])
        .map_err(|e| anyhow::anyhow!("获取提交列表失败: {e}"))?;
    let mut entries = Vec::new();
    for line in out.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Some((hash, msg)) = line.split_once(' ') {
            entries.push(CommitEntry { action: "pick".into(), hash: hash.to_string(), msg: msg.to_string() });
        }
    }
    Ok(entries)
}

/// 预览 rebase 提交列表。
pub fn dry_run(repo: &Path, opts: &TargetOptions) -> Result<()> {
    let target = resolve_target(repo, opts)?;
    let entries = commits_between(repo, &target)?;
    if entries.is_empty() {
        println!("没有需要 rebase 的提交");
        return Ok(());
    }
    println!("预览 rebase {target} 的提交列表 ({} 个):\n", entries.len());
    for e in &entries {
        println!("  pick {} {}", &e.hash[..7.min(e.hash.len())], e.msg);
    }
    println!("\n将 rebase {} 个提交到 {target}", entries.len());
    Ok(())
}

#[derive(Debug, Clone, Default)]
pub struct AutoSquashOptions {
    pub branch: String,
    pub count: usize,
    pub dry_run: bool,
}

/// fixup!/squash! 自动整理（预览重排 + 执行 git rebase -i --autosquash）。
pub fn auto_squash(repo: &Path, opts: &AutoSquashOptions) -> Result<()> {
    let target = resolve_target(repo, &TargetOptions { branch: opts.branch.clone(), count: opts.count })?;
    let entries = commits_between(repo, &target)?;
    if entries.is_empty() {
        println!("没有需要处理的提交");
        return Ok(());
    }

    let is_fixup = |msg: &str| msg.starts_with("fixup! ") || msg.starts_with("squash! ");
    let strip_fixup = |msg: &str| msg.trim_start_matches("fixup! ").trim_start_matches("squash! ").to_string();

    let normal: Vec<&CommitEntry> = entries.iter().filter(|e| !is_fixup(&e.msg)).collect();
    let fixups: Vec<&CommitEntry> = entries.iter().filter(|e| is_fixup(&e.msg)).collect();

    // 目标消息集合（用于展示）
    println!("当前提交列表 ({} 个):\n", entries.len());
    for e in &entries {
        println!("  pick {} {}", &e.hash[..7.min(e.hash.len())], e.msg);
    }
    println!();

    // 重排：fixup 插到目标提交之后
    let mut remaining: Vec<CommitEntry> = normal.iter().map(|e| (*e).clone()).collect();
    for fu in &fixups {
        let prefix = strip_fixup(&fu.msg);
        let idx = remaining.iter().position(|n| n.msg == prefix);
        let mut entry = (*fu).clone();
        entry.action = if fu.msg.starts_with("squash! ") { "squash".into() } else { "fixup".into() };
        match idx {
            Some(i) => remaining.insert(i + 1, entry),
            None => remaining.push(entry),
        }
    }

    println!("Auto-squash 提交列表 ({} 个):\n", remaining.len());
    for e in &remaining {
        println!("  {} {} {}", e.action, &e.hash[..7.min(e.hash.len())], e.msg);
    }
    println!("\n发现 {} 个 fixup/squash 提交，将合并到对应目标", fixups.len());

    if opts.dry_run {
        return Ok(());
    }

    println!("\n正在执行 interactive rebase --autosquash...");
    // GIT_SEQUENCE_EDITOR=true：接受 autosquash 生成的 todo，无需人工编辑器（非交互环境）
    let status = std::process::Command::new("git")
        .arg("-C").arg(repo)
        .args(["rebase", "-i", "--autosquash"])
        .arg(&target)
        .env("GIT_SEQUENCE_EDITOR", "true")
        .status()
        .map_err(|e| anyhow::anyhow!("启动 git rebase 失败: {e}"))?;
    if !status.success() {
        anyhow::bail!("git rebase --autosquash 失败（退出码 {:?}）", status.code());
    }
    Ok(())
}

/// 供上层检查是否有上游分支。
pub fn has_upstream(repo: &Path) -> bool {
    default_upstream(repo).is_some()
}
