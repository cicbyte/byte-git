//! rewrite 域命令：AI 提交信息改写 + 元数据改写 + JSON 编辑器。
//!
//! 移植自 git-mate `gui/internal/rewrite`（rewrite.go / metarewrite.go /
//! metarewrite_rules.go / jsoneditor.go）。行为语义与 Go 版保持一致；
//! 仓库级数据路径复用 byte-git-core 约定（`.git/byte-git/rewrite/...`）。

use byte_git_core::ai::{self, ChatClient, ChatMessage};
use byte_git_core::airewrite;
use byte_git_core::backup;
use byte_git_core::config::Settings;
use byte_git_core::rewrite::{self, CommitEntry, CommitMeta};
use byte_git_core::util::{git_dir, run_capture};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;
use tauri::Emitter;

fn repo_path(repo: Option<String>) -> PathBuf {
    crate::state::resolve(repo)
}

// ---------------------------------------------------------------------------
// DTO（json tag 与 Go 版对齐，camelCase）
// ---------------------------------------------------------------------------

/// RewriteEntryDTO：前端视角的 airewrite.AICommitEntry。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RewriteEntryDto {
    #[serde(default)]
    pub hash: String,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub date: String,
    #[serde(default)]
    pub original: String,
    #[serde(default)]
    pub suggested: String,
    /// pending / suggested / approved / skipped / retry
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub need_ai: bool,
    #[serde(default)]
    pub pushed: bool,
    #[serde(default)]
    pub files: Vec<String>,
    #[serde(default)]
    pub diff_summary: String,
    #[serde(default)]
    pub diff: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RewriteEntryDiffDto {
    #[serde(default)]
    pub diff_summary: String,
    #[serde(default)]
    pub files: Vec<String>,
    #[serde(default)]
    pub diff: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DetectRuleDto {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub pattern: String,
    #[serde(default)]
    pub desc: String,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RewriteGenerateAllResult {
    pub hash: String,
    pub suggested: String,
    pub ok: bool,
    pub error: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RewriteApplyPreviewDto {
    pub entries: Vec<RewriteEntryDto>,
    pub total_affected: i64,
    pub extra_affected: i64,
    pub range: String,
}

/// CommitMetaDTO（Go metarewrite.go）。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitMetaDto {
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
    #[serde(default)]
    pub message: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MetaEntryDto {
    #[serde(default)]
    pub hash: String,
    #[serde(default)]
    pub pushed: bool,
    #[serde(default)]
    pub original: CommitMetaDto,
    #[serde(default)]
    pub rewrite: CommitMetaDto,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DailyAllocDto {
    #[serde(default)]
    pub date: String,
    #[serde(default)]
    pub count: i64,
}

/// MetaExportRemap：导出向导的可选日期重映射配置。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MetaExportRemap {
    #[serde(default)]
    pub date_range: String,
    #[serde(default)]
    pub mode: String,
    #[serde(default)]
    pub daily_limit: i64,
    #[serde(default)]
    pub active_hour: String,
    #[serde(default)]
    pub allocs: Vec<DailyAllocDto>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MetaExportOptions {
    #[serde(default)]
    pub mode: String,
    #[serde(default)]
    pub n: i64,
    #[serde(default)]
    pub skip: i64,
    #[serde(default)]
    pub rules: Vec<String>,
    #[serde(default)]
    pub remap: Option<MetaExportRemap>,
    #[serde(default)]
    pub persist: bool,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitCountResult {
    pub total: i64,
    pub unpushed: i64,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FieldDiff {
    pub field: String,
    pub old: String,
    pub new: String,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewChangeEntry {
    pub hash: String,
    pub pushed: bool,
    pub diffs: Vec<FieldDiff>,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MetaRewritePreviewDto {
    pub changed: Vec<PreviewChangeEntry>,
    pub total_affected: i64,
    pub extra_affected: i64,
    pub range: String,
    pub pushed_changed: i64,
}

// ---------------------------------------------------------------------------
// 基础 git 辅助
// ---------------------------------------------------------------------------

fn pushed_hashes(repo: &Path) -> HashSet<String> {
    run_capture(repo, ["log", "--format=%H", "--remotes"])
        .map(|out| out.lines().map(|l| l.trim().to_string()).filter(|l| !l.is_empty()).collect())
        .unwrap_or_default()
}

/// 按字节截断（尊重 UTF-8 字符边界），尾部追加截断标记。
fn truncate_bytes(s: &str, max: usize) -> String {
    if s.len() <= max {
        return s.to_string();
    }
    let mut end = max.min(s.len());
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}\n... (已截断)", &s[..end])
}

/// 移植 airewrite.GetCommitDiff：`git diff-tree -p <hash>`，超过 maxChars 截断。
fn get_commit_diff(repo: &Path, hash: &str, max_chars: usize) -> Result<String, String> {
    let out = run_capture(repo, ["diff-tree", "-p", hash]).map_err(|e| format!("git diff-tree 失败: {e:#}"))?;
    Ok(truncate_bytes(&out, max_chars))
}

/// 移植 airewrite.GetCommitStat：返回 (shortstat 摘要, 变更文件列表)。
fn get_commit_stat(repo: &Path, hash: &str) -> Result<(String, Vec<String>), String> {
    let out = run_capture(repo, ["diff-tree", "--stat", "--no-commit-id", "-r", hash])
        .map_err(|e| format!("git diff-tree --stat 失败: {e:#}"))?;
    let mut files = Vec::new();
    for line in out.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('|') {
            continue;
        }
        // 最后一行是统计摘要，如 "5 files changed, 145 insertions(+), 23 deletions(-)"
        if line.contains("file") && line.contains("changed") {
            continue;
        }
        if let Some(first) = line.split_whitespace().next() {
            files.push(first.to_string());
        }
    }
    // shortstat 失败时 Go 版静默返回空摘要
    let summary = run_capture(repo, ["diff-tree", "--shortstat", "--no-commit-id", "-r", hash])
        .map(|s| s.trim().to_string())
        .unwrap_or_default();
    Ok((summary, files))
}

/// 移植 commit.CleanCommitMessage：剥离 markdown 代码块包裹与首尾引号。
fn clean_commit_message(s: &str) -> String {
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
        let (a, b) = (bytes[0], bytes[bytes.len() - 1]);
        if (a == b'"' && b == b'"') || (a == b'\'' && b == b'\'') {
            s = s[1..s.len() - 1].to_string();
        }
    }
    s
}

/// 移植 airewrite.BuildPrompt：airewrite.md 模板 + 占位符替换 + 长度截断。
fn build_prompt(original: &str, files: &[String], diff_summary: &str, diff: &str, lang: &str, max_chars: usize) -> String {
    let lang_str = if lang == "en" { "English" } else { "中文" };
    let tpl = ai::get_prompt_file("airewrite.md")
        .unwrap_or_else(|_| "你是一个 Git 提交消息专家。根据 diff 改写 commit message。".into());
    let mut s = tpl
        .replace("{{language}}", lang_str)
        .replace("{{original}}", original)
        .replace("{{files}}", &files.join(", "))
        .replace("{{diff_summary}}", diff_summary)
        .replace("{{diff}}", diff);
    if max_chars > 0 {
        s = truncate_bytes(&s, max_chars);
    }
    s
}

/// 本地时间戳（用于 AI 缓存条目）：借 git 的 format-local 日期输出。
fn local_now_timestamp(repo: &Path) -> String {
    run_capture(repo, ["log", "-1", "--date=format-local:%Y-%m-%dT%H:%M:%S", "--format=%cd"])
        .map(|s| s.trim().to_string())
        .unwrap_or_default()
}

/// AI 缓存写入：`<git-dir>/byte-git/ai-cache/<hash>.json`（与 core airewrite 一致）。
fn write_ai_cache(repo: &Path, hash: &str, suggested: &str, model: &str) {
    let Ok(dir) = byte_git_core::util::repo_data_dir(repo) else { return };
    let dir = dir.join("ai-cache");
    if std::fs::create_dir_all(&dir).is_err() {
        return;
    }
    let entry = serde_json::json!({
        "hash": hash,
        "suggested": suggested,
        "model": model,
        "timestamp": local_now_timestamp(repo),
    });
    let _ = std::fs::write(dir.join(format!("{hash}.json")), serde_json::to_string_pretty(&entry).unwrap_or_default());
}

// ---------------------------------------------------------------------------
// 纯 Rust 日期运算（无 chrono 依赖；Howard Hinnant civil 算法）
// ---------------------------------------------------------------------------

fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m, d)
}

fn tz_to_secs(tz: &str) -> Option<i64> {
    let b = tz.as_bytes();
    if b.len() < 5 {
        return None;
    }
    let sign = match b[0] {
        b'+' => 1i64,
        b'-' => -1i64,
        _ => return None,
    };
    let hh: i64 = tz[1..3].parse().ok()?;
    let mm: i64 = tz[tz.len() - 2..].parse().ok()?;
    Some(sign * (hh * 3600 + mm * 60))
}

/// 解析 "YYYY-MM-DD HH:MM:SS [±HH:MM|±HHMM]" → (墙上时钟秒, 时区偏移秒, 时区串)。
/// 无时区时偏移为 0、时区串为 "+00:00"（与 Go extractTimezone 回退一致）。
fn parse_meta_date(s: &str) -> Option<(i64, i64, String)> {
    let s = s.trim();
    let (main, tz) = match s.rfind(' ') {
        Some(i) => {
            let tz = &s[i + 1..];
            if (tz.starts_with('+') || tz.starts_with('-')) && tz.len() >= 5 {
                (&s[..i], tz.to_string())
            } else {
                (s, String::new())
            }
        }
        None => (s, String::new()),
    };
    let off = if tz.is_empty() { 0 } else { tz_to_secs(&tz)? };
    let tz = if tz.is_empty() { "+00:00".to_string() } else { tz };
    let (ymd, hms) = main.split_once(' ')?;
    let mut dit = ymd.split('-');
    let y: i64 = dit.next()?.parse().ok()?;
    let m: i64 = dit.next()?.parse().ok()?;
    let d: i64 = dit.next()?.parse().ok()?;
    let mut tit = hms.split(':');
    let hh: i64 = tit.next()?.parse().ok()?;
    let mi: i64 = tit.next()?.parse().ok()?;
    let se: i64 = match tit.next() {
        Some(s) => s.parse().ok()?,
        None => 0,
    };
    let wall = days_from_civil(y, m, d) * 86400 + hh * 3600 + mi * 60 + se;
    Some((wall, off, tz))
}

/// 墙上时钟秒 → "YYYY-MM-DD HH:MM:SS ±HH:MM"。
fn format_wall(wall: i64, tz: &str) -> String {
    let days = wall.div_euclid(86400);
    let rem = wall.rem_euclid(86400);
    let (y, m, d) = civil_from_days(days);
    format!(
        "{y:04}-{m:02}-{d:02} {:02}:{:02}:{:02} {tz}",
        rem / 3600,
        (rem % 3600) / 60,
        rem % 60
    )
}

/// "2026-06-23" → 当天 0 点的墙上时钟秒。
fn parse_ymd(s: &str) -> Option<i64> {
    let mut it = s.trim().split('-');
    let y: i64 = it.next()?.parse().ok()?;
    let m: i64 = it.next()?.parse().ok()?;
    let d: i64 = it.next()?.parse().ok()?;
    if it.next().is_some() {
        return None;
    }
    Some(days_from_civil(y, m, d) * 86400)
}

/// 取日期串中的时区（无时区回退 "+00:00"，对齐 Go extractTimezone）。
fn extract_timezone(date_str: &str) -> String {
    if let Some(idx) = date_str.rfind(' ') {
        let tz = &date_str[idx + 1..];
        if tz.starts_with('-') || tz.starts_with('+') {
            return tz.to_string();
        }
    }
    "+00:00".into()
}

/// "+0800" → "+08:00"（git %z 输出转 Go time.Format 布局）。
fn colon_tz(tz: &str) -> String {
    if tz.len() == 5 && (tz.starts_with('+') || tz.starts_with('-')) {
        format!("{}:{}", &tz[..3], &tz[3..])
    } else {
        tz.to_string()
    }
}

fn parse_active_hour(s: &str) -> Result<(u32, u32), String> {
    let Some((a, b)) = s.split_once('-') else {
        return Err("active-hours 格式错误，如 9-22".into());
    };
    let start: u32 = a.trim().parse().map_err(|_| format!("active-hours 起始小时无效: {a}"))?;
    let end: u32 = b.trim().parse().map_err(|_| format!("active-hours 结束小时无效: {b}"))?;
    if start > 23 || end < 1 || end > 24 || end <= start {
        return Err(format!("active-hours 无效: {s}（0-23 起，1-24 止，止 > 起）"));
    }
    Ok((start, end))
}

/// 简易 xorshift PRNG（无外部 rand 依赖）。
struct Rng(u64);

impl Rng {
    fn new() -> Self {
        let n = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| (d.as_nanos() as u64) ^ (d.as_secs().wrapping_mul(0x9E3779B9)))
            .unwrap_or(0x9E3779B97F4A7C15);
        Rng(n | 1)
    }
    fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
    fn below(&mut self, n: u64) -> u64 {
        if n <= 1 { 0 } else { self.next_u64() % n }
    }
}

fn shuffle(v: &mut [i64], rng: &mut Rng) {
    for i in (1..v.len()).rev() {
        let j = rng.below((i + 1) as u64) as usize;
        v.swap(i, j);
    }
}

fn ensure_ascending(v: &mut [i64]) {
    for i in 1..v.len() {
        if v[i] <= v[i - 1] {
            v[i] = v[i - 1] + 1;
        }
    }
}

/// 单日时间槽：day 为当天 0 点墙上时钟秒。
fn build_day_slots(day: i64, count: i64, hour_start: u32, hour_end: u32, rng: &mut Rng) -> Vec<i64> {
    let hs = hour_start as i64;
    let he = hour_end as i64;
    let count = count.max(1);
    let active_span = (he - hs) * 3600;
    let denom = (active_span / count).max(1);
    let base_offset = rng.below(denom as u64) as i64;
    (0..count)
        .map(|i| day + hs * 3600 + base_offset + i * active_span / count)
        .collect()
}

/// 移植 rewrite.DistributeByDays：按逐日分配重写 entries 日期。
fn distribute_by_days(
    entries: &mut [CommitEntry],
    allocs: &[DailyAllocDto],
    mode: &str,
    active_hour: &str,
) -> Result<(), String> {
    let n = entries.len();
    if n == 0 {
        return Ok(());
    }
    if allocs.is_empty() {
        return Err("未配置任何天数的分配".into());
    }
    let (hour_start, hour_end) = if !active_hour.is_empty() {
        parse_active_hour(active_hour)?
    } else {
        (0, 24)
    };
    let mode = if mode.is_empty() { "spread" } else { mode };

    // 解析并按日期升序排序
    let mut parsed: Vec<(i64, i64)> = Vec::new();
    for a in allocs {
        if a.count <= 0 {
            continue;
        }
        let day = parse_ymd(&a.date)
            .ok_or_else(|| format!("日期格式错误 {:?}（期望 2026-06-23）", a.date))?;
        parsed.push((day, a.count));
    }
    if parsed.is_empty() {
        return Err("所有天数的提交数都为 0".into());
    }
    parsed.sort_by_key(|(d, _)| *d);

    let mut rng = Rng::new();
    let mut all_slots: Vec<i64> = Vec::new();
    for &(day, count) in &parsed {
        all_slots.extend(build_day_slots(day, count, hour_start, hour_end, &mut rng));
    }

    if all_slots.len() > n {
        all_slots.truncate(n);
    } else if all_slots.len() < n {
        // 不足：把剩余条目按比例摊分到各天
        let remaining = (n - all_slots.len()) as i64;
        all_slots.clear();
        let total_base: i64 = parsed.iter().map(|(_, c)| c).sum();
        let base = total_base.max(1);
        let mut allocated = 0i64;
        let mut extra = vec![0i64; parsed.len()];
        for i in 0..parsed.len() {
            if i == parsed.len() - 1 {
                extra[i] = remaining - allocated;
            } else {
                extra[i] = remaining * parsed[i].1 / base;
                allocated += extra[i];
            }
        }
        for (i, &(day, count)) in parsed.iter().enumerate() {
            let c = count + extra[i];
            if c <= 0 {
                continue;
            }
            all_slots.extend(build_day_slots(day, c, hour_start, hour_end, &mut rng));
        }
        // 仍不足则强制补齐（理论上不会发生）
        while all_slots.len() < n {
            let day0 = parsed[0].0;
            all_slots.push(day0 + all_slots.len() as i64);
        }
    }

    if mode == "random" {
        shuffle(&mut all_slots, &mut rng);
    }
    all_slots.sort_unstable();
    ensure_ascending(&mut all_slots);

    // entries 来自 git log 是 新→旧；all_slots 是 旧→新，对齐反转
    for (i, e) in entries.iter_mut().enumerate() {
        let tz = extract_timezone(&e.rewrite.author_date);
        let new_date = format_wall(all_slots[n - 1 - i], &tz);
        e.rewrite.author_date = new_date.clone();
        e.rewrite.committer_date = new_date;
    }
    Ok(())
}

/// 取 git 父提交（hash^）的绝对时间（解析失败视为无约束）。
fn get_parent_commit_date(repo: &Path, hash: &str) -> Option<i64> {
    if hash.is_empty() {
        return None;
    }
    let out = run_capture(repo, ["log", "-1", "--format=%ai", &format!("{hash}^")]).ok()?;
    let s = out.trim();
    if s.is_empty() {
        return None;
    }
    let (wall, off, _tz) = parse_meta_date(s)?;
    Some(wall - off)
}

/// 移植 rewrite.ClampToParent：窗口内日期不早于 git 父提交（整体平移）。
fn clamp_to_parent(entries: &mut [CommitEntry], parent_abs: i64) {
    if entries.is_empty() {
        return;
    }
    let last = entries[entries.len() - 1].rewrite.author_date.clone();
    let Some((earliest_wall, earliest_off, _)) = parse_meta_date(&last) else { return };
    let earliest_abs = earliest_wall - earliest_off;
    if earliest_abs >= parent_abs {
        return;
    }
    let shift = parent_abs + 1 - earliest_abs;
    for e in entries.iter_mut() {
        if let Some((wall, _off, tz)) = parse_meta_date(&e.rewrite.author_date) {
            let nd = format_wall(wall + shift, &tz);
            e.rewrite.author_date = nd.clone();
            e.rewrite.committer_date = nd;
        }
    }
}

// ---------------------------------------------------------------------------
// filter-branch 脚本构造（移植 internal/rewrite rules.go，引号策略与 core 一致）
// ---------------------------------------------------------------------------

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

/// AI 改写的 msg-filter（8 位 hash 前缀 case，对齐 buildMsgFilterGUI）。
fn build_msg_filter_gui(entries: &[(String, String)]) -> String {
    let mut buf = String::from("case \"$GIT_COMMIT\" in\n");
    for (hash, suggested) in entries {
        if hash.len() < 8 {
            continue;
        }
        let prefix = &hash[..8];
        let msg = suggested.replace('\'', "'\\''");
        buf.push_str(&format!("  {prefix}*)\n    echo '{msg}'\n    ;;\n"));
    }
    buf.push_str("  *)\n    cat\n    ;;\nesac\n");
    buf
}

fn to_posix(p: &Path) -> String {
    let abs = std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
    abs.to_string_lossy().replace('\\', "/")
}

/// 执行 git filter-branch（捕获合并输出，附加 FILTER_BRANCH_SQUELCH_WARNING）。
fn run_filter_branch(repo: &Path, args: &[String]) -> Result<(), String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .env("FILTER_BRANCH_SQUELCH_WARNING", "1")
        .output()
        .map_err(|e| format!("启动 git filter-branch 失败: {e}"))?;
    if !out.status.success() {
        let mut combined = String::from_utf8_lossy(&out.stdout).into_owned();
        combined.push_str(&String::from_utf8_lossy(&out.stderr));
        return Err(format!(
            "filter-branch 失败: exit status {:?}: {}",
            out.status.code(),
            combined.trim()
        ));
    }
    Ok(())
}

fn sync_remote_refs_gui(repo: &Path, branch: &str) {
    let Ok(head) = run_capture(repo, ["rev-parse", &format!("refs/heads/{branch}")]) else { return };
    let head = head.trim().to_string();
    let Ok(remotes) = run_capture(repo, ["remote"]) else { return };
    for r in remotes.lines().map(str::trim).filter(|s| !s.is_empty()) {
        let remote_ref = format!("refs/remotes/{r}/{branch}");
        if run_capture(repo, ["rev-parse", "--verify", &remote_ref]).is_err() {
            continue;
        }
        let _ = run_capture(repo, ["update-ref", &remote_ref, &head]);
    }
}

fn create_auto_backup(repo: &Path) -> Result<(), String> {
    // 与 Go CreateBackupFull 等价：整库克隆备份（core 为固定路径镜像，update 增量刷新）
    backup::create(&repo.to_string_lossy(), true)
        .map(|_| ())
        .map_err(|e| format!("整库备份失败（已中止改写）: {e:#}"))
}

// ---------------------------------------------------------------------------
// DTO ↔ core 转换
// ---------------------------------------------------------------------------

fn meta_to_dto(m: &CommitMeta) -> CommitMetaDto {
    CommitMetaDto {
        author_name: m.author_name.clone(),
        author_email: m.author_email.clone(),
        author_date: m.author_date.clone(),
        committer_name: m.committer_name.clone(),
        committer_email: m.committer_email.clone(),
        committer_date: m.committer_date.clone(),
        message: m.message.clone(),
    }
}

fn dto_to_meta(d: &CommitMetaDto) -> CommitMeta {
    CommitMeta {
        author_name: d.author_name.clone(),
        author_email: d.author_email.clone(),
        author_date: d.author_date.clone(),
        committer_name: d.committer_name.clone(),
        committer_email: d.committer_email.clone(),
        committer_date: d.committer_date.clone(),
        message: d.message.clone(),
    }
}

fn entry_to_dto(e: &CommitEntry) -> MetaEntryDto {
    MetaEntryDto {
        hash: e.hash.clone(),
        pushed: e.pushed,
        original: meta_to_dto(&e.original),
        rewrite: meta_to_dto(&e.rewrite),
    }
}

fn dto_to_entry(d: &MetaEntryDto) -> CommitEntry {
    CommitEntry {
        hash: d.hash.clone(),
        pushed: d.pushed,
        original: dto_to_meta(&d.original),
        rewrite: dto_to_meta(&d.rewrite),
    }
}

/// 改写数据落盘路径：<git-dir>/byte-git/rewrite/rewrite.json（与 core 约定一致）。
fn rewrite_json_file(repo: &Path) -> Result<PathBuf, String> {
    rewrite::default_rewrite_file(repo).map_err(|e| format!("{e:#}"))
}

// ---------------------------------------------------------------------------
// AI 改写命令（rewrite.go）
// ---------------------------------------------------------------------------

/// RewriteExport：导出提交清单（仅元数据，diff 懒加载）。
/// mode: "all" | "last" | "unpushed" | "bad"；n 为 "last" 的条数，skip 为分页偏移。
#[tauri::command]
pub fn rewrite_export(repo: Option<String>, mode: String, n: i64, skip: i64) -> Result<Vec<RewriteEntryDto>, String> {
    let repo = repo_path(repo);
    let mut log_args: Vec<String> = vec![
        "log".into(),
        "--format=%H%x00%an%x00%ai%x00%B%x00".into(),
    ];
    match mode.as_str() {
        "last" => {
            let n = if n <= 0 { 20 } else { n };
            log_args.push(format!("-{n}"));
            if skip > 0 {
                log_args.push(format!("--skip={skip}"));
            }
        }
        "unpushed" => {
            log_args.push("--branches".into());
            log_args.push("--not".into());
            log_args.push("--remotes".into());
        }
        _ => {} // "all" / "bad"：拉全量（bad 在下方按规则过滤）
    }

    let output = run_capture(&repo, log_args).map_err(|e| format!("git log 失败: {e:#}"))?;
    let raw = output.trim_end_matches('\n');
    if raw.is_empty() {
        return Err("没有找到提交记录".into());
    }
    let recs: Vec<&str> = raw.split("\0\n").collect();
    if recs.is_empty() {
        return Err("没有找到提交记录".into());
    }

    let pushed = pushed_hashes(&repo);
    let rules = airewrite::load_rules().unwrap_or_default();
    let mut dtos = Vec::with_capacity(recs.len());
    for rec in recs {
        if rec.is_empty() {
            continue;
        }
        let parts: Vec<&str> = rec.split('\0').collect();
        if parts.len() < 4 {
            continue;
        }
        let hash = parts[0];
        let full_msg = parts[3].trim_end_matches('\n');
        let subject = full_msg.split('\n').next().unwrap_or("");
        let need_ai = airewrite::match_any(subject, &rules);
        if mode == "bad" && !need_ai {
            continue;
        }
        dtos.push(RewriteEntryDto {
            hash: hash.to_string(),
            author: parts[1].to_string(),
            date: parts[2].to_string(),
            original: full_msg.to_string(),
            suggested: String::new(),
            status: "pending".into(),
            need_ai,
            pushed: pushed.contains(hash),
            files: Vec::new(),
            diff_summary: String::new(),
            diff: String::new(),
        });
    }
    Ok(dtos)
}

/// RewriteEntryDiff：按需加载单条提交的 diff + stat。
#[tauri::command]
pub fn rewrite_entry_diff(repo: Option<String>, hash: String) -> Result<RewriteEntryDiffDto, String> {
    let repo = repo_path(repo);
    let (diff_summary, files) = get_commit_stat(&repo, &hash).map_err(|e| format!("获取 stat 失败: {e}"))?;
    let diff = get_commit_diff(&repo, &hash, 4000).map_err(|e| format!("获取 diff 失败: {e}"))?;
    Ok(RewriteEntryDiffDto { diff_summary, files, diff })
}

/// RewriteRulesLoad：读取用户检测规则。
#[tauri::command]
pub fn rewrite_rules_load() -> Result<Vec<DetectRuleDto>, String> {
    let rules = airewrite::load_rules().map_err(|e| format!("{e:#}"))?;
    Ok(rules.into_iter().map(|r| DetectRuleDto { name: r.name, pattern: r.pattern, desc: r.desc }).collect())
}

/// RewriteRulesSave：保存用户检测规则。
#[tauri::command]
pub fn rewrite_rules_save(rules: Vec<DetectRuleDto>) -> Result<(), String> {
    let internal: Vec<airewrite::DetectRule> = rules
        .into_iter()
        .map(|r| airewrite::DetectRule { name: r.name, pattern: r.pattern, desc: r.desc })
        .collect();
    airewrite::save_rules(&internal).map_err(|e| format!("{e:#}"))
}

/// RewriteRulesReset：恢复默认规则并返回。
#[tauri::command]
pub fn rewrite_rules_reset() -> Result<Vec<DetectRuleDto>, String> {
    let defaults = airewrite::default_rules();
    airewrite::save_rules(&defaults).map_err(|e| format!("{e:#}"))?;
    Ok(defaults.into_iter().map(|r| DetectRuleDto { name: r.name, pattern: r.pattern, desc: r.desc }).collect())
}

/// RewriteRuleGenerate：用 AI 根据自然语言描述生成正则检测规则。
#[tauri::command]
pub fn rewrite_rule_generate(desc: String) -> Result<DetectRuleDto, String> {
    let desc = desc.trim().to_string();
    if desc.is_empty() {
        return Err("描述不能为空".into());
    }
    let cfg = Settings::load().map_err(|e| format!("读取配置失败: {e:#}"))?;

    let prompt = format!(
        "你是一个正则表达式专家。用户想创建一条 commit message 检测规则，\
         请根据描述生成一个 Go (RE2) 兼容的正则表达式，用于 MatchString 匹配不规范的 commit subject。\n\n\
         用户描述: {desc}\n\n\
         要求:\n\
         1. 只返回 JSON，不要任何额外文字或 markdown 代码块\n\
         2. 格式: {{\"name\":\"简短中文名\",\"pattern\":\"正则\",\"desc\":\"说明\"}}\n\
         3. pattern 不要加前后斜杠\n\
         4. 正则要匹配的是「不规范」的提交，而非规范的"
    );

    let client = ChatClient::new(&cfg.ai.base_url, &cfg.ai.api_key, &cfg.ai.model);
    let raw = client
        .chat(&[ChatMessage::user(prompt)])
        .map_err(|e| format!("AI 生成失败: {e:#}"))?;

    let raw = clean_commit_message(&raw);
    let rule: DetectRuleDto = serde_json::from_str(&raw)
        .map_err(|e| format!("解析 AI 返回失败: {e} (raw: {raw})"))?;
    if rule.pattern.is_empty() {
        return Err("AI 返回的正则为空".into());
    }
    Ok(rule)
}

/// RewriteGenerate：为单条提交生成 AI 建议 message，流式转发 "rewrite:token" 事件。
#[tauri::command]
pub fn rewrite_generate(
    app: tauri::AppHandle,
    repo: Option<String>,
    entry: RewriteEntryDto,
) -> Result<String, String> {
    let repo = repo_path(repo);
    let cfg = Settings::load().map_err(|e| format!("读取配置失败: {e:#}"))?;

    let mut diff = entry.diff.clone();
    if diff.is_empty() {
        diff = get_commit_diff(&repo, &entry.hash, 4000).unwrap_or_default();
    }
    let max_chars = cfg.ai.max_tokens.max(0) as usize;
    let prompt = build_prompt(&entry.original, &entry.files, &entry.diff_summary, &diff, &cfg.ai.lang, max_chars);

    let client = ChatClient::new(&cfg.ai.base_url, &cfg.ai.api_key, &cfg.ai.model);
    let hash = entry.hash.clone();
    let suggested = client
        .chat_stream(&[ChatMessage::user(prompt)], |token| {
            let _ = app.emit("rewrite:token", serde_json::json!({ "hash": hash, "token": token }));
        })
        .map_err(|e| format!("AI 生成失败: {e:#}"))?;
    let suggested = clean_commit_message(&suggested);

    write_ai_cache(&repo, &entry.hash, &suggested, &cfg.ai.model);
    Ok(suggested)
}

/// RewriteGenerateAll：批量并发 AI 生成（并发 3），
/// 逐 token 发 "rewrite:token"，每条完成发 "rewrite:progress"。
#[tauri::command]
pub fn rewrite_generate_all(
    app: tauri::AppHandle,
    repo: Option<String>,
    entries: Vec<RewriteEntryDto>,
) -> Result<Vec<RewriteGenerateAllResult>, String> {
    let repo = repo_path(repo);
    let cfg = Settings::load().map_err(|e| format!("读取配置失败: {e:#}"))?;
    let client = ChatClient::new(&cfg.ai.base_url, &cfg.ai.api_key, &cfg.ai.model);

    let total = entries.len();
    if total == 0 {
        return Ok(Vec::new());
    }
    let max_chars = cfg.ai.max_tokens.max(0) as usize;

    // 预取每条任务所需数据（避免跨线程借用 entries）
    let work: Vec<(String, String, String, Vec<String>, String)> = entries
        .iter()
        .map(|e| {
            (
                e.hash.clone(),
                e.original.clone(),
                e.diff_summary.clone(),
                e.files.clone(),
                e.diff.clone(),
            )
        })
        .collect();

    let results_mtx: Mutex<Vec<RewriteGenerateAllResult>> = Mutex::new((0..total)
        .map(|_| RewriteGenerateAllResult::default())
        .collect());
    let next = AtomicUsize::new(0);
    let done = AtomicUsize::new(0);
    const CONCURRENCY: usize = 3;

    std::thread::scope(|s| {
        for _ in 0..CONCURRENCY.min(total) {
            s.spawn(|| loop {
                let idx = next.fetch_add(1, Ordering::SeqCst);
                if idx >= total {
                    break;
                }
                let (hash, original, diff_summary, files, diff_pre) = work[idx].clone();
                let diff = if diff_pre.is_empty() {
                    get_commit_diff(&repo, &hash, 4000).unwrap_or_default()
                } else {
                    diff_pre
                };
                let prompt = build_prompt(&original, &files, &diff_summary, &diff, &cfg.ai.lang, max_chars);

                let res = match client.chat_stream(&[ChatMessage::user(prompt)], |token| {
                    let _ = app.emit("rewrite:token", serde_json::json!({ "hash": hash, "token": token }));
                }) {
                    Ok(suggested) => {
                        let suggested = clean_commit_message(&suggested);
                        write_ai_cache(&repo, &hash, &suggested, &cfg.ai.model);
                        RewriteGenerateAllResult {
                            hash: hash.clone(),
                            suggested,
                            ok: true,
                            error: String::new(),
                        }
                    }
                    Err(e) => RewriteGenerateAllResult {
                        hash: hash.clone(),
                        suggested: String::new(),
                        ok: false,
                        error: format!("{e:#}"),
                    },
                };

                let ok = res.ok;
                if let Ok(mut guard) = results_mtx.lock() {
                    guard[idx] = res;
                }
                let d = done.fetch_add(1, Ordering::SeqCst) + 1;
                let _ = app.emit(
                    "rewrite:progress",
                    serde_json::json!({ "hash": hash, "ok": ok, "done": d, "total": total }),
                );
            });
        }
    });

    let results = results_mtx.into_inner().unwrap_or_default();
    Ok(results)
}

fn rewrite_apply_preview_impl(repo: &Path, entries: &[RewriteEntryDto]) -> Result<RewriteApplyPreviewDto, String> {
    let changed: Vec<RewriteEntryDto> = entries
        .iter()
        .filter(|e| e.status == "approved" && !e.suggested.trim().is_empty())
        .cloned()
        .collect();
    if changed.is_empty() {
        return Err("没有已批准(approved)的改写".into());
    }

    let mut preview = RewriteApplyPreviewDto {
        total_affected: changed.len() as i64,
        extra_affected: 0,
        range: String::new(),
        entries: changed.clone(),
    };
    let branch = "HEAD";
    if let Ok(rev) = run_capture(repo, ["rev-list", branch]) {
        let all: Vec<&str> = rev.lines().map(str::trim).filter(|s| !s.is_empty()).collect();
        let set: HashSet<&str> = changed.iter().map(|e| e.hash.as_str()).collect();
        for i in (0..all.len()).rev() {
            if set.contains(all[i]) {
                preview.total_affected = (i + 1) as i64;
                if i < all.len() - 1 {
                    preview.range = format!("{}..{}", all[i + 1], branch);
                } else if run_capture(repo, ["rev-parse", "--verify", &format!("{}^", all[i])]).is_ok() {
                    preview.range = format!("{}^..{}", all[i], branch);
                } else {
                    preview.range = "--all".into();
                }
                break;
            }
        }
    } else {
        preview.range = branch.into();
    }
    preview.extra_affected = preview.total_affected - changed.len() as i64;
    Ok(preview)
}

/// RewriteApplyPreview：计算改写的级联影响（不改历史）。
#[tauri::command]
pub fn rewrite_apply_preview(repo: Option<String>, entries: Vec<RewriteEntryDto>) -> Result<RewriteApplyPreviewDto, String> {
    rewrite_apply_preview_impl(&repo_path(repo), &entries)
}

/// RewriteApply：filter-branch 执行 AI message 改写；autoBackup 时先整库备份。
#[tauri::command]
pub fn rewrite_apply(repo: Option<String>, entries: Vec<RewriteEntryDto>, auto_backup: bool) -> Result<bool, String> {
    let repo = repo_path(repo);
    let preview = rewrite_apply_preview_impl(&repo, &entries)?;

    let mut backed_up = false;
    if auto_backup {
        create_auto_backup(&repo)?;
        backed_up = true;
    }

    let ai_entries: Vec<(String, String)> = preview
        .entries
        .iter()
        .map(|e| (e.hash.clone(), e.suggested.clone()))
        .collect();
    let script = build_msg_filter_gui(&ai_entries);

    let args: Vec<String> = vec![
        "filter-branch".into(),
        "-f".into(),
        "--msg-filter".into(),
        script,
        "--original".into(),
        "refs/original/ai-rewrite".into(),
        "--".into(),
        preview.range.clone(),
    ];
    run_filter_branch(&repo, &args)?;
    let _ = run_capture(&repo, ["update-ref", "-d", "refs/original/ai-rewrite"]);
    Ok(backed_up)
}

// ---------------------------------------------------------------------------
// 元数据改写命令（metarewrite.go / metarewrite_rules.go）
// ---------------------------------------------------------------------------

/// CommitCount：轻量计数，供导出范围预览。
#[tauri::command]
pub fn commit_count(repo: Option<String>) -> Result<CommitCountResult, String> {
    let repo = repo_path(repo);
    let total = run_capture(&repo, ["rev-list", "--count", "HEAD"])
        .ok()
        .and_then(|s| s.trim().parse::<i64>().ok())
        .unwrap_or(0);
    let unpushed = run_capture(&repo, ["rev-list", "--count", "--branches", "--not", "--remotes"])
        .ok()
        .and_then(|s| s.trim().parse::<i64>().ok())
        .unwrap_or(0);
    Ok(CommitCountResult { total, unpushed })
}

/// MetaRewriteExport：导出提交元数据（可带规则与日期重映射），
/// persist=true 时落盘 rewrite.json 供 JSON 编辑器读取。
#[tauri::command]
pub fn meta_rewrite_export(repo: Option<String>, opts: MetaExportOptions) -> Result<Vec<MetaEntryDto>, String> {
    let repo = repo_path(repo);
    // NUL 分隔字段；日期用 git format-local 直接产出本地时区文本（等价 Go parseTs）
    let mut log_args: Vec<String> = vec![
        "log".into(),
        "--format=%H%x00%an%x00%ae%x00%ad%x00%cn%x00%ce%x00%cd%x00%B%x00".into(),
        "--date=format-local:%Y-%m-%d %H:%M:%S %z".into(),
    ];
    match opts.mode.as_str() {
        "last" => {
            let n = if opts.n <= 0 { 20 } else { opts.n };
            log_args.push(format!("-{n}"));
            if opts.skip > 0 {
                log_args.push(format!("--skip={}", opts.skip));
            }
        }
        "unpushed" => {
            log_args.push("--branches".into());
            log_args.push("--not".into());
            log_args.push("--remotes".into());
        }
        _ => {}
    }

    let output = run_capture(&repo, log_args).map_err(|e| format!("git log 失败: {e:#}"))?;
    let raw = output.trim_end_matches('\n');
    if raw.is_empty() {
        return Err("没有找到提交记录".into());
    }
    let recs: Vec<&str> = raw.split("\0\n").collect();
    if recs.is_empty() {
        return Err("没有找到提交记录".into());
    }

    let pushed = pushed_hashes(&repo);
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
            author_date: colon_tz(parts[3].trim()),
            committer_name: parts[4].to_string(),
            committer_email: parts[5].to_string(),
            committer_date: colon_tz(parts[6].trim()),
            message: parts[7].trim_end_matches('\n').to_string(),
        };
        entries.push(CommitEntry {
            hash: parts[0].to_string(),
            pushed: pushed.contains(parts[0]),
            rewrite: meta.clone(),
            original: meta,
        });
    }

    // 应用规则（对应 GenJSON）
    for e in entries.iter_mut() {
        for rule in &opts.rules {
            rewrite::apply_rule(&mut e.rewrite, rule);
        }
    }

    // 日期重映射
    if let Some(remap) = &opts.remap {
        if !remap.allocs.is_empty() {
            distribute_by_days(&mut entries, &remap.allocs, &remap.mode, &remap.active_hour)
                .map_err(|e| format!("逐日分配失败: {e}"))?;
        } else if !remap.date_range.is_empty() {
            rewrite::redistribute_dates(&mut entries, &remap.date_range, &remap.mode, remap.daily_limit, &remap.active_hour)
                .map_err(|e| format!("日期重映射失败: {e:#}"))?;
        }
    }

    // 钳制到 git 父提交日期，避免窗口内提交早于其真实父提交
    if let Some(last) = entries.last() {
        if let Some(parent_abs) = get_parent_commit_date(&repo, &last.hash) {
            clamp_to_parent(&mut entries, parent_abs);
        }
    }

    let dtos: Vec<MetaEntryDto> = entries.iter().map(entry_to_dto).collect();

    // 显式导出时落盘（best-effort，失败不影响导出结果）
    if opts.persist {
        if let Ok(p) = rewrite_json_file(&repo) {
            if let Some(dir) = p.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            if let Ok(data) = serde_json::to_string_pretty(&dtos) {
                let _ = std::fs::write(&p, data);
            }
        }
    }
    Ok(dtos)
}

