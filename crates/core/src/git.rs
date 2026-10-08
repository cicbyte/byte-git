//! git 仓库基础操作：分支、远程、用户配置、提交查询。

use crate::util::run_capture;
use anyhow::Result;
use std::path::Path;

/// 当前分支名（分离 HEAD 时返回 "HEAD (detached)"，空仓库返回 "(空仓库)"）。
pub fn current_branch(repo: &Path) -> Result<String> {
    // 空仓库（无任何提交）时 HEAD 无法解析
    if run_capture(repo, ["rev-parse", "--verify", "HEAD"]).is_err() {
        return Ok("(空仓库)".into());
    }
    let out = run_capture(repo, ["rev-parse", "--abbrev-ref", "HEAD"])?;
    let name = out.trim();
    if name.is_empty() || name == "HEAD" {
        Ok("HEAD (detached)".into())
    } else {
        Ok(name.into())
    }
}

/// 远程 URL 列表（name\turl）。
pub fn remotes(repo: &Path) -> Result<Vec<(String, String)>> {
    let out = run_capture(repo, ["remote", "-v"])?;
    let mut seen = std::collections::BTreeSet::new();
    let mut list = Vec::new();
    for line in out.lines() {
        if let Some((name, rest)) = line.split_once('\t') {
            if let Some(url) = rest.split_whitespace().next() {
                let key = name.to_string();
                if seen.insert(key.clone()) {
                    list.push((key, url.to_string()));
                }
            }
        }
    }
    Ok(list)
}

/// origin 远程 URL（无 origin 时取第一个远程）。
pub fn origin_url(repo: &Path) -> Result<Option<String>> {
    let list = remotes(repo)?;
    Ok(list
        .iter()
        .find(|(n, _)| n == "origin")
        .or_else(|| list.first())
        .map(|(_, u)| u.clone()))
}

/// 全局/仓库 git 配置读取（空值返回 None）。
pub fn config_get(repo: Option<&Path>, key: &str) -> Option<String> {
    let out = match repo {
        Some(r) => run_capture(r, ["config", "--get", key]).ok()?,
        None => crate::util::exec_capture("git", &["config", "--get", key]).ok()?,
    };
    let v = out.trim().to_string();
    if v.is_empty() { None } else { Some(v) }
}

/// 写 git 配置（--global 由 global flag 控制）。
pub fn config_set(repo: Option<&Path>, key: &str, value: &str, global: bool) -> Result<()> {
    let mut args: Vec<String> = vec!["config".into()];
    if global {
        args.push("--global".into());
    }
    args.push(key.into());
    args.push(value.into());
    match repo {
        Some(r) => crate::util::run_inherit(r, args)?,
        None => {
            let refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
            crate::util::exec_inherit("git", &refs)?;
        }
    }
    Ok(())
}

/// 提交总数（空仓库返回 0）。
pub fn commit_count(repo: &Path) -> Result<u64> {
    match run_capture(repo, ["rev-list", "--count", "HEAD"]) {
        Ok(out) => Ok(out.trim().parse().unwrap_or(0)),
        Err(_) => Ok(0),
    }
}

/// 工作区状态（porcelain 解析：未暂存/已暂存/未跟踪计数）。
#[derive(Debug, Default, Clone, serde::Serialize)]
pub struct WorktreeStatus {
    pub staged: usize,
    pub unstaged: usize,
    pub untracked: usize,
    pub dirty: bool,
}

pub fn status(repo: &Path) -> Result<WorktreeStatus> {
    let out = run_capture(repo, ["status", "--porcelain"])?;
    let mut s = WorktreeStatus::default();
    for line in out.lines() {
        if line.len() < 2 { continue; }
        let x = line.as_bytes()[0];
        let y = line.as_bytes()[1];
        if x != ' ' as u8 && x != '?' as u8 { s.staged += 1; }
        if y != ' ' as u8 && y != '?' as u8 { s.unstaged += 1; }
        if x == '?' as u8 && y == '?' as u8 { s.untracked += 1; }
    }
    s.dirty = s.staged + s.unstaged + s.untracked > 0;
    Ok(s)
}

/// 仓库根目录（任意子目录进入仓库根）。
pub fn repo_root(repo: &Path) -> Result<std::path::PathBuf> {
    let out = run_capture(repo, ["rev-parse", "--show-toplevel"])?;
    Ok(std::path::PathBuf::from(out.trim()))
}

/// 仓库名（目录名）。
pub fn repo_name(repo: &Path) -> String {
    repo_root(repo)
        .map(|p| {
            p.file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| "repo".into())
        })
        .unwrap_or_else(|_| "repo".into())
}
