//! 带名称的 stash 管理：保存/弹出/列表/删除/搜索。
//!
//! 移植自 git-mate `internal/stash`——命名 stash 以 `byte-git-stash:<name>`
//! 消息前缀实现（原为 git-mate-stash:）。

use crate::util::run_capture;
use anyhow::{bail, Result};
use std::path::Path;

const STASH_PREFIX: &str = "byte-git-stash:";

#[derive(Debug, Clone, serde::Serialize)]
pub struct StashEntry {
    pub index: usize,
    pub message: String,
    /// 命名 stash 的名称（非命名 stash 为空）。
    pub name: String,
}

pub fn save(repo: &Path, name: &str) -> Result<()> {
    if name.is_empty() {
        bail!("请指定 --name <name>");
    }
    let msg = format!("{STASH_PREFIX}{name}");
    run_capture(repo, ["stash", "push", "-m", &msg])?;
    println!("已保存 stash: {name}");
    Ok(())
}

pub fn pop(repo: &Path, name: &str) -> Result<()> {
    if name.is_empty() {
        bail!("请指定 --name <name>");
    }
    let idx = find_by_name(repo, name)?;
    run_capture(repo, ["stash", "pop", &format!("stash@{{{idx}}}")])?;
    println!("已弹出 stash: {name}");
    Ok(())
}

pub fn drop(repo: &Path, index: usize) -> Result<()> {
    run_capture(repo, ["stash", "drop", &format!("stash@{{{index}}}")])?;
    println!("已删除 stash@{{{index}}}");
    Ok(())
}

/// 列出（可按关键词过滤：匹配 message 或 name）。
pub fn list(repo: &Path, search: &str) -> Result<Vec<StashEntry>> {
    let entries = list_entries(repo)?;
    let filtered: Vec<StashEntry> = if search.is_empty() {
        entries
    } else {
        entries
            .into_iter()
            .filter(|e| e.message.contains(search) || e.name.contains(search))
            .collect()
    };
    if filtered.is_empty() {
        println!("没有 stash");
        return Ok(filtered);
    }
    for e in &filtered {
        let label = if e.name.is_empty() { &e.message } else { &e.name };
        println!("  stash@{{{: <2}}}  {}", e.index, label);
    }
    Ok(filtered)
}

pub fn show(repo: &Path, reference: &str, patch: bool) -> Result<()> {
    let r = if reference.is_empty() { "stash@{0}" } else { reference };
    let out = if patch {
        run_capture(repo, ["stash", "show", "-p", r])?
    } else {
        run_capture(repo, ["stash", "show", r])?
    };
    print!("{out}");
    Ok(())
}

pub fn list_entries(repo: &Path) -> Result<Vec<StashEntry>> {
    let out = match run_capture(repo, ["stash", "list"]) {
        Ok(o) => o,
        Err(_) => return Ok(Vec::new()),
    };
    let mut entries = Vec::new();
    for (i, line) in out.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() { continue; }
        // 格式: stash@{0}: WIP on main: abc123 msg / stash@{0}: On main: byte-git-stash:foo
        let msg = match line.split_once(": ") {
            Some((_, rest)) => rest.to_string(),
            None => line.to_string(),
        };
        let name = msg
            .strip_prefix(STASH_PREFIX)
            .unwrap_or("")
            .to_string();
        entries.push(StashEntry { index: i, message: msg, name });
    }
    Ok(entries)
}

fn find_by_name(repo: &Path, name: &str) -> Result<usize> {
    let entries = list_entries(repo)?;
    let prefix = format!("{STASH_PREFIX}{name}");
    for e in entries {
        if e.message.contains(&prefix) {
            return Ok(e.index);
        }
    }
    bail!("未找到名为 {name:?} 的 stash")
}
