//! system 域命令：设置（config.yaml）、代理规则（rules.lua）、
//! 命令片段（snippets.json）与 GitHub Release 查询。
//!
//! 移植自 git-mate `gui/internal/system/{settings,rules,snippets,download}.go`，
//! 行为对齐 Go 实现；存储布局迁移到 `~/.cicbyte/apps/byte-git/`：
//! - 设置：core::config::Settings（config/config.yaml）
//! - 规则：core::clone::ensure_rules_file（rules/rules.lua，内嵌默认模板）
//! - 片段：`~/.cicbyte/apps/byte-git/snippets.json`（应用级、跨仓库）

use byte_git_core::config::Settings;
use byte_git_core::util::AppPaths;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

/// 与 Go GUI 一致：无仓库上下文的命令统一接收可选 repo 参数；
/// 缺省回落到当前仓库状态（state.rs，对应 Core.CurrentRepo），最终 "."。
fn repo_path(repo: Option<String>) -> std::path::PathBuf { crate::state::resolve(repo) }

/// 子进程隐藏控制台窗口（sysproc.HideExec 的 Windows 等价物）。
#[cfg(windows)]
fn hide_window(cmd: &mut std::process::Command) {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    cmd.creation_flags(CREATE_NO_WINDOW);
}
#[cfg(not(windows))]
fn hide_window(_cmd: &mut std::process::Command) {}

/// 阻塞任务挪 std 线程（Tauri async 命令里长 spawn 会卡运行时；
/// 返回闭包结果本身——闭包体常为 Result<T,String>，调用侧再 `?` 解包）。
fn run_blocking<T, F>(f: F) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce() -> T + Send + 'static,
{
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(f());
    });
    rx.recv().map_err(|_| "任务线程异常退出".to_string())
}

// =====================================================================
// Settings（前端扁平视图，json tag 对齐 Go SettingsDTO）
// =====================================================================

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsDto {
    pub model: String,
    pub base_url: String,
    pub api_key: String,
    pub lang: String,
    pub max_tokens: i64,
    pub github_token: String,
    pub ocr_exe_path: String,
    pub ocr_concurrency: i64,
    pub ocr_max_tools: i64,
    pub ocr_audience: String,
    pub ocr_language: String,
    pub gitleaks_exe_path: String,
    pub reference_exe_path: String,
    pub rewrite_auto_backup: bool,
    pub term_shell_path: String,
    pub term_font_size: i64,
    /// 侧边栏外部依赖功能开关（Go tag 为 featuresGitHub，含大写 HUB，需显式 rename）。
    #[serde(rename = "featuresGitHub")]
    pub features_github: bool,
    pub features_reference: bool,
    pub features_secret: bool,
    pub features_review: bool,
    pub config_path: String,
}

fn to_dto(s: &Settings) -> Result<SettingsDto, String> {
    Ok(SettingsDto {
        model: s.ai.model.clone(),
        base_url: s.ai.base_url.clone(),
        api_key: s.ai.api_key.clone(),
        lang: s.ai.lang.clone(),
        max_tokens: s.ai.max_tokens,
        github_token: s.github.token.clone(),
        ocr_exe_path: s.ocr.exe_path.clone(),
        ocr_concurrency: s.ocr.concurrency,
        ocr_max_tools: s.ocr.max_tools,
        ocr_audience: s.ocr.audience.clone(),
        ocr_language: s.ocr.language.clone(),
        gitleaks_exe_path: s.gitleaks.exe_path.clone(),
        reference_exe_path: s.reference.exe_path.clone(),
        rewrite_auto_backup: s.rewrite.auto_backup,
        term_shell_path: s.terminal.shell_path.clone(),
        term_font_size: s.terminal.font_size,
        features_github: s.features.github,
        features_reference: s.features.reference,
        features_secret: s.features.secret,
        features_review: s.features.review,
        config_path: AppPaths::new()
            .map_err(|e| e.to_string())?
            .settings_file()
            .display()
            .to_string(),
    })
}

