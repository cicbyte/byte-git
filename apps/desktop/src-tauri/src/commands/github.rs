//! github 域命令：GitHub REST API 集成（Actions / Issues / PR / Release /
//! Search / Trending）。
//!
//! 移植自 git-mate：
//! - `gui/internal/github/*.go`（GUI 命令层：actions.go / issues.go / pr.go /
//!   pr_detail.go / releases.go / search.go / trending.go）
//! - `internal/gh/*.go`（REST 细节：gh.go 的 token/remote 解析、actions.go 的
//!   per_page 与日志 zip、pr.go / releases.go / search.go 的端点与 DTO、
//!   trending.go + go-trending 的 HTML 抓取）
//!
//! 方法对照（Go → Rust 命令，尾部 * 表示 Go 无 repo 参数、Rust 追加）：
//! | Go 方法 | Rust 命令 |
//! |---|---|
//! | GhActionsList / GhActionsJobs / GhActionsJobLogs / GhActionsRerun / GhActionsCancel | gh_actions_list / gh_actions_jobs / gh_actions_job_logs / gh_actions_rerun / gh_actions_cancel |
//! | GhIssueList / GhIssueCreate / GhIssueClose / GhIssueComment / GhIssueLabels / GhIssueAddLabels / GhIssueRemoveLabel | gh_issue_list / gh_issue_create / gh_issue_close / gh_issue_comment / gh_issue_labels / gh_issue_add_labels / gh_issue_remove_label |
//! | GhPRList / GhPRCreate / GhPRComments / GhPRCreateComment / GhPRFiles / GhPRReview / GhPRMerge / GhPRCheckout | gh_pr_list / gh_pr_create / gh_pr_comments / gh_pr_create_comment / gh_pr_files / gh_pr_review / gh_pr_merge / gh_pr_checkout |
//! | GhReleaseList / GhReleaseCreate / GhReleaseDelete / GhReleaseDeleteAsset / GhReleaseUploadAsset | gh_release_list / gh_release_create / gh_release_delete / gh_release_delete_asset / gh_release_upload_asset |
//! | GhSearchRepos / GhSearchCode / GhTrendingProjects / GhTrendingLanguages / GhSpokenLanguages | gh_search_repos / gh_search_code / gh_trending_projects / gh_trending_languages / gh_spoken_languages |
//!
//! 与 Go 的差异：
//! - REST 不走 go-github，统一 `gh_request`（token 注入 + User-Agent + 4xx
//!   message 解析）；DTO 字段与 Go json tag 一致。
//! - 非 GitHub remote 时 Go 返回 (nil, nil)（JSON null），这里返回空数组。
//! - token：config.yaml `github.token`（Go 唯一来源），为空回落 GITHUB_TOKEN
//!   环境变量（超集，便于 CI 覆盖）。
//! - trending 用 go-trending 同款 URL（?since=&l=&spoken_language_code=）抓
//!   github.com/trending HTML，字符串切割解析（无 HTML 依赖）。
//! - 日志 zip 解包：zip crate 已移除，改用 flate2 手写最小 local file header
//!   读取，语义对齐 Go unzipLogs（拼接所有 .txt）。

use byte_git_core::config::Settings;
use serde::Serialize;
use serde_json::{json, Value};
use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const API_BASE: &str = "https://api.github.com";
const UPLOADS_BASE: &str = "https://uploads.github.com";
const GH_ACCEPT: &str = "application/vnd.github+json";
/// Search.Code 需要这个 Accept 才返回 text_matches 片段。
const ACCEPT_TEXT_MATCH: &str = "application/vnd.github.text-match+json";
const USER_AGENT: &str = "byte-git";
/// 趋势页抓取 UA（github.com 对默认 UA 不友好）。
const HTML_UA: &str = "Mozilla/5.0 (compatible; byte-git)";

/// 对应 Go 侧 15s ctx 的短超时（变更类操作）。
const T_SHORT: Duration = Duration::from_secs(15);
/// 对应 Go 侧 30s ctx 的标准超时（列表/查询）。
const T_STD: Duration = Duration::from_secs(30);
/// 日志下载（Go 60s ctx）。
const T_LOGS: Duration = Duration::from_secs(60);
/// 资产上传（Go 5min ctx）。
const T_UPLOAD: Duration = Duration::from_secs(300);
/// git remote 检测的宽松超时。
const GIT_TIMEOUT: Duration = Duration::from_secs(30);

// ---------------------------------------------------------------------------
// 基础设施（run_blocking / git / token / REST）
// ---------------------------------------------------------------------------

/// 在独立线程执行阻塞任务。reqwest::blocking 不能在 tokio 异步上下文里
/// 运行（会 panic），因此把阻塞工作挪到专用 std 线程，命令本身保持 async。
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

/// Windows 下避免子进程闪现控制台窗口（对应 sysproc 的 CREATE_NO_WINDOW）。
#[cfg(windows)]
fn hide_console(cmd: &mut Command) {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    cmd.creation_flags(CREATE_NO_WINDOW);
}

#[cfg(not(windows))]
fn hide_console(_cmd: &mut Command) {}

/// 空串回落到 "."（对应 Go 的 CurrentRepoOrDot 回落语义）。
fn repo_or_dot(repo: &str) -> &str {
    if repo.is_empty() {
        "."
    } else {
        repo
    }
}

/// 在仓库目录执行 `git -C <repo> <args...>`，成功返回 trim 后的 stdout。
/// 隐藏窗口 + 超时 + 失败时回传 stderr（对应 sysproc.Command().Output()）。
fn git_output(repo: &str, args: &[&str], timeout: Duration) -> Result<String, String> {
    let mut cmd = Command::new("git");
    cmd.arg("-C")
        .arg(repo_or_dot(repo))
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    hide_console(&mut cmd);

    let mut child = cmd.spawn().map_err(|e| format!("启动 git 失败: {e}"))?;
    let deadline = Instant::now() + timeout;
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if Instant::now() >= deadline => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!("git {} 超时", args.join(" ")));
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(15)),
            Err(e) => return Err(format!("等待 git 进程失败: {e}")),
        }
    }
    let out = child.wait_with_output().map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// GitHub Token：config.yaml 的 `github.token`（Go 侧唯一来源，加载失败视
/// 为空 token，与 Go `cfg, _ := config.LoadSettings()` 一致）；为空回落
/// GITHUB_TOKEN 环境变量（超集）。对应 Go NewClient 的空 token 报错。
fn gh_token() -> Result<String, String> {
    let mut token = Settings::load().unwrap_or_default().github.token;
    if token.is_empty() {
        token = std::env::var("GITHUB_TOKEN").unwrap_or_default();
    }
    if token.is_empty() {
        return Err("GitHub Token 未配置".into());
    }
    Ok(token)
}

/// git remote get-url origin → "owner/repo"；非 GitHub remote 返回 None
/// （对应 Go DetectStatus 的 remote 部分）。
fn detect_remote(repo: &str) -> Option<String> {
    let url = git_output(repo, &["remote", "get-url", "origin"], GIT_TIMEOUT).ok()?;
    parse_github_url(&url)
}