fn meta_rewrite_preview_impl(repo: &Path, entries: &[MetaEntryDto]) -> Result<MetaRewritePreviewDto, String> {
    let mut changed: Vec<PreviewChangeEntry> = Vec::new();
    let mut pushed_changed = 0i64;
    for e in entries {
        let diffs = build_field_diffs(&e.original, &e.rewrite);
        if diffs.is_empty() {
            continue;
        }
        if e.pushed {
            pushed_changed += 1;
        }
        changed.push(PreviewChangeEntry {
            hash: e.hash.clone(),
            pushed: e.pushed,
            diffs,
        });
    }
    if changed.is_empty() {
        return Err("没有需要改写的提交".into());
    }

    let mut preview = MetaRewritePreviewDto {
        changed,
        total_affected: 0,
        extra_affected: 0,
        range: String::new(),
        pushed_changed,
    };
    preview.total_affected = preview.changed.len() as i64;

    // 解析当前分支（仅改写本地分支）
    let branch_name = match run_capture(repo, ["symbolic-ref", "--short", "HEAD"]) {
        Ok(out) => out.trim().to_string(),
        Err(_) => "HEAD".to_string(),
    };
    let local_ref = format!("refs/heads/{branch_name}");
    preview.range = local_ref.clone();

    if let Ok(rev) = run_capture(repo, ["rev-list", &local_ref]) {
        let all: Vec<&str> = rev.lines().map(str::trim).filter(|s| !s.is_empty()).collect();
        let set: HashSet<&str> = preview.changed.iter().map(|c| c.hash.as_str()).collect();
        for i in (0..all.len()).rev() {
            if set.contains(all[i]) {
                preview.total_affected = (i + 1) as i64;
                if i < all.len() - 1 {
                    preview.range = format!("{}..{}", all[i + 1], local_ref);
                } else if run_capture(repo, ["rev-parse", "--verify", &format!("{}^", all[i])]).is_ok() {
                    preview.range = format!("{}^..{}", all[i], local_ref);
                } else {
                    preview.range = local_ref.clone();
                }
                break;
            }
        }
    }
    preview.extra_affected = preview.total_affected - preview.changed.len() as i64;
    Ok(preview)
}

