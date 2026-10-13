//! AI 提交历史改写：规则检测 → 批量生成建议 → filter-branch 应用。
//!
//! 移植自 git-mate `internal/airewrite`。数据文件 `.git/byte-git/rewrite/ai-rewrite.json`，
//! 缓存 `.git/byte-git/ai-cache/<hash>.json`，规则 `~/.cicbyte/apps/byte-git/airewrite/rules.json`。

use crate::ai::{self, ChatClient, ChatMessage};
use crate::util::{run_capture, run_inherit_env, AppPaths};
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

// ---------------------------------------------------------------------------
// 条目模型
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AICommitEntry {
    pub hash: String,
    pub original: String,
    pub diff_summary: String,
    #[serde(default)]
    pub files: Vec<String>,
    #[serde(default)]
    pub diff: String,
    #[serde(default)]
    pub need_ai: bool,
    #[serde(default)]
    pub suggested: String,
    /// pending / approved / skipped / retry
    #[serde(default)]
    pub status: String,
}

#[derive(Debug, Clone, Default)]
pub struct GenOptions {
    pub repo: String,
    pub all: bool,
    pub range: String,
    pub last: usize,
    pub unpushed: bool,
    pub auto_detect: bool,
    pub pattern: String,
    pub output: String,
}

#[derive(Debug, Clone, Default)]
pub struct RunOptions {
    pub repo: String,
    pub file: String,
    pub model: String,
    pub base_url: String,
    pub api_key: String,
    pub lang: String,
    pub max_chars: usize,
    pub retry: bool,
    pub dry_run: bool,
}