/// SettingsGet 读取 config.yaml（缺失时写入默认值）并返回扁平 DTO。
#[tauri::command]
pub fn settings_get(repo: Option<String>) -> Result<SettingsDto, String> {
    let _ = repo_path(repo.clone()); // 应用级命令，repo 仅为适配层统一注入
    let s = Settings::load().map_err(|e| e.to_string())?;
    to_dto(&s)
}

/// SettingsReload 丢弃内存中未保存的修改，重新读取 config.yaml。
#[tauri::command]
pub fn settings_reload(repo: Option<String>) -> Result<SettingsDto, String> {
    let _ = repo_path(repo.clone());
    let s = Settings::load().map_err(|e| e.to_string())?;
    to_dto(&s)
}

/// SettingsSave 按点路径 patch（如 {"ai.model": "gpt-4o"}）合并后整体落盘。
/// exe_path 变更时同步失效对应可用性检测缓存（对齐 Go 缓存失效钩子）。
#[tauri::command]
pub fn settings_save(repo: Option<String>, patch: HashMap<String, String>) -> Result<(), String> {
    let _ = repo_path(repo.clone());
    let mut s = Settings::load().map_err(|e| e.to_string())?;
    for (key, value) in &patch {
        apply_setting(&mut s, key, value)
            .map_err(|e| format!("invalid value for {key}: {e}"))?;
    }
    s.save().map_err(|e| e.to_string())?;
    // 缓存失效：下次 check 重新探测（可能已变更的）可执行文件
    let gitleaks = patch.contains_key("gitleaks.exe_path");
    let ocr = patch.contains_key("ocr.exe_path");
    if gitleaks || ocr {
        crate::commands::secret::clear_check_cache(gitleaks, ocr);
    }
    if patch.contains_key("reference.exe_path") {
        crate::commands::reference::clear_check_cache();
    }
    Ok(())
}

/// 点路径写入（对齐 Go applySetting 的 switch；值为字符串，类型按字段转换）。
/// 相比 Go 多支持 `gitignore.default_source`（core Settings schema 含该字段）。
fn apply_setting(s: &mut Settings, key: &str, value: &str) -> Result<(), String> {
    match key {
        "ai.model" => s.ai.model = value.to_string(),
        "ai.base_url" => s.ai.base_url = value.to_string(),
        "ai.api_key" => s.ai.api_key = value.to_string(),
        "ai.lang" => s.ai.lang = value.to_string(),
        "ai.max_tokens" => s.ai.max_tokens = parse_int(value)?,
        "github.token" => s.github.token = value.to_string(),
        "ocr.exe_path" => s.ocr.exe_path = value.to_string(),
        "ocr.concurrency" => s.ocr.concurrency = parse_int(value)?,
        "ocr.max_tools" => s.ocr.max_tools = parse_int(value)?,
        "ocr.audience" => s.ocr.audience = value.to_string(),
        "ocr.language" => s.ocr.language = value.to_string(),
        "gitleaks.exe_path" => s.gitleaks.exe_path = value.to_string(),
        "reference.exe_path" => s.reference.exe_path = value.to_string(),
        "gitignore.default_source" => s.gitignore.default_source = value.to_string(),
        "features.github" => s.features.github = parse_bool(value)?,
        "features.reference" => s.features.reference = parse_bool(value)?,
        "features.secret" => s.features.secret = parse_bool(value)?,
        "features.review" => s.features.review = parse_bool(value)?,
        "rewrite.auto_backup" => s.rewrite.auto_backup = parse_bool(value)?,
        "terminal.shell_path" => s.terminal.shell_path = value.to_string(),
        "terminal.font_size" => s.terminal.font_size = parse_int(value)?,
        _ => return Err(format!("unknown config key: {key}")),
    }
    Ok(())
}

/// strconv.Atoi 等价。
fn parse_int(v: &str) -> Result<i64, String> {
    v.trim().parse::<i64>().map_err(|e| e.to_string())
}

/// strconv.ParseBool 等价（接受 1/t/T/true/TRUE/True/0/f/F/false/FALSE/False）。
fn parse_bool(v: &str) -> Result<bool, String> {
    match v {
        "1" | "t" | "T" | "true" | "TRUE" | "True" => Ok(true),
        "0" | "f" | "F" | "false" | "FALSE" | "False" => Ok(false),
        _ => Err(format!("strconv.ParseBool: parsing {v:?}: invalid syntax")),
    }
}

