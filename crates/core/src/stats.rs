//! 开发统计：多仓库提交统计、活跃时段热力图、连续天数、语言分布。
//!
//! 输出为无第三方样式的简洁 ANSI 渲染。

use crate::util::exec_capture;
use anyhow::{bail, Context, Result};
use chrono::{Datelike, Local, NaiveDate, Timelike};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default)]
pub struct StatsOptions {
    pub dir: String,
    pub days: i64,
    pub author: String,
}

#[derive(Debug, Default, Clone, serde::Serialize)]
pub struct RepoStats {
    pub path: String,
    pub commits: u64,
}

pub fn stats(opts: &StatsOptions) -> Result<()> {
    let author = if opts.author.is_empty() {
        let out = exec_capture("git", &["config", "user.name"]).unwrap_or_default();
        let a = out.trim().to_string();
        if a.is_empty() {
            bail!("无法获取 git 用户名，请用 --author 指定");
        }
        a
    } else {
        opts.author.clone()
    };

    let repos = scan_repos(Path::new(&opts.dir))?;
    if repos.is_empty() {
        println!("未找到 git 仓库");
        return Ok(());
    }

    let since_arg = if opts.days > 0 {
        Some(format!("--since={}.days.ago", opts.days))
    } else {
        None
    };

    let mut total_commits: u64 = 0;
    let mut hours = [0u64; 24];
    let mut weekday_commits = [0u64; 7];
    let mut date_commits: BTreeMap<String, u64> = BTreeMap::new();
    let mut repo_list: Vec<RepoStats> = Vec::new();
    let mut lang_sizes: BTreeMap<String, u64> = BTreeMap::new();

    for repo in &repos {
        let mut args: Vec<String> = vec![
            "-C".into(),
            repo.display().to_string(),
            "log".into(),
            "--author".into(),
            author.clone(),
            "--format=%ad".into(),
            "--date=iso".into(),
            "--no-merges".into(),
        ];
        if let Some(s) = &since_arg {
            args.push(s.clone());
        }
        let refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        let Ok(out) = exec_capture("git", &refs) else { continue };
        if out.trim().is_empty() { continue; }

        let mut count: u64 = 0;
        for line in out.lines() {
            let line = line.trim();
            if line.is_empty() { continue; }
            // "2026-07-17 15:04:05 +0800"
            let dt = chrono::NaiveDateTime::parse_from_str(
                &line.split(" +").next().unwrap_or(line).split(" -").next().unwrap_or(line),
                "%Y-%m-%d %H:%M:%S",
            );
            let Some(t) = dt.ok() else { continue };
            count += 1;
            hours[t.hour() as usize] += 1;
            weekday_commits[t.weekday().num_days_from_sunday() as usize] += 1;
            *date_commits.entry(t.format("%Y-%m-%d").to_string()).or_insert(0) += 1;
        }

        if count > 0 {
            repo_list.push(RepoStats { path: repo.display().to_string(), commits: count });
            total_commits += count;
        }

        // 语言分布（按文件扩展名累计大小）
        let ls = exec_capture(
            "git",
            &["-C", &repo.display().to_string(), "ls-files"],
        )
        .unwrap_or_default();
        for f in ls.lines() {
            let f = f.trim();
            if f.is_empty() { continue; }
            if let Ok(meta) = std::fs::metadata(repo.join(f)) {
                let ext = Path::new(f)
                    .extension()
                    .map(|e| format!(".{}", e.to_string_lossy().to_lowercase()))
                    .unwrap_or_default();
                *lang_sizes.entry(lang_name(&ext)).or_insert(0) += meta.len();
            }
        }
    }

    let streak = calc_streak(&date_commits);
    let longest = calc_longest_streak(&date_commits);
    let active_days = date_commits.len();

    println!();
    println!("  开发统计 ({author}, {} 个仓库)", repo_list.len());
    if opts.days > 0 {
        println!("  最近 {} 天", opts.days);
    }
    println!();
    println!("  总提交:      {total_commits}");
    println!("  活跃天数:    {active_days}");
    println!("  当前连续:    {streak} 天");
    println!("  最长连续:    {longest} 天");
    println!();

    print_weekly_heatmap(&weekday_commits, &hours);

    if !repo_list.is_empty() {
        repo_list.sort_by(|a, b| b.commits.cmp(&a.commits));
        println!("  活跃仓库");
        println!();
        let max = repo_list[0].commits.max(1);
        for (i, r) in repo_list.iter().take(10).enumerate() {
            let name = Path::new(&r.path)
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            let bar_len = (r.commits as f64 / max as f64 * 15.0) as usize;
            println!("  {:2}. {:<25} {:>4} {}", i + 1, truncate(&name, 25), r.commits, "█".repeat(bar_len));
        }
        println!();
    }

    if !lang_sizes.is_empty() {
        println!("  语言分布");
        println!();
        print_lang_distribution(&lang_sizes);
        println!();
    }

    Ok(())
}

