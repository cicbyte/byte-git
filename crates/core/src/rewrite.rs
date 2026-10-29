//! 提交元数据改写：导出 JSON → 编辑 → filter-branch 应用。
//!

use crate::util::{git_dir, run_capture, run_inherit_env};
use anyhow::{bail, Context, Result};
use chrono::{Duration, Local, NaiveDate, TimeZone, Timelike};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CommitMeta {
    #[serde(default)]
    pub author_name: String,
    #[serde(default)]
    pub author_email: String,
    #[serde(default)]
    pub author_date: String,
    #[serde(default)]
    pub committer_name: String,
    #[serde(default)]
    pub committer_email: String,
    #[serde(default)]
    pub committer_date: String,
    /// 完整 commit message（%B），subject+body 不拆分
    #[serde(default)]
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitEntry {
    pub hash: String,
    pub pushed: bool,
    pub original: CommitMeta,
    pub rewrite: CommitMeta,
}

#[derive(Debug, Clone, Default)]
pub struct GenOptions {
    pub repo: String,
    pub range: String,
    pub last: usize,
    pub unpushed: bool,
    pub output: String,
    pub rules: Vec<String>,
    /// "2024-01-01,2024-06-30"
    pub date_range: String,
    /// spread | random
    pub date_mode: String,
    pub daily_limit: i64,
    /// "9-22"
    pub active_hour: String,
}

#[derive(Debug, Clone, Default)]
pub struct ApplyOptions {
    pub repo: String,
    pub branch: String,
    pub file: String,
    pub dry_run: bool,
    pub sync_refs: bool,
}

pub fn default_rewrite_file(repo: &Path) -> Result<std::path::PathBuf> {
    Ok(crate::util::repo_data_dir(repo)?.join("rewrite").join("rewrite.json"))
}

// ---------------------------------------------------------------------------
// 规则
// ---------------------------------------------------------------------------

/// 解析规则并应用到元数据。
///
/// 规则应用格式：
/// - `author:旧名=新名` / `author:=新名`（同时改 committer 名）
/// - `email:旧邮箱=新邮箱` / `email:=新邮箱`
/// - `committer-name:=新名` / `committer-email:=新邮箱`
/// - `date-shift:+2h30m`（h/m/s/d，支持 +/-）
pub fn apply_rule(meta: &mut CommitMeta, rule: &str) {
    let Some(eq) = rule.rfind('=') else { return };
    let left = &rule[..eq];
    let new_val = &rule[eq + 1..];

    let (field, condition) = match left.split_once(':') {
        Some((f, c)) => (f, c),
        None => (left, ""),
    };

    match field {
        "author" => {
            if !condition.is_empty() && meta.author_name != condition {
                return;
            }
            meta.author_name = new_val.into();
            meta.committer_name = new_val.into();
        }
        "email" => {
            if !condition.is_empty() && meta.author_email != condition {
                return;
            }
            meta.author_email = new_val.into();
            meta.committer_email = new_val.into();
        }
        "committer-name" => {
            if !condition.is_empty() && meta.committer_name != condition {
                return;
            }
            meta.committer_name = new_val.into();
        }
        "committer-email" => {
            if !condition.is_empty() && meta.committer_email != condition {
                return;
            }
            meta.committer_email = new_val.into();
        }
        "date-shift" => {
            let Ok(dur) = parse_duration(new_val) else { return };
            meta.author_date = shift_date(&meta.author_date, dur);
            meta.committer_date = shift_date(&meta.committer_date, dur);
        }
        _ => {}
    }
}

/// 解析 +2h / -30m / 1h30m / 2d 等时长（秒）。
pub fn parse_duration(s: &str) -> Result<i64> {
    let negative = s.starts_with('-');
    let s = s.trim_start_matches(['+', '-']);
    let mut total: i64 = 0;
    let mut chars = s.chars().peekable();
    while chars.peek().is_some() {
        let mut num = String::new();
        while let Some(c) = chars.peek() {
            if c.is_ascii_digit() {
                num.push(chars.next().unwrap_or('0'));
            } else {
                break;
            }
        }
        if num.is_empty() {
            bail!("invalid duration: {s}");
        }
        let v: i64 = num.parse().context("invalid number")?;
        let unit = chars.next().ok_or_else(|| anyhow::anyhow!("missing unit: {s}"))?;
        total += match unit {
            'h' => v * 3600,
            'm' => v * 60,
            's' => v,
            'd' => v * 86400,
            other => bail!("unknown unit: {other}"),
        };
    }
    Ok(if negative { -total } else { total })
}

/// 日期平移（保持 `YYYY-MM-DD HH:MM:SS ±ZZ:ZZ` 格式）。
pub fn shift_date(date_str: &str, secs: i64) -> String {
    match parse_date_str(date_str) {
        Some((naive, tz)) => {
            let shifted = naive + Duration::seconds(secs);
            format!("{} {}", shifted.format("%Y-%m-%d %H:%M:%S"), tz)
        }
        None => date_str.to_string(),
    }
}

/// 解析为 (naive datetime, 时区串)。支持带 ±ZZ:ZZ 与不带时区（按本地）。
fn parse_date_str(s: &str) -> Option<(chrono::NaiveDateTime, String)> {
    let s = s.trim();
    if let Some((dt, tz)) = split_tz(s) {
        let naive = chrono::NaiveDateTime::parse_from_str(dt, "%Y-%m-%d %H:%M:%S").ok()?;
        return Some((naive, tz.to_string()));
    }
    for fmt in ["%Y-%m-%d %H:%M:%S", "%Y-%m-%d %H:%M"] {
        if let Ok(naive) = chrono::NaiveDateTime::parse_from_str(s, fmt) {
            return Some((naive, local_tz_str()));
        }
    }
    if let Ok(d) = NaiveDate::parse_from_str(s, "%Y-%m-%d") {
        return Some((d.and_hms_opt(0, 0, 0)?, local_tz_str()));
    }
    None
}

fn split_tz(s: &str) -> Option<(&str, &str)> {
    let idx = s.rfind(' ')?;
    let tz = &s[idx + 1..];
    if tz.starts_with('+') || tz.starts_with('-') {
        Some((&s[..idx], tz))
    } else {
        None
    }
}

fn local_tz_str() -> String {
    let offset = Local::now().offset().local_minus_utc();
    let sign = if offset < 0 { '-' } else { '+' };
    let a = offset.abs();
    format!("{sign}{:02}:{:02}", a / 3600, (a % 3600) / 60)
}

fn extract_timezone(date_str: &str) -> String {
    split_tz(date_str).map(|(_, tz)| tz.to_string()).unwrap_or_else(|| "+00:00".into())
}

fn parse_timestamp(ts: &str) -> String {
    let Ok(secs) = ts.parse::<i64>() else { return ts.to_string() };
    let Some(dt) = Local.timestamp_opt(secs, 0).single() else { return ts.to_string() };
    let offset = dt.offset().local_minus_utc();
    let sign = if offset < 0 { '-' } else { '+' };
    let a = offset.abs();
    format!("{} {sign}{:02}:{:02}", dt.format("%Y-%m-%d %H:%M:%S"), a / 3600, (a % 3600) / 60)
}

// ---------------------------------------------------------------------------
// gen：导出 JSON
// ---------------------------------------------------------------------------

fn get_pushed_hashes(repo: &Path) -> std::collections::HashSet<String> {
    run_capture(repo, ["log", "--format=%H", "--remotes"])
        .map(|out| out.lines().map(|l| l.trim().to_string()).filter(|l| !l.is_empty()).collect())
        .unwrap_or_default()
}

pub fn gen(opts: &GenOptions) -> Result<()> {
    let repo = Path::new(&opts.repo);
    // NUL 分隔字段；%B 完整 message（多行安全）
    let mut log_args: Vec<String> = vec![
        "log".into(),
        "--format=%H%x00%an%x00%ae%x00%at%x00%cn%x00%ce%x00%ct%x00%B%x00".into(),
    ];
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
    let raw = output.trim_end_matches('\n');
    if raw.is_empty() {
        bail!("没有找到提交记录");
    }
    // 记录间分隔：NUL + 换行（git 每条 format 输出后追加 \n）
    let recs: Vec<&str> = raw.split("\0\n").collect();

    let pushed = get_pushed_hashes(repo);
    let mut entries: Vec<CommitEntry> = Vec::new();
    for rec in recs {
        if rec.is_empty() {
            continue;
        }
        let parts: Vec<&str> = rec.split('\0').collect();
        if parts.len() < 8 {
            continue;
        }
        let meta = CommitMeta {
            author_name: parts[1].to_string(),
            author_email: parts[2].to_string(),
            author_date: parse_timestamp(parts[3]),
            committer_name: parts[4].to_string(),
            committer_email: parts[5].to_string(),
            committer_date: parse_timestamp(parts[6]),
            message: parts[7].trim_end_matches('\n').to_string(),
        };
        let mut rewrite = meta.clone();
        for rule in &opts.rules {
            apply_rule(&mut rewrite, rule);
        }
        entries.push(CommitEntry {
            hash: parts[0].to_string(),
            pushed: pushed.contains(parts[0]),
            original: meta,
            rewrite,
        });
    }

    if !opts.date_range.is_empty() {
        redistribute_dates(&mut entries, &opts.date_range, &opts.date_mode, opts.daily_limit, &opts.active_hour)
            .context("日期范围分配失败")?;
    }

    let out = if opts.output.is_empty() {
        default_rewrite_file(repo)?.display().to_string()
    } else {
        opts.output.clone()
    };
    if let Some(dir) = Path::new(&out).parent() {
        crate::util::AppPaths::ensure_dir(dir)?;
    }
    std::fs::write(&out, serde_json::to_string_pretty(&entries)?)
        .with_context(|| format!("写入文件失败: {out}"))?;
    println!("已导出 {} 条提交到 {out}", entries.len());
    Ok(())
}

// ---------------------------------------------------------------------------
// 日期重分配（spread / random）
// ---------------------------------------------------------------------------

pub fn redistribute_dates(
    entries: &mut [CommitEntry],
    date_range: &str,
    mode: &str,
    daily_limit: i64,
    active_hour: &str,
) -> Result<()> {
    let Some((start_s, end_s)) = date_range.split_once(',') else {
        bail!("日期范围格式错误，如 2024-01-01,2024-06-30");
    };
    let (start, _) = parse_date_str(start_s.trim()).ok_or_else(|| anyhow::anyhow!("开始日期格式错误: {start_s}"))?;
    let (end, _) = parse_date_str(end_s.trim()).ok_or_else(|| anyhow::anyhow!("结束日期格式错误: {end_s}"))?;
    if start > end {
        bail!("开始日期不能晚于结束日期");
    }
    let mode = if mode.is_empty() { "spread" } else { mode };

    let (hour_start, hour_end) = if !active_hour.is_empty() {
        parse_active_hour(active_hour)?
    } else {
        let (hs, he) = (start.hour(), end.hour());
        if end.minute() > 0 || end.second() > 0 {
            (hs, (he + 1).min(24))
        } else {
            (hs, he)
        }
    };
    let (hour_start, hour_end) = if hour_end <= hour_start { (0, 24) } else { (hour_start, hour_end) };

    let n = entries.len();
    if n == 0 {
        return Ok(());
    }
    let total_days = ((end - start).num_hours() / 24 + 1).max(1) as usize;

    let day_slots = build_day_slots(start.date(), total_days, daily_limit, hour_start, hour_end);
    let total_slots: usize = day_slots.iter().map(|s| s.len()).sum::<usize>().max(1);

    let dates: Vec<chrono::NaiveDateTime> = match mode {
        "spread" => {
            (0..n)
                .map(|i| {
                    let idx = (i * total_slots / n).min(total_slots - 1);
                    slot_at(&day_slots, idx).unwrap_or(start)
                })
                .collect()
        }
        "random" => {
            let mut idxs: Vec<usize> = (0..n).map(|_| rand_slot(total_slots)).collect();
            idxs.sort_unstable();
            idxs.into_iter()
                .map(|idx| slot_at(&day_slots, idx).unwrap_or(start))
                .collect()
        }
        other => bail!("未知 date-mode: {other}，可选 spread 或 random"),
    };
    let dates = ensure_ascending(dates);

    // entries 为 新→旧；dates 为 旧→新，反转对齐；保留原时区
    for (i, e) in entries.iter_mut().enumerate() {
        let tz = extract_timezone(&e.rewrite.author_date);
        let new_date = format!("{} {tz}", dates[n - 1 - i].format("%Y-%m-%d %H:%M:%S"));
        e.rewrite.author_date = new_date.clone();
        e.rewrite.committer_date = new_date;
    }
    Ok(())
}

fn rand_slot(total: usize) -> usize {
    // 无外部 rand 依赖：用系统时间纳秒做简易熵源（分布要求不高，模式内再排序）
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos() as u64 ^ d.as_secs())
        .unwrap_or(0);
    // xorshift 一次打散
    let mut x = nanos | 1;
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    (x as usize) % total.max(1)
}