// =====================================================================
// Rules（rules.lua 代理规则脚本）
// =====================================================================

/// RulesRead 返回 rules.lua 内容；文件不存在时落盘默认模板并返回（与 Go 的
/// “返回默认模板”等价——core 未暴露内嵌默认值，故经 ensure 初始化后读取）。
#[tauri::command]
pub fn rules_read(repo: Option<String>) -> Result<String, String> {
    let _ = repo_path(repo.clone());
    let file = byte_git_core::clone::ensure_rules_file().map_err(|e| e.to_string())?;
    std::fs::read_to_string(&file).map_err(|e| e.to_string())
}

/// RulesSave 将给定内容写入 rules.lua。
#[tauri::command]
pub fn rules_save(repo: Option<String>, content: String) -> Result<(), String> {
    let _ = repo_path(repo.clone());
    let file = byte_git_core::clone::ensure_rules_file().map_err(|e| e.to_string())?;
    std::fs::write(&file, content).map_err(|e| e.to_string())
}

/// RulesReset 恢复默认模板：删除现有文件后由 ensure_rules_file 重新释放内嵌默认值。
#[tauri::command]
pub fn rules_reset(repo: Option<String>) -> Result<(), String> {
    let _ = repo_path(repo.clone());
    let file = byte_git_core::clone::ensure_rules_file().map_err(|e| e.to_string())?;
    match std::fs::remove_file(&file) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(e.to_string()),
    }
    byte_git_core::clone::ensure_rules_file().map_err(|e| e.to_string())?;
    Ok(())
}

/// RulesPath 返回 rules.lua 绝对路径（供 UI 展示）。
#[tauri::command]
pub fn rules_path(repo: Option<String>) -> Result<String, String> {
    let _ = repo_path(repo.clone());
    let paths = AppPaths::new().map_err(|e| e.to_string())?;
    Ok(paths.rules_file().display().to_string())
}

// =====================================================================
// Snippets（个人 git 命令库，应用级跨仓库）
// =====================================================================

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Snippet {
    pub id: String,
    pub command: String,
    pub note: String,
    pub tag: String,
    pub favorite: bool,
    pub created_at: i64,
}

/// snippets.json 路径：`~/.cicbyte/apps/byte-git/snippets.json`。
fn snippets_file() -> Result<PathBuf, String> {
    Ok(AppPaths::new().map_err(|e| e.to_string())?.root().join("snippets.json"))
}

fn load_snippets() -> Result<Vec<Snippet>, String> {
    let path = snippets_file()?;
    let data = match std::fs::read(&path) {
        Ok(d) => d,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e.to_string()),
    };
    serde_json::from_slice(&data).map_err(|e| format!("解析 snippets.json 失败: {e}"))
}

fn save_snippets(list: &[Snippet]) -> Result<(), String> {
    let path = snippets_file()?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let mut data = serde_json::to_string_pretty(list).map_err(|e| e.to_string())?;
    data.push('\n');
    std::fs::write(&path, data).map_err(|e| e.to_string())
}

/// 短 ID：时间戳 + 随机后缀（约定实现；Go 为 3 字节随机 hex）。
fn gen_id() -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let noise = (now.subsec_nanos() as u64)
        .wrapping_mul(2654435761)
        ^ ((std::process::id() as u64) << 16);
    format!("{}-{:04x}", now.as_secs(), (noise & 0xffff) as u16)
}

/// 排序：收藏优先，其次按创建时间倒序（稳定排序）。
fn sort_snippets(list: &mut [Snippet]) {
    list.sort_by(|a, b| {
        if a.favorite != b.favorite {
            return b.favorite.cmp(&a.favorite);
        }
        b.created_at.cmp(&a.created_at)
    });
}

/// SnippetsList 返回全部片段（收藏优先，时间倒序）。
#[tauri::command]
pub fn snippets_list(repo: Option<String>) -> Result<Vec<Snippet>, String> {
    let _ = repo_path(repo.clone());
    let mut list = load_snippets()?;
    sort_snippets(&mut list);
    Ok(list)
}

