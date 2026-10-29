//! info 域命令：仓库洞察（概览/健康/体积/大文件/分支/活动/作者/变更日志/
//! 热点/TODO）与跨仓库开发统计。
//!
//! 数据装配对齐
//! `internal/info` 的 Exported 实现（GetSummary/GetHealth/GetSize/GetBranches/
//! GetActivity/GetAuthors/GetChangelog/GetHotspots/GetTodos/ScanLargeFiles/
//! GetStats）。core 侧 info/stats 是 CLI 打印输出，GUI 需要结构化 DTO，
//! 故此处基于 git 取数自行装配（对应 Go gitCmd/cmd.Output 的 stdout 语义）。
//!
//! Go 的 InfoXxx(repo string) 以 repo=="" 回落 CurrentRepoOrDot；Rust 统一经
//! repo_path -> crate::state::resolve（显式 repo > 当前仓库状态 > "."）。
//! 命令均为 async：git 子进程与全仓文件扫描是阻塞操作（commit.rs 同款
//! run_blocking 挪到 std 线程，避免冻结窗口）。

use chrono::{Datelike, Local, NaiveDate, NaiveDateTime, TimeZone, Timelike};
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// Go 侧 gitCmd 设 30 秒超时、misc/stats 路径无超时；这里统一给宽松上限
/// （全量 git log 在大仓库上可能较慢），防止异常悬挂。
const GIT_TIMEOUT: Duration = Duration::from_secs(120);

// ---------------------------------------------------------------------------
// 基础设施（与 commit.rs 同款约定）
// ---------------------------------------------------------------------------

/// 仓库路径解析：显式 repo > 当前仓库状态 > "."（空串回落见 state::resolve）。
fn repo_path(repo: Option<String>) -> std::path::PathBuf {
    crate::state::resolve(repo)
}

/// Windows 下避免子进程闪现控制台窗口（对应 sysproc 的 CREATE_NO_WINDOW）。
#[cfg(windows)]
fn hide_console(cmd: &mut Command) {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    cmd.creation_flags(CREATE_NO_WINDOW);
}

#[cfg(not(windows))]
fn hide_console(_cmd: &mut Command) {}

/// git_exec 的执行结果（stdout/stderr 分离，对齐 Go cmd.Output() 只取 stdout）。
struct GitOut {
    /// 退出码是否为 0。
    success: bool,
    /// 是否因超时被强制终止。
    timed_out: bool,
    stdout: String,
    stderr: String,
}

/// 在仓库目录执行 `git -C <repo> <args...>`：隐藏窗口 + 超时 + 分离捕获。
fn git_exec(repo: &Path, args: &[&str], timeout: Duration) -> GitOut {
    fn drain<R: std::io::Read>(mut r: R) -> Vec<u8> {
        let mut buf = Vec::new();
        let _ = std::io::Read::read_to_end(&mut r, &mut buf);
        buf
    }

    let mut cmd = Command::new("git");
    cmd.arg("-C")
        .arg(repo)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    hide_console(&mut cmd);

    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            return GitOut {
                success: false,
                timed_out: false,
                stdout: String::new(),
                stderr: format!("启动 git 失败: {e}"),
            }
        }
    };

    // 独立线程持续读管道，避免大输出写满管道导致子进程阻塞
    let out_handle = child.stdout.take().map(|p| std::thread::spawn(move || drain(p)));
    let err_handle = child.stderr.take().map(|p| std::thread::spawn(move || drain(p)));

    let deadline = Instant::now() + timeout;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let stdout = out_handle.and_then(|h| h.join().ok()).unwrap_or_default();
                let stderr = err_handle.and_then(|h| h.join().ok()).unwrap_or_default();
                return GitOut {
                    success: status.success(),
                    timed_out: false,
                    stdout: String::from_utf8_lossy(&stdout).into_owned(),
                    stderr: String::from_utf8_lossy(&stderr).into_owned(),
                };
            }
            Ok(None) => {
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    return GitOut {
                        success: false,
                        timed_out: true,
                        stdout: String::new(),
                        stderr: format!(
                            "git 命令执行超时 (超过 {} 秒): git {}",
                            timeout.as_secs(),
                            args.join(" ")
                        ),
                    };
                }
                std::thread::sleep(Duration::from_millis(15));
            }
            Err(e) => {
                return GitOut {
                    success: false,
                    timed_out: false,
                    stdout: String::new(),
                    stderr: format!("等待 git 进程失败: {e}"),
                }
            }
        }
    }
}

/// 在独立线程执行阻塞任务（commit.rs 同款：阻塞工作不占异步执行线程）。
fn run_blocking<T, F>(f: F) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce() -> T + Send + 'static,
{
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(f());
    });
    rx.recv().map_err(|_| "后台任务线程异常退出".to_string())
}

// ---------------------------------------------------------------------------
// DTO（json tag 与 Go 结构体逐一对齐）
// ---------------------------------------------------------------------------

