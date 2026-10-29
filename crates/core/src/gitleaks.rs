//! 密钥泄露扫描（集成外部 gitleaks）。
//!
//! `gitleaks git <repo> --report-format json --report-path - --exit-code 0 --no-banner`

use crate::config::Settings;
use anyhow::{bail, Context, Result};
use serde::Deserialize;
use std::path::Path;
use std::process::Command;

#[derive(Debug, Clone, Deserialize, serde::Serialize, Default)]
pub struct Finding {
    #[serde(rename = "RuleID", default)]
    pub rule_id: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub file: String,
    #[serde(default)]
    pub start_line: i64,
    #[serde(default)]
    pub end_line: i64,
    #[serde(default, rename = "Secret")]
    pub secret: String,
    #[serde(default)]
    pub commit: String,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub email: String,
    #[serde(default)]
    pub date: String,
    #[serde(default)]
    pub message: String,
    #[serde(default)]
    pub fingerprint: String,
}

#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct ScanResult {
    pub findings: Vec<Finding>,
}

/// 解析可执行文件路径：flag > config > PATH。
pub fn find_exe(flag_path: &str) -> Result<String> {
    let mut candidates: Vec<String> = Vec::new();
    if !flag_path.is_empty() {
        candidates.push(flag_path.to_string());
    } else if let Ok(s) = Settings::load() {
        if !s.gitleaks.exe_path.is_empty() {
            candidates.push(s.gitleaks.exe_path.clone());
        }
    }
    candidates.push("gitleaks".into());

    for c in &candidates {
        if Path::new(c).is_file() {
            return Ok(c.clone());
        }
        if which(c) {
            return Ok(c.clone());
        }
    }
    bail!("未找到 gitleaks 可执行文件");
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
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "unknown".into())
}

#[derive(Debug, Clone, Default)]
pub struct ScanOptions {
    pub repo_dir: String,
    pub exe_path: String,
    pub config_path: String,
    pub verbose: bool,
    /// 秒；0 = 默认 300
    pub timeout: u64,
}

/// 运行 gitleaks 扫描并解析 JSON 结果（密钥默认脱敏）。
pub fn scan(opts: &ScanOptions) -> Result<ScanResult> {
    if opts.exe_path.is_empty() {
        bail!("gitleaks 可执行文件路径为空");
    }
    let repo = if opts.repo_dir.is_empty() { "." } else { &opts.repo_dir };

    let mut args: Vec<String> = vec![
        "git".into(),
        repo.to_string(),
        "--report-format".into(),
        "json".into(),
        "--report-path".into(),
        "-".into(),
        "--exit-code".into(),
        "0".into(),
        "--no-banner".into(),
    ];
    if !opts.config_path.is_empty() {
        args.push("--config".into());
        args.push(opts.config_path.clone());
    }
    // 注意：不要传 -v——会切换为人类可读输出破坏 JSON 解析；Verbose 只控制脱敏
    let timeout = if opts.timeout == 0 { 300 } else { opts.timeout };
    args.push("--timeout".into());
    args.push(timeout.to_string());

    let out = Command::new(&opts.exe_path)
        .args(&args)
        .output()
        .with_context(|| format!("执行 gitleaks 失败: {}", opts.exe_path))?;
    if !out.status.success() {
        let stderr_text = String::from_utf8_lossy(&out.stderr).into_owned();
        bail!("gitleaks 扫描失败 (exit {:?}): {}", out.status.code(), stderr_text.trim());
    }

    let stdout = String::from_utf8_lossy(&out.stdout);
    let trimmed = stdout.trim();
    let mut findings: Vec<Finding> = if trimmed.is_empty() {
        vec![]
    } else {
        serde_json::from_str(trimmed).with_context(|| {
            format!("解析 gitleaks 输出失败 (原始: {})", &trimmed[..trimmed.len().min(300)])
        })?
    };
    if !opts.verbose {
        for f in &mut findings {
            f.secret = mask_secret(&f.secret);
        }
    }
    Ok(ScanResult { findings })
}

/// 脱敏：保留前 4 后 4，中间 ***；短密钥全遮蔽。
pub fn mask_secret(s: &str) -> String {
    if s.len() <= 8 {
        return "*".repeat(s.len());
    }
    format!("{}***{}", &s[..4], &s[s.len() - 4..])
}

/// 控制台输出扫描结果。
pub fn print_result(res: &ScanResult) {
    let n = res.findings.len();
    if n == 0 {
        println!("✅ 未发现密钥泄露");
        return;
    }
    println!("🚨 发现 {n} 处可能的密钥泄露\n");
    for (i, f) in res.findings.iter().enumerate() {
        let mut loc = f.file.clone();
        if f.start_line > 0 {
            if f.end_line > f.start_line {
                loc.push_str(&format!(":{}-{}", f.start_line, f.end_line));
            } else {
                loc.push_str(&format!(":{}", f.start_line));
            }
        }
        println!("[{}] {}  {}", i + 1, loc, f.rule_id);
        if !f.description.is_empty() {
            println!("    {}", f.description);
        }
        if !f.author.is_empty() {
            let email = if f.email.is_empty() { String::new() } else { format!(" <{}>", f.email) };
            let date = &f.date[..f.date.len().min(10)];
            println!("    作者: {}{}  {date}", f.author, email);
        }
        if !f.commit.is_empty() {
            let msg = if f.message.is_empty() { "(no message)" } else { f.message.lines().next().unwrap_or("") };
            println!("    commit: {}  {}", &f.commit[..f.commit.len().min(7)], msg);
        }
        if !f.secret.is_empty() {
            println!("    密钥: {}", f.secret);
        }
        if !f.fingerprint.is_empty() {
            println!("    指纹: {}", f.fingerprint);
        }
        println!();
    }
}