/// 对应 Go parseGitHubURL：匹配 SSH `git@github.com:o/r` 与 HTTPS
/// `https://github.com/o/r`（可带 .git 后缀 / 尾部斜杠）。
fn parse_github_url(url: &str) -> Option<String> {
    let idx = url.find("github.com")? + "github.com".len();
    let rest = &url[idx..];
    let rest = rest.strip_prefix(':').or_else(|| rest.strip_prefix('/'))?;
    let (owner, repo) = rest.split_once('/')?;
    let repo = repo.split('/').next().unwrap_or(repo);
    let repo = repo.strip_suffix(".git").unwrap_or(repo);
    if owner.is_empty() || repo.is_empty() {
        return None;
    }
    Some(format!("{owner}/{repo}"))
}

/// "owner/repo" → (owner, repo)。对应 Go splitOwnerRepo + 调用方的
/// "无法从 remote 解析 owner/repo" 报错。
fn split_owner_repo(owner_repo: &str) -> Result<(String, String), String> {
    let (o, r) = owner_repo
        .split_once('/')
        .ok_or_else(|| "无法从 remote 解析 owner/repo".to_string())?;
    if o.is_empty() || r.is_empty() {
        return Err("无法从 remote 解析 owner/repo".into());
    }
    Ok((o.to_string(), r.to_string()))
}

/// 最小 URL 百分号编码（路径段 / query 值通用）。
fn url_encode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// 发送 GitHub REST 请求，返回原始字节。注入 Bearer token、User-Agent、
/// Accept；自动跟随重定向（Actions 日志 302 → 签名下载地址）；4xx/5xx 时
/// 解析响应 body 里的 message（GitHub 错误体）。
#[allow(clippy::too_many_arguments)]
fn gh_send(
    method: &str,
    url: &str,
    params: &[(&str, String)],
    body: Option<Vec<u8>>,
    content_type: Option<&str>,
    token: &str,
    accept: &str,
    timeout: Duration,
) -> Result<Vec<u8>, String> {
    let client = reqwest::blocking::Client::builder()
        .timeout(timeout)
        .user_agent(USER_AGENT)
        .build()
        .map_err(|e| format!("创建 HTTP 客户端失败: {e}"))?;
    let m = reqwest::Method::from_bytes(method.as_bytes()).map_err(|e| e.to_string())?;
    let mut req = client
        .request(m, url)
        .query(params)
        .header(reqwest::header::ACCEPT, accept)
        .header(reqwest::header::AUTHORIZATION, format!("Bearer {token}"));
    if let Some(b) = body {
        req = req
            .header(
                reqwest::header::CONTENT_TYPE,
                content_type.unwrap_or("application/json"),
            )
            .body(b);
    }
    let resp = req.send().map_err(|e| format!("请求 GitHub 失败: {e}"))?;
    let status = resp.status();
    let bytes = resp.bytes().map_err(|e| format!("读取响应失败: {e}"))?;
    if !status.is_success() {
        let msg = serde_json::from_slice::<Value>(&bytes)
            .ok()
            .and_then(|v| v.get("message").and_then(|m| m.as_str()).map(str::to_string))
            .unwrap_or_else(|| String::from_utf8_lossy(&bytes).trim().to_string());
        return Err(format!("GitHub API {status}: {msg}"));
    }
    Ok(bytes.to_vec())
}

/// gh_send 的 JSON 封装：body 为 Value（None 表示无 body），空响应体 → Null。
fn gh_request(
    method: &str,
    url: &str,
    params: &[(&str, String)],
    body: Option<Value>,
    token: &str,
    accept: &str,
    timeout: Duration,
) -> Result<Value, String> {
    let raw = gh_send(
        method,
        url,
        params,
        body.map(|b| serde_json::to_vec(&b).unwrap_or_default()),
        Some("application/json"),
        token,
        accept,
        timeout,
    )?;
    if raw.is_empty() {
        return Ok(Value::Null);
    }
    serde_json::from_slice(&raw).map_err(|e| format!("解析 GitHub 响应失败: {e}"))
}

/// 抓取 github.com/trending 的 HTML（无 token，浏览器 UA）。
fn fetch_html(url: &str, params: &[(&str, String)]) -> Result<String, String> {
    let client = reqwest::blocking::Client::builder()
        .timeout(T_STD)
        .user_agent(HTML_UA)
        .build()
        .map_err(|e| format!("创建 HTTP 客户端失败: {e}"))?;
    let resp = client
        .get(url)
        .query(params)
        .send()
        .map_err(|e| format!("请求 github.com 失败: {e}"))?;
    let status = resp.status();
    let text = resp.text().map_err(|e| format!("读取响应失败: {e}"))?;
    if !status.is_success() {
        return Err(format!("github.com {status}"));
    }
    Ok(text)
}

// ---------------------------------------------------------------------------
// Value / 日期 / HTML 小工具
// ---------------------------------------------------------------------------

fn vs(v: &Value, k: &str) -> String {
    v.get(k).and_then(Value::as_str).unwrap_or("").to_string()
}
fn v64(v: &Value, k: &str) -> i64 {
    v.get(k).and_then(Value::as_i64).unwrap_or(0)
}
fn v32(v: &Value, k: &str) -> i32 {
    v.get(k).and_then(Value::as_i64).unwrap_or(0) as i32
}
fn vbool(v: &Value, k: &str) -> bool {
    v.get(k).and_then(Value::as_bool).unwrap_or(false)
}
/// 嵌套取字符串，如 /user/login。
fn vptr(v: &Value, ptr: &str) -> String {
    v.pointer(ptr)
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string()
}

/// GitHub API 的 ISO8601（UTC："2024-01-02T15:04:05Z"）→ "2024-01-02"
/// （对应 Go Format("2006-01-02")）。
fn fmt_date(iso: &str) -> String {
    if iso.len() >= 10 {
        iso[..10].to_string()
    } else {
        String::new()
    }
}
/// → "01-02 15:04"（对应 Go Format("01-02 15:04")）。
fn fmt_md_hm(iso: &str) -> String {
    if iso.len() >= 16 {
        format!("{} {}", &iso[5..7], &iso[11..16])
    } else {
        String::new()
    }
}
/// → "2024-01-02 15:04"（对应 Go Format("2006-01-02 15:04")）。
fn fmt_dt_hm(iso: &str) -> String {
    if iso.len() >= 16 {
        format!("{} {}", &iso[..10], &iso[11..16])
    } else {
        String::new()
    }
}

fn strip_tags(s: &str) -> String {
    let mut out = String::new();
    let mut in_tag = false;
    for c in s.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    out
}

fn decode_entities(s: &str) -> String {
    s.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&#x27;", "'")
        .replace("&#x2F;", "/")
}