fn slot_at(day_slots: &[Vec<chrono::NaiveDateTime>], global_idx: usize) -> Option<chrono::NaiveDateTime> {
    let mut accum = 0;
    for slots in day_slots {
        if accum + slots.len() > global_idx {
            return slots.get(global_idx - accum).copied();
        }
        accum += slots.len();
    }
    None
}

fn parse_active_hour(s: &str) -> Result<(u32, u32)> {
    let Some((a, b)) = s.split_once('-') else {
        bail!("active-hours 格式错误，如 9-22");
    };
    let start: u32 = a.trim().parse().map_err(|_| anyhow::anyhow!("active-hours 起始小时无效: {a}"))?;
    let end: u32 = b.trim().parse().map_err(|_| anyhow::anyhow!("active-hours 结束小时无效: {b}"))?;
    if start > 23 || end < 1 || end > 24 || end <= start {
        bail!("active-hours 无效: {s}（0-23 起，1-24 止，止 > 起）");
    }
    Ok((start, end))
}

fn build_day_slots(
    start: NaiveDate,
    total_days: usize,
    daily_limit: i64,
    hour_start: u32,
    hour_end: u32,
) -> Vec<Vec<chrono::NaiveDateTime>> {
    let count = if daily_limit <= 0 { 100 } else { daily_limit as usize };
    let active_span = ((hour_end - hour_start) * 3600) as i64;
    let base_offset = rand_slot(active_span.max(1) as usize / count.max(1)) as i64;
    (0..total_days)
        .map(|d| {
            let day = start + Duration::days(d as i64);
            (0..count)
                .map(|i| {
                    let sec = hour_start as i64 * 3600 + base_offset + i as i64 * active_span / count as i64;
                    day.and_hms_opt(0, 0, 0)
                        .map(|midnight| midnight + Duration::seconds(sec))
                        .unwrap_or_else(|| day.and_hms_opt(0, 0, 0).unwrap())
                })
                .collect()
        })
        .collect()
}

