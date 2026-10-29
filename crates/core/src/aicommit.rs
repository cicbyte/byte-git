//! AI 提交信息生成：暂存 diff → LLM → 确认 → 提交。
//!

use crate::ai::{self, ChatClient, ChatMessage};
use crate::util::{run_capture, run_inherit};
use anyhow::{bail, Context, Result};
use std::io::Write;
use std::path::Path;

#[derive(Debug, Clone, Default)]
pub struct CommitOptions {
    pub repo: String,
    pub model: String,
    pub base_url: String,
    pub api_key: String,
    pub lang: String,
    pub max_tokens: i64,
    pub dry_run: bool,
    pub no_verify: bool,
}

pub fn staged_diff(repo: &Path) -> Result<String> {
    run_capture(repo, ["diff", "--cached"])
}

pub fn staged_files(repo: &Path) -> Result<Vec<String>> {
    let out = run_capture(repo, ["diff", "--cached", "--name-only"])?;
    Ok(out.lines().map(|l| l.trim().to_string()).filter(|l| !l.is_empty()).collect())
}

pub fn staged_stat(repo: &Path) -> Result<String> {
    run_capture(repo, ["diff", "--cached", "--stat"]).map(|s| s.trim().to_string())
}

pub fn has_staged_changes(repo: &Path) -> bool {
    run_capture(repo, ["diff", "--cached", "--quiet"]).is_err()
}

/// 组装消息（系统提示词 + 变更文件/统计 + 截断 diff）。
pub fn build_messages(diff: &str, stat: &str, files: &[String], lang: &str, max_tokens: i64) -> Vec<ChatMessage> {
    let lang_str = match lang {
        "en" => "English",
        _ => "中文",
    };
    let system_prompt = ai::get_prompt_file("aicommit.md")
        .unwrap_or_else(|_| "你是一个 Git 提交消息专家。根据 diff 生成 conventional commit message。".into());
    let sys = system_prompt.replace("{{language}}", lang_str);

    let mut user = String::new();
    user.push_str(&format!("变更文件: {}\n", files.join(", ")));
    if !stat.is_empty() {
        user.push_str(&format!("变更统计: {stat}\n"));
    }
    user.push_str("\nDiff:\n");
    let prefix_tokens = ai::estimate_tokens(&user);
    let available = max_tokens - prefix_tokens;
    let diff_content = if ai::estimate_tokens(diff) > available {
        ai::truncate_to_token_limit(diff, available)
    } else {
        diff.to_string()
    };
    user.push_str(&diff_content);

    vec![ChatMessage::system(sys), ChatMessage::user(user)]
}

/// 生成提交信息（流式打印）。
pub fn generate(opts: &CommitOptions) -> Result<String> {
    let repo = Path::new(&opts.repo);
    if !has_staged_changes(repo) {
        bail!("暂存区没有改动，请先 git add");
    }
    let raw_diff = staged_diff(repo)?;
    let files = staged_files(repo)?;
    let stat = staged_stat(repo).unwrap_or_default();

    let messages = build_messages(&raw_diff, &stat, &files, &opts.lang, opts.max_tokens);
    let client = ChatClient::new(&opts.base_url, &opts.api_key, &opts.model);
    let suggested = client
        .chat_stream(&messages, |_| {})
        .context("AI 生成失败")?;
    println!();
    Ok(clean_message(&suggested))
}

/// 清理 LLM 输出：去 markdown 代码块包裹与引号。
pub fn clean_message(s: &str) -> String {
    let mut s = s.trim().to_string();
    if s.starts_with("```") {
        if let Some(idx) = s[3..].find('\n') {
            s = s[3 + idx + 1..].to_string();
        }
        if let Some(idx) = s.rfind("```") {
            s = s[..idx].to_string();
        }
        s = s.trim().to_string();
    }
    let bytes = s.as_bytes();
    if bytes.len() >= 2 {
        let first = bytes[0] as char;
        let last = bytes[bytes.len() - 1] as char;
        if (first == '"' && last == '"') || (first == '\'' && last == '\'') {
            s = s[1..s.len() - 1].to_string();
        }
    }
    s
}

/// 执行提交。
pub fn commit(repo: &Path, message: &str, no_verify: bool) -> Result<()> {
    let mut args: Vec<String> = vec!["commit".into()];
    if no_verify {
        args.push("--no-verify".into());
    }
    args.push("-m".into());
    args.push(message.to_string());
    run_inherit(repo, args)
}

/// 交互确认循环：返回 (action, message)。action ∈ accept / regenerate / abort。
pub fn confirm(suggested: &str) -> Result<(String, String)> {
    println!("\n生成的 commit message:");
    println!("  {suggested}");
    print!("\n确认提交? [Y=提交 / r=重新生成 / n=取消]: ");
    std::io::stdout().flush()?;
    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;
    match input.trim().to_lowercase().as_str() {
        "" | "y" | "yes" => Ok(("accept".into(), suggested.to_string())),
        "r" | "regenerate" => Ok(("regenerate".into(), suggested.to_string())),
        _ => Ok(("abort".into(), suggested.to_string())),
    }
}

/// 完整流程：生成 → 确认 → （重新生成循环）→ 提交。
pub fn run_commit_flow(opts: &CommitOptions) -> Result<()> {
    let mut suggested = generate(opts)?;
    if suggested.is_empty() {
        bail!("AI 返回空结果");
    }
    if opts.dry_run {
        println!("生成的 commit message:\n  {suggested}");
        return Ok(());
    }
    loop {
        let (action, message) = confirm(&suggested)?;
        match action.as_str() {
            "accept" => {
                commit(Path::new(&opts.repo), &message, opts.no_verify)?;
                println!("已提交: {message}");
                return Ok(());
            }
            "regenerate" => {
                suggested = generate(opts)?;
                if suggested.is_empty() {
                    bail!("AI 返回空结果");
                }
            }
            _ => {
                println!("已取消");
                return Ok(());
            }
        }
    }
}