/// 字段级差异（中文字段名，对齐 buildFieldDiffs）。
fn build_field_diffs(o: &CommitMetaDto, r: &CommitMetaDto) -> Vec<FieldDiff> {
    let pairs = [
        ("作者名", &o.author_name, &r.author_name),
        ("作者邮箱", &o.author_email, &r.author_email),
        ("作者日期", &o.author_date, &r.author_date),
        ("提交者名", &o.committer_name, &r.committer_name),
        ("提交者邮箱", &o.committer_email, &r.committer_email),
        ("提交者日期", &o.committer_date, &r.committer_date),
        ("message", &o.message, &r.message),
    ];
    pairs
        .iter()
        .filter(|(_label, old, new)| old != new)
        .map(|(label, old, new)| FieldDiff {
            field: (*label).to_string(),
            old: (*old).clone(),
            new: (*new).clone(),
        })
        .collect()
}

/// MetaRewritePreview：计算元数据改写的级联影响。
#[tauri::command]
pub fn meta_rewrite_preview(repo: Option<String>, entries: Vec<MetaEntryDto>) -> Result<MetaRewritePreviewDto, String> {
    meta_rewrite_preview_impl(&repo_path(repo), &entries)
}

/// MetaRewriteApply：filter-branch 执行元数据（作者/日期/邮件/message）改写。
#[tauri::command]
pub fn meta_rewrite_apply(
    repo: Option<String>,
    entries: Vec<MetaEntryDto>,
    sync_refs: bool,
    auto_backup: bool,
) -> Result<bool, String> {
    let repo = repo_path(repo);
    let preview = meta_rewrite_preview_impl(&repo, &entries)?;

    let mut backed_up = false;
    if auto_backup {
        create_auto_backup(&repo)?;
        backed_up = true;
    }

    // 以 Rewrite != Original 的条目构建脚本（而非 preview.Changed）
    let ce: Vec<CommitEntry> = entries
        .iter()
        .filter(|e| dto_to_meta(&e.original) != dto_to_meta(&e.rewrite))
        .map(dto_to_entry)
        .collect();
    let script = build_env_filter(&ce);
    let msg_script = build_msg_filter(&ce);

    let git_dir = git_dir(&repo).map_err(|e| format!("获取 .git 路径失败: {e:#}"))?;
    let env_path = git_dir.join("byte-git-env-filter.sh");
    std::fs::write(&env_path, &script).map_err(|e| format!("写入脚本失败: {e}"))?;
    let env_posix = to_posix(&env_path);

    let msg_posix = if !msg_script.is_empty() {
        let p = git_dir.join("byte-git-msg-filter.sh");
        std::fs::write(&p, &msg_script).map_err(|e| format!("写入 msg 脚本失败: {e}"))?;
        Some(to_posix(&p))
    } else {
        None
    };

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
    args.push(preview.range.clone());

    let result = run_filter_branch(&repo, &args);

    // 清理临时脚本（无论成败）
    let _ = std::fs::remove_file(&env_path);
    if msg_posix.is_some() {
        let _ = std::fs::remove_file(git_dir.join("byte-git-msg-filter.sh"));
    }
    result?;

    let _ = run_capture(&repo, ["update-ref", "-d", "refs/original/rewrite"]);

    // 清理已落盘的导出 JSON（数据已应用到真实历史，避免误导 JSON 编辑器）
    if let Ok(p) = rewrite_json_file(&repo) {
        let _ = std::fs::remove_file(p);
    }

    if sync_refs {
        let branch_name = match run_capture(&repo, ["symbolic-ref", "--short", "HEAD"]) {
            Ok(out) => out.trim().to_string(),
            Err(_) => "HEAD".to_string(),
        };
        sync_remote_refs_gui(&repo, &branch_name);
    }
    Ok(backed_up)
}