fn ensure_ascending(mut dates: Vec<chrono::NaiveDateTime>) -> Vec<chrono::NaiveDateTime> {
    for i in 1..dates.len() {
        if dates[i] <= dates[i - 1] {
            dates[i] = dates[i - 1] + Duration::seconds(1);
        }
    }
    dates
}

// ---------------------------------------------------------------------------
// apply：filter-branch 应用
// ---------------------------------------------------------------------------

pub fn apply(opts: &ApplyOptions) -> Result<()> {
    let repo = Path::new(&opts.repo);
    let data = std::fs::read_to_string(&opts.file).context("读取文件失败")?;
    let entries: Vec<CommitEntry> = serde_json::from_str(&data).context("解析 JSON 失败")?;

    let changed: Vec<&CommitEntry> = entries.iter().filter(|e| e.original != e.rewrite).collect();
    if changed.is_empty() {
        println!("没有需要改写的提交");
        return Ok(());
    }

    println!("共 {} 条提交需要改写:", changed.len());
    for e in &changed {
        print!("  {} ", &e.hash[..8]);
        if e.original.author_date != e.rewrite.author_date {
            print!("[日期 {} → {}] ", e.original.author_date, e.rewrite.author_date);
        }
        if e.original.author_name != e.rewrite.author_name {
            print!("[作者 {} → {}] ", e.original.author_name, e.rewrite.author_name);
        }
        if e.original.author_email != e.rewrite.author_email {
            print!("[邮箱 {} → {}] ", e.original.author_email, e.rewrite.author_email);
        }
        if e.original.message != e.rewrite.message {
            print!("[message → {}]", e.rewrite.message.lines().next().unwrap_or(""));
        }
        println!();
    }

    // 当前分支（仅改写本地分支）
    let branch = if opts.branch.is_empty() || opts.branch == "HEAD" {
        run_capture(repo, ["symbolic-ref", "--short", "HEAD"]).context("无法确定当前分支")?.trim().to_string()
    } else {
        opts.branch.clone()
    };
    let local_ref = format!("refs/heads/{branch}");

    // 级联区间计算
    let mut revision_range = local_ref.clone();
    let mut total_affected = changed.len();
    if let Ok(rev) = run_capture(repo, ["rev-list", &local_ref]) {
        let all: Vec<&str> = rev.lines().map(str::trim).filter(|s| !s.is_empty()).collect();
        let set: std::collections::HashSet<&str> = changed.iter().map(|e| e.hash.as_str()).collect();
        for i in (0..all.len()).rev() {
            if set.contains(all[i]) {
                total_affected = i + 1;
                if i < all.len() - 1 {
                    revision_range = format!("{}..{}", all[i + 1], local_ref);
                } else if run_capture(repo, ["rev-parse", "--verify", &format!("{}^", all[i])]).is_ok() {
                    revision_range = format!("{}^..{}", all[i], local_ref);
                } else {
                    revision_range = local_ref.clone(); // 根提交
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

    // 生成 env-filter / msg-filter 脚本，写入 .git 临时文件（避免 Windows 引号/长度问题）
    let env_script = build_env_filter(&changed.iter().map(|e| (*e).clone()).collect::<Vec<_>>());
    let msg_script = build_msg_filter(&changed.iter().map(|e| (*e).clone()).collect::<Vec<_>>());
    let gd = git_dir(repo)?;
    let env_path = gd.join("byte-git-env-filter.sh");
    std::fs::write(&env_path, &env_script).context("写入脚本文件失败")?;
    let env_posix = to_posix(&env_path);

    let msg_posix = if !msg_script.is_empty() {
        let p = gd.join("byte-git-msg-filter.sh");
        std::fs::write(&p, &msg_script).context("写入 msg 脚本失败")?;
        Some(to_posix(&p))
    } else {
        None
    };

    println!("\n开始执行 filter-branch...");
    let result = (|| -> Result<()> {
        let mut args: Vec<String> = vec![
            "filter-branch".into(),
            "-f".into(),
            "--env-filter".into(),
            format!(". {env_posix}"),
        ];
        if let Some(mp) = &msg_posix {
            args.push("--msg-filter".into());
            args.push(format!(". {mp}"));
        }
        args.push("--original".into());
        args.push("refs/original/rewrite".into());
        args.push("--".into());
        args.push(revision_range.clone());
        run_inherit_env(repo, args, &[("FILTER_BRANCH_SQUELCH_WARNING", "1")])
            .context("filter-branch 执行失败")
    })();

    // 清理脚本与备份 ref
    let _ = std::fs::remove_file(&env_path);
    if msg_posix.is_some() {
        let _ = std::fs::remove_file(gd.join("byte-git-msg-filter.sh"));
    }
    let _ = run_capture(repo, ["update-ref", "-d", "refs/original/rewrite"]);
    result?;

    if opts.sync_refs {
        if let Err(e) = sync_remote_refs(repo, &branch) {
            println!("警告: 同步远程追踪 ref 失败: {e:#}");
        }
    }

    println!("\n改写完成！");
    Ok(())
}

fn to_posix(p: &Path) -> String {
    // 绝对路径 + 正斜杠（filter-branch 在临时工作目录执行，相对路径不可靠）
    let abs = std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
    abs.to_string_lossy().replace('\\', "/")
}

fn sh_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

fn build_env_filter(entries: &[CommitEntry]) -> String {
    let mut buf = String::from("case \"$GIT_COMMIT\" in\n");
    for e in entries {
        buf.push_str(&format!("  {})\n", e.hash));
        if e.original.author_name != e.rewrite.author_name {
            buf.push_str(&format!("    export GIT_AUTHOR_NAME={}\n", sh_quote(&e.rewrite.author_name)));
        }
        if e.original.author_email != e.rewrite.author_email {
            buf.push_str(&format!("    export GIT_AUTHOR_EMAIL={}\n", sh_quote(&e.rewrite.author_email)));
        }
        if e.original.author_date != e.rewrite.author_date {
            buf.push_str(&format!("    export GIT_AUTHOR_DATE={}\n", sh_quote(&e.rewrite.author_date)));
        }
        if e.original.committer_name != e.rewrite.committer_name {
            buf.push_str(&format!("    export GIT_COMMITTER_NAME={}\n", sh_quote(&e.rewrite.committer_name)));
        }
        if e.original.committer_email != e.rewrite.committer_email {
            buf.push_str(&format!("    export GIT_COMMITTER_EMAIL={}\n", sh_quote(&e.rewrite.committer_email)));
        }
        if e.original.committer_date != e.rewrite.committer_date {
            buf.push_str(&format!("    export GIT_COMMITTER_DATE={}\n", sh_quote(&e.rewrite.committer_date)));
        }
        buf.push_str("    ;;\n");
    }
    buf.push_str("esac\n");
    buf
}

/// msg-filter 脚本：heredoc 输出多行 message；无改动返回空串。
fn build_msg_filter(entries: &[CommitEntry]) -> String {
    let pairs: Vec<(&str, &str)> = entries
        .iter()
        .filter(|e| e.original.message != e.rewrite.message && !e.rewrite.message.is_empty())
        .map(|e| (e.hash.as_str(), e.rewrite.message.as_str()))
        .collect();
    if pairs.is_empty() {
        return String::new();
    }
    let mut buf = String::from("case \"$GIT_COMMIT\" in\n");
    for (hash, msg) in pairs {
        buf.push_str(&format!("  {hash})\n    cat <<'BYTEGIT_EOF'\n{msg}\nBYTEGIT_EOF\n    ;;\n"));
    }
    buf.push_str("  *)\n    cat\n    ;;\nesac\n");
    buf
}

fn sync_remote_refs(repo: &Path, branch: &str) -> Result<()> {
    let head = run_capture(repo, ["rev-parse", &format!("refs/heads/{branch}")])?
        .trim()
        .to_string();
    let remotes = run_capture(repo, ["remote"])?;
    let mut synced = Vec::new();
    for r in remotes.lines().map(str::trim).filter(|s| !s.is_empty()) {
        let remote_ref = format!("refs/remotes/{r}/{branch}");
        if run_capture(repo, ["rev-parse", "--verify", &remote_ref]).is_err() {
            continue;
        }
        if run_capture(repo, ["update-ref", &remote_ref, &head]).is_ok() {
            synced.push(remote_ref);
        }
    }
    if !synced.is_empty() {
        println!("已同步远程追踪 ref: {}", synced.join(", "));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn duration_parsing() {
        assert_eq!(parse_duration("+2h").unwrap(), 7200);
        assert_eq!(parse_duration("-30m").unwrap(), -1800);
        assert_eq!(parse_duration("1h30m").unwrap(), 5400);
        assert_eq!(parse_duration("2d").unwrap(), 172800);
        assert!(parse_duration("abc").is_err());
    }

    #[test]
    fn rules_apply() {
        let mut m = CommitMeta {
            author_name: "old".into(),
            author_email: "o@x.com".into(),
            ..Default::default()
        };
        apply_rule(&mut m, "author:old=new");
        assert_eq!(m.author_name, "new");
        assert_eq!(m.committer_name, "new");
        // 条件不匹配：不改
        apply_rule(&mut m, "email:wrong=right@x.com");
        assert_eq!(m.author_email, "o@x.com");
        // 无条件替换
        apply_rule(&mut m, "email:=right@x.com");
        assert_eq!(m.author_email, "right@x.com");
    }

    #[test]
    fn date_shift_keeps_format() {
        let shifted = shift_date("2024-01-01 10:00:00 +08:00", 3600);
        assert!(shifted.starts_with("2024-01-01 11:00:00 +08:00"), "{shifted}");
    }
}
