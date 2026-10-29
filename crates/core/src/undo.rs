//! 安全撤销：上一次提交 / rebase / merge。
//!

use crate::util::{run_capture, run_inherit};
use anyhow::{bail, Result};
use std::path::Path;

fn head_hash(repo: &Path) -> Result<String> {
    run_capture(repo, ["rev-parse", "HEAD"])
        .map(|s| s.trim().to_string())
        .map_err(|_| anyhow::anyhow!("获取 HEAD 失败"))
}

fn commit_msg(repo: &Path, hash: &str) -> Result<String> {
    run_capture(repo, ["log", "-1", "--format=%s", hash])
        .map(|s| s.trim().to_string())
        .map_err(|_| anyhow::anyhow!("获取提交信息失败"))
}

#[derive(Debug, Clone, Default)]
pub struct LastCommitOptions {
    pub keep_changes: bool,
}

/// 撤销上一次提交（--keep-changes 保留改动于工作区）。
pub fn last_commit(repo: &Path, opts: &LastCommitOptions) -> Result<()> {
    let hash = head_hash(repo)?;
    let short = &hash[..7.min(hash.len())];
    let msg = commit_msg(repo, &hash)?;

    let mode = if opts.keep_changes { "soft" } else { "hard" };
    let verb = if opts.keep_changes { "保留改动在工作区" } else { "并丢弃改动" };
    println!("将撤销提交 {short} ({msg}) {verb}");
    run_inherit(repo, vec!["reset".to_string(), format!("--{mode}"), "HEAD~1".to_string()])?;
    println!("已撤销提交 {short}，{}", if opts.keep_changes { "改动保留在工作区" } else { "改动已丢弃" });
    Ok(())
}

#[derive(Debug, Clone, Default)]
pub struct RebaseOptions {
    pub backup: bool,
    pub abort: bool,
}

/// 撤销进行中的 rebase（--abort 或 先备份再 abort）。
pub fn rebase(repo: &Path, opts: &RebaseOptions) -> Result<()> {
    if opts.abort {
        run_inherit(repo, ["rebase", "--abort"])?;
        println!("已终止 rebase");
        return Ok(());
    }
    if !is_rebasing(repo) {
        bail!("当前没有进行中的 rebase");
    }
    if opts.backup {
        if let Ok(head) = head_hash(repo) {
            let backup_ref = "refs/byte-git-undo/rebase-backup";
            if run_capture(repo, ["update-ref", backup_ref, &head]).is_ok() {
                println!("已备份当前 HEAD 到 {backup_ref} ({})", &head[..7.min(head.len())]);
            }
        }
    }
    run_inherit(repo, ["rebase", "--abort"])?;
    println!("已终止 rebase 并恢复到之前状态");
    Ok(())
}

#[derive(Debug, Clone, Default)]
pub struct MergeOptions {
    pub abort: bool,
}

/// 撤销进行中的 merge。
pub fn merge(repo: &Path, opts: &MergeOptions) -> Result<()> {
    if opts.abort {
        run_inherit(repo, ["merge", "--abort"])?;
        println!("已终止合并");
        return Ok(());
    }
    if !is_merging(repo) {
        bail!("当前没有进行中的合并");
    }
    if let Ok(head) = head_hash(repo) {
        let backup_ref = "refs/byte-git-undo/merge-backup";
        if run_capture(repo, ["update-ref", backup_ref, &head]).is_ok() {
            println!("已备份当前 HEAD 到 {backup_ref} ({})", &head[..7.min(head.len())]);
        }
    }
    run_inherit(repo, ["merge", "--abort"])?;
    println!("已终止合并并恢复到合并前状态");
    Ok(())
}

pub fn is_rebasing(repo: &Path) -> bool {
    run_capture(repo, ["rev-parse", "--verify", "REBASE_HEAD"]).is_ok()
}

pub fn is_merging(repo: &Path) -> bool {
    run_capture(repo, ["rev-parse", "--verify", "MERGE_HEAD"]).is_ok()
}