/// 标签剥离 + 实体解码 + 空白折叠。
fn html_text(s: &str) -> String {
    decode_entities(&strip_tags(s))
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// 从 HTML 标签里取属性值（已解码实体）。
fn extract_attr(tag: &str, name: &str) -> Option<String> {
    let pat = format!("{name}=\"");
    let i = tag.find(&pat)? + pat.len();
    let rest = &tag[i..];
    let j = rest.find('"')?;
    Some(decode_entities(&rest[..j]))
}

// ---------------------------------------------------------------------------
// 日志 zip 解包（flate2 手写最小 zip 读取，对齐 Go unzipLogs）
// ---------------------------------------------------------------------------

/// 遍历 zip local file header（签名 PK\x03\x04），拼接所有 `.txt` 条目：
/// `=== 文件名 ===\n内容\n`。支持 STORED（方式 0）与 DEFLATE（方式 8）。
/// 带数据描述符（压缩大小为 0）的条目：DEFLATE 喂剩余字节（解码器到流尾
/// 即停）；GitHub Actions 日志 zip 均为固定大小条目，正常走精确切片。
fn unzip_logs(data: &[u8]) -> Result<String, String> {
    fn u16le(d: &[u8], o: usize) -> usize {
        u16::from_le_bytes([d[o], d[o + 1]]) as usize
    }
    fn u32le(d: &[u8], o: usize) -> usize {
        u32::from_le_bytes([d[o], d[o + 1], d[o + 2], d[o + 3]]) as usize
    }
    if data.len() < 4 || &data[..4] != b"PK\x03\x04" {
        return Err("不是有效的 zip 数据".into());
    }
    let mut out = String::new();
    let mut pos = 0usize;
    while pos + 30 <= data.len() && &data[pos..pos + 4] == b"PK\x03\x04" {
        let method = u16le(data, pos + 8);
        let csize = u32le(data, pos + 18);
        let nlen = u16le(data, pos + 26);
        let elen = u16le(data, pos + 28);
        let name_start = pos + 30;
        if name_start + nlen > data.len() {
            break;
        }
        let name = String::from_utf8_lossy(&data[name_start..name_start + nlen]).to_string();
        let data_start = name_start + nlen + elen;
        if data_start > data.len() {
            break;
        }
        if name.ends_with(".txt") {
            let raw: &[u8] = if csize > 0 && data_start + csize <= data.len() {
                &data[data_start..data_start + csize]
            } else {
                // 数据描述符场景：剩余字节兜底（DEFLATE 自终止）
                &data[data_start..]
            };
            let content = match method {
                0 => String::from_utf8_lossy(raw).to_string(),
                8 => {
                    let mut s = String::new();
                    let _ = flate2::read::DeflateDecoder::new(raw).read_to_string(&mut s);
                    s
                }
                _ => String::new(),
            };
            out.push_str(&format!("=== {name} ===\n{content}\n"));
        }
        if csize > 0 {
            pos = data_start + csize;
        } else {
            break; // 无法定位下一条目
        }
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// DTO（json tag 与 Go 结构体逐一对齐）
// ---------------------------------------------------------------------------

/// gh.WorkflowRun：CI 运行条目。
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowRun {
    pub id: i64,
    pub name: String,
    /// queued / in_progress / completed
    pub status: String,
    /// success / failure / cancelled / 空（运行中）
    pub conclusion: String,
    pub branch: String,
    pub commit: String,
    /// push / pull_request / manual
    pub event: String,
    pub actor: String,
    pub created_at: String,
    pub html_url: String,
}

/// gh.WorkflowJob：CI 运行内的单个 job。
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowJob {
    pub id: i64,
    pub name: String,
    pub status: String,
    pub conclusion: String,
    pub started_at: String,
    pub html_url: String,
}

/// gh.Issue。
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Issue {
    pub number: i32,
    pub title: String,
    pub state: String,
    pub author: String,
    pub body: String,
    pub url: String,
    pub labels: Vec<String>,
    pub comments: i32,
    pub created_at: String,
    pub updated_at: String,
}

/// gh.Label。
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Label {
    pub name: String,
    pub color: String,
    pub description: String,
}

/// gh.PullRequest。
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PullRequest {
    pub number: i32,
    pub title: String,
    pub state: String,
    pub is_draft: bool,
    pub author: String,
    pub head_ref: String,
    pub base_ref: String,
    pub updated_at: String,
    pub url: String,
    pub body: String,
}

/// gh.PRFile：PR 内变更文件。
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PRFile {
    pub filename: String,
    /// added / removed / modified / renamed
    pub status: String,
    pub additions: i32,
    pub deletions: i32,
    /// unified diff hunk
    pub patch: String,
}

/// gh.PRCommentItem。
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PRCommentItem {
    pub id: i64,
    pub author: String,
    pub body: String,
    pub path: String,
    pub line: i32,
    pub created_at: String,
}

/// gh.Asset。
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Asset {
    pub id: i64,
    pub name: String,
    pub size: i64,
    pub url: String,
}

/// gh.Release。
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Release {
    pub id: i64,
    pub tag_name: String,
    pub name: String,
    /// release notes（markdown）
    pub body: String,
    pub draft: bool,
    pub pre_release: bool,
    pub url: String,
    pub created_at: String,
    pub assets: Vec<Asset>,
}

/// gh.CodeResult。
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CodeResult {
    pub name: String,
    pub path: String,
    pub repo: String,
    pub html_url: String,
    /// 命中代码片段（text_matches[0].fragment，含 <mark> 标签，透传）
    pub fragment: String,
}

/// gh.RepoResult。
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepoResult {
    pub full_name: String,
    pub description: String,
    pub stars: i32,
    pub language: String,
    pub html_url: String,
}

/// gh.TrendingProject。
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrendingProject {
    pub name: String,
    pub owner: String,
    pub repo: String,
    pub description: String,
    pub language: String,
    /// stargazers 链接上的星数
    pub stars: i32,
    pub url: String,
}

/// gh.TrendingLanguage（语言筛选项，也是口音语言筛选项的形状）。
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrendingLanguage {
    pub name: String,
    pub url_name: String,
}

// ---------------------------------------------------------------------------
// Value → DTO 映射（对齐 Go 各 List/Create 的取值与回退逻辑）
// ---------------------------------------------------------------------------

fn workflow_run_dto(r: &Value) -> WorkflowRun {
    let mut name = vs(r, "name");
    if name.is_empty() {
        name = vs(r, "display_title");
    }
    if name.is_empty() {
        name = vptr(r, "/head_repository/full_name");
    }
    let sha = vs(r, "head_sha");
    WorkflowRun {
        id: v64(r, "id"),
        name,
        status: vs(r, "status"),
        conclusion: vs(r, "conclusion"),
        branch: vs(r, "head_branch"),
        commit: if sha.len() > 7 { sha[..7].to_string() } else { sha },
        event: vs(r, "event"),
        actor: vptr(r, "/actor/login"),
        created_at: fmt_md_hm(&vs(r, "created_at")),
        html_url: vs(r, "html_url"),
    }
}

fn workflow_job_dto(j: &Value) -> WorkflowJob {
    WorkflowJob {
        id: v64(j, "id"),
        name: vs(j, "name"),
        status: vs(j, "status"),
        conclusion: vs(j, "conclusion"),
        started_at: fmt_md_hm(&vs(j, "started_at")),
        html_url: vs(j, "html_url"),
    }
}

fn issue_dto(v: &Value) -> Issue {
    Issue {
        number: v32(v, "number"),
        title: vs(v, "title"),
        state: vs(v, "state"),
        author: vptr(v, "/user/login"),
        body: vs(v, "body"),
        url: vs(v, "html_url"),
        labels: v
            .get("labels")
            .and_then(Value::as_array)
            .map(|a| a.iter().map(|l| vs(l, "name")).collect())
            .unwrap_or_default(),
        comments: v32(v, "comments"),
        created_at: fmt_date(&vs(v, "created_at")),
        updated_at: fmt_date(&vs(v, "updated_at")),
    }
}

fn label_dto(l: &Value) -> Label {
    Label {
        name: vs(l, "name"),
        color: vs(l, "color"),
        description: vs(l, "description"),
    }
}

fn pull_request_dto(p: &Value) -> PullRequest {
    PullRequest {
        number: v32(p, "number"),
        title: vs(p, "title"),
        state: vs(p, "state"),
        is_draft: vbool(p, "draft"),
        author: vptr(p, "/user/login"),
        head_ref: vptr(p, "/head/ref"),
        base_ref: vptr(p, "/base/ref"),
        updated_at: fmt_dt_hm(&vs(p, "updated_at")),
        url: vs(p, "html_url"),
        body: vs(p, "body"),
    }
}

fn pr_file_dto(f: &Value) -> PRFile {
    PRFile {
        filename: vs(f, "filename"),
        status: vs(f, "status"),
        additions: v32(f, "additions"),
        deletions: v32(f, "deletions"),
        patch: vs(f, "patch"),
    }
}

fn pr_comment_dto(c: &Value) -> PRCommentItem {
    PRCommentItem {
        id: v64(c, "id"),
        author: vptr(c, "/user/login"),
        body: vs(c, "body"),
        path: vs(c, "path"),
        line: v32(c, "line"),
        created_at: fmt_md_hm(&vs(c, "created_at")),
    }
}

fn asset_dto(a: &Value) -> Asset {
    Asset {
        id: v64(a, "id"),
        name: vs(a, "name"),
        size: v64(a, "size"),
        url: vs(a, "browser_download_url"),
    }
}

fn release_dto(r: &Value) -> Release {
    Release {
        id: v64(r, "id"),
        tag_name: vs(r, "tag_name"),
        name: vs(r, "name"),
        body: vs(r, "body"),
        draft: vbool(r, "draft"),
        pre_release: vbool(r, "prerelease"),
        url: vs(r, "html_url"),
        created_at: fmt_date(&vs(r, "created_at")),
        assets: r
            .get("assets")
            .and_then(Value::as_array)
            .map(|a| a.iter().map(asset_dto).collect())
            .unwrap_or_default(),
    }
}

fn code_result_dto(c: &Value) -> CodeResult {
    CodeResult {
        name: vs(c, "name"),
        path: vs(c, "path"),
        repo: vptr(c, "/repository/full_name"),
        html_url: vs(c, "html_url"),
        fragment: c
            .get("text_matches")
            .and_then(Value::as_array)
            .and_then(|a| a.first())
            .map(|m| vs(m, "fragment"))
            .unwrap_or_default(),
    }
}

fn repo_result_dto(r: &Value) -> RepoResult {
    RepoResult {
        full_name: vs(r, "full_name"),
        description: vs(r, "description"),
        stars: v32(r, "stargazers_count"),
        language: vs(r, "language"),
        html_url: vs(r, "html_url"),
    }
}

// ---------------------------------------------------------------------------
// Trending HTML 解析（字符串切割，对齐 go-trending 的选择器语义）
// ---------------------------------------------------------------------------

/// 解析 trending 页的 `article.Box-row`：
/// - 仓库名/链接：`h2 a`（lh-condensed 标题内首个 <a> 的 href="/owner/repo"）
/// - 描述：首个 `<p>`；语言：`span[itemprop=programmingLanguage]`
/// - 星数：`a[href$="/stargazers"]` 文本（与 go-trending 一致，取总星数）
fn parse_trending_projects(html: &str) -> Vec<TrendingProject> {
    let mut out = Vec::new();
    let mut rest = html;
    while let Some(start) = rest.find("<article") {
        let after = &rest[start..];
        let end = after
            .find("</article>")
            .map(|i| i + "</article>".len())
            .unwrap_or(after.len());
        let art = &after[..end];
        rest = &after[end..];

        // h2 标题内的首个 <a>：href="/owner/repo"
        let h2 = art.find("lh-condensed").map(|i| &art[i..]).unwrap_or(art);
        let Some(a_start) = h2.find("<a ") else { continue };
        let Some(tag_end_rel) = h2[a_start..].find('>') else { continue };
        let a_tag = &h2[a_start..a_start + tag_end_rel + 1];
        let href = extract_attr(a_tag, "href").unwrap_or_default();
        let mut segs = href.trim_start_matches('/').split('/');
        let owner = segs.next().unwrap_or("").to_string();
        let repo = segs.next().unwrap_or("").to_string();
        if owner.is_empty() || repo.is_empty() {
            continue;
        }

        // 描述：首个 <p> ... </p>
        let description = art
            .find("<p ")
            .and_then(|i| art[i..].find("</p>").map(|j| &art[i..i + j]))
            .map(html_text)
            .unwrap_or_default();

        // 语言：span[itemprop=programmingLanguage]
        let language = art
            .find("programmingLanguage")
            .and_then(|i| art[i..].find('>').map(|j| i + j + 1))
            .and_then(|i| art[i..].find("</span>").map(|j| art[i..i + j].to_string()))
            .map(|s| html_text(&s))
            .unwrap_or_default();

        // 星数：/stargazers 链接的文本（去千分位逗号）
        let stars = art.find("/stargazers").and_then(|i| {
            let a = art[..i].rfind("<a ")?;
            let close = art[i..].find("</a>").map(|j| i + j)?;
            let tag_end = art[a..].find('>').map(|j| a + j + 1)?;
            let inner = &art[tag_end..close];
            let text = html_text(inner).replace(',', "");
            text.parse::<i32>().ok()
        }).unwrap_or(0);

        let full_name = format!("{owner}/{repo}");
        out.push(TrendingProject {
            url: format!("https://github.com/{full_name}"),
            name: full_name,
            owner,
            repo,
            description,
            language,
            stars,
        });
    }
    out
}

/// 解析 trending 页语言筛选菜单（`#languages-menuitems a.select-menu-item`）：
/// href="/trending/<urlName>?since=daily" + 菜单文本。
fn parse_trending_languages(html: &str) -> Vec<TrendingLanguage> {
    let Some(menu) = html.find("id=\"languages-menuitems\"") else {
        return Vec::new();
    };
    let section = &html[menu..];
    let end = section.find("</details>").unwrap_or(section.len());
    let section = &section[..end];

    let mut out = Vec::new();
    let mut pos = 0usize;
    while let Some(off) = section[pos..].find("<a ") {
        let a_start = pos + off;
        let Some(tag_end_rel) = section[a_start..].find('>') else { break };
        let tag_end = a_start + tag_end_rel + 1;
        let Some(close) = section[a_start..].find("</a>").map(|i| a_start + i) else { break };
        let tag = &section[a_start..tag_end];
        if tag.contains("select-menu-item") && close > tag_end {
            let name = html_text(&section[tag_end..close]);
            let href = extract_attr(tag, "href").unwrap_or_default();
            let url_name = href
                .find("/trending/")
                .map(|i| &href[i + "/trending/".len()..])
                .map(|s| s.split(['?', '/']).next().unwrap_or(""))
                .unwrap_or("");
            if !url_name.is_empty() && !name.is_empty() {
                out.push(TrendingLanguage {
                    name,
                    url_name: url_name.to_string(),
                });
            }
        }
        pos = (close + 4).min(section.len());
    }
    out
}

// ---------------------------------------------------------------------------
// Actions / CI（Go gui/internal/github/actions.go + internal/gh/actions.go）
// ---------------------------------------------------------------------------

/// GhActionsList：当前仓库最近的 CI 运行列表（per_page=30）。
/// 非 GitHub remote 返回空数组（Go 返回 nil, nil）。
#[tauri::command]
pub async fn gh_actions_list(repo: Option<String>) -> Result<Vec<WorkflowRun>, String> {
    let repo = repo.unwrap_or_default();
    run_blocking(move || {
        let Some(owner_repo) = detect_remote(&repo) else {
            return Ok(Vec::new());
        };
        let token = gh_token()?;
        let (o, r) = split_owner_repo(&owner_repo)?;
        let v = gh_request(
            "GET",
            &format!("{API_BASE}/repos/{o}/{r}/actions/runs"),
            &[("per_page", "30".to_string())],
            None,
            &token,
            GH_ACCEPT,
            T_STD,
        )
        .map_err(|e| format!("获取 CI 运行列表失败: {e}"))?;
        let runs = v.get("workflow_runs").and_then(Value::as_array).cloned().unwrap_or_default();
        Ok(runs.iter().map(workflow_run_dto).collect())
    })?
}

/// GhActionsJobs：某次 CI 运行的 job 列表（per_page=30）。
#[tauri::command]
pub async fn gh_actions_jobs(run_id: i64, repo: Option<String>) -> Result<Vec<WorkflowJob>, String> {
    let repo = repo.unwrap_or_default();
    run_blocking(move || {
        let token = gh_token()?;
        let owner_repo = detect_remote(&repo).unwrap_or_default();
        let (o, r) = split_owner_repo(&owner_repo)?;
        let v = gh_request(
            "GET",
            &format!("{API_BASE}/repos/{o}/{r}/actions/runs/{run_id}/jobs"),
            &[("per_page", "30".to_string())],
            None,
            &token,
            GH_ACCEPT,
            T_STD,
        )
        .map_err(|e| format!("获取 Jobs 失败: {e}"))?;
        let jobs = v.get("jobs").and_then(Value::as_array).cloned().unwrap_or_default();
        Ok(jobs.iter().map(workflow_job_dto).collect())
    })?
}

/// GhActionsJobLogs：拉取某 job 的日志 zip（302 → 签名地址），解包拼接
/// 所有 .txt，超 200KB 截断（对齐 Go GetJobLogs）。
#[tauri::command]
pub async fn gh_actions_job_logs(job_id: i64, repo: Option<String>) -> Result<String, String> {
    let repo = repo.unwrap_or_default();
    run_blocking(move || {
        let token = gh_token()?;
        let owner_repo = detect_remote(&repo).unwrap_or_default();
        let (o, r) = split_owner_repo(&owner_repo)?;
        let bytes = gh_send(
            "GET",
            &format!("{API_BASE}/repos/{o}/{r}/actions/jobs/{job_id}/logs"),
            &[],
            None,
            None,
            &token,
            GH_ACCEPT,
            T_LOGS,
        )
        .map_err(|e| format!("获取日志失败: {e}"))?;
        let mut log = unzip_logs(&bytes).map_err(|e| format!("解压日志失败: {e}"))?;
        // 限制 200KB，避免刷爆前端
        if log.len() > 200 * 1024 {
            let mut cut = 200 * 1024;
            while !log.is_char_boundary(cut) {
                cut -= 1;
            }
            log.truncate(cut);
            log.push_str("\n\n…（日志过长，已截断。完整日志请用浏览器打开）");
        }
        Ok(log)
    })?
}

/// GhActionsRerun：重跑某次 CI 运行。
#[tauri::command]
pub async fn gh_actions_rerun(run_id: i64, repo: Option<String>) -> Result<(), String> {
    let repo = repo.unwrap_or_default();
    run_blocking(move || {
        let token = gh_token()?;
        let owner_repo = detect_remote(&repo).unwrap_or_default();
        let (o, r) = split_owner_repo(&owner_repo)?;
        gh_request(
            "POST",
            &format!("{API_BASE}/repos/{o}/{r}/actions/runs/{run_id}/rerun"),
            &[],
            None,
            &token,
            GH_ACCEPT,
            T_SHORT,
        )
        .map(|_| ())
    })?
}

/// GhActionsCancel：取消运行中的 CI。
#[tauri::command]
pub async fn gh_actions_cancel(run_id: i64, repo: Option<String>) -> Result<(), String> {
    let repo = repo.unwrap_or_default();
    run_blocking(move || {
        let token = gh_token()?;
        let owner_repo = detect_remote(&repo).unwrap_or_default();
        let (o, r) = split_owner_repo(&owner_repo)?;
        gh_request(
            "POST",
            &format!("{API_BASE}/repos/{o}/{r}/actions/runs/{run_id}/cancel"),
            &[],
            None,
            &token,
            GH_ACCEPT,
            T_SHORT,
        )
        .map(|_| ())
    })?
}

// ---------------------------------------------------------------------------
// Issues（Go gui/internal/github/issues.go + internal/gh/issues.go）
// ---------------------------------------------------------------------------

/// GhIssueList：当前仓库的 Issue 列表（排除 PR；state 默认 open；per_page=50）。
#[tauri::command]
pub async fn gh_issue_list(state: String, repo: Option<String>) -> Result<Vec<Issue>, String> {
    let repo = repo.unwrap_or_default();
    run_blocking(move || {
        let Some(owner_repo) = detect_remote(&repo) else {
            return Ok(Vec::new());
        };
        let token = gh_token()?;
        let (o, r) = split_owner_repo(&owner_repo)?;
        let state = if state.is_empty() { "open".to_string() } else { state };
        let v = gh_request(
            "GET",
            &format!("{API_BASE}/repos/{o}/{r}/issues"),
            &[("state", state), ("per_page", "50".to_string())],
            None,
            &token,
            GH_ACCEPT,
            T_STD,
        )
        .map_err(|e| format!("获取 Issue 列表失败: {e}"))?;
        // GitHub 会把 PR 混进 issues 列表，跳过带 pull_request 字段的条目
        let issues = v
            .as_array()
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .filter(|i| i.get("pull_request").is_none())
            .map(|i| issue_dto(&i))
            .collect();
        Ok(issues)
    })?
}

/// GhIssueCreate：创建 Issue，返回创建结果。
#[tauri::command]
pub async fn gh_issue_create(
    title: String,
    body: String,
    repo: Option<String>,
) -> Result<Issue, String> {
    let repo = repo.unwrap_or_default();
    run_blocking(move || {
        let token = gh_token()?;
        let owner_repo = detect_remote(&repo).unwrap_or_default();
        let (o, r) = split_owner_repo(&owner_repo)?;
        let v = gh_request(
            "POST",
            &format!("{API_BASE}/repos/{o}/{r}/issues"),
            &[],
            Some(json!({ "title": title, "body": body })),
            &token,
            GH_ACCEPT,
            T_SHORT,
        )?;
        Ok(issue_dto(&v))
    })?
}

/// GhIssueClose：关闭 Issue（PATCH state=closed）。
#[tauri::command]
pub async fn gh_issue_close(number: i32, repo: Option<String>) -> Result<(), String> {
    let repo = repo.unwrap_or_default();
    run_blocking(move || {
        let token = gh_token()?;
        let owner_repo = detect_remote(&repo).unwrap_or_default();
        let (o, r) = split_owner_repo(&owner_repo)?;
        gh_request(
            "PATCH",
            &format!("{API_BASE}/repos/{o}/{r}/issues/{number}"),
            &[],
            Some(json!({ "state": "closed" })),
            &token,
            GH_ACCEPT,
            T_SHORT,
        )
        .map(|_| ())
    })?
}

/// GhIssueComment：给 Issue 加评论。
#[tauri::command]
pub async fn gh_issue_comment(
    number: i32,
    body: String,
    repo: Option<String>,
) -> Result<(), String> {
    let repo = repo.unwrap_or_default();
    run_blocking(move || {
        let token = gh_token()?;
        let owner_repo = detect_remote(&repo).unwrap_or_default();
        let (o, r) = split_owner_repo(&owner_repo)?;
        gh_request(
            "POST",
            &format!("{API_BASE}/repos/{o}/{r}/issues/{number}/comments"),
            &[],
            Some(json!({ "body": body })),
            &token,
            GH_ACCEPT,
            T_SHORT,
        )
        .map(|_| ())
    })?
}

/// GhIssueLabels：仓库可用标签列表（per_page=100）。
#[tauri::command]
pub async fn gh_issue_labels(repo: Option<String>) -> Result<Vec<Label>, String> {
    let repo = repo.unwrap_or_default();
    run_blocking(move || {
        let Some(owner_repo) = detect_remote(&repo) else {
            return Ok(Vec::new());
        };
        let token = gh_token()?;
        let (o, r) = split_owner_repo(&owner_repo)?;
        let v = gh_request(
            "GET",
            &format!("{API_BASE}/repos/{o}/{r}/labels"),
            &[("per_page", "100".to_string())],
            None,
            &token,
            GH_ACCEPT,
            T_SHORT,
        )?;
        Ok(v.as_array().cloned().unwrap_or_default().iter().map(label_dto).collect())
    })?
}

/// GhIssueAddLabels：给 Issue 批量加标签。
#[tauri::command]
pub async fn gh_issue_add_labels(
    number: i32,
    labels: Vec<String>,
    repo: Option<String>,
) -> Result<(), String> {
    let repo = repo.unwrap_or_default();
    run_blocking(move || {
        let token = gh_token()?;
        let owner_repo = detect_remote(&repo).unwrap_or_default();
        let (o, r) = split_owner_repo(&owner_repo)?;
        gh_request(
            "POST",
            &format!("{API_BASE}/repos/{o}/{r}/issues/{number}/labels"),
            &[],
            Some(json!(labels)),
            &token,
            GH_ACCEPT,
            T_SHORT,
        )
        .map(|_| ())
    })?
}

/// GhIssueRemoveLabel：移除 Issue 的单个标签。
#[tauri::command]
pub async fn gh_issue_remove_label(
    number: i32,
    label: String,
    repo: Option<String>,
) -> Result<(), String> {
    let repo = repo.unwrap_or_default();
    run_blocking(move || {
        let token = gh_token()?;
        let owner_repo = detect_remote(&repo).unwrap_or_default();
        let (o, r) = split_owner_repo(&owner_repo)?;
        gh_request(
            "DELETE",
            &format!(
                "{API_BASE}/repos/{o}/{r}/issues/{number}/labels/{}",
                url_encode(&label)
            ),
            &[],
            None,
            &token,
            GH_ACCEPT,
            T_SHORT,
        )
        .map(|_| ())
    })?
}

// ---------------------------------------------------------------------------
// PR（Go gui/internal/github/pr.go + pr_detail.go + internal/gh/pr.go）
// ---------------------------------------------------------------------------

/// GhPRList：当前仓库的 PR 列表（state 默认 open；sort=updated；per_page=50）。
#[tauri::command]
pub async fn gh_pr_list(state: String, repo: Option<String>) -> Result<Vec<PullRequest>, String> {
    let repo = repo.unwrap_or_default();
    run_blocking(move || {
        let Some(owner_repo) = detect_remote(&repo) else {
            return Ok(Vec::new());
        };
        let token = gh_token()?;
        let (o, r) = split_owner_repo(&owner_repo)?;
        let state = if state.is_empty() { "open".to_string() } else { state };
        let v = gh_request(
            "GET",
            &format!("{API_BASE}/repos/{o}/{r}/pulls"),
            &[
                ("state", state),
                ("sort", "updated".to_string()),
                ("direction", "desc".to_string()),
                ("per_page", "50".to_string()),
            ],
            None,
            &token,
            GH_ACCEPT,
            T_STD,
        )
        .map_err(|e| format!("获取 PR 列表失败: {e}"))?;
        Ok(v.as_array().cloned().unwrap_or_default().iter().map(pull_request_dto).collect())
    })?
}

/// GhPRCreate：从 head 到 base 创建 PR。
#[tauri::command]
pub async fn gh_pr_create(
    title: String,
    head: String,
    base: String,
    body: String,
    repo: Option<String>,
) -> Result<PullRequest, String> {
    let repo = repo.unwrap_or_default();
    run_blocking(move || {
        let token = gh_token()?;
        let owner_repo = detect_remote(&repo).unwrap_or_default();
        let (o, r) = split_owner_repo(&owner_repo)?;
        let v = gh_request(
            "POST",
            &format!("{API_BASE}/repos/{o}/{r}/pulls"),
            &[],
            Some(json!({ "title": title, "head": head, "base": base, "body": body })),
            &token,
            GH_ACCEPT,
            T_SHORT,
        )
        .map_err(|e| format!("创建 PR 失败: {e}"))?;
        Ok(pull_request_dto(&v))
    })?
}

/// GhPRComments：PR 的 review 评论列表（per_page=30，go-github 默认）。
#[tauri::command]
pub async fn gh_pr_comments(number: i32, repo: Option<String>) -> Result<Vec<PRCommentItem>, String> {
    let repo = repo.unwrap_or_default();
    run_blocking(move || {
        let token = gh_token()?;
        let owner_repo = detect_remote(&repo).unwrap_or_default();
        let (o, r) = split_owner_repo(&owner_repo)?;
        let v = gh_request(
            "GET",
            &format!("{API_BASE}/repos/{o}/{r}/pulls/{number}/comments"),
            &[("per_page", "30".to_string())],
            None,
            &token,
            GH_ACCEPT,
            T_STD,
        )
        .map_err(|e| format!("获取 PR 评论失败: {e}"))?;
        Ok(v.as_array().cloned().unwrap_or_default().iter().map(pr_comment_dto).collect())
    })?
}

/// GhPRCreateComment：给 PR 加评论（对齐 Go：仅 body，不带 commit_id）。
#[tauri::command]
pub async fn gh_pr_create_comment(
    number: i32,
    body: String,
    repo: Option<String>,
) -> Result<(), String> {
    let repo = repo.unwrap_or_default();
    run_blocking(move || {
        let token = gh_token()?;
        let owner_repo = detect_remote(&repo).unwrap_or_default();
        let (o, r) = split_owner_repo(&owner_repo)?;
        gh_request(
            "POST",
            &format!("{API_BASE}/repos/{o}/{r}/pulls/{number}/comments"),
            &[],
            Some(json!({ "body": body })),
            &token,
            GH_ACCEPT,
            T_SHORT,
        )
        .map(|_| ())
    })?
}

/// GhPRFiles：PR 变更文件列表（per_page=100）。
#[tauri::command]
pub async fn gh_pr_files(number: i32, repo: Option<String>) -> Result<Vec<PRFile>, String> {
    let repo = repo.unwrap_or_default();
    run_blocking(move || {
        let token = gh_token()?;
        let owner_repo = detect_remote(&repo).unwrap_or_default();
        let (o, r) = split_owner_repo(&owner_repo)?;
        let v = gh_request(
            "GET",
            &format!("{API_BASE}/repos/{o}/{r}/pulls/{number}/files"),
            &[("per_page", "100".to_string())],
            None,
            &token,
            GH_ACCEPT,
            T_STD,
        )
        .map_err(|e| format!("获取 PR 文件失败: {e}"))?;
        Ok(v.as_array().cloned().unwrap_or_default().iter().map(pr_file_dto).collect())
    })?
}

/// GhPRReview：提交 review（先建 PENDING 再提交；event: APPROVE /
/// REQUEST_CHANGES / COMMENT）。
#[tauri::command]
pub async fn gh_pr_review(
    number: i32,
    event: String,
    body: String,
    repo: Option<String>,
) -> Result<(), String> {
    let repo = repo.unwrap_or_default();
    run_blocking(move || {
        let token = gh_token()?;
        let owner_repo = detect_remote(&repo).unwrap_or_default();
        let (o, r) = split_owner_repo(&owner_repo)?;
        let pending = gh_request(
            "POST",
            &format!("{API_BASE}/repos/{o}/{r}/pulls/{number}/reviews"),
            &[],
            Some(json!({ "body": body, "event": "PENDING" })),
            &token,
            GH_ACCEPT,
            T_SHORT,
        )
        .map_err(|e| format!("创建 review 失败: {e}"))?;
        let review_id = v64(&pending, "id");
        gh_request(
            "POST",
            &format!("{API_BASE}/repos/{o}/{r}/pulls/{number}/reviews/{review_id}/events"),
            &[],
            Some(json!({ "body": body, "event": event })),
            &token,
            GH_ACCEPT,
            T_SHORT,
        )
        .map(|_| ())
    })?
}

/// GhPRMerge：合并 PR（strategy 默认 merge；merge/squash/rebase）。
#[tauri::command]
pub async fn gh_pr_merge(number: i32, strategy: String, repo: Option<String>) -> Result<(), String> {
    let repo = repo.unwrap_or_default();
    run_blocking(move || {
        let token = gh_token()?;
        let owner_repo = detect_remote(&repo).unwrap_or_default();
        let (o, r) = split_owner_repo(&owner_repo)?;
        let strategy = if strategy.is_empty() { "merge".to_string() } else { strategy };
        gh_request(
            "PUT",
            &format!("{API_BASE}/repos/{o}/{r}/pulls/{number}/merge"),
            &[],
            Some(json!({ "merge_method": strategy })),
            &token,
            GH_ACCEPT,
            T_STD,
        )
        .map(|_| ())
    })?
}

/// GhPRCheckout：本地检出 PR 分支（git fetch origin pull/N/head:pr-N +
/// checkout pr-N；不走 API）。
#[tauri::command]
pub async fn gh_pr_checkout(number: i32, repo: Option<String>) -> Result<(), String> {
    let repo = repo.unwrap_or_default();
    let inner = move || -> Result<(), String> {
        let branch = format!("pr-{number}");
        let refspec = format!("pull/{number}/head:{branch}");
        git_output(&repo, &["fetch", "origin", refspec.as_str()], T_STD)
            .map_err(|e| format!("fetch PR 失败: {e}"))?;
        git_output(&repo, &["checkout", branch.as_str()], T_STD)?;
        Ok(())
    };
    run_blocking(inner)?
}

// ---------------------------------------------------------------------------
// Releases（Go gui/internal/github/releases.go + internal/gh/releases.go）
// ---------------------------------------------------------------------------

/// GhReleaseList：当前仓库的 Release 列表（per_page=30）。
#[tauri::command]
pub async fn gh_release_list(repo: Option<String>) -> Result<Vec<Release>, String> {
    let repo = repo.unwrap_or_default();
    run_blocking(move || {
        let Some(owner_repo) = detect_remote(&repo) else {
            return Ok(Vec::new());
        };
        let token = gh_token()?;
        let (o, r) = split_owner_repo(&owner_repo)?;
        let v = gh_request(
            "GET",
            &format!("{API_BASE}/repos/{o}/{r}/releases"),
            &[("per_page", "30".to_string())],
            None,
            &token,
            GH_ACCEPT,
            T_STD,
        )
        .map_err(|e| format!("获取 Release 列表失败: {e}"))?;
        Ok(v.as_array().cloned().unwrap_or_default().iter().map(release_dto).collect())
    })?
}

/// GhReleaseCreate：创建 Release（tag/title/body/draft/prerelease）。
#[tauri::command]
pub async fn gh_release_create(
    tag: String,
    title: String,
    body: String,
    is_draft: bool,
    is_pre: bool,
    repo: Option<String>,
) -> Result<Release, String> {
    let repo = repo.unwrap_or_default();
    run_blocking(move || {
        let token = gh_token()?;
        let owner_repo = detect_remote(&repo).unwrap_or_default();
        let (o, r) = split_owner_repo(&owner_repo)?;
        let v = gh_request(
            "POST",
            &format!("{API_BASE}/repos/{o}/{r}/releases"),
            &[],
            Some(json!({
                "tag_name": tag,
                "name": title,
                "body": body,
                "draft": is_draft,
                "prerelease": is_pre,
                "generate_release_notes": false,
            })),
            &token,
            GH_ACCEPT,
            T_STD,
        )
        .map_err(|e| format!("创建 Release 失败: {e}"))?;
        Ok(release_dto(&v))
    })?
}

/// GhReleaseDelete：按 ID 删除 Release。
#[tauri::command]
pub async fn gh_release_delete(release_id: i64, repo: Option<String>) -> Result<(), String> {
    let repo = repo.unwrap_or_default();
    run_blocking(move || {
        let token = gh_token()?;
        let owner_repo = detect_remote(&repo).unwrap_or_default();
        let (o, r) = split_owner_repo(&owner_repo)?;
        gh_request(
            "DELETE",
            &format!("{API_BASE}/repos/{o}/{r}/releases/{release_id}"),
            &[],
            None,
            &token,
            GH_ACCEPT,
            T_SHORT,
        )
        .map(|_| ())
    })?
}

/// GhReleaseDeleteAsset：删除 Release 的单个资产。
#[tauri::command]
pub async fn gh_release_delete_asset(asset_id: i64, repo: Option<String>) -> Result<(), String> {
    let repo = repo.unwrap_or_default();
    run_blocking(move || {
        let token = gh_token()?;
        let owner_repo = detect_remote(&repo).unwrap_or_default();
        let (o, r) = split_owner_repo(&owner_repo)?;
        gh_request(
            "DELETE",
            &format!("{API_BASE}/repos/{o}/{r}/releases/assets/{asset_id}"),
            &[],
            None,
            &token,
            GH_ACCEPT,
            T_SHORT,
        )
        .map(|_| ())
    })?
}

/// GhReleaseUploadAsset：上传文件为 Release 资产（uploads.github.com，
/// octet-stream，5 分钟超时），返回资产信息。
#[tauri::command]
pub async fn gh_release_upload_asset(
    release_id: i64,
    file_path: String,
    repo: Option<String>,
) -> Result<Asset, String> {
    let repo = repo.unwrap_or_default();
    run_blocking(move || {
        let token = gh_token()?;
        let owner_repo = detect_remote(&repo).unwrap_or_default();
        let (o, r) = split_owner_repo(&owner_repo)?;
        let bytes = std::fs::read(&file_path).map_err(|e| format!("打开文件失败: {e}"))?;
        let name = Path::new(&file_path)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| file_path.clone());
        let v = gh_send(
            "POST",
            &format!("{UPLOADS_BASE}/repos/{o}/{r}/releases/{release_id}/assets"),
            &[("name", url_encode(&name))],
            Some(bytes),
            Some("application/octet-stream"),
            &token,
            GH_ACCEPT,
            T_UPLOAD,
        )
        .map_err(|e| format!("上传失败: {e}"))?;
        let v: Value = serde_json::from_slice(&v).map_err(|e| format!("解析 GitHub 响应失败: {e}"))?;
        Ok(asset_dto(&v))
    })?
}

// ---------------------------------------------------------------------------
// Search / Trending（Go gui/internal/github/search.go + trending.go +
// internal/gh/search.go + trending.go）
// ---------------------------------------------------------------------------

/// GhSearchRepos：全站仓库搜索（per_page=20，sort=stars）。
#[tauri::command]
pub async fn gh_search_repos(query: String, repo: Option<String>) -> Result<Vec<RepoResult>, String> {
    let _ = repo;
    run_blocking(move || {
        let token = gh_token()?;
        let v = gh_request(
            "GET",
            &format!("{API_BASE}/search/repositories"),
            &[
                ("q", query),
                ("per_page", "20".to_string()),
                ("sort", "stars".to_string()),
            ],
            None,
            &token,
            GH_ACCEPT,
            T_STD,
        )
        .map_err(|e| format!("搜索失败: {e}"))?;
        let items = v.get("items").and_then(Value::as_array).cloned().unwrap_or_default();
        Ok(items.iter().map(repo_result_dto).collect())
    })?
}

/// GhSearchCode：代码搜索；origin 是 GitHub 仓库时限定 repo: 前缀
/// （Accept text-match，per_page=30）。
#[tauri::command]
pub async fn gh_search_code(query: String, repo: Option<String>) -> Result<Vec<CodeResult>, String> {
    let repo = repo.unwrap_or_default();
    run_blocking(move || {
        let owner_repo = detect_remote(&repo).unwrap_or_default();
        let token = gh_token()?;
        let q = if owner_repo.is_empty() {
            query
        } else {
            format!("repo:{owner_repo} {query}")
        };
        let v = gh_request(
            "GET",
            &format!("{API_BASE}/search/code"),
            &[("q", q), ("per_page", "30".to_string())],
            None,
            &token,
            ACCEPT_TEXT_MATCH,
            T_STD,
        )
        .map_err(|e| format!("搜索失败: {e}"))?;
        let items = v.get("items").and_then(Value::as_array).cloned().unwrap_or_default();
        Ok(items.iter().map(code_result_dto).collect())
    })?
}

/// GhTrendingProjects：趋势仓库（github.com/trending HTML；timeframe:
/// daily/weekly/monthly；language 为 urlName；spokenLang 为口音语言码）。
/// URL 形如 `?since=&l=&spoken_language_code=`，与 go-trending + Go 注入
/// transport 一致。
#[tauri::command]
pub async fn gh_trending_projects(
    timeframe: String,
    language: String,
    spoken_lang: String,
    repo: Option<String>,
) -> Result<Vec<TrendingProject>, String> {
    let _ = repo;
    run_blocking(move || {
        let timeframe = if timeframe.is_empty() { "daily".to_string() } else { timeframe };
        let mut params = vec![("since", timeframe)];
        if !language.is_empty() {
            params.push(("l", language));
        }
        if !spoken_lang.is_empty() {
            params.push(("spoken_language_code", spoken_lang));
        }
        let html = fetch_html("https://github.com/trending", &params)
            .map_err(|e| format!("获取 Trending 失败: {e}"))?;
        Ok(parse_trending_projects(&html))
    })?
}

/// GhTrendingLanguages：趋势页可用的编程语言筛选列表。
#[tauri::command]
pub async fn gh_trending_languages(repo: Option<String>) -> Result<Vec<TrendingLanguage>, String> {
    let _ = repo;
    run_blocking(move || {
        let html = fetch_html("https://github.com/trending", &[])
            .map_err(|e| format!("获取语言列表失败: {e}"))?;
        Ok(parse_trending_languages(&html))
    })?
}

/// GhSpokenLanguages：口音语言筛选列表（静态表，同步命令，无网络）。
#[tauri::command]
pub fn gh_spoken_languages(repo: Option<String>) -> Vec<TrendingLanguage> {
    let _ = repo;
    [
        ("简体中文", "zh"),
        ("English", "en"),
        ("Español", "es"),
        ("Français", "fr"),
        ("Deutsch", "de"),
        ("日本語", "ja"),
        ("Português", "pt"),
        ("Русский", "ru"),
        ("한국어", "ko"),
        ("Italiano", "it"),
    ]
    .into_iter()
    .map(|(name, url_name)| TrendingLanguage {
        name: name.to_string(),
        url_name: url_name.to_string(),
    })
    .collect()
}