/// SummaryResult（gui info.go）。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SummaryResult {
    pub repo_name: String,
    pub file_count: i64,
    pub total_size: String,
    /// Go 侧为 string（rev-list --count 的原样输出）。
    pub total_commits: String,
    pub author_count: i64,
    pub branch_count: i64,
    pub tag_count: i64,
    pub first_date: String,
    pub last_date: String,
    pub timeline_days: i64,
    pub languages: Vec<LangEntry>,
    pub top_authors: Vec<AuthorEntry>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LangEntry {
    pub name: String,
    pub pct: f64,
    pub size: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthorEntry {
    pub name: String,
    pub commits: i64,
}

/// AuthorStatEntry：贡献者排行条目（多指标）。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthorStatEntry {
    pub name: String,
    pub commits: i64,
    pub active_days: i64,
    pub recent30: i64,
    pub streak: i64,
    pub first_commit: String,
    pub last_commit: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivityEntry {
    pub date: String,
    pub count: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthResult {
    pub items: Vec<HealthItem>,
    pub grade: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthItem {
    /// ok | warn | info
    pub level: String,
    pub title: String,
    pub details: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HotspotEntry {
    pub file: String,
    pub commits: i64,
    pub additions: i64,
    pub deletions: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangelogEntry {
    pub hash: String,
    #[serde(rename = "type")]
    pub ty: String,
    pub scope: String,
    pub description: String,
    pub breaking: bool,
    pub date: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BranchInfo {
    pub name: String,
    pub is_current: bool,
    pub last_commit: String,
    pub ahead: i64,
    pub behind: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BranchesResult {
    pub current: Vec<BranchInfo>,
    pub active: Vec<BranchInfo>,
    pub merged: Vec<BranchInfo>,
    pub stale: Vec<BranchInfo>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SizeByType {
    pub ext: String,
    pub count: i64,
    pub size: i64,
    pub size_str: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LargeFile {
    pub path: String,
    pub size: i64,
    pub size_str: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SizeResult {
    pub total_size: String,
    pub total_files: i64,
    pub by_type: Vec<SizeByType>,
    pub large_files: Vec<LargeFile>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TodoItem {
    pub tag: String,
    pub file: String,
    pub line: i64,
    pub content: String,
}

/// FileSizeEntry：大文件扫描条目（internal/info largefiles.go）。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileSizeEntry {
    pub path: String,
    pub language: String,
    pub lines: i64,
    pub size: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LargeFilesResult {
    pub files: Vec<FileSizeEntry>,
    pub warning: i64,
    pub critical: i64,
}

/// StatsResult（gui stats.go）。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatsResult {
    pub total_commits: i64,
    pub active_days: i64,
    pub current_streak: i64,
    pub longest_streak: i64,
    pub weekday_activity: Vec<i64>,
    pub hour_activity: Vec<i64>,
    pub daily_activity: Vec<DailyEntry>,
    pub day_hour_activity: Vec<DayHourEntry>,
    pub authors: Vec<AuthorStatEntry>,
    pub top_repos: Vec<RepoStat>,
    pub languages: Vec<LangEntry>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepoStat {
    pub name: String,
    pub commits: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DailyEntry {
    pub date: String,
    pub count: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DayHourEntry {
    pub date: String,
    pub hours: [i64; 24],
    pub count: i64,
}

// ---------------------------------------------------------------------------
// 私有辅助
// ---------------------------------------------------------------------------

/// 小写扩展名（含点；无扩展名为空串），对齐 filepath.Ext + ToLower。
fn ext_lower(path: &str) -> String {
    Path::new(path)
        .extension()
        .map(|e| format!(".{}", e.to_string_lossy().to_lowercase()))
        .unwrap_or_default()
}

/// path 最后一段（对齐 filepath.Base；取不到时回退整个路径的显示形式）。
fn path_base_name(p: &Path) -> String {
    p.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| p.display().to_string())
}

/// 文件体积人类可读形式（对齐 Go formatFileSize）。
fn format_file_size(b: i64) -> String {
    const KB: i64 = 1024;
    const MB: i64 = KB * 1024;
    const GB: i64 = MB * 1024;
    match b {
        x if x >= GB => format!("{:.1} GB", x as f64 / GB as f64),
        x if x >= MB => format!("{:.1} MB", x as f64 / MB as f64),
        x if x >= KB => format!("{:.1} KB", x as f64 / KB as f64),
        x => format!("{x} B"),
    }
}

/// 非代码扩展名（图片/字体/音视频/二进制/归档/文档/锁文件），
/// 语言统计时跳过（对齐 lang_shared.go 的 nonCodeExts）。
fn is_excluded_ext(ext: &str) -> bool {
    const EXTS: &[&str] = &[
        // images
        ".png", ".jpg", ".jpeg", ".gif", ".bmp", ".ico", ".icns", ".tiff", ".tif", ".webp",
        ".svgz", ".raw", ".heic", ".avif",
        // fonts
        ".woff", ".woff2", ".ttf", ".otf", ".eot",
        // audio / video
        ".mp3", ".wav", ".flac", ".aac", ".ogg", ".mp4", ".webm", ".avi", ".mov", ".mkv",
        ".m4a", ".m4v",
        // binaries / executables / libraries
        ".exe", ".dll", ".so", ".dylib", ".bin", ".o", ".a", ".lib", ".class", ".jar",
        ".wasm", ".pdb", ".obj",
        // archives
        ".zip", ".gz", ".tar", ".tgz", ".bz2", ".xz", ".7z", ".rar", ".lz",
        // databases / structured blobs
        ".db", ".sqlite", ".sqlite3", ".mdb",
        // documents
        ".pdf", ".doc", ".docx", ".xls", ".xlsx", ".ppt", ".pptx", ".epub",
        // lock files & generated metadata
        ".lock", ".sum",
    ];
    EXTS.contains(&ext)
}

/// summary 视图的语言名映射（对齐 summary_cli.go 的 langNames）。
fn summary_lang_name(ext: &str) -> String {
    let known = match ext {
        ".go" => "Go",
        ".rs" => "Rust",
        ".py" => "Python",
        ".js" => "JavaScript",
        ".ts" => "TypeScript",
        ".tsx" => "TypeScript",
        ".jsx" => "JavaScript",
        ".java" => "Java",
        ".kt" => "Kotlin",
        ".c" => "C",
        ".h" => "C",
        ".cpp" => "C++",
        ".hpp" => "C++",
        ".cs" => "C#",
        ".rb" => "Ruby",
        ".php" => "PHP",
        ".swift" => "Swift",
        ".zig" => "Zig",
        ".lua" => "Lua",
        ".sh" => "Shell",
        ".bash" => "Shell",
        ".html" => "HTML",
        ".css" => "CSS",
        ".scss" => "SCSS",
        ".vue" => "Vue",
        ".sql" => "SQL",
        ".proto" => "Protocol Buffers",
        ".md" => "Markdown",
        ".rst" => "reStructuredText",
        ".txt" => "Text",
        ".yaml" => "YAML",
        ".yml" => "YAML",
        ".toml" => "TOML",
        ".json" => "JSON",
        ".xml" => "XML",
        ".ini" => "INI",
        ".cfg" => "INI",
        _ => "",
    };
    if known.is_empty() {
        if ext.is_empty() {
            return "Other".to_string();
        }
        let name = ext.trim_start_matches('.');
        let mut chars = name.chars();
        match chars.next() {
            Some(f) => f.to_uppercase().collect::<String>() + chars.as_str(),
            None => "Other".to_string(),
        }
    } else {
        known.to_string()
    }
}

/// stats 视图的语言名映射（对齐 stats_exported.go 的 langName）。
fn stats_lang_name(ext: &str) -> String {
    let known = match ext {
        ".go" => "Go",
        ".rs" => "Rust",
        ".py" => "Python",
        ".js" => "JavaScript",
        ".ts" => "TypeScript",
        ".java" => "Java",
        ".kt" => "Kotlin",
        ".c" => "C",
        ".cpp" => "C++",
        ".h" => "C/C++ Header",
        ".rb" => "Ruby",
        ".php" => "PHP",
        ".swift" => "Swift",
        ".lua" => "Lua",
        ".sh" => "Shell",
        ".bat" => "Batch",
        ".html" => "HTML",
        ".css" => "CSS",
        ".scss" => "SCSS",
        ".vue" => "Vue",
        ".sql" => "SQL",
        ".md" => "Markdown",
        ".yaml" | ".yml" => "YAML",
        ".json" => "JSON",
        ".xml" => "XML",
        ".toml" => "TOML",
        ".mod" => "Go Module",
        ".sum" => "Go Checksum",
        _ => "",
    };
    if known.is_empty() {
        if ext.is_empty() {
            return "Other".to_string();
        }
        let name = ext.trim_start_matches('.');
        let mut chars = name.chars();
        match chars.next() {
            Some(f) => f.to_uppercase().collect::<String>() + chars.as_str(),
            None => "Other".to_string(),
        }
    } else {
        known.to_string()
    }
}

/// 大文件扫描的语言名映射（对齐 largefiles.go 的 extToLang）。
fn large_file_lang(ext: &str) -> String {
    let known = match ext {
        ".go" => "Go",
        ".vue" => "Vue",
        ".ts" => "TypeScript",
        ".js" => "JavaScript",
        ".jsx" => "JSX",
        ".tsx" => "TSX",
        ".py" => "Python",
        ".java" => "Java",
        ".c" => "C",
        ".cpp" => "C++",
        ".h" => "C/C++ Header",
        ".rs" => "Rust",
        ".rb" => "Ruby",
        ".php" => "PHP",
        ".swift" => "Swift",
        ".kt" => "Kotlin",
        ".lua" => "Lua",
        ".sh" => "Shell",
        ".css" => "CSS",
        ".scss" => "SCSS",
        ".html" => "HTML",
        _ => return ext.trim_start_matches('.').to_string(),
    };
    known.into()
}

/// 解析 git 日期（"2006-01-02 15:04:05 -0700"，回退无时区形式按 UTC）。
fn parse_git_datetime(s: &str) -> Option<chrono::DateTime<chrono::FixedOffset>> {
    if let Ok(dt) = chrono::DateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S %z") {
        return Some(dt);
    }
    let naive = NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S").ok()?;
    Some(chrono::Utc.from_utc_datetime(&naive).fixed_offset())
}

/// 概览时间线的短日期（对齐 formatSummaryTime：空串 "-"，解析失败取前 10 字符）。
fn format_summary_time(date_str: &str) -> String {
    if date_str.is_empty() {
        return "-".into();
    }
    if let Some(dt) = parse_git_datetime(date_str) {
        return dt.naive_local().format("%Y-%m-%d").to_string();
    }
    date_str.chars().take(10).collect()
}

/// 解析 %(upstream:track) 的 ahead/behind 数值（对齐 Go 的 `ahead (\d+)` 正则）。
fn parse_track(track: &str) -> (i64, i64) {
    fn count_after(hay: &str, key: &str) -> i64 {
        hay.find(key)
            .map(|i| {
                let rest = hay[i + key.len()..].trim_start();
                let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
                digits.parse().unwrap_or(0)
            })
            .unwrap_or(0)
    }
    let ahead = if track.contains("ahead") { count_after(track, "ahead") } else { 0 };
    let behind = if track.contains("behind") { count_after(track, "behind") } else { 0 };
    (ahead, behind)
}

/// 解析 conventional commit 主题：`feat(scope)!: desc`。
/// 对应 Go convCommitRe（^(feat|fix|...)(?:\(([^)]+)\))?(!)?:\s*(.+)$）；
/// 桌面端未引入 regex 依赖，手写等价解析（类型词表顺序与 Go 一致）。
/// 返回 (type, scope, breaking, description)。
fn parse_conventional(subject: &str) -> Option<(String, String, bool, String)> {
    const TYPES: [&str; 10] = [
        "feat", "fix", "refactor", "perf", "docs", "style", "test", "chore", "build", "ci",
    ];
    for ty in TYPES {
        let Some(mut rest) = subject.strip_prefix(ty) else { continue };
        let mut scope = String::new();
        let mut breaking = false;
        if let Some(after) = rest.strip_prefix('(') {
            let Some(close) = after.find(')') else { continue };
            if close == 0 {
                continue; // scope 至少 1 个字符（[^)]+）
            }
            scope = after[..close].to_string();
            rest = &after[close + 1..];
        }
        if let Some(after) = rest.strip_prefix('!') {
            breaking = true;
            rest = after;
        }
        let Some(after) = rest.strip_prefix(':') else { continue };
        // \s*(.+)：跳过前导空白，描述不能为空
        let desc = after.trim_start_matches(&[' ', '\t', '\n', '\r', '\x0b', '\x0c'][..]);
        if desc.is_empty() {
            continue;
        }
        return Some((ty.to_string(), scope, breaking, desc.to_string()));
    }
    None
}

fn is_word_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

/// 在行内容中定位首个 TODO/FIXME/HACK/XXX 标记，对应 Go todoRe
/// （(?i)\b(TODO|FIXME|HACK|XXX)\b[:\s]*(.*)，最左优先）。
/// 返回 (大写 tag, 标记后去掉 [:\s] 前缀并 trim 的文本)。
fn find_todo_tag(content: &str) -> Option<(String, String)> {
    const TAGS: [&str; 4] = ["TODO", "FIXME", "HACK", "XXX"];
    let lower = content.to_ascii_lowercase();
    let lb = lower.as_bytes();
    for start in 0..lb.len() {
        // \b：标记前一个字节不能是 word 字符
        if start > 0 && is_word_byte(lb[start - 1]) {
            continue;
        }
        for tag in TAGS {
            let t = tag.to_ascii_lowercase();
            if !lb[start..].starts_with(t.as_bytes()) {
                continue;
            }
            let end = start + tag.len();
            // \b：标记后一个字节不能是 word 字符（串尾视为边界）
            if end < lb.len() && is_word_byte(lb[end]) {
                continue;
            }
            // [:\s]*(.*)：跳过冒号/空白后取剩余
            let mut rest = end;
            while rest < lb.len() && (lb[rest] == b':' || lb[rest].is_ascii_whitespace()) {
                rest += 1;
            }
            let text = content[rest..].trim().to_string();
            return Some((tag.to_string(), text));
        }
    }
    None
}

/// 当前连续提交天数（对齐 calcStreak：从今天起往回数，最多 365 天）。
fn calc_streak(date_commits: &HashMap<String, i64>) -> i64 {
    let today = Local::now().date_naive();
    let mut streak = 0i64;
    for d in 0..365i64 {
        let date = today - chrono::Duration::days(d);
        if date_commits.get(&date.format("%Y-%m-%d").to_string()).copied().unwrap_or(0) > 0 {
            streak += 1;
        } else {
            break;
        }
    }
    streak
}

/// 最长连续提交天数（对齐 calcLongestStreak：相邻日期严格差 24 小时）。
fn calc_longest_streak(date_commits: &HashMap<String, i64>) -> i64 {
    if date_commits.is_empty() {
        return 0;
    }
    let mut dates: Vec<&String> = date_commits.keys().collect();
    dates.sort();
    let mut longest = 1i64;
    let mut current = 1i64;
    for i in 1..dates.len() {
        let (prev, curr) = (
            NaiveDate::parse_from_str(dates[i - 1], "%Y-%m-%d"),
            NaiveDate::parse_from_str(dates[i], "%Y-%m-%d"),
        );
        match (prev, curr) {
            (Ok(p), Ok(c)) if (c - p).num_hours() == 24 => {
                current += 1;
                if current > longest {
                    longest = current;
                }
            }
            _ => current = 1,
        }
    }
    longest
}

/// 扫描目录下的 git 仓库（对齐 stats_exported.go scanRepos：
/// dir 本身是仓库则只返回它（绝对化），否则返回一级子目录中的仓库）。
fn scan_repos(dir: &Path) -> std::io::Result<Vec<PathBuf>> {
    if dir.join(".git").is_dir() {
        let abs = abs_path(dir).unwrap_or_else(|_| dir.to_path_buf());
        return Ok(vec![abs]);
    }
    let mut repos = Vec::new();
    for e in std::fs::read_dir(dir)? {
        let e = e?;
        if !e.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            continue;
        }
        if e.path().join(".git").is_dir() {
            repos.push(e.path());
        }
    }
    Ok(repos)
}

/// 相对路径绝对化（对齐 Go filepath.Abs：相对路径拼进程 CWD，不解析符号链接）。
fn abs_path(p: &Path) -> std::io::Result<PathBuf> {
    if p.is_absolute() {
        return Ok(p.to_path_buf());
    }
    let cwd = std::env::current_dir()?;
    Ok(cwd.join(p))
}

// ---------------------------------------------------------------------------
// 数据装配（internal/info Exported 函数的等价实现）
// ---------------------------------------------------------------------------

/// GetSummary：仓库概览。
fn get_summary(repo: &Path) -> SummaryResult {
    let out = git_exec(repo, &["ls-files"], GIT_TIMEOUT);
    let ls = out.stdout.trim().to_string();
    let file_count = (if ls.is_empty() { 0 } else { ls.split('\n').count() }) as i64;

    // %ai 全量历史日期：首行=最近一次提交，末行=第一次提交
    let log_dates = git_exec(repo, &["log", "--format=%ai"], GIT_TIMEOUT);
    let mut first_date = String::new();
    let mut last_date = String::new();
    for line in log_dates.stdout.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if last_date.is_empty() {
            last_date = line.to_string();
        }
        first_date = line.to_string();
    }

    let total_commits = git_exec(repo, &["rev-list", "--count", "HEAD"], GIT_TIMEOUT)
        .stdout
        .trim()
        .to_string();
    let authors_out = git_exec(repo, &["shortlog", "-sn", "HEAD"], GIT_TIMEOUT)
        .stdout
        .trim()
        .to_string();
    let author_count = (if authors_out.is_empty() { 0 } else { authors_out.split('\n').count() }) as i64;
    let branches_out = git_exec(repo, &["branch"], GIT_TIMEOUT).stdout.trim().to_string();
    let branch_count = (if branches_out.is_empty() { 0 } else { branches_out.split('\n').count() }) as i64;
    let tags_out = git_exec(repo, &["tag"], GIT_TIMEOUT).stdout.trim().to_string();
    let tag_count = (if tags_out.is_empty() { 0 } else { tags_out.split('\n').count() }) as i64;

    let mut lang_sizes: HashMap<String, i64> = HashMap::new();
    let mut total_size: i64 = 0;
    if !ls.is_empty() {
        for path in ls.split('\n') {
            let path = path.trim();
            if path.is_empty() {
                continue;
            }
            let Ok(meta) = std::fs::metadata(repo.join(path)) else { continue };
            total_size += meta.len() as i64;
            let ext = ext_lower(path);
            if is_excluded_ext(&ext) {
                continue;
            }
            *lang_sizes.entry(summary_lang_name(&ext)).or_insert(0) += meta.len() as i64;
        }
    }

    // 仓库名：默认目录名；有 origin 远程时取 URL 最后一段（去 .git 后缀）
    let mut repo_name = path_base_name(repo);
    let remote_out = git_exec(repo, &["remote", "get-url", "origin"], GIT_TIMEOUT)
        .stdout
        .trim()
        .to_string();
    if !remote_out.is_empty() {
        if let Some(last) = remote_out.split('/').next_back() {
            repo_name = last.strip_suffix(".git").unwrap_or(last).to_string();
        }
    }

    let timeline_days = match (parse_git_datetime(&first_date), parse_git_datetime(&last_date)) {
        (Some(ft), Some(lt)) => (lt - ft).num_hours() / 24,
        _ => 0,
    };

    let mut languages: Vec<LangEntry> = Vec::new();
    if total_size > 0 {
        let mut entries: Vec<(String, i64)> = lang_sizes.into_iter().collect();
        entries.sort_by(|a, b| b.1.cmp(&a.1));
        for (name, size) in entries {
            languages.push(LangEntry {
                name,
                pct: size as f64 / total_size as f64 * 100.0,
                size,
            });
        }
    }

    let mut top_authors: Vec<AuthorEntry> = Vec::new();
    if !authors_out.is_empty() {
        for line in authors_out.split('\n') {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let Some((count, name)) = line.split_once('\t') else { continue };
            top_authors.push(AuthorEntry {
                name: name.trim().to_string(),
                commits: count.trim().parse().unwrap_or(0),
            });
        }
    }

    SummaryResult {
        repo_name,
        file_count,
        total_size: format_file_size(total_size),
        total_commits,
        author_count,
        branch_count,
        tag_count,
        first_date: format_summary_time(&first_date),
        last_date: format_summary_time(&last_date),
        timeline_days,
        languages,
        top_authors,
    }
}

/// GetHealth：仓库健康检查（导出版本，非 CLI 渲染版）。
fn get_health(repo: &Path) -> HealthResult {
    let mut items: Vec<HealthItem> = Vec::new();

    // 未提交改动
    let status_out = git_exec(repo, &["status", "--porcelain"], GIT_TIMEOUT).stdout.trim().to_string();
    if !status_out.is_empty() {
        let details: Vec<String> = status_out
            .split('\n')
            .map(|l| l.trim())
            .filter(|l| !l.is_empty())
            .map(String::from)
            .collect();
        items.push(HealthItem {
            level: "warn".into(),
            title: format!("未提交改动 ({})", details.len()),
            details,
        });
    }

    // 大文件 >10MB
    let files_out = git_exec(repo, &["ls-files"], GIT_TIMEOUT).stdout.trim().to_string();
    if !files_out.is_empty() {
        let mut large: Vec<String> = Vec::new();
        for f in files_out.split('\n') {
            let f = f.trim();
            if f.is_empty() {
                continue;
            }
            let Ok(meta) = std::fs::metadata(repo.join(f)) else { continue };
            if meta.len() > 10 * 1024 * 1024 {
                large.push(format!("{} ({})", f, format_file_size(meta.len() as i64)));
            }
        }
        if !large.is_empty() {
            items.push(HealthItem {
                level: "warn".into(),
                title: format!("大文件 >10MB ({})", large.len()),
                details: large,
            });
        }
    }

    // 已合并可删除分支
    let merged_out = git_exec(repo, &["branch", "--merged"], GIT_TIMEOUT).stdout.trim().to_string();
    if !merged_out.is_empty() {
        let current = git_exec(repo, &["branch", "--show-current"], GIT_TIMEOUT).stdout.trim().to_string();
        let mut merged: Vec<String> = Vec::new();
        for b in merged_out.split('\n') {
            let b = b.trim().strip_prefix("* ").unwrap_or(b.trim());
            if !b.is_empty() && b != current && b != "main" && b != "master" {
                merged.push(b.to_string());
            }
        }
        if !merged.is_empty() {
            items.push(HealthItem {
                level: "info".into(),
                title: format!("已合并可删除分支 ({})", merged.len()),
                details: merged,
            });
        }
    }

    // 过期分支 >30 天（Go 按自提交日零点起的耗时时长判断）
    let branches_out = git_exec(
        repo,
        &[
            "for-each-ref",
            "--sort=-committerdate",
            "--format=%(refname:short)|%(committerdate:short)",
            "refs/heads/",
        ],
        GIT_TIMEOUT,
    )
    .stdout
    .trim()
    .to_string();
    if !branches_out.is_empty() {
        let current = git_exec(repo, &["branch", "--show-current"], GIT_TIMEOUT).stdout.trim().to_string();
        let now = Local::now().naive_local();
        let mut stale: Vec<String> = Vec::new();
        for line in branches_out.split('\n') {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let Some((name, date_str)) = line.split_once('|') else { continue };
            if name == current {
                continue;
            }
            let Ok(d) = NaiveDate::parse_from_str(date_str, "%Y-%m-%d") else { continue };
            let elapsed_hours = (now - d.and_hms_opt(0, 0, 0).unwrap()).num_hours();
            if elapsed_hours > 30 * 24 {
                stale.push(format!(
                    "{} (最后提交: {}, {} 天前)",
                    name,
                    date_str,
                    elapsed_hours / 24
                ));
            }
        }
        if !stale.is_empty() {
            items.push(HealthItem {
                level: "warn".into(),
                title: format!("过期分支 >30天 ({})", stale.len()),
                details: stale,
            });
        }
    }

    // Stash 堆积
    let stash_out = git_exec(repo, &["stash", "list"], GIT_TIMEOUT).stdout.trim().to_string();
    if !stash_out.is_empty() {
        items.push(HealthItem {
            level: "info".into(),
            title: format!("Stash 堆积 ({})", stash_out.split('\n').count()),
            // Go 此处 details 为 nil（JSON null）；空数组语义等价且对前端更安全
            details: Vec::new(),
        });
    }

    // 远程未同步
    let upstream = git_exec(repo, &["rev-parse", "--abbrev-ref", "@{upstream}"], GIT_TIMEOUT)
        .stdout
        .trim()
        .to_string();
    if !upstream.is_empty() {
        let local = git_exec(repo, &["rev-parse", "HEAD"], GIT_TIMEOUT).stdout.trim().to_string();
        let remote = git_exec(repo, &["rev-parse", "@{upstream}"], GIT_TIMEOUT).stdout.trim().to_string();
        if local != remote {
            let behind = git_exec(repo, &["rev-list", "--count", "HEAD..@{upstream}"], GIT_TIMEOUT)
                .stdout
                .trim()
                .to_string();
            let ahead = git_exec(repo, &["rev-list", "--count", "@{upstream}..HEAD"], GIT_TIMEOUT)
                .stdout
                .trim()
                .to_string();
            items.push(HealthItem {
                level: "warn".into(),
                title: "远程未同步".into(),
                details: vec![format!("ahead {ahead}, behind {behind}")],
            });
        }
    }

    // 评分：0 警告 A，1 B+，2 B，>=3 C
    let warn_count = items.iter().filter(|i| i.level == "warn").count();
    let grade = match warn_count {
        0 => "A",
        1 => "B+",
        2 => "B",
        _ => "C",
    };
    if items.is_empty() {
        items.push(HealthItem {
            level: "ok".into(),
            title: "未发现异常".into(),
            details: Vec::new(),
        });
    }

    HealthResult { items, grade: grade.into() }
}

/// GetSize：仓库体积分析（按类型 Top10 + >5MB 大文件全列表）。
fn get_size(repo: &Path) -> SizeResult {
    let out = git_exec(repo, &["ls-files"], GIT_TIMEOUT);
    let ls = out.stdout.trim().to_string();
    if ls.is_empty() {
        // Go 返回零值 SizeResult（TotalSize 为空串）
        return SizeResult {
            total_size: String::new(),
            total_files: 0,
            by_type: Vec::new(),
            large_files: Vec::new(),
        };
    }

    let mut type_sizes: HashMap<String, i64> = HashMap::new();
    let mut type_counts: HashMap<String, i64> = HashMap::new();
    let mut total_size: i64 = 0;
    let mut large_files: Vec<LargeFile> = Vec::new();
    let mut total_files: i64 = 0;

    for path in ls.split('\n') {
        let path = path.trim();
        if path.is_empty() {
            continue;
        }
        let Ok(meta) = std::fs::metadata(repo.join(path)) else { continue };
        total_files += 1;
        total_size += meta.len() as i64;

        let mut ext = ext_lower(path);
        if ext.is_empty() {
            ext = "(无扩展名)".to_string();
        }
        *type_sizes.entry(ext.clone()).or_insert(0) += meta.len() as i64;
        *type_counts.entry(ext).or_insert(0) += 1;

        if meta.len() > 5 * 1024 * 1024 {
            large_files.push(LargeFile {
                path: path.to_string(),
                size: meta.len() as i64,
                size_str: format_file_size(meta.len() as i64),
            });
        }
    }

    let mut entries: Vec<(String, i64, i64)> = type_sizes
        .into_iter()
        .map(|(ext, size)| {
            let cnt = type_counts.get(&ext).copied().unwrap_or(0);
            (ext, cnt, size)
        })
        .collect();
    entries.sort_by(|a, b| b.2.cmp(&a.2));

    let by_type: Vec<SizeByType> = entries
        .into_iter()
        .take(10)
        .map(|(ext, count, size)| SizeByType {
            ext,
            count,
            size,
            size_str: format_file_size(size),
        })
        .collect();

    large_files.sort_by(|a, b| b.size.cmp(&a.size));

    SizeResult {
        total_size: format_file_size(total_size),
        total_files,
        by_type,
        large_files,
    }
}

/// ScanLargeFiles：按行数扫描大源码文件（默认 warn 300 / crit 500 行，
/// 只统计 git 跟踪的源码扩展名，跳过 <50 行，Top 50）。
fn scan_large_files(repo: &Path) -> LargeFilesResult {
    let warn_lines: i64 = 300;
    let crit_lines: i64 = 500;

    let out = git_exec(repo, &["ls-files"], GIT_TIMEOUT);
    let ls = out.stdout.trim().to_string();
    if ls.is_empty() {
        return LargeFilesResult { files: Vec::new(), warning: 0, critical: 0 };
    }

    const COUNTED_EXTS: [&str; 21] = [
        ".go", ".vue", ".ts", ".js", ".jsx", ".py", ".java", ".c", ".cpp", ".h", ".rs", ".rb",
        ".php", ".swift", ".kt", ".lua", ".sh", ".css", ".scss", ".html", ".tsx",
    ];

    let mut files: Vec<FileSizeEntry> = Vec::new();
    for rel_path in ls.split('\n') {
        let rel_path = rel_path.trim();
        if rel_path.is_empty() {
            continue;
        }
        let ext = ext_lower(rel_path);
        if !COUNTED_EXTS.contains(&ext.as_str()) {
            continue;
        }
        let abs = repo.join(rel_path);
        let Ok(meta) = std::fs::metadata(&abs) else { continue };
        if meta.is_dir() {
            continue;
        }
        let Ok(data) = std::fs::read(&abs) else { continue };
        let line_count = (data.iter().filter(|&&b| b == b'\n').count() + 1) as i64;
        if line_count < 50 {
            continue;
        }
        files.push(FileSizeEntry {
            path: rel_path.replace('\\', "/"),
            language: large_file_lang(&ext),
            lines: line_count,
            size: meta.len() as i64,
        });
    }

    files.sort_by(|a, b| b.lines.cmp(&a.lines));
    files.truncate(50);

    let mut warning = 0i64;
    let mut critical = 0i64;
    for f in &files {
        if f.lines >= crit_lines {
            critical += 1;
        } else if f.lines >= warn_lines {
            warning += 1;
        }
    }

    LargeFilesResult { files, warning, critical }
}

/// GetBranches：分支分组（当前/活跃/已合并/过期）。
fn get_branches(repo: &Path) -> BranchesResult {
    let out = git_exec(
        repo,
        &[
            "for-each-ref",
            "--sort=-committerdate",
            "--format=%(refname:short)|%(committerdate:iso)|%(upstream:track)",
            "refs/heads/",
        ],
        GIT_TIMEOUT,
    )
    .stdout
    .trim()
    .to_string();
    let current_branch = git_exec(repo, &["branch", "--show-current"], GIT_TIMEOUT)
        .stdout
        .trim()
        .to_string();

    // Go 在循环内逐分支执行 `branch --merged <cur>`（N+1 次调用）；
    // 结果只取决于 currentBranch，提到循环外一次执行，行为等价。
    let merged_out = if current_branch.is_empty() {
        String::new()
    } else {
        git_exec(repo, &["branch", "--merged", current_branch.as_str()], GIT_TIMEOUT)
            .stdout
            .trim()
            .to_string()
    };

    let mut current = Vec::new();
    let mut active = Vec::new();
    let mut merged = Vec::new();
    let mut stale = Vec::new();
    let now = Local::now().naive_local();

    for line in out.split('\n') {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let parts: Vec<&str> = line.splitn(3, '|').collect();
        if parts.len() < 2 {
            continue;
        }
        let name = parts[0];
        let date_str = parts[1];
        let is_current = name == current_branch;

        let t = parse_git_datetime(date_str);
        let (ahead, behind) = if parts.len() > 2 { parse_track(parts[2]) } else { (0, 0) };

        // Go 零时间 Format("2006-01-02") = "0001-01-01"
        let last_commit = t
            .map(|dt| dt.naive_local().format("%Y-%m-%d").to_string())
            .unwrap_or_else(|| "0001-01-01".to_string());
        // 过期：距最后提交超过 30 天（按提交时刻起的真实耗时时长）
        let is_stale = t
            .map(|dt| (now - dt.naive_local()).num_hours() > 30 * 24)
            .unwrap_or(false);

        let bi = BranchInfo {
            name: name.to_string(),
            is_current,
            last_commit,
            ahead,
            behind,
        };
        if is_current {
            current.push(bi);
        } else if merged_out.contains(name) {
            merged.push(bi);
        } else if is_stale {
            stale.push(bi);
        } else {
            active.push(bi);
        }
    }

    BranchesResult { current, active, merged, stale }
}

/// GetActivity：按天的提交活动（days>0 时限定最近 N 天）。
fn get_activity(repo: &Path, days: i64) -> Result<Vec<ActivityEntry>, String> {
    let mut args: Vec<String> = vec!["log".into(), "--format=%ad".into(), "--date=short".into()];
    if days > 0 {
        args.push(format!("--since={days}.days.ago"));
    }
    let refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    let res = git_exec(repo, &refs, GIT_TIMEOUT);
    if !res.success {
        return Err(format!("获取提交记录失败: {}", res.stderr.trim()));
    }

    let mut counts: HashMap<String, i64> = HashMap::new();
    for line in res.stdout.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        *counts.entry(line.to_string()).or_insert(0) += 1;
    }

    let mut dates: Vec<String> = counts.keys().cloned().collect();
    dates.sort();
    Ok(dates
        .into_iter()
        .map(|d| ActivityEntry { count: counts[&d], date: d })
        .collect())
}

/// GetAuthors：作者提交排行（days>0 时限定最近 N 天）。
fn get_authors(repo: &Path, days: i64) -> Result<Vec<AuthorEntry>, String> {
    let mut args: Vec<String> = vec!["shortlog".into(), "-sn".into(), "HEAD".into()];
    if days > 0 {
        args.push(format!("--since={days}.days.ago"));
    }
    let refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    let res = git_exec(repo, &refs, GIT_TIMEOUT);
    if !res.success {
        return Err(format!("获取作者统计失败: {}", res.stderr.trim()));
    }

    let mut result: Vec<AuthorEntry> = Vec::new();
    for line in res.stdout.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let Some((count, name)) = line.split_once('\t') else { continue };
        result.push(AuthorEntry {
            name: name.trim().to_string(),
            commits: count.trim().parse().unwrap_or(0),
        });
    }
    Ok(result)
}

/// GetChangelog：conventional commit 变更日志（from 为空时取全部 HEAD 历史）。
fn get_changelog(repo: &Path, from: &str) -> Result<Vec<ChangelogEntry>, String> {
    // GUI 侧固定 To="HEAD"：from 非空 → "from..HEAD"，否则 → "HEAD"
    let range = if !from.is_empty() {
        format!("{from}..HEAD")
    } else {
        "HEAD".to_string()
    };
    let res = git_exec(repo, &["log", "--format=%h|%ai|%an|%s", range.as_str()], GIT_TIMEOUT);
    if !res.success {
        return Err(format!("获取日志失败: {}", res.stderr.trim()));
    }
    if res.stdout.trim().is_empty() {
        return Ok(Vec::new());
    }

    let mut result: Vec<ChangelogEntry> = Vec::new();
    for line in res.stdout.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let parts: Vec<&str> = line.splitn(4, '|').collect();
        if parts.len() < 4 {
            continue;
        }
        let (hash, date, subject) = (parts[0], parts[1], parts[3]);
        let date = date.chars().take(10).collect::<String>();

        let mut entry = ChangelogEntry {
            hash: hash.to_string(),
            ty: String::new(),
            scope: String::new(),
            description: subject.to_string(),
            breaking: false,
            date,
        };
        match parse_conventional(subject) {
            Some((ty, scope, breaking, desc)) => {
                entry.ty = ty;
                entry.scope = scope;
                entry.breaking = breaking;
                entry.description = desc;
            }
            None => entry.ty = "other".into(),
        }
        result.push(entry);
    }
    Ok(result)
}

/// GetHotspots：文件变更热点（按提交数降序，limit<=0 取全部）。
fn get_hotspots(repo: &Path, days: i64, limit: i64) -> Result<Vec<HotspotEntry>, String> {
    let mut args: Vec<String> = vec!["log".into(), "--format=%h".into(), "--numstat".into()];
    if days > 0 {
        args.push(format!("--since={days}.days.ago"));
    }
    let refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    let res = git_exec(repo, &refs, GIT_TIMEOUT);
    if !res.success {
        return Err(format!("获取日志失败: {}", res.stderr.trim()));
    }
    if res.stdout.trim().is_empty() {
        return Ok(Vec::new());
    }

    let mut file_data: HashMap<String, HotspotEntry> = HashMap::new();
    for line in res.stdout.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        // 提交哈希行（%h 通常 7 字符且无制表符）
        if line.len() == 7 && !line.contains('\t') {
            continue;
        }
        let parts: Vec<&str> = line.splitn(3, '\t').collect();
        if parts.len() < 3 {
            continue;
        }
        let (Ok(adds), Ok(dels)) = (parts[0].parse::<i64>(), parts[1].parse::<i64>()) else {
            continue; // 二进制文件显示为 "-"
        };
        let file = parts[2].trim();
        let entry = file_data.entry(file.to_string()).or_insert(HotspotEntry {
            file: file.to_string(),
            commits: 0,
            additions: 0,
            deletions: 0,
        });
        entry.commits += 1;
        entry.additions += adds;
        entry.deletions += dels;
    }

    let mut result: Vec<HotspotEntry> = file_data.into_values().collect();
    result.sort_by(|a, b| b.commits.cmp(&a.commits));

    let limit = if limit <= 0 || limit > result.len() as i64 {
        result.len()
    } else {
        limit as usize
    };
    result.truncate(limit);
    Ok(result)
}

/// GetTodos：TODO/FIXME/HACK/XXX 标记扫描（限定常见源码扩展名）。
fn get_todos(repo: &Path) -> Vec<TodoItem> {
    let res = git_exec(
        repo,
        &[
            "grep",
            "-n",
            "-i",
            "-E",
            r"(TODO|FIXME|HACK|XXX)[:\s]",
            "--",
            "*.go",
            "*.js",
            "*.ts",
            "*.py",
            "*.java",
            "*.rs",
            "*.c",
            "*.cpp",
            "*.h",
            "*.lua",
            "*.sh",
        ],
        GIT_TIMEOUT,
    );
    // Go：err（含无匹配的 exit 1）或输出为空 → 空结果
    if !res.success || res.stdout.trim().is_empty() {
        return Vec::new();
    }

    let mut result: Vec<TodoItem> = Vec::new();
    for line in res.stdout.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let parts: Vec<&str> = line.splitn(3, ':').collect();
        if parts.len() < 3 {
            continue;
        }
        let line_num = parts[1].trim().parse::<i64>().unwrap_or(0);
        let content = parts[2];
        let (tag, text) = match find_todo_tag(content) {
            Some((tag, text)) => (tag, text),
            None => ("TODO".to_string(), content.to_string()),
        };
        result.push(TodoItem {
            tag,
            file: parts[0].to_string(),
            line: line_num,
            content: text,
        });
    }
    result
}

/// 贡献者聚合的中间结构。
struct AuthorAgg {
    commits: i64,
    dates: HashSet<String>,
    recent30: i64,
    first: String,
    last: String,
}

/// GetStats：跨仓库开发统计（dir 为仓库或包含多个仓库的目录）。
/// since/until 为可选的日期边界；author 为可选的作者过滤。
fn get_stats(dir: &Path, since: &str, until: &str, author: &str) -> Result<StatsResult, String> {
    let repos = scan_repos(dir).map_err(|e| format!("扫描目录失败: {e}"))?;

    let mut date_args: Vec<String> = Vec::new();
    if !since.is_empty() {
        date_args.push(format!("--since={since}"));
    }
    if !until.is_empty() {
        date_args.push(format!("--until={until}"));
    }

    let mut total_commits: i64 = 0;
    let mut hours = vec![0i64; 24];
    let mut weekday_commits = vec![0i64; 7];
    let mut date_commits: HashMap<String, i64> = HashMap::new();
    let mut repo_list: Vec<RepoStat> = Vec::new();
    let mut lang_sizes: HashMap<String, i64> = HashMap::new();

    for repo in &repos {
        // 每仓库提交时间分布（受 since/until 与 author 过滤）
        let mut args: Vec<String> = vec![
            "log".into(),
            "--format=%ad".into(),
            "--date=iso".into(),
            "--no-merges".into(),
        ];
        if !author.is_empty() {
            args.push("--author".into());
            args.push(author.to_string());
        }
        args.extend(date_args.iter().cloned());
        let refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        let res = git_exec(repo, &refs, GIT_TIMEOUT);
        if !res.success || res.stdout.trim().is_empty() {
            continue;
        }

        let mut count: i64 = 0;
        for line in res.stdout.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let Some(t) = parse_git_datetime(line) else { continue };
            count += 1;
            hours[t.naive_local().hour() as usize] += 1;
            weekday_commits[t.naive_local().weekday().num_days_from_sunday() as usize] += 1;
            *date_commits
                .entry(t.naive_local().format("%Y-%m-%d").to_string())
                .or_insert(0) += 1;
        }

        if count > 0 {
            repo_list.push(RepoStat { name: path_base_name(repo), commits: count });
            total_commits += count;
        }

        // 语言分布（git 跟踪文件的磁盘体积，跳过非代码扩展名）
        let ls = git_exec(repo, &["ls-files"], GIT_TIMEOUT).stdout;
        for f in ls.lines() {
            let f = f.trim();
            if f.is_empty() {
                continue;
            }
            let Ok(meta) = std::fs::metadata(repo.join(f)) else { continue };
            let ext = ext_lower(f);
            if is_excluded_ext(&ext) {
                continue;
            }
            *lang_sizes.entry(stats_lang_name(&ext)).or_insert(0) += meta.len() as i64;
        }
    }

    let streak = calc_streak(&date_commits);
    let longest_streak = calc_longest_streak(&date_commits);
    let active_days = date_commits.len() as i64;

    // 按天提交序列（升序）
    let mut daily: Vec<DailyEntry> = date_commits
        .iter()
        .map(|(d, c)| DailyEntry { date: d.clone(), count: *c })
        .collect();
    daily.sort_by(|a, b| a.date.cmp(&b.date));

    // 按天×小时的分布（全历史，忽略 since/until，仍受 author 过滤），
    // 供热力图任意年份切换
    let mut full_day_hour: HashMap<String, [i64; 24]> = HashMap::new();
    for repo in &repos {
        let mut args: Vec<String> = vec![
            "log".into(),
            "--format=%ad".into(),
            "--date=iso".into(),
            "--no-merges".into(),
        ];
        if !author.is_empty() {
            args.push("--author".into());
            args.push(author.to_string());
        }
        let refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        let res = git_exec(repo, &refs, GIT_TIMEOUT);
        if !res.success || res.stdout.trim().is_empty() {
            continue;
        }
        for line in res.stdout.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let Some(t) = parse_git_datetime(line) else { continue };
            let day_key = t.naive_local().format("%Y-%m-%d").to_string();
            let dh = full_day_hour.entry(day_key).or_insert([0i64; 24]);
            dh[t.naive_local().hour() as usize] += 1;
        }
    }
    let mut day_hour: Vec<DayHourEntry> = full_day_hour
        .into_iter()
        .map(|(date, h)| {
            let count = h.iter().sum();
            DayHourEntry { date, hours: h, count }
        })
        .collect();
    day_hour.sort_by(|a, b| a.date.cmp(&b.date));

    // 贡献者排行（全员，忽略 author 过滤，受 since/until 约束）
    let cutoff_ts = (Local::now() - chrono::Duration::days(30)).timestamp();
    let mut stats: HashMap<String, AuthorAgg> = HashMap::new();
    for repo in &repos {
        let mut args: Vec<String> = vec![
            "log".into(),
            "--format=%an%x1f%ad".into(),
            "--date=iso".into(),
            "--no-merges".into(),
        ];
        args.extend(date_args.iter().cloned());
        let refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        let res = git_exec(repo, &refs, GIT_TIMEOUT);
        if !res.success {
            continue;
        }
        for line in res.stdout.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let Some((name, date)) = line.split_once('\x1f') else { continue };
            let name = name.trim();
            let Some(t) = parse_git_datetime(date.trim()) else { continue };
            let day_key = t.naive_local().format("%Y-%m-%d").to_string();
            let s = stats.entry(name.to_string()).or_insert(AuthorAgg {
                commits: 0,
                dates: HashSet::new(),
                recent30: 0,
                first: String::new(),
                last: String::new(),
            });
            s.commits += 1;
            s.dates.insert(day_key.clone());
            if t.timestamp() >= cutoff_ts {
                s.recent30 += 1;
            }
            if s.first.is_empty() || day_key < s.first {
                s.first = day_key.clone();
            }
            if s.last.is_empty() || day_key > s.last {
                s.last = day_key;
            }
        }
    }
    let mut authors: Vec<AuthorStatEntry> = stats
        .into_iter()
        .map(|(name, s)| {
            // 连续天数：从最近提交日逐日回退
            let mut streak = 0i64;
            if !s.last.is_empty() {
                if let Ok(mut d) = NaiveDate::parse_from_str(&s.last, "%Y-%m-%d") {
                    while s.dates.contains(&d.format("%Y-%m-%d").to_string()) {
                        streak += 1;
                        d -= chrono::Duration::days(1);
                    }
                }
            }
            AuthorStatEntry {
                name,
                commits: s.commits,
                active_days: s.dates.len() as i64,
                recent30: s.recent30,
                streak,
                first_commit: s.first,
                last_commit: s.last,
            }
        })
        .collect();
    authors.sort_by(|a, b| b.commits.cmp(&a.commits));

    // 语言分布 Top 8（分母 +1 对齐 Go）
    let total_lang_size: i64 = lang_sizes.values().sum();
    let mut lang_entries: Vec<(String, i64)> = lang_sizes.into_iter().collect();
    lang_entries.sort_by(|a, b| b.1.cmp(&a.1));
    let languages: Vec<LangEntry> = lang_entries
        .into_iter()
        .take(8)
        .map(|(name, size)| LangEntry {
            name,
            pct: size as f64 / (total_lang_size as f64 + 1.0) * 100.0,
            size,
        })
        .collect();

    repo_list.sort_by(|a, b| b.commits.cmp(&a.commits));

    Ok(StatsResult {
        total_commits,
        active_days,
        current_streak: streak,
        longest_streak,
        weekday_activity: weekday_commits,
        hour_activity: hours,
        daily_activity: daily,
        day_hour_activity: day_hour,
        authors,
        top_repos: repo_list,
        languages,
    })
}

// ---------------------------------------------------------------------------
// 命令（Go InfoModule/StatsGet 绑定）
// ---------------------------------------------------------------------------

/// InfoSummary 仓库概览（文件数/体积/提交数/贡献者/分支/标签/时间线/语言/Top 作者）。
#[tauri::command]
pub async fn info_summary(repo: String) -> Result<SummaryResult, String> {
    let repo = repo_path(Some(repo));
    run_blocking(move || Ok(get_summary(&repo)))?
}

/// InfoHealth 仓库健康检查（脏工作区/大文件/可删分支/过期分支/Stash/远程同步 + 评分）。
#[tauri::command]
pub async fn info_health(repo: String) -> Result<HealthResult, String> {
    let repo = repo_path(Some(repo));
    run_blocking(move || Ok(get_health(&repo)))?
}

/// InfoLargeFiles 按行数扫描大源码文件（警告 300 行 / 严重 500 行，Top 50）。
#[tauri::command]
pub async fn info_large_files(repo: String) -> Result<LargeFilesResult, String> {
    let repo = repo_path(Some(repo));
    run_blocking(move || Ok(scan_large_files(&repo)))?
}

/// InfoSize 仓库体积分析（总量/按类型 Top10/>5MB 大文件列表）。
#[tauri::command]
pub async fn info_size(repo: String) -> Result<SizeResult, String> {
    let repo = repo_path(Some(repo));
    run_blocking(move || Ok(get_size(&repo)))?
}

/// InfoBranches 分支分组（当前/活跃/已合并/过期 >30 天）。
#[tauri::command]
pub async fn info_branches(repo: String) -> Result<BranchesResult, String> {
    let repo = repo_path(Some(repo));
    run_blocking(move || Ok(get_branches(&repo)))?
}

/// InfoActivity 按天提交活动（days>0 限定最近 N 天）。
#[tauri::command]
pub async fn info_activity(repo: String, days: i64) -> Result<Vec<ActivityEntry>, String> {
    let repo = repo_path(Some(repo));
    run_blocking(move || get_activity(&repo, days))?
}

/// InfoAuthors 作者提交排行（days>0 限定最近 N 天）。
#[tauri::command]
pub async fn info_authors(repo: String, days: i64) -> Result<Vec<AuthorEntry>, String> {
    let repo = repo_path(Some(repo));
    get_authors(&repo, days)
}

/// InfoChangelog conventional commit 变更日志（from 为标签/哈希，空则全部）。
#[tauri::command]
pub async fn info_changelog(repo: String, from: String) -> Result<Vec<ChangelogEntry>, String> {
    let repo = repo_path(Some(repo));
    run_blocking(move || get_changelog(&repo, &from))?
}

/// InfoHotspots 文件变更热点（days>0 限定最近 N 天；limit<=0 取全部）。
#[tauri::command]
pub async fn info_hotspots(repo: String, days: i64, limit: i64) -> Result<Vec<HotspotEntry>, String> {
    let repo = repo_path(Some(repo));
    run_blocking(move || get_hotspots(&repo, days, limit))?
}

/// InfoTodos TODO/FIXME/HACK/XXX 标记扫描。
#[tauri::command]
pub async fn info_todos(repo: String) -> Result<Vec<TodoItem>, String> {
    let repo = repo_path(Some(repo));
    run_blocking(move || Ok(get_todos(&repo)))?
}

/// StatsGet 跨仓库开发统计（dir 为仓库或仓库父目录；since/until/author 可选）。
#[tauri::command]
pub async fn stats_get(
    dir: String,
    since: String,
    until: String,
    author: String,
) -> Result<StatsResult, String> {
    // Go 直传 dir（空串时按 CWD 扫描）；这里经 state 解析空串，
    // 与其它命令的回落语义保持一致
    let dir = repo_path(Some(dir));
    run_blocking(move || get_stats(&dir, &since, &until, &author))?
}