#[derive(Debug, Clone, Default)]
pub struct ApplyOptions {
    pub repo: String,
    pub branch: String,
    pub file: String,
    pub dry_run: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CacheEntry {
    hash: String,
    suggested: String,
    model: String,
    timestamp: String,
}

// ---------------------------------------------------------------------------
// 检测规则
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectRule {
    pub name: String,
    pub pattern: String,
    #[serde(default)]
    pub desc: String,
}

pub fn default_rules() -> Vec<DetectRule> {
    vec![
        DetectRule { name: "太短".into(), pattern: r"^.{0,4}$".into(), desc: "少于 5 个字符".into() },
        DetectRule { name: "无意义词".into(), pattern: r"(?i)^(auto|update|updated|wip|fix|fixed|test|tmp|temp|hack|misc)$".into(), desc: "wip/fix/tmp 等单字".into() },
        DetectRule { name: "纯数字".into(), pattern: r"^\d+$".into(), desc: "纯数字 message".into() },
        DetectRule { name: "纯特殊字符".into(), pattern: r"^[^a-zA-Z0-9]+$".into(), desc: "无字母数字".into() },
        DetectRule { name: "无 type 前缀".into(), pattern: r"^(?!(feat|fix|docs|style|refactor|perf|test|build|ci|chore|revert)(\(.+\))?:).+".into(), desc: "不符合 conventional commit 规范".into() },
    ]
}

fn rules_path() -> Result<std::path::PathBuf> {
    Ok(AppPaths::new()?.root().join("airewrite").join("rules.json"))
}

pub fn load_rules() -> Result<Vec<DetectRule>> {
    let path = rules_path()?;
    let Ok(data) = std::fs::read_to_string(&path) else {
        let r = default_rules();
        save_rules(&r)?;
        return Ok(r);
    };
    match serde_json::from_str::<Vec<DetectRule>>(&data) {
        Ok(r) if !r.is_empty() => Ok(r),
        _ => Ok(default_rules()),
    }
}

pub fn save_rules(rules: &[DetectRule]) -> Result<()> {
    let path = rules_path()?;
    if let Some(dir) = path.parent() {
        AppPaths::ensure_dir(dir)?;
    }
    let data = serde_json::to_string_pretty(rules)?;
    std::fs::write(path, data).context("写入 rules.json 失败")?;
    Ok(())
}

pub fn match_any(msg: &str, rules: &[DetectRule]) -> bool {
    rules.iter().any(|r| {
        // 规则允许环视（如负向前瞻），用 fancy-regex 编译；无效模式静默跳过
        fancy_regex::Regex::new(&r.pattern)
            .ok()
            .and_then(|re| re.is_match(msg).ok())
            .unwrap_or(false)
    })
}

fn is_bad_message(msg: &str) -> bool {
    match load_rules() {
        Ok(rules) => match_any(msg, &rules),
        Err(_) => false,
    }
}

// ---------------------------------------------------------------------------
// gen：导出提交清单
// ---------------------------------------------------------------------------

fn commit_stat(repo: &Path, hash: &str) -> (String, Vec<String>) {
    let stat = run_capture(repo, ["show", "--stat", "--format=", hash])
        .unwrap_or_default()
        .trim()
        .to_string();
    let files = run_capture(repo, ["show", "--name-only", "--format=", hash])
        .unwrap_or_default()
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect();
    (stat, files)
}

fn commit_diff(repo: &Path, hash: &str, max_chars: usize) -> String {
    let diff = run_capture(repo, ["show", "--format=", hash]).unwrap_or_default();
    if max_chars > 0 && diff.len() > max_chars {
        format!("{}\n... (diff 已截断)", &diff[..max_chars])
    } else {
        diff
    }
}

pub fn default_rewrite_file(repo: &Path) -> Result<std::path::PathBuf> {
    crate::util::repo_data_dir(repo).map(|d| d.join("rewrite").join("ai-rewrite.json"))
}

pub fn gen(opts: &GenOptions) -> Result<()> {
    let repo = Path::new(&opts.repo);
    let mut log_args: Vec<String> = vec!["log".into(), "--format=%H|%s".into()];
    if !opts.range.is_empty() {
        log_args.push(opts.range.clone());
    } else if opts.last > 0 {
        log_args.push(format!("-{}", opts.last));
    } else if opts.unpushed {
        log_args.push("--branches".into());
        log_args.push("--not".into());
        log_args.push("--remotes".into());
    }

    let output = run_capture(repo, log_args).context("git log 失败")?;
    let lines: Vec<&str> = output.lines().filter(|l| !l.trim().is_empty()).collect();
    if lines.is_empty() {
        bail!("没有找到提交记录");
    }

    let mut entries = Vec::new();
    let mut auto_count = 0;
    for line in lines {
        let Some((hash, subject)) = line.split_once('|') else { continue };
        let (diff_summary, files) = commit_stat(repo, hash);
        let diff = commit_diff(repo, hash, 4000);

        let mut need_ai = false;
        if opts.auto_detect {
            need_ai = is_bad_message(subject);
            if need_ai { auto_count += 1; }
        } else if !opts.pattern.is_empty() {
            if let Ok(re) = regex::Regex::new(&opts.pattern) {
                if re.is_match(subject) {
                    need_ai = true;
                    auto_count += 1;
                }
            }
        }

        entries.push(AICommitEntry {
            hash: hash.to_string(),
            original: subject.to_string(),
            diff_summary,
            files,
            diff,
            need_ai,
            suggested: String::new(),
            status: "pending".into(),
        });
    }

    let out = if opts.output.is_empty() {
        default_rewrite_file(repo)?.display().to_string()
    } else {
        opts.output.clone()
    };
    if let Some(dir) = Path::new(&out).parent() {
        AppPaths::ensure_dir(dir)?;
    }
    std::fs::write(&out, serde_json::to_string_pretty(&entries)?)
        .with_context(|| format!("写入文件失败: {out}"))?;
    println!("已导出 {} 条提交到 {out}", entries.len());
    if auto_count > 0 {
        let detect_type = if opts.pattern.is_empty() { "auto-detect" } else { "pattern" };
        println!("  自动标记 {auto_count} 条需要 AI 改写（{detect_type}）");
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// run：批量 AI 生成（含缓存）
// ---------------------------------------------------------------------------

fn cache_path(repo: &Path, hash: &str) -> Result<std::path::PathBuf> {
    Ok(crate::util::repo_data_dir(repo)?.join("ai-cache").join(format!("{hash}.json")))
}

fn read_cache(repo: &Path, hash: &str) -> Option<CacheEntry> {
    let p = cache_path(repo, hash).ok()?;
    serde_json::from_str(&std::fs::read_to_string(p).ok()?).ok()
}

fn write_cache(repo: &Path, e: &CacheEntry) -> Result<()> {
    let p = cache_path(repo, &e.hash)?;
    AppPaths::ensure_dir(p.parent().unwrap())?;
    std::fs::write(&p, serde_json::to_string_pretty(e)?)?;
    Ok(())
}

/// hook run（commit-msg AI 改写）复用的 prompt 构建入口。
pub fn build_prompt_for_hook(entry: &AICommitEntry, diff: &str, lang: &str, max_chars: usize) -> String {
    build_prompt(entry, diff, lang, max_chars)
}

fn build_prompt(entry: &AICommitEntry, diff: &str, lang: &str, max_chars: usize) -> String {
    let lang_str = match lang {
        "en" => "English",
        _ => "中文",
    };
    let tpl = ai::get_prompt_file("airewrite.md")
        .unwrap_or_else(|_| "你是一个 Git 提交消息专家。根据 diff 改写 commit message。".into());
    let mut s = tpl
        .replace("{{language}}", lang_str)
        .replace("{{original}}", &entry.original)
        .replace("{{files}}", &entry.files.join(", "))
        .replace("{{diff_summary}}", &entry.diff_summary)
        .replace("{{diff}}", diff);
    if max_chars > 0 && s.len() > max_chars {
        s = format!("{}\n... (已截断)", &s[..max_chars]);
    }
    s
}

pub fn run(opts: &RunOptions) -> Result<()> {
    let repo = Path::new(&opts.repo);
    let data = std::fs::read_to_string(&opts.file).context("读取文件失败")?;
    let mut entries: Vec<AICommitEntry> = serde_json::from_str(&data).context("解析 JSON 失败")?;

    let pending: Vec<usize> = entries
        .iter()
        .enumerate()
        .filter(|(_, e)| {
            if opts.retry {
                e.status == "retry"
            } else {
                e.need_ai && e.suggested.is_empty()
            }
        })
        .map(|(i, _)| i)
        .collect();
    if pending.is_empty() {
        println!("没有需要处理的条目");
        return Ok(());
    }

    let mut to_call = Vec::new();
    let mut cached_count = 0;
    for idx in &pending {
        if let Some(c) = read_cache(repo, &entries[*idx].hash) {
            if !c.suggested.is_empty() {
                entries[*idx].suggested = c.suggested;
                entries[*idx].status = "pending".into();
                cached_count += 1;
                continue;
            }
        }
        to_call.push(*idx);
    }

    println!("处理中: {} 条需要 AI 生成", pending.len());
    if cached_count > 0 {
        println!("  已缓存跳过: {cached_count} 条");
    }
    println!("  待调用 AI: {} 条\n", to_call.len());

    if opts.dry_run {
        println!("(dry-run 模式，未调用 AI)");
        return save_json(&opts.file, &entries);
    }

    let client = ChatClient::new(&opts.base_url, &opts.api_key, &opts.model);
    let mut success = 0;
    let mut failed = 0;
    for seq_idx in 0..to_call.len() {
        let idx = to_call[seq_idx];
        // 先取快照，避免与后面的可变借用冲突
        let entry: AICommitEntry = entries[idx].clone();
        let diff = if entry.diff.is_empty() {
            commit_diff(repo, &entry.hash, 4000)
        } else {
            entry.diff.clone()
        };
        let prompt = build_prompt(&entry, &diff, &opts.lang, opts.max_chars);

        match client.chat(&[ChatMessage::user(prompt)]) {
            Ok(suggested) => {
                let suggested = suggested.trim().to_string();
                let _ = write_cache(
                    repo,
                    &CacheEntry {
                        hash: entry.hash.clone(),
                        suggested: suggested.clone(),
                        model: opts.model.clone(),
                        timestamp: chrono::Local::now().format("%Y-%m-%dT%H:%M:%S").to_string(),
                    },
                );
                success += 1;
                println!("[{}/{}] {} → {}", success + failed, to_call.len(), &entry.hash[..8], suggested);
                entries[idx].suggested = suggested;
                entries[idx].status = "pending".into();
            }
            Err(e) => {
                failed += 1;
                println!("[{}/{}] {} → 失败: {e:#}", success + failed, to_call.len(), &entry.hash[..8]);
                entries[idx].status = "retry".into();
            }
        }
    }

    println!("\n完成: {success} 条成功, {failed} 条失败");
    save_json(&opts.file, &entries)
}

fn save_json(file: &str, entries: &[AICommitEntry]) -> Result<()> {
    std::fs::write(file, serde_json::to_string_pretty(entries)?).context("写入 JSON 失败")?;
    println!("已更新 {file}");
    Ok(())
}

// ---------------------------------------------------------------------------
// apply：filter-branch 应用改写
// ---------------------------------------------------------------------------

pub fn apply(opts: &ApplyOptions) -> Result<()> {
    let repo = Path::new(&opts.repo);
    let data = std::fs::read_to_string(&opts.file).context("读取文件失败")?;
    let entries: Vec<AICommitEntry> = serde_json::from_str(&data).context("解析 JSON 失败")?;

    let changed: Vec<&AICommitEntry> = entries
        .iter()
        .filter(|e| e.status == "approved" && !e.suggested.is_empty())
        .collect();
    if changed.is_empty() {
        println!("没有需要改写的提交");
        return Ok(());
    }

    println!("共 {} 条提交需要改写:", changed.len());
    for e in &changed {
        let orig: String = e.original.chars().take(40).collect();
        let orig = if e.original.chars().count() > 40 { format!("{orig}...") } else { orig };
        println!("  {} [{} → {}]", &e.hash[..8], orig, e.suggested);
    }

    // 级联影响计算：找最旧被改提交，确定改写区间
    let branch = if opts.branch.is_empty() { "HEAD".to_string() } else { opts.branch.clone() };
    let mut revision_range = branch.clone();
    let mut total_affected = changed.len();
    if let Ok(rev) = run_capture(repo, ["rev-list", &branch]) {
        let all: Vec<&str> = rev.lines().map(str::trim).filter(|s| !s.is_empty()).collect();
        let set: std::collections::HashSet<&str> = changed.iter().map(|e| e.hash.as_str()).collect();
        for i in (0..all.len()).rev() {
            if set.contains(all[i]) {
                total_affected = i + 1;
                if i < all.len() - 1 {
                    revision_range = format!("{}..{}", all[i + 1], branch);
                } else if run_capture(repo, ["rev-parse", "--verify", &format!("{}^", all[i])]).is_ok() {
                    revision_range = format!("{}^..{}", all[i], branch);
                } else {
                    revision_range = "--all".into();
                }
                break;
            }
        }
    }
    if total_affected > changed.len() {
        println!("\n注意: 因 hash 级联，实际将改写 {total_affected} 条提交（含 {} 条未直接修改的后续提交）",
            total_affected - changed.len());
    }

    if opts.dry_run {
        println!("\n(dry-run 模式，未执行)");
        return Ok(());
    }

    // 建议先 byte-git backup create 做整库克隆备份
    let script = build_msg_filter(&changed.iter().map(|e| (*e).clone()).collect::<Vec<_>>());

    println!("\n开始执行 filter-branch...");
    run_inherit_env(repo, ["filter-branch", "-f", "--msg-filter", &script,
        "--original", "refs/original/ai-rewrite", "--", &revision_range],
        &[("FILTER_BRANCH_SQUELCH_WARNING", "1")])
        .context("filter-branch 执行失败")?;
    let _ = run_capture(repo, ["update-ref", "-d", "refs/original/ai-rewrite"]);
    println!("\n改写完成！");
    Ok(())
}

fn build_msg_filter(entries: &[AICommitEntry]) -> String {
    let mut buf = String::from("case \"$GIT_COMMIT\" in\n");
    for e in entries {
        let prefix = &e.hash[..8];
        // POSIX sh 单引号转义
        let msg = e.suggested.replace('\'', "'\\''");
        buf.push_str(&format!("  {prefix}*)\n    echo '{msg}'\n    ;;\n"));
    }
    buf.push_str("  *)\n    cat\n    ;;\nesac\n");
    buf
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_rules_semantics() {
        let rules = default_rules();
        // 太短
        assert!(match_any("w", &rules));
        // 无意义词（大小写不敏感）
        assert!(match_any("wip", &rules));
        assert!(match_any("UPDATE", &rules));
        // 纯数字 / 纯特殊字符
        assert!(match_any("12345", &rules));
        assert!(match_any("。。。", &rules));
        // 无 type 前缀（负向前瞻规则，须由 fancy-regex 编译）
        assert!(match_any("fix stuff", &rules));
        assert!(match_any("随便改了点", &rules));
        // 规范消息不命中
        assert!(!match_any("feat: add x", &rules));
        assert!(!match_any("fix(core): repair y", &rules));
    }
}