fn print_weekly_heatmap(weekdays: &[u64; 7], hours: &[u64; 24]) {
    println!("  活跃时段");
    println!();
    let max_val = hours.iter().copied().max().unwrap_or(0);
    let day_names = ["日", "一", "二", "三", "四", "五", "六"];

    print!("  {:<4}", "");
    let mut h = 0;
    while h < 24 {
        print!("{h:<3}");
        h += 3;
    }
    println!();

    for d in 0..7 {
        print!("  {:<4}", day_names[d]);
        let mut hh = 0;
        while hh < 24 {
            let mut sum = hours[hh];
            if hh + 1 < 24 { sum += hours[hh + 1]; }
            if hh + 2 < 24 { sum += hours[hh + 2]; }
            let cell = if sum == 0 {
                "· ".to_string()
            } else {
                let intensity = sum as f64 / (max_val as f64 + 1.0);
                match intensity {
                    x if x > 0.6 => "██ ",
                    x if x > 0.3 => "█▓ ",
                    x if x > 0.1 => "▓░ ",
                    _ => "░░ ",
                }
                .to_string()
            };
            print!("{cell}"); // + 空格
            hh += 3;
        }
        println!(" ({})", weekdays[d]);
    }
    println!();
}

fn print_lang_distribution(lang_sizes: &BTreeMap<String, u64>) {
    let mut entries: Vec<(&String, &u64)> = lang_sizes.iter().collect();
    entries.sort_by(|a, b| b.1.cmp(a.1));
    let total: u64 = entries.iter().map(|(_, s)| **s).sum();
    let max = entries.first().map(|(_, s)| **s).unwrap_or(1).max(1) as f64;
    for (lang, size) in entries.iter().take(8) {
        let pct = **size as f64 / (total as f64 + 1.0) * 100.0;
        let bar_len = (**size as f64 / max * 15.0) as usize;
        println!("    {:<15} {:>5.1}% {} {}", lang, pct, "█".repeat(bar_len), format_size(**size));
    }
}

fn calc_streak(date_commits: &BTreeMap<String, u64>) -> u64 {
    let mut streak = 0u64;
    let mut day = Local::now().date_naive();
    for _ in 0..365 {
        let key = day.format("%Y-%m-%d").to_string();
        if date_commits.get(&key).copied().unwrap_or(0) > 0 {
            streak += 1;
        } else {
            break;
        }
        day -= chrono::Duration::days(1);
    }
    streak
}

fn calc_longest_streak(date_commits: &BTreeMap<String, u64>) -> u64 {
    if date_commits.is_empty() { return 0; }
    let mut longest = 1u64;
    let mut current = 1u64;
    let mut prev: Option<NaiveDate> = None;
    for key in date_commits.keys() {
        let Ok(curr) = NaiveDate::parse_from_str(key, "%Y-%m-%d") else { continue };
        if let Some(p) = prev {
            if (curr - p).num_days() == 1 {
                current += 1;
                longest = longest.max(current);
            } else {
                current = 1;
            }
        }
        prev = Some(curr);
    }
    longest
}

fn scan_repos(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut repos = Vec::new();
    if dir.join(".git").is_dir() {
        let abs = dir.canonicalize().with_context(|| format!("解析路径失败: {}", dir.display()))?;
        return Ok(vec![abs]);
    }
    let entries = std::fs::read_dir(dir).with_context(|| format!("扫描目录失败: {}", dir.display()))?;
    for e in entries.flatten() {
        if e.path().join(".git").is_dir() {
            repos.push(e.path());
        }
    }
    Ok(repos)
}

fn lang_name(ext: &str) -> String {
    let m = match ext {
        ".go" => "Go", ".rs" => "Rust", ".py" => "Python", ".js" => "JavaScript",
        ".ts" => "TypeScript", ".java" => "Java", ".kt" => "Kotlin", ".c" => "C",
        ".cpp" => "C++", ".h" => "C/C++ Header", ".rb" => "Ruby", ".php" => "PHP",
        ".swift" => "Swift", ".lua" => "Lua", ".sh" => "Shell", ".bat" => "Batch",
        ".html" => "HTML", ".css" => "CSS", ".scss" => "SCSS", ".vue" => "Vue",
        ".sql" => "SQL", ".md" => "Markdown", ".yaml" | ".yml" => "YAML",
        ".json" => "JSON", ".xml" => "XML", ".toml" => "TOML",
        _ => return {
            if ext.is_empty() {
                "Other".into()
            } else {
                let n = ext.trim_start_matches('.');
                let mut c = n.chars();
                match c.next() {
                    Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                    None => "Other".into(),
                }
            }
        },
    };
    m.into()
}

fn format_size(b: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    match b {
        x if x >= MB => format!("{:.1} MB", x as f64 / MB as f64),
        x if x >= KB => format!("{:.1} KB", x as f64 / KB as f64),
        x => format!("{x} B"),
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let t: String = s.chars().take(max.saturating_sub(2)).collect();
        format!("{t}..")
    }
}
