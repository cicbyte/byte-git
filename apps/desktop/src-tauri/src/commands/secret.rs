//! secret 域命令：密钥扫描/处置 + AI 代码审查（OCR）+ OCR 配置桥接。
//!
//! 移植自 git-mate `gui/internal/secret`（secret.go / review.go / ocrconfig.go）。

use byte_git_core::backup;
use byte_git_core::config::Settings;
use byte_git_core::gitleaks;
use byte_git_core::ocr;
use byte_git_core::util::{git_dir, run_capture};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::io::BufRead;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Mutex, OnceLock};
use tauri::Emitter;

fn repo_path(repo: Option<String>) -> PathBuf {
    crate::state::resolve(repo)
}

/// 前端文件读取上限（对齐 Go MaxSecretFileSize = 1MB）。
const MAX_SECRET_FILE_SIZE: usize = 1 << 20;

// ---------------------------------------------------------------------------
// DTO
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SecretCheckResult {
    pub available: bool,
    pub exe_path: String,
    pub version: String,
    pub message: String,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewCheckResult {
    pub available: bool,
    pub exe_path: String,
    pub version: String,
    pub message: String,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileResult {
    pub content: String,
    pub lines: i64,
    pub binary: bool,
    pub not_found: bool,
    /// 图片内联 data URL（前端直接 <img> 展示；仅图片填）
    #[serde(default)]
    pub data_url: String,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlatformHintDto {
    pub url: String,
    pub hint: String,
    pub found: bool,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkingTreeFixDto {
    #[serde(default)]
    pub file: String,
    #[serde(default)]
    pub secret: String,
    #[serde(default)]
    pub replacement: String,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkingTreeResult {
    pub ok: bool,
    pub file_exists: bool,
    pub changed_count: i64,
    pub message: String,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScrubResult {
    pub ok: bool,
    pub backup_id: String,
    pub affected_commits: i64,
    pub remote_removed: bool,
    pub message: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewRunOptionsDto {
    #[serde(default)]
    pub from: String,
    #[serde(default)]
    pub to: String,
    #[serde(default)]
    pub commit: String,
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub background: String,
}

// ---------------------------------------------------------------------------
// OCR config.json 桥接结构（omitempty 语义对齐 Go）
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct OcrProviderEntry {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub api_key: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub url: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub protocol: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub model: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub models: Vec<String>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub auth_header: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct OcrLlmConfig {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub url: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub auth_token: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub auth_header: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub model: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct OcrConfig {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub provider: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub model: String,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub providers: BTreeMap<String, OcrProviderEntry>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub custom_providers: BTreeMap<String, OcrProviderEntry>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub llm: Option<OcrLlmConfig>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub language: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OcrProviderPreset {
    pub name: &'static str,
    pub display_name: &'static str,
    pub protocol: &'static str,
    pub base_url: &'static str,
    pub auth_header: &'static str,
    pub models: &'static [&'static str],
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OcrConfigResult {
    pub config: OcrConfig,
    pub path: String,
    pub exists: bool,
    pub presets: Vec<OcrProviderPreset>,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OcrConfigRawResult {
    pub path: String,
    pub exists: bool,
    pub content: String,
}

// ---------------------------------------------------------------------------
// 可用性检测（进程内缓存，避免每次挂载都跑 --version）
// ---------------------------------------------------------------------------

fn gitleaks_cache() -> &'static Mutex<Option<SecretCheckResult>> {
    static CACHE: OnceLock<Mutex<Option<SecretCheckResult>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(None))
}

fn ocr_check_cache() -> &'static Mutex<Option<ReviewCheckResult>> {
    static CACHE: OnceLock<Mutex<Option<ReviewCheckResult>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(None))
}

/// 清空 gitleaks/ocr 可用性检测缓存（设置保存且 exe_path 变更时调用，
/// 对齐 Go SettingsSave 的缓存失效钩子）。gitleaks=true 清 gitleaks，ocr=true 清 ocr。
pub fn clear_check_cache(gitleaks: bool, ocr: bool) {
    if gitleaks {
        *gitleaks_cache().lock().unwrap() = None;
    }
    if ocr {
        *ocr_check_cache().lock().unwrap() = None;
    }
}

/// SecretCheck：gitleaks 是否就绪（配置 > PATH；结果进程内缓存）。
#[tauri::command]
pub fn secret_check() -> Result<SecretCheckResult, String> {
    let cell = gitleaks_cache();
    let mut guard = cell.lock().unwrap();
    if let Some(cached) = guard.clone() {
        return Ok(cached);
    }
    let res = match gitleaks::find_exe("") {
        Err(e) => SecretCheckResult {
            message: format!("{e:#}"),
            ..Default::default()
        },
        Ok(exe_path) => {
            let ver = gitleaks::version(&exe_path);
            if ver == "unknown" {
                SecretCheckResult {
                    exe_path,
                    message: "无法获取版本: 命令执行失败".into(),
                    ..Default::default()
                }
            } else {
                SecretCheckResult {
                    available: true,
                    exe_path,
                    version: ver,
                    message: String::new(),
                }
            }
        }
    };
    *guard = Some(res.clone());
    Ok(res)
}

/// SecretScan：对当前仓库全量历史执行 gitleaks 扫描（密钥默认脱敏）。
#[tauri::command]
pub fn secret_scan(repo: Option<String>) -> Result<serde_json::Value, String> {
    let exe_path = gitleaks::find_exe("").map_err(|e| format!("{e:#}"))?;
    let res = gitleaks::scan(&gitleaks::ScanOptions {
        repo_dir: crate::state::resolve(repo).display().to_string(),
        exe_path,
        ..Default::default()
    })
    .map_err(|e| format!("{e:#}"))?;
    serde_json::to_value(res).map_err(|e| e.to_string())
}

/// SecretFileContent：读取指定 commit 时点的文件内容（git show commit:path）。
/// gitleaks 扫全历史，密钥常在被删/被改文件的历史版本里。
#[tauri::command]
pub fn secret_file_content(repo: Option<String>, commit: String, file_path: String) -> Result<FileResult, String> {
    let repo = repo_path(repo);
    let out = Command::new("git")
        .arg("-C")
        .arg(&repo)
        .args(["show", &format!("{commit}:{file_path}")])
        .output()
        .map_err(|e| format!("git show 失败: {e}"))?;
    if !out.status.success() {
        let msg = String::from_utf8_lossy(&out.stderr).trim().to_string();
        if msg.contains("does not exist") || msg.contains("exists on disk") {
            return Ok(FileResult { not_found: true, ..Default::default() });
        }
        return Err(format!("git show 失败: {msg}"));
    }
    let raw = out.stdout;
    // 图片？内联 data URL（对齐上游 ReadFileAtCommit；超大图回落 binary）
    let url = super::repo::encode_image_data_url(&file_path, &raw);
    if !url.is_empty() {
        if raw.len() > super::repo::MAX_IMAGE_SIZE {
            return Ok(FileResult { binary: true, ..Default::default() });
        }
        return Ok(FileResult { data_url: url, ..Default::default() });
    }
    // 二进制检测：前 8KB 出现 NUL 即视为二进制
    let scan_len = raw.len().min(8192);
    if raw[..scan_len].contains(&0) {
        return Ok(FileResult { binary: true, ..Default::default() });
    }
    let mut truncated = false;
    let capped: &[u8] = if raw.len() > MAX_SECRET_FILE_SIZE {
        truncated = true;
        &raw[..MAX_SECRET_FILE_SIZE]
    } else {
        &raw
    };
    let mut content = String::from_utf8_lossy(capped).into_owned();
    let lines = content.matches('\n').count() as i64 + 1;
    if truncated {
        content.push_str("\n\n…（文件超过 1MB，已截断显示）");
    }
    Ok(FileResult { content, lines, ..Default::default() })
}

// ---------------------------------------------------------------------------
// rule → 吊销平台映射（对齐 gitleaks.PlatformForRule）
// ---------------------------------------------------------------------------

/// (ruleID, url, hint)
const RULE_PLATFORM_MAP: &[(&str, &str, &str)] = &[
    ("aws-access-token", "https://console.aws.amazon.com/iam/", "AWS IAM 控制台 → 删除该 Access Key"),
    ("github-pat", "https://github.com/settings/tokens", "GitHub Settings → Tokens → 删除/轮换"),
    ("github-oauth", "https://github.com/settings/applications", "GitHub Settings → Applications → 撤销授权"),
    ("github-app-token", "https://github.com/settings/apps", "GitHub Settings → Developer settings → Apps"),
    ("github-fine-grained-pat", "https://github.com/settings/personal-access-tokens", "GitHub Settings → Fine-grained tokens"),
    ("github-token", "https://github.com/settings/tokens", "GitHub Settings → Tokens → 删除/轮换"),
    ("gitlab-pat", "https://gitlab.com/-/profile/personal_access_tokens", "GitLab → 个人访问令牌 → 撤销"),
    ("anthropic-api-key", "https://console.anthropic.com/settings/keys", "Anthropic Console → 撤销 API Key"),
    ("openai-api-key", "https://platform.openai.com/api-keys", "OpenAI Platform → 撤销 API Key"),
    ("alibaba-access-key-id", "https://ram.console.aliyun.com/manage/ak", "阿里云 RAM → 禁用/删除 AccessKey"),
    ("digitalocean-pat", "https://cloud.digitalocean.com/account/api/tokens", "DigitalOcean → 重新生成 Token"),
    ("stripe-access-token", "https://dashboard.stripe.com/apikeys", "Stripe Dashboard → 滚动密钥"),
    ("square-access-token", "https://developer.squareup.com/apps", "Square Developer → 应用 → 重置凭证"),
    ("twilio-api-key", "https://console.twilio.com/", "Twilio Console → 撤销 API Key"),
    ("discord-api-token", "https://discord.com/developers/applications", "Discord Developer Portal → 重置 Token"),
    ("heroku-api-key", "https://dashboard.heroku.com/account", "Heroku → 账户设置 → 撤销 API Key"),
    ("slack-token", "https://api.slack.com/authentication/token-types", "Slack → 撤销对应类型的 Token"),
    ("private-key", "", "重新签发对应的证书/密钥对（PEM 私钥无法吊销，必须更换）"),
    ("jwt", "", "JWT 本身无法吊销，需更换签名密钥并使旧 token 失效"),
    ("generic-api-key", "", "联系对应服务的控制台，吊销/轮换该 API Key"),
];

/// SecretPlatformHint：返回规则对应的吊销/轮换平台信息。
#[tauri::command]
pub fn secret_platform_hint(rule_id: String) -> Result<PlatformHintDto, String> {
    let mk = |url: &str, hint: &str| PlatformHintDto {
        url: url.to_string(),
        hint: hint.to_string(),
        found: !url.is_empty() || !hint.is_empty(),
    };
    for (id, url, hint) in RULE_PLATFORM_MAP {
        if *id == rule_id {
            return Ok(mk(url, hint));
        }
    }
    let id = rule_id.to_lowercase();
    for (k, url, hint) in RULE_PLATFORM_MAP {
        if id.contains(k) || k.contains(&id) {
            return Ok(mk(url, hint));
        }
    }
    Ok(PlatformHintDto {
        url: String::new(),
        hint: "联系对应服务的控制台，吊销/轮换该密钥".into(),
        found: false,
    })
}

/// SecretRemoveFromWorkingTree：替换工作区文件中的密钥并 git add 暂存。
/// 只修复当前版本——密钥仍在 git 历史中（前端须向用户说明）。
#[tauri::command]
pub fn secret_remove_from_working_tree(repo: Option<String>, dto: WorkingTreeFixDto) -> Result<WorkingTreeResult, String> {
    let repo = repo_path(repo);
    if dto.secret.is_empty() {
        return Err("密钥内容为空".into());
    }
    let replacement = if dto.replacement.is_empty() { "REMOVED_SECRET" } else { dto.replacement.as_str() };

    let rel = Path::new(&dto.file);
    let abs = if rel.is_absolute() { rel.to_path_buf() } else { repo.join(rel) };

    let data = match std::fs::read(&abs) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Ok(WorkingTreeResult {
                file_exists: false,
                message: "文件在工作区不存在（可能已被删除或仅在历史 commit 中）".into(),
                ..Default::default()
            });
        }
        Err(e) => return Err(format!("读取文件失败: {e}")),
        Ok(d) => d,
    };

    let text = String::from_utf8_lossy(&data);
    let count = text.matches(dto.secret.as_str()).count();
    if count == 0 {
        return Ok(WorkingTreeResult {
            ok: true,
            file_exists: true,
            changed_count: 0,
            message: "密钥未在当前文件中找到（可能已被修改）".into(),
        });
    }

    let new_data = text.replace(dto.secret.as_str(), replacement);
    std::fs::write(&abs, new_data).map_err(|e| format!("写入文件失败: {e}"))?;

    // 暂存改动（失败不致命，提示手动 add）
    let staged = Command::new("git")
        .arg("-C")
        .arg(&repo)
        .args(["add", "--", &dto.file])
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if !staged {
        return Ok(WorkingTreeResult {
            ok: true,
            file_exists: true,
            changed_count: count as i64,
            message: format!("已替换 {count} 处，但 git add 失败（请手动添加）"),
        });
    }
    Ok(WorkingTreeResult {
        ok: true,
        file_exists: true,
        changed_count: count as i64,
        message: format!("已替换 {count} 处并暂存"),
    })
}

// ---------------------------------------------------------------------------
// 历史抹除（git filter-repo --replace-text，对齐 ScrubFromHistory）
// ---------------------------------------------------------------------------

const SCRUB_PLACEHOLDER: &str = "REMOVED_SECRET";

enum FilterRepoExe {
    /// 经 git 子命令调用（`git filter-repo`）
    GitSubcommand,
    /// 独立可执行文件
    Path(PathBuf),
}

fn look_path(prog: &str) -> Option<PathBuf> {
    if prog.contains('/') || prog.contains('\\') || prog.contains(std::path::MAIN_SEPARATOR) {
        return None;
    }
    let exts = std::env::var("PATHEXT").unwrap_or_else(|_| ".exe;.bat;.cmd".into());
    std::env::var_os("PATH").and_then(|paths| {
        std::env::split_paths(&paths).find_map(|dir| {
            let p = dir.join(prog);
            if p.is_file() {
                return Some(p);
            }
            exts.split(';')
                .filter(|e| !e.is_empty())
                .find_map(|e| {
                    let pe = dir.join(format!("{prog}{e}"));
                    pe.is_file().then_some(pe)
                })
        })
    })
}

/// 探测 git-filter-repo：优先 `git filter-repo` 子命令形态，再找独立二进制。
fn detect_filter_repo() -> Result<FilterRepoExe, String> {
    let via_git = Command::new("git")
        .args(["filter-repo", "--version"])
        .output()
        .map(|o| o.status.success() && !o.stdout.is_empty())
        .unwrap_or(false);
    if via_git {
        if let Some(p) = look_path("git-filter-repo") {
            return Ok(FilterRepoExe::Path(p));
        }
        return Ok(FilterRepoExe::GitSubcommand);
    }
    if let Some(p) = look_path("git-filter-repo") {
        return Ok(FilterRepoExe::Path(p));
    }
    Err("未找到 git-filter-repo\n安装: pip install git-filter-repo".into())
}

fn remote_exists(repo: &Path) -> bool {
    run_capture(repo, ["remote"])
        .map(|o| !o.trim().is_empty())
        .unwrap_or(false)
}

fn count_commits(repo: &Path) -> i64 {
    run_capture(repo, ["rev-list", "--count", "HEAD"])
        .ok()
        .and_then(|s| s.trim().parse::<i64>().ok())
        .unwrap_or(0)
}

/// 工作区是否有未提交改动（附带最多 3 行摘要）。
fn working_tree_dirty(repo: &Path) -> Option<String> {
    let out = run_capture(repo, ["status", "--porcelain"]).ok()?;
    let trimmed = out.trim();
    if trimmed.is_empty() {
        return None;
    }
    let lines: Vec<&str> = trimmed.lines().collect();
    if lines.len() > 3 {
        Some(format!("{}\n... (共 {} 处改动)", lines[..3].join("\n"), lines.len()))
    } else {
        Some(trimmed.to_string())
    }
}

fn prune_after_scrub(repo: &Path) {
    // 过期 reflog + 激进 GC：旧提交（仍含密钥）才能真正消失，
    // 否则 gitleaks 之后仍能经 reflog/悬空对象扫到密钥。best-effort。
    let _ = Command::new("git").arg("-C").arg(repo).args(["reflog", "expire", "--all", "--expire=now"]).status();
    let _ = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["reflog", "expire", "--expire-unreachable=now", "--all"])
        .status();
    let _ = Command::new("git").arg("-C").arg(repo).args(["gc", "--prune=now", "--aggressive"]).status();
}

/// SecretScrubHistory：用 git filter-repo --replace-text 重写全部历史替换密钥。
/// 高危操作；autoBackup=true 时先做整库克隆备份。
#[tauri::command]
pub fn secret_scrub_history(repo: Option<String>, secret: String, auto_backup: bool) -> Result<ScrubResult, String> {
    let _busy = crate::commands::acquire_rewrite_busy()?;
    let repo = repo_path(repo);
    if secret.is_empty() {
        return Err("密钥内容为空".into());
    }
    if auto_backup {
        backup::create_full(&repo.to_string_lossy()).map_err(|e| format!("整库备份失败（已中止抹除）: {e:#}"))?;
    }

    let exe = detect_filter_repo()?;

    // filter-repo 会按设计移除 origin remote —— 先记录
    let had_remote = remote_exists(&repo);
    let before_count = count_commits(&repo);

    // 替换规则文件："<secret>==>REMOVED_SECRET"
    let tmp = std::env::temp_dir().join(format!(
        "gitleaks-replace-{}-{}.txt",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    std::fs::write(&tmp, format!("{secret}==>{SCRUB_PLACEHOLDER}\n"))
        .map_err(|e| format!("写入替换规则失败: {e}"))?;

    let result = (|| -> Result<ScrubResult, String> {
        // 脏工作区：filter-repo 会拒绝运行
        if let Some(detail) = working_tree_dirty(&repo) {
            return Err(format!("工作区不干净，filter-repo 拒绝运行: {detail}"));
        }
        // 清理上次 filter-branch/filter-repo 残留的 refs/original/*
        if let Ok(out) = run_capture(&repo, ["for-each-ref", "--format=%(refname)", "refs/original/"]) {
            for r in out.lines().map(str::trim).filter(|s| !s.is_empty()) {
                let _ = run_capture(&repo, ["update-ref", "-d", r]);
            }
        }
        // 移除 already_ran 标记，避免非交互运行时命中 continuation 提示
        if let Ok(gd) = git_dir(&repo) {
            let _ = std::fs::remove_dir_all(gd.join("filter-repo"));
        }

        let tmp_str = tmp.display().to_string();
        let out = match &exe {
            FilterRepoExe::GitSubcommand => Command::new("git")
                .arg("-C")
                .arg(&repo)
                .args(["filter-repo", "--replace-text", &tmp_str, "--force"])
                .output(),
            FilterRepoExe::Path(p) => Command::new(p)
                .current_dir(&repo)
                .args(["--replace-text", &tmp_str, "--force"])
                .output(),
        }
        .map_err(|e| format!("执行 filter-repo 失败: {e}"))?;

        if !out.status.success() {
            let mut detail = String::from_utf8_lossy(&out.stderr).into_owned();
            detail.push_str(&String::from_utf8_lossy(&out.stdout));
            return Err(format!("filter-repo 失败: exit status {:?}: {}", out.status.code(), detail.trim()));
        }

        let remote_removed = had_remote && !remote_exists(&repo);
        prune_after_scrub(&repo);
        Ok(ScrubResult {
            ok: true,
            backup_id: String::new(),
            affected_commits: before_count,
            remote_removed,
            message: format!("已抹除历史中的密钥（共涉及 {before_count} 个 commit），并清理了 reflog 与悬空对象。"),
        })
    })();

    let _ = std::fs::remove_file(&tmp);
    result
}

/// RewriteAutoBackupDefault：读取「改写前自动备份」设置默认值。
#[tauri::command]
pub fn rewrite_auto_backup_default() -> Result<bool, String> {
    // Go 版读取配置失败时返回 (false, nil)
    Ok(Settings::load().map(|s| s.rewrite.auto_backup).unwrap_or(false))
}

// ---------------------------------------------------------------------------
// AI 代码审查（review.go）
// ---------------------------------------------------------------------------

/// ReviewCheck：OCR 是否就绪（配置 > PATH；结果进程内缓存）。
#[tauri::command]
pub fn review_check() -> Result<ReviewCheckResult, String> {
    let cell = ocr_check_cache();
    let mut guard = cell.lock().unwrap();
    if let Some(cached) = guard.clone() {
        return Ok(cached);
    }
    let res = match ocr::find_exe("") {
        Err(e) => ReviewCheckResult {
            message: format!("{e:#}"),
            ..Default::default()
        },
        Ok(exe_path) => {
            let ver = ocr::version(&exe_path);
            if ver == "unknown" {
                ReviewCheckResult {
                    exe_path,
                    message: "无法获取版本: 命令执行失败".into(),
                    ..Default::default()
                }
            } else {
                ReviewCheckResult {
                    available: true,
                    exe_path,
                    version: ver,
                    message: String::new(),
                }
            }
        }
    };
    *guard = Some(res.clone());
    Ok(res)
}

fn audience_flag(s: &str) -> &'static str {
    if s.eq_ignore_ascii_case("human") { "human" } else { "agent" }
}

/// stderr 进度行 → stage 分类（对齐 classifyLine，未识别也当 analyzing 计活跃度）。
fn classify_line(line: &str) -> (&'static str, &str) {
    let low = line.to_lowercase();
    if low.contains("diff") || low.contains("collect") || low.contains("gather") {
        ("collecting", line)
    } else if low.contains("parse") || low.contains("format") || low.contains("result") {
        ("parsing", line)
    } else {
        // error/fail 也归 analyzing，UI 直接展示原文
        ("analyzing", line)
    }
}

fn truncate_str(s: &str, n: usize) -> String {
    if s.len() <= n {
        s.to_string()
    } else {
        let mut end = n;
        while end > 0 && !s.is_char_boundary(end) {
            end -= 1;
        }
        format!("{}...", &s[..end])
    }
}

/// ReviewRun：执行 ocr review，实时转发 "review:progress" 事件 {stage, message}，
/// 返回 OCR 的完整 JSON 结果（status/summary/comments/warnings）。
/// 审查行为默认值（并发/工具上限/受众/语言）读自 config；LLM 端点配置由 OCR
/// 自己的 ~/.opencodereview/config.json 提供。
#[tauri::command]
pub fn review_run(
    app: tauri::AppHandle,
    repo: Option<String>,
    dto: ReviewRunOptionsDto,
) -> Result<serde_json::Value, String> {
    let repo = repo_path(repo);
    let exe_path = ocr::find_exe("").map_err(|e| format!("{e:#}"))?;

    // 审查行为默认值来自 config.yaml，dto 可按次覆盖 model/background
    let cfg = Settings::load().unwrap_or_default();
    let audience = if cfg.ocr.audience.is_empty() { "agent".to_string() } else { cfg.ocr.audience.clone() };

    let mut args: Vec<String> = vec![
        "review".into(),
        "--format".into(),
        "json".into(),
        "--audience".into(),
        audience_flag(&audience).into(),
        "--repo".into(),
        repo.to_string_lossy().into_owned(),
    ];
    // 恰好一种审查模式（都不传 = workspace 模式）
    if !dto.commit.is_empty() {
        args.push("--commit".into());
        args.push(dto.commit.clone());
    } else if !dto.from.is_empty() && !dto.to.is_empty() {
        args.push("--from".into());
        args.push(dto.from.clone());
        args.push("--to".into());
        args.push(dto.to.clone());
    }
    if !dto.model.is_empty() {
        args.push("--model".into());
        args.push(dto.model.clone());
    }
    if !dto.background.is_empty() {
        args.push("--background".into());
        args.push(dto.background.clone());
    }
    if cfg.ocr.concurrency > 0 {
        args.push("--concurrency".into());
        args.push(cfg.ocr.concurrency.to_string());
    }
    if cfg.ocr.max_tools > 0 {
        args.push("--max-tools".into());
        args.push(cfg.ocr.max_tools.to_string());
    }
    if !cfg.ocr.language.is_empty() {
        args.push("--language".into());
        args.push(cfg.ocr.language.clone());
    }

    let mut child = Command::new(&exe_path)
        .args(&args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("启动 OCR 失败: {e}"))?;

    // 首个 tick：让 UI 立即离开 starting 状态
    let _ = app.emit("review:progress", serde_json::json!({ "stage": "collecting", "message": "收集改动差异…" }));

    // stderr 实时透传为进度事件；stdout 为最终 JSON（结束时一次性写出）
    if let Some(stderr) = child.stderr.take() {
        let app2 = app.clone();
        std::thread::spawn(move || {
            let reader = std::io::BufReader::new(stderr);
            for line in reader.lines() {
                let Ok(l) = line else { break };
                let l = l.trim();
                if l.is_empty() {
                    continue;
                }
                let (stage, message) = classify_line(l);
                let _ = app2.emit("review:progress", serde_json::json!({ "stage": stage, "message": message }));
            }
        });
    }

    let output = child
        .wait_with_output()
        .map_err(|e| format!("等待 OCR 结束失败: {e}"))?;
    if !output.status.success() {
        let stderr_text = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(format!("OCR 审查失败 (exit {:?}): {stderr_text}", output.status.code()));
    }

    let _ = app.emit("review:progress", serde_json::json!({ "stage": "parsing", "message": "解析审查结果…" }));
    let stdout = String::from_utf8_lossy(&output.stdout);
    let trimmed = stdout.trim();
    let parse = serde_json::from_str::<serde_json::Value>(trimmed)
        .map_err(|e| format!("解析 OCR 输出失败: {e} (原始: {})", truncate_str(trimmed, 300)));
    let _ = app.emit("review:progress", serde_json::json!({ "stage": "done", "message": "完成" }));
    parse
}

// ---------------------------------------------------------------------------
// OCR config.json 读写（ocrconfig.go）
// ---------------------------------------------------------------------------

fn home_dir() -> Result<PathBuf, String> {
    #[cfg(windows)]
    let var = std::env::var_os("USERPROFILE");
    #[cfg(not(windows))]
    let var = std::env::var_os("HOME");
    var.map(PathBuf::from)
        .ok_or_else(|| "无法确定用户主目录".to_string())
}

fn ocr_config_path() -> Result<PathBuf, String> {
    Ok(home_dir()?.join(".opencodereview").join("config.json"))
}

/// 12 个内置 provider 预设（镜像 OCR internal/llm/providers.go 注册表，保持展示顺序）。
fn ocr_presets() -> Vec<OcrProviderPreset> {
    vec![
        OcrProviderPreset {
            name: "anthropic",
            display_name: "Anthropic Claude API",
            protocol: "anthropic",
            base_url: "https://api.anthropic.com",
            auth_header: "x-api-key",
            models: &["claude-opus-4-8", "claude-opus-4-7", "claude-opus-4-6", "claude-sonnet-4-6"],
        },
        OcrProviderPreset {
            name: "openai",
            display_name: "OpenAI API",
            protocol: "openai",
            base_url: "https://api.openai.com/v1",
            auth_header: "",
            models: &["gpt-5.5", "gpt-5.4", "gpt-5.4-mini"],
        },
        OcrProviderPreset {
            name: "dashscope",
            display_name: "Alibaba DashScope API",
            protocol: "openai",
            base_url: "https://dashscope.aliyuncs.com/compatible-mode/v1",
            auth_header: "",
            models: &["qwen3.7-max", "qwen3.7-plus", "qwen3.6-plus", "qwen3.6-flash"],
        },
        OcrProviderPreset {
            name: "dashscope-tokenplan",
            display_name: "Alibaba DashScope Token Plan",
            protocol: "openai",
            base_url: "https://token-plan.cn-beijing.maas.aliyuncs.com/compatible-mode/v1",
            auth_header: "",
            models: &[
                "qwen3.7-max", "qwen3.7-plus", "qwen3.6-plus", "qwen3.6-flash", "deepseek-v4-pro",
                "deepseek-v4-flash", "kimi-k2.6", "kimi-k2.5", "glm-5.2", "glm-5.1", "glm-5", "MiniMax-M2.5",
            ],
        },
        OcrProviderPreset {
            name: "volcengine",
            display_name: "Volcano Engine Ark (豆包)",
            protocol: "openai",
            base_url: "https://ark.cn-beijing.volces.com/api/v3",
            auth_header: "",
            models: &["doubao-seed-2-0-lite-260428", "doubao-seed-2-0-mini-260428", "doubao-seed-2-0-pro-260215"],
        },
        OcrProviderPreset {
            name: "deepseek",
            display_name: "DeepSeek API",
            protocol: "openai",
            base_url: "https://api.deepseek.com",
            auth_header: "",
            models: &["deepseek-v4-pro", "deepseek-v4-flash"],
        },
        OcrProviderPreset {
            name: "tencent-tokenhub",
            display_name: "Tencent TokenHub API",
            protocol: "openai",
            base_url: "https://tokenhub.tencentmaas.com/v1",
            auth_header: "",
            models: &["hy3-preview"],
        },
        OcrProviderPreset {
            name: "hy-tokenplan",
            display_name: "Tencent Hunyuan Token Plan",
            protocol: "openai",
            base_url: "https://lkeap.cloud.tencent.com/plan/v3",
            auth_header: "",
            models: &["hy3-preview"],
        },
        OcrProviderPreset {
            name: "kimi",
            display_name: "Kimi Moonshot API",
            protocol: "openai",
            base_url: "https://api.moonshot.cn/v1",
            auth_header: "",
            models: &["kimi-k2.7-code", "kimi-k2.6", "kimi-k2.5"],
        },
        OcrProviderPreset {
            name: "z-ai",
            display_name: "Z.AI (智谱)",
            protocol: "openai",
            base_url: "https://open.bigmodel.cn/api/paas/v4",
            auth_header: "",
            models: &["glm-5.2", "glm-5.1", "glm-5-turbo", "glm-4.7"],
        },
        OcrProviderPreset {
            name: "mimo",
            display_name: "Xiaomi MiMo API",
            protocol: "openai",
            base_url: "https://api.xiaomimimo.com/v1",
            auth_header: "",
            models: &["mimo-v2.5-pro", "mimo-v2.5", "mimo-v2-pro", "mimo-v2-omni", "mimo-v2-flash"],
        },
        OcrProviderPreset {
            name: "minimax",
            display_name: "MiniMax API",
            protocol: "openai",
            base_url: "https://api.minimaxi.com/v1",
            auth_header: "",
            models: &[
                "MiniMax-M3", "MiniMax-M2.7", "MiniMax-M2.7-highspeed", "MiniMax-M2.5", "MiniMax-M2.5-highspeed",
            ],
        },
    ]
}

/// 密钥打码："" → ""，短密钥 → "****"，否则 "****<末4位>"。
fn mask_key(k: &str) -> String {
    if k.is_empty() {
        return String::new();
    }
    let b = k.as_bytes();
    if b.len() <= 4 {
        return "****".into();
    }
    format!("****{}", String::from_utf8_lossy(&b[b.len() - 4..]))
}

fn mask_config(c: &mut OcrConfig) {
    for e in c.providers.values_mut() {
        e.api_key = mask_key(&e.api_key);
    }
    for e in c.custom_providers.values_mut() {
        e.api_key = mask_key(&e.api_key);
    }
    if let Some(llm) = c.llm.as_mut() {
        llm.auth_token = mask_key(&llm.auth_token);
    }
}

/// OcrConfigGet：读取 ~/.opencodereview/config.json（密钥打码后返回）+ 预设列表。
#[tauri::command]
pub fn ocr_config_get() -> Result<OcrConfigResult, String> {
    let path = ocr_config_path()?;
    let mut res = OcrConfigResult {
        path: path.display().to_string(),
        presets: ocr_presets(),
        ..Default::default()
    };
    let data = match std::fs::read_to_string(&path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(res),
        Err(e) => return Err(format!("读取 OCR 配置失败: {e}")),
        Ok(d) => d,
    };
    let mut cfg: OcrConfig =
        serde_json::from_str(&data).map_err(|e| format!("解析 OCR 配置失败: {e}"))?;
    mask_config(&mut cfg);
    res.config = cfg;
    res.exists = true;
    Ok(res)
}

fn apply_llm_patch(cfg: &mut OcrConfig, field: &str, value: &str) -> Result<(), String> {
    if cfg.llm.is_none() {
        cfg.llm = Some(OcrLlmConfig::default());
    }
    let llm = cfg.llm.as_mut().unwrap();
    match field {
        "url" => llm.url = value.to_string(),
        "auth_token" => {
            // 空值/打码回显不覆盖真实密钥
            if value.is_empty() || value.starts_with("****") {
                return Ok(());
            }
            llm.auth_token = value.to_string();
        }
        "model" => llm.model = value.to_string(),
        other => return Err(format!("未知 llm 字段: {other}")),
    }
    Ok(())
}

fn apply_ocr_patch(cfg: &mut OcrConfig, key: &str, value: &str) -> Result<(), String> {
    match key {
        "provider" => {
            cfg.provider = value.to_string();
            return Ok(());
        }
        "model" => {
            cfg.model = value.to_string();
            return Ok(());
        }
        "language" => {
            cfg.language = value.to_string();
            return Ok(());
        }
        _ => {}
    }

    let parts: Vec<&str> = key.split('.').collect();
    if parts.len() < 3 {
        return Err("未知配置项".into());
    }
    let (bucket, name, field) = (parts[0], parts[1], parts[2]);
    if bucket == "llm" {
        // llm.<field>（两段式键）
        return apply_llm_patch(cfg, parts[1], value);
    }
    let m: &mut BTreeMap<String, OcrProviderEntry> = if bucket == "providers" {
        &mut cfg.providers
    } else if bucket == "custom_providers" {
        &mut cfg.custom_providers
    } else {
        return Err(format!("未知配置分组: {bucket}"));
    };

    let mut entry = m.get(name).cloned().unwrap_or_default();
    match field {
        "api_key" => {
            // 空值/打码回显不覆盖真实密钥
            if value.is_empty() || value == "****" || value.starts_with("****") {
                return Ok(());
            }
            entry.api_key = value.to_string();
        }
        "url" => entry.url = value.to_string(),
        "model" => entry.model = value.to_string(),
        "protocol" => entry.protocol = value.to_string(),
        "auth_header" => entry.auth_header = value.to_string(),
        other => return Err(format!("未知字段: {other}")),
    }
    m.insert(name.to_string(), entry);
    Ok(())
}

/// OcrConfigSave：点路径 patch 写回 config.json（先读旧值合并，不整体覆盖）。
#[tauri::command]
pub fn ocr_config_save(patch: std::collections::HashMap<String, String>) -> Result<(), String> {
    let path = ocr_config_path()?;
    let mut cfg = OcrConfig::default();
    if let Ok(data) = std::fs::read_to_string(&path) {
        let _ = serde_json::from_str::<OcrConfig>(&data).map(|c| cfg = c);
    }

    for (key, value) in &patch {
        apply_ocr_patch(&mut cfg, key, value).map_err(|e| format!("无效的配置项 {key}: {e}"))?;
    }

    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("创建配置目录失败: {e}"))?;
    }
    let mut out = serde_json::to_string_pretty(&cfg).map_err(|e| e.to_string())?;
    out.push('\n');
    std::fs::write(&path, out).map_err(|e| format!("写入 OCR 配置失败: {e}"))
}

/// OcrConfigGetRaw：返回 config.json 原文（含真实密钥，仅供高级 JSON 编辑模式）。
#[tauri::command]
pub fn ocr_config_get_raw() -> Result<OcrConfigRawResult, String> {
    let path = ocr_config_path()?;
    match std::fs::read_to_string(&path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(OcrConfigRawResult {
            path: path.display().to_string(),
            exists: false,
            content: String::new(),
        }),
        Err(e) => Err(format!("读取 OCR 配置失败: {e}")),
        Ok(content) => Ok(OcrConfigRawResult {
            path: path.display().to_string(),
            exists: true,
            content,
        }),
    }
}

/// OcrConfigSaveRaw：整体覆写 config.json（仅校验 JSON 可解析），返回写入路径。
#[tauri::command]
pub fn ocr_config_save_raw(content: String) -> Result<String, String> {
    // 校验：必须是可解析的 JSON 对象
    let probe: serde_json::Map<String, serde_json::Value> = serde_json::from_str(&content)
        .map_err(|e| format!("JSON 格式无效: {e}"))?;
    let path = ocr_config_path()?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("创建配置目录失败: {e}"))?;
    }
    // 对象根时重排格式保持稳定；非对象（空 map 之外不可能出现）不改动
    let content = if !probe.is_empty() {
        serde_json::to_string_pretty(&probe).unwrap_or(content)
    } else {
        content
    };
    let mut out = content;
    out.push('\n');
    std::fs::write(&path, out).map_err(|e| format!("写入 OCR 配置失败: {e}"))?;
    Ok(path.display().to_string())
}