/// SnippetsAdd 新建片段；ID 与 createdAt 由后端生成并在返回值中回填。
#[tauri::command]
pub fn snippets_add(repo: Option<String>, s: Snippet) -> Result<Snippet, String> {
    let _ = repo_path(repo.clone());
    if s.command.trim().is_empty() {
        return Err("命令不能为空".into());
    }
    let mut s = s;
    s.id = gen_id();
    s.created_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let mut list = load_snippets()?;
    list.push(s.clone());
    save_snippets(&list)?;
    Ok(s)
}

/// SnippetsUpdate 按 ID 整体替换一条片段。
#[tauri::command]
pub fn snippets_update(repo: Option<String>, s: Snippet) -> Result<(), String> {
    let _ = repo_path(repo.clone());
    if s.id.is_empty() {
        return Err("ID 不能为空".into());
    }
    if s.command.trim().is_empty() {
        return Err("命令不能为空".into());
    }
    let mut list = load_snippets()?;
    match list.iter_mut().find(|item| item.id == s.id) {
        Some(item) => *item = s,
        None => return Err(format!("未找到 ID: {}", s.id)),
    }
    save_snippets(&list)
}

/// SnippetsDelete 按 ID 删除一条片段。
#[tauri::command]
pub fn snippets_delete(repo: Option<String>, id: String) -> Result<(), String> {
    let _ = repo_path(repo.clone());
    let mut list = load_snippets()?;
    let before = list.len();
    list.retain(|item| item.id != id);
    if list.len() == before {
        return Err(format!("未找到 ID: {id}"));
    }
    save_snippets(&list)
}

/// SnippetsToggleFavorite 翻转一条片段的收藏标记。
#[tauri::command]
pub fn snippets_toggle_favorite(repo: Option<String>, id: String) -> Result<(), String> {
    let _ = repo_path(repo.clone());
    let mut list = load_snippets()?;
    match list.iter_mut().find(|item| item.id == id) {
        Some(item) => item.favorite = !item.favorite,
        None => return Err(format!("未找到 ID: {id}")),
    }
    save_snippets(&list)
}

/// SnippetsFilePath 返回 snippets.json 绝对路径（供 UI 展示）。
#[tauri::command]
pub fn snippets_file_path(repo: Option<String>) -> Result<String, String> {
    let _ = repo_path(repo.clone());
    Ok(snippets_file()?.display().to_string())
}

// =====================================================================
// Download（GitHub Release 列表；与 Go 一致经 curl 调 GitHub API）
// =====================================================================

