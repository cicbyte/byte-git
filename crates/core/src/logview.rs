//! 日志浏览数据层：提交列表（conventional type/scope 解析）+ diff 获取。
//!
//! 数据装配层；TUI 交互在 apps/cli（ratatui）。

use crate::util::run_capture;
use anyhow::Result;
use std::path::Path;

#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct CommitEntry {
    pub hash: String,
    pub author: String,
    pub date: String,
    pub subject: String,
    #[serde(rename = "type")]
    pub ty: String,
    pub scope: String,
}

/// 拉取提交列表（limit 0 = 全部；filter 非空时按关键词过滤 hash/author/subject）。
pub fn fetch_commits(repo: &Path, limit: usize, filter: &str) -> Result<Vec<CommitEntry>> {
    let fmt = "--format=%H%x1f%an%x1f%ad%x1f%s";
    let mut args: Vec<String> = vec!["log".into(), fmt.into(), "--date=format:%Y-%m-%d %H:%M".into()];
    if limit > 0 {
        args.push(format!("-{limit}"));
    }
    let out = run_capture(repo, args)?;
    let mut entries = Vec::new();
    for line in out.lines() {
        let parts: Vec<&str> = line.split('\x1f').collect();
        if parts.len() < 4 {
            continue;
        }
        let subject = parts[3].to_string();
        let (ty, scope) = parse_conventional(&subject);
        let e = CommitEntry {
            hash: parts[0].to_string(),
            author: parts[1].to_string(),
            date: parts[2].to_string(),
            subject,
            ty,
            scope,
        };
        if filter.is_empty()
            || e.hash.contains(filter)
            || e.author.to_lowercase().contains(&filter.to_lowercase())
            || e.subject.to_lowercase().contains(&filter.to_lowercase())
        {
            entries.push(e);
        }
    }
    Ok(entries)
}

/// conventional commit 解析：`feat(scope): subject` → ("feat", "scope")。
pub fn parse_conventional(subject: &str) -> (String, String) {
    let re = regex::Regex::new(r"^(feat|fix|refactor|perf|docs|style|test|chore|build|ci)(?:\(([^)]+)\))?:\s*(.+)$").unwrap();
    match re.captures(subject) {
        Some(c) => (
            c.get(1).map(|m| m.as_str().to_string()).unwrap_or_default(),
            c.get(2).map(|m| m.as_str().to_string()).unwrap_or_default(),
        ),
        None => (String::new(), String::new()),
    }
}

/// 获取提交完整信息（message + diff stat）。
pub fn fetch_detail(repo: &Path, hash: &str) -> Result<String> {
    let meta = run_capture(repo, ["show", "--format=fuller", "--stat", hash])?;
    Ok(meta)
}

/// 获取提交 diff。
pub fn fetch_diff(repo: &Path, hash: &str) -> Result<String> {
    run_capture(repo, ["show", "--format=", hash])
}