/// MetaRewriteApplyRule：对全部条目的 Rewrite 应用单条规则。
#[tauri::command]
pub fn meta_rewrite_apply_rule(entries: Vec<MetaEntryDto>, rule: String) -> Result<Vec<MetaEntryDto>, String> {
    let mut entries = entries;
    for e in entries.iter_mut() {
        let mut m = dto_to_meta(&e.rewrite);
        rewrite::apply_rule(&mut m, &rule);
        e.rewrite = meta_to_dto(&m);
    }
    Ok(entries)
}

/// MetaRewriteRemap（Go 源码有、清单未列）：对已加载条目做日期范围重分配。
#[tauri::command]
pub fn meta_rewrite_remap(
    entries: Vec<MetaEntryDto>,
    date_range: String,
    mode: String,
    daily_limit: i64,
    active_hour: String,
) -> Result<Vec<MetaEntryDto>, String> {
    let mut ce: Vec<CommitEntry> = entries.iter().map(dto_to_entry).collect();
    rewrite::redistribute_dates(&mut ce, &date_range, &mode, daily_limit, &active_hour)
        .map_err(|e| format!("{e:#}"))?;
    Ok(ce.iter().map(entry_to_dto).collect())
}

/// MetaRewriteDistributeByDays：按逐日分配重写已加载条目的日期。
#[tauri::command]
pub fn meta_rewrite_distribute_by_days(
    entries: Vec<MetaEntryDto>,
    allocs: Vec<DailyAllocDto>,
    mode: String,
    active_hour: String,
) -> Result<Vec<MetaEntryDto>, String> {
    let mut ce: Vec<CommitEntry> = entries.iter().map(dto_to_entry).collect();
    distribute_by_days(&mut ce, &allocs, &mode, &active_hour)?;
    Ok(ce.iter().map(entry_to_dto).collect())
}