#[derive(Debug, Clone, Serialize)]
pub struct AssetInfo {
    pub name: String,
    pub size: i64,
    /// Go tag 为 browserDownloadURL（URL 全大写），serde camelCase 会得到
    /// browserDownloadUrl，故显式 rename。
    #[serde(rename = "browserDownloadURL")]
    pub browser_download_url: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseInfo {
    pub tag_name: String,
    pub published_at: String,
    pub body: String,
    pub assets: Vec<AssetInfo>,
}

/// GitHub API 原始字段（snake_case）。
#[derive(Debug, Deserialize)]
struct GhAsset {
    #[serde(default)]
    name: String,
    #[serde(default)]
    size: i64,
    #[serde(default)]
    browser_download_url: String,
}

#[derive(Debug, Deserialize)]
struct GhRelease {
    #[serde(default)]
    tag_name: String,
    #[serde(default)]
    published_at: String,
    #[serde(default)]
    body: String,
    #[serde(default)]
    assets: Vec<GhAsset>,
}

/// DownloadReleases 拉取仓库最近 30 条 Release（Go 同款 curl -sL 调用）。
#[tauri::command]
pub fn download_releases(owner: String, repo: String) -> Result<Vec<ReleaseInfo>, String> {
    let url = format!("https://api.github.com/repos/{owner}/{repo}/releases?per_page=30");
    let mut cmd = std::process::Command::new("curl");
    cmd.args(["-sL", &url]);
    hide_window(&mut cmd);
    let out = cmd.output().map_err(|e| format!("启动 curl 失败: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "curl 失败: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    let releases: Vec<GhRelease> =
        serde_json::from_slice(&out.stdout).map_err(|e| format!("解析 Release 响应失败: {e}"))?;
    Ok(releases
        .into_iter()
        .map(|r| ReleaseInfo {
            tag_name: r.tag_name,
            published_at: r.published_at,
            body: r.body,
            assets: r
                .assets
                .into_iter()
                .map(|a| AssetInfo {
                    name: a.name,
                    size: a.size,
                    browser_download_url: a.browser_download_url,
                })
                .collect(),
        })
        .collect())
}

/// PATH 查找可执行文件（PATHEXT 感知；对应 Go exec.LookPath）。
fn look_path(exe: &str) -> bool {
    let path_var = std::env::var("PATH").unwrap_or_default();
    let candidate = |dir: &str, name: &str| -> Vec<std::path::PathBuf> {
        let p = std::path::Path::new(dir).join(name);
        if p.is_file() {
            return vec![p];
        }
        #[cfg(windows)]
        {
            std::env::var("PATHEXT")
                .unwrap_or_else(|_| ".COM;.EXE;.BAT;.CMD".into())
                .split(';')
                .filter(|e| !e.is_empty())
                    .map(|e| std::path::Path::new(dir).join(format!("{name}{e}")))
                .filter(|p| p.is_file())
                .collect()
        }
        #[cfg(not(windows))]
        Vec::new()
    };
    path_var.split(';').filter(|d| !d.is_empty()).any(|d| !candidate(d, exe).is_empty())
}

/// NpmAvailable：npm 是否在 PATH 上（对应 Go exec.LookPath("npm")）。
/// 前端当前未调用，为对齐 Go SystemModule 导出面补齐。
#[tauri::command]
pub fn npm_available() -> bool {
    look_path("npm")
}

// ---------------------------------------------------------------------------
// 窗口控制（Go gui/internal/window.go WindowModule）
// ---------------------------------------------------------------------------

fn main_window(app: &tauri::AppHandle) -> Result<tauri::WebviewWindow, String> {
    use tauri::Manager;
    app.get_webview_window("main").ok_or_else(|| "主窗口未就绪".to_string())
}

/// WindowMinimize：最小化主窗口。
#[tauri::command]
pub fn window_minimize(app: tauri::AppHandle) -> Result<(), String> {
    main_window(&app)?.minimize().map_err(|e| e.to_string())
}

/// WindowMaximize：**切换**最大化（已最大化则还原；对齐 Go WindowModule）。
#[tauri::command]
pub fn window_maximize(app: tauri::AppHandle) -> Result<(), String> {
    let win = main_window(&app)?;
    if win.is_maximized().map_err(|e| e.to_string())? {
        win.unmaximize().map_err(|e| e.to_string())
    } else {
        win.maximize().map_err(|e| e.to_string())
    }
}

/// WindowClose：退出整个应用（对应 Go runtime.Quit，非隐藏窗口）。
#[tauri::command]
pub fn window_close(app: tauri::AppHandle) -> Result<(), String> {
    app.exit(0);
    Ok(())
}

/// WindowIsMaximized：主窗口是否处于最大化状态。
#[tauri::command]
pub fn window_is_maximized(app: tauri::AppHandle) -> Result<bool, String> {
    main_window(&app)?.is_maximized().map_err(|e| e.to_string())
}

// ---------------------------------------------------------------------------
// npm 管理（Go gui/internal/system/npm.go：安装/卸载/升级 OCR）
// ---------------------------------------------------------------------------

/// npm.NpmResult 镜像。
#[derive(Debug, Clone, Default, Serialize)]
pub struct NpmResultDto {
    pub ok: bool,
    pub output: String,
    /// 解析出的版本（version/latest 场景）
    #[serde(skip_serializing_if = "String::is_empty")]
    pub version: String,
}

/// OCR 审查工具的 npm 包名（Go ocrNpmPackage）。
const OCR_NPM_PACKAGE: &str = "@alibaba-group/open-code-review";

/// 执行 npm 命令：合并 stdout+stderr（npm 把进度/警告写 stderr，拆流会让成功安装看起来空），
/// CREATE_NO_WINDOW + 超时（GUI 中 npm 可能因 registry 卡死挂起，必须有截止）。
fn run_npm(timeout: std::time::Duration, args: &[&str]) -> (String, bool) {
    let mut cmd = std::process::Command::new("npm");
    cmd.args(args);
    hide_window(&mut cmd);
    let Ok(mut child) = cmd
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
    else {
        return ("npm 不可用（未安装或不在 PATH）".into(), false);
    };
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let out = child.wait_with_output();
        let _ = tx.send(out);
    });
    match rx.recv_timeout(timeout) {
        Ok(Ok(out)) => {
            let mut combined = String::from_utf8_lossy(&out.stdout).into_owned();
            combined.push_str(&String::from_utf8_lossy(&out.stderr));
            (combined.trim().to_string(), out.status.success())
        }
        Ok(Err(e)) => (format!("npm 执行失败: {e}"), false),
        Err(_) => ("npm 超时（网络/registry 卡死）".into(), false),
    }
}

/// `npm ls -g --json` 输出 → 已安装版本（JSON 解析，不字符串扫描）。
fn parse_npm_ls_version(json_out: &str) -> String {
    serde_json::from_str::<serde_json::Value>(json_out)
        .ok()
        .and_then(|v| {
            v.get("dependencies")
                .and_then(|d| d.get(OCR_NPM_PACKAGE))
                .and_then(|p| p.get("version"))
                .and_then(|s| s.as_str())
                .map(str::to_string)
        })
        .unwrap_or_default()
}

/// NpmManage：install / uninstall / update / version / latest。
/// 超时对齐 Go：install/update 5min（碰 registry），其余 2min。
/// 偏差：失败时错误文案带 npm 输出（Go Wails 拒绝时只回 "exit status 1"，信息量更少）。
#[tauri::command]
pub async fn npm_manage(action: String) -> Result<NpmResultDto, String> {
    let two = std::time::Duration::from_secs(120);
    let five = std::time::Duration::from_secs(300);
    let run = move || -> Result<NpmResultDto, String> {
        match action.as_str() {
            "install" => {
                let (out, ok) = run_npm(five, &["install", "-g", OCR_NPM_PACKAGE]);
                if !ok {
                    return Err(format!("npm install 失败: {out}"));
                }
                Ok(NpmResultDto { ok: true, output: out, ..Default::default() })
            }
            "uninstall" => {
                let (out, ok) = run_npm(two, &["uninstall", "-g", OCR_NPM_PACKAGE]);
                if !ok {
                    return Err(format!("npm uninstall 失败: {out}"));
                }
                Ok(NpmResultDto { ok: true, output: out, ..Default::default() })
            }
            "update" => {
                // npm update -g 跨版本不可靠 → 重装 @latest 强制升级（Go 同）
                let pkg_latest = format!("{OCR_NPM_PACKAGE}@latest");
                let (out, ok) = run_npm(five, &["install", "-g", pkg_latest.as_str()]);
                if !ok {
                    return Err(format!("npm update 失败: {out}"));
                }
                // 回读新装版本，UI 免二次往返
                let (ver_out, _) = run_npm(two, &["ls", "-g", OCR_NPM_PACKAGE, "--depth=0", "--json"]);
                Ok(NpmResultDto { ok: true, output: out, version: parse_npm_ls_version(&ver_out) })
            }
            "version" => {
                // npm ls 在包缺席时 exit 1 但 JSON 有效——常态非错误；仅输出不可解析才算真失败
                let (out, _) = run_npm(two, &["ls", "-g", OCR_NPM_PACKAGE, "--depth=0", "--json"]);
                let ver = parse_npm_ls_version(&out);
                if ver.is_empty() && !out.starts_with('{') {
                    return Err(format!("npm ls 失败: {out}"));
                }
                Ok(NpmResultDto { ok: true, output: out, version: ver })
            }
            "latest" => {
                let (out, ok) = run_npm(two, &["view", OCR_NPM_PACKAGE, "version"]);
                if !ok {
                    return Err(format!("npm view 失败: {out}"));
                }
                Ok(NpmResultDto { ok: true, output: out.clone(), version: out })
            }
            other => Err(format!("未知 npm 操作: {other}")),
        }
    };
    run_blocking(run)?
}
