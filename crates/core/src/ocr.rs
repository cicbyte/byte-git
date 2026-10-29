//! AI 代码审查（集成外部 OpenCodeReview / ocr）。
//!
//! `ocr review --format json --audience agent …`，
//! stdout 收 JSON 结果，stderr 实时透传为进度。

use crate::config::Settings;
use anyhow::{bail, Context, Result};
use serde::Deserialize;
use std::io::BufRead;
use std::path::Path;
use std::process::{Command, Stdio};

#[derive(Debug, Clone, Deserialize, serde::Serialize, Default)]
pub struct ReviewComment {
    #[serde(default)]
    pub file: String,
    #[serde(default)]
    pub line: i64,
    #[serde(default)]
    pub severity: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub suggestion: String,
}

#[derive(Debug, Clone, Deserialize, serde::Serialize, Default)]
pub struct ReviewResult {
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub comments: Vec<ReviewComment>,
    #[serde(default)]
    pub raw: serde_json::Value,
}

/// 解析 ocr 可执行文件：flag > config > PATH（含 npm 全局常见名）。
pub fn find_exe(flag_path: &str) -> Result<String> {
    let mut candidates: Vec<String> = Vec::new();
    if !flag_path.is_empty() {
        candidates.push(flag_path.to_string());
    } else if let Ok(s) = Settings::load() {
        if !s.ocr.exe_path.is_empty() {
            candidates.push(s.ocr.exe_path.clone());
        }
    }
    candidates.push("ocr".into());
    candidates.push("open-code-review".into());

    for c in &candidates {
        if Path::new(c).is_file() {
            return Ok(c.clone());
        }
        if which(c) {
            return Ok(c.clone());
        }
    }
    bail!("未找到 ocr (OpenCodeReview) 可执行文件");
}

fn which(prog: &str) -> bool {
    if prog.contains(std::path::MAIN_SEPARATOR) || prog.contains('/') {
        return false;
    }
    let exts = std::env::var("PATHEXT").unwrap_or_else(|_| ".exe;.bat;.cmd".into());
    std::env::var_os("PATH")
        .map(|paths| {
            std::env::split_paths(&paths).any(|dir| {
                let p = dir.join(prog);
                p.is_file()
                    || exts.split(';').filter(|e| !e.is_empty()).any(|e| dir.join(format!("{prog}{e}")).is_file())
            })
        })
        .unwrap_or(false)
}

pub fn version(exe: &str) -> String {
    Command::new(exe)
        .arg("--version")
        .output()
        .ok()
        .map(|o| {
            let s = String::from_utf8_lossy(&o.stdout).trim().to_string();
            if s.is_empty() { String::from_utf8_lossy(&o.stderr).trim().to_string() } else { s }
        })
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "unknown".into())
}

#[derive(Debug, Clone, Default)]
pub struct ReviewOptions {
    pub repo_dir: String,
    pub from: String,
    pub to: String,
    pub commit: String,
    pub model: String,
    pub background: String,
    pub exe_path: String,
    pub concurrency: i64,
    pub max_tools: i64,
    pub audience: String,
    pub language: String,
}

/// audience 归一化："human"（stdout 进度行）/ 其它 → "agent"（纯 JSON）。
fn audience_flag(s: &str) -> &'static str {
    if s.eq_ignore_ascii_case("human") {
        "human"
    } else {
        "agent"
    }
}

/// 执行 ocr review：stderr 实时透传（进度），stdout 结束后解析 JSON。
pub fn review(opts: &ReviewOptions) -> Result<ReviewResult> {
    if opts.exe_path.is_empty() {
        bail!("OCR 可执行文件路径为空");
    }
    let repo = if opts.repo_dir.is_empty() { "." } else { &opts.repo_dir };

    let mut args: Vec<String> = vec![
        "review".into(),
        "--format".into(),
        "json".into(),
        "--audience".into(),
        audience_flag(&opts.audience).into(),
        "--repo".into(),
        repo.to_string(),
    ];
    if !opts.commit.is_empty() {
        args.push("--commit".into());
        args.push(opts.commit.clone());
    } else if !opts.from.is_empty() && !opts.to.is_empty() {
        args.push("--from".into());
        args.push(opts.from.clone());
        args.push("--to".into());
        args.push(opts.to.clone());
    }
    if !opts.model.is_empty() {
        args.push("--model".into());
        args.push(opts.model.clone());
    }
    if !opts.background.is_empty() {
        args.push("--background".into());
        args.push(opts.background.clone());
    }
    if opts.concurrency > 0 {
        args.push("--concurrency".into());
        args.push(opts.concurrency.to_string());
    }
    if opts.max_tools > 0 {
        args.push("--max-tools".into());
        args.push(opts.max_tools.to_string());
    }
    if !opts.language.is_empty() {
        args.push("--language".into());
        args.push(opts.language.clone());
    }

    let mut child = Command::new(&opts.exe_path)
        .args(&args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| format!("启动 ocr 失败: {}", opts.exe_path))?;

    // stderr → 终端实时透传（进度）
    if let Some(stderr) = child.stderr.take() {
        let reader = std::io::BufReader::new(stderr);
        for line in reader.lines() {
            match line {
                Ok(l) => eprintln!("\r{l}\u{1b}[K"),
                Err(_) => break,
            }
        }
    }

    let output = child.wait_with_output().context("等待 ocr 结束失败")?;
    if !output.status.success() {
        let stderr_text = String::from_utf8_lossy(&output.stderr).into_owned();
        bail!("ocr review 失败 (exit {:?}): {}", output.status.code(), stderr_text.trim());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let trimmed = stdout.trim();
    if trimmed.is_empty() {
        bail!("ocr 无输出");
    }
    let raw: serde_json::Value = serde_json::from_str(trimmed).context("解析 ocr JSON 输出失败")?;
    let mut res = ReviewResult { raw: raw.clone(), ..Default::default() };
    res.summary = raw.get("summary").and_then(|v| v.as_str()).unwrap_or_default().to_string();
    if let Some(comments) = raw.get("comments").and_then(|v| v.as_array()) {
        res.comments = comments
            .iter()
            .filter_map(|c| serde_json::from_value(c.clone()).ok())
            .collect();
    }
    Ok(res)
}

/// 控制台输出审查结果。
pub fn print_result(res: &ReviewResult) {
    println!();
    if !res.summary.is_empty() {
        println!("📋 审查总结\n{}\n", res.summary);
    }
    if res.comments.is_empty() {
        println!("✅ 没有发现问题");
        return;
    }
    println!("发现 {} 条意见：\n", res.comments.len());
    for (i, c) in res.comments.iter().enumerate() {
        let mut loc = c.file.clone();
        if c.line > 0 {
            loc.push_str(&format!(":{}", c.line));
        }
        println!("[{}] {} ({})", i + 1, loc, c.severity);
        println!("    {}", c.description);
        if !c.suggestion.is_empty() {
            println!("    建议: {}", c.suggestion);
        }
        println!();
    }
}