// ---------------------------------------------------------------------------
// JSON 编辑器（jsoneditor.go）
// ---------------------------------------------------------------------------

/// JsonEditorPath：改写数据 JSON 的固定绝对路径（供 UI 展示）。
#[tauri::command]
pub fn json_editor_path(repo: Option<String>) -> Result<String, String> {
    let p = rewrite_json_file(&repo_path(repo))?;
    Ok(p.display().to_string())
}

/// JsonEditorRead：读取 rewrite.json；未导出时返回空串。
#[tauri::command]
pub fn json_editor_read(repo: Option<String>) -> Result<String, String> {
    let p = rewrite_json_file(&repo_path(repo))?;
    match std::fs::read_to_string(&p) {
        Ok(s) => Ok(s),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
        Err(e) => Err(format!("{e}")),
    }
}

/// JsonEditorWrite：写入 rewrite.json。
#[tauri::command]
pub fn json_editor_write(repo: Option<String>, content: String) -> Result<(), String> {
    let p = rewrite_json_file(&repo_path(repo))?;
    std::fs::write(&p, content).map_err(|e| format!("{e}"))
}

/// JsonEditorRemove：删除 rewrite.json；文件不存在时为 no-op。
#[tauri::command]
pub fn json_editor_remove(repo: Option<String>) -> Result<(), String> {
    let p = rewrite_json_file(&repo_path(repo))?;
    match std::fs::remove_file(&p) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(format!("{e}")),
    }
}

