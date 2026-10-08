//! 分支治理：已合并分支清理、孤立分支发现。
//!
//! 移植自 git-mate `internal/branch`。

use crate::util::{run_capture, run_inherit};
use anyhow::Result;
use std::path::Path;

const DEFAULT_PROTECTED: [&str; 4] = ["main", "master", "develop", "dev"];

#[derive(Debug, Clone, Default)]
pub struct CleanOptions {
    pub merged_into: String,
    pub dry_run: bool,
    pub exclude: Vec<String>,
    pub no_protected: bool,
}

/// 清理已合并分支。返回（候选数, 实际删除数）。
pub fn clean(repo: &Path, opts: &CleanOptions) -> Result<(usize, usize)> {
    let r = if opts.merged_into.is_empty() { "HEAD" } else { &opts.merged_into };
    let branches = merged_branches(repo, r)?;

    let mut protected: Vec<String> = DEFAULT_PROTECTED.iter().map(|s| s.to_string()).collect();
    if !opts.no_protected {
        protected.extend(opts.exclude.iter().cloned());
    } else {
        protected = opts.exclude.clone();
    }

    let to_delete: Vec<String> = branches
        .into_iter()
        .filter(|b| !protected.iter().any(|p| p == b))
        .collect();

    if to_delete.is_empty() {
        println!("没有需要清理的分支");
        return Ok((0, 0));
    }

    if opts.dry_run {
        println!("将删除 {} 个分支 (dry-run):", to_delete.len());
        for b in &to_delete {
            println!("  {b}");
        }
        return Ok((to_delete.len(), 0));
    }

    let mut deleted = 0;
    for b in &to_delete {
        if run_inherit(repo, ["branch", "-d", b]).is_err() {
            eprintln!("  跳过 {b}: 删除失败");
            continue;
        }
        deleted += 1;
    }
    println!("已删除 {deleted} 个分支");
    Ok((to_delete.len(), deleted))
}

/// 列出无上游远程的孤立分支。
pub fn list_orphans(repo: &Path) -> Result<Vec<String>> {
    let branches = local_branches(repo)?;
    let orphans: Vec<String> = branches
        .into_iter()
        .filter(|b| !has_upstream(repo, b))
        .collect();
    if orphans.is_empty() {
        println!("没有孤立分支");
    } else {
        println!("孤立分支 ({}):", orphans.len());
        for b in &orphans {
            println!("  {b}");
        }
    }
    Ok(orphans)
}

pub fn merged_branches(repo: &Path, reference: &str) -> Result<Vec<String>> {
    let out = run_capture(repo, ["branch", "--merged", reference])?;
    Ok(parse_branch_list(&out))
}

pub fn local_branches(repo: &Path) -> Result<Vec<String>> {
    let out = run_capture(repo, ["branch"])?;
    Ok(parse_branch_list(&out))
}

fn parse_branch_list(raw: &str) -> Vec<String> {
    raw.lines()
        .map(|l| l.trim().trim_start_matches("* ").trim())
        .filter(|l| !l.is_empty())
        .map(String::from)
        .collect()
}

fn has_upstream(repo: &Path, branch: &str) -> bool {
    run_capture(repo, ["config", &format!("branch.{branch}.remote")]).is_ok()
}