// ---------------------------------------------------------------------------
// 测试（纯函数部分）
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn civil_roundtrip() {
        for (y, m, d) in [(1970, 1, 1), (2000, 2, 29), (2026, 10, 8), (1999, 12, 31)] {
            let z = days_from_civil(y, m, d);
            assert_eq!(civil_from_days(z), (y, m, d));
        }
        assert_eq!(days_from_civil(1970, 1, 1), 0);
    }

    #[test]
    fn parse_and_format_date() {
        let (wall, off, tz) = parse_meta_date("2024-01-01 10:00:00 +08:00").unwrap();
        assert_eq!(off, 8 * 3600);
        assert_eq!(tz, "+08:00");
        assert_eq!(format_wall(wall, &tz), "2024-01-01 10:00:00 +08:00");
        // 无冒号时区（git %z 原始输出）
        let (w2, o2, _) = parse_meta_date("2024-01-01 10:00:00 +0800").unwrap();
        assert_eq!((w2, o2), (wall, off));
        assert_eq!(colon_tz("+0800"), "+08:00");
    }

    #[test]
    fn clamp_to_parent_shifts_window() {
        let mk = |date: &str| CommitEntry {
            hash: "h".into(),
            pushed: false,
            original: CommitMeta { author_date: date.into(), ..Default::default() },
            rewrite: CommitMeta { author_date: date.into(), ..Default::default() },
        };
        let parent_abs = parse_meta_date("2024-06-01 12:00:00 +08:00").unwrap().0 - 8 * 3600;
        let mut entries = vec![mk("2024-06-03 10:00:00 +08:00"), mk("2024-05-01 08:00:00 +08:00")];
        clamp_to_parent(&mut entries, parent_abs);
        // 最老条目（entries[1]）应等于 parent + 1s
        assert_eq!(entries[1].rewrite.author_date, "2024-06-01 12:00:01 +08:00");
    }

    #[test]
    fn clean_message_strips_fence() {
        assert_eq!(clean_commit_message("```json\n{\"a\":1}\n```"), "{\"a\":1}");
        assert_eq!(clean_commit_message("\"feat: x\""), "feat: x");
    }
}
