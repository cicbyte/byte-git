//! reference 域命令：外部 `reference` CLI 进程集成 + `.reference/` 直读。
//!
//! 移植自 git-mate `gui/internal/reference/reference.go`（绑定层）与
//! `internal/reference/{client,discover,types}.go`（CLI 客户端，随本域一并
//! 内联实现——core 无对应模块）。镜像 secret 域的外部 exe 集成约定：
//! config.yaml `reference.exe_path` > PATH（PATHEXT 感知）> Windows go/bin
//! 探测；子进程 CREATE_NO_WINDOW + 超时 + stdout/stderr 分离捕获；
//! 结果进程内缓存（对应 Go refCheckCache）。
//!
//! reference 是项目级作用域（`.reference/` 位于项目根），所有命令经
//! `repo: Option<String>` → `crate::state::resolve` 解析项目目录。
//!
//! 与 Go 侧的对应关系：
//! - map / settings / wiki 文件 / 源码浏览：绕过 CLI 直读磁盘（Go 同款）；
//! - wiki 系列多为人类可读文本输出，按 Go 的前缀解析规则提取结构字段；
//! - Go 的 ReferenceModule 无 EventsEmit 进度事件，本域亦无事件。
//!
//! 已知偏差（详见各命令注释）：
//! - ReferenceClearCache（Go 设置保存时清可用性缓存）不在本域 23 方法清单，
//!   Rust settings_save 亦无清缓存钩子（secret 域同款现状）；
//! - Go nil 切片序列化为 null，Rust 空切片为 []（repos/topics/agents 为空时）。

use byte_git_core::config::Settings;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

/// 与 Go GUI 一致：无仓库上下文的命令统一接收可选 repo 参数；
/// 缺省回落到当前仓库状态（state.rs，对应 Core.CurrentRepo），最终 "."。
fn repo_path(repo: Option<String>) -> PathBuf {
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

/// 在独立线程执行阻塞任务（commit.rs 同款：克隆/拉取/大目录走查不冻结窗口）。
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
// 超时表（对齐 Go internal/reference 各 runRaw/runJSON 的时限）
// ---------------------------------------------------------------------------

/// 常规 JSON 只读命令（repo list / doctor 等，Go runJSON 30s）。
const SHORT_TIMEOUT: Duration = Duration::from_secs(30);
/// `init`（Go 30s）。
const INIT_TIMEOUT: Duration = Duration::from_secs(30);
/// `repo remove`（Go 5min）。
const REMOVE_TIMEOUT: Duration = Duration::from_secs(300);
/// `repo add`（Go 10min，大仓库克隆可达数十秒以上）。
const ADD_TIMEOUT: Duration = Duration::from_secs(600);
/// `repo update`（Go TaskContext(10min) 与 UpdateCtx 内层 10min 合并等价）。
const UPDATE_TIMEOUT: Duration = Duration::from_secs(600);
/// `repo scc`（Go 2min，大仓库代码统计较慢）。
const SCC_TIMEOUT: Duration = Duration::from_secs(120);
/// `wiki` 状态 / `wiki remote` 读取（Go 15s）。
const WIKI_SHORT_TIMEOUT: Duration = Duration::from_secs(15);
/// `wiki sync`（Go 5min，含网络 pull/push）。
const WIKI_SYNC_TIMEOUT: Duration = Duration::from_secs(300);
/// `wiki commit`（Go 60s）。
const WIKI_COMMIT_TIMEOUT: Duration = Duration::from_secs(60);
/// `wiki remote <url>` 设置（Go 30s）。
const WIKI_SET_TIMEOUT: Duration = Duration::from_secs(30);
/// `wiki trash` / `wiki restore`（Go 30s）。
const WIKI_TRASH_TIMEOUT: Duration = Duration::from_secs(30);

/// 前端文件读取上限（对齐 Go MaxSecretFileSize 与 ReadWiki maxBytes = 1MB）。
const MAX_FILE_SIZE: usize = 1 << 20;

// ---------------------------------------------------------------------------
// DTO（json tag 与 Go 结构体逐一对齐；CLI 输出解析 + 前端序列化复用）
// ---------------------------------------------------------------------------

/// ReferenceCheckResult（gui reference.go）：reference CLI 可用性。
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReferenceCheckResult {
    pub available: bool,
    pub exe_path: String,
    pub version: String,
    /// 不可用时的安装提示
    pub message: String,
}

/// ReferenceAddRequest（gui reference.go）：添加引用表单。
#[derive(Debug, Clone, Default, Deserialize)]
pub struct ReferenceAddRequest {
    /// URL（远程）或绝对路径（本地）
    #[serde(default)]
    pub target: String,
    /// 按本地仓库路径处理
    #[serde(default)]
    pub local: bool,
    /// 可选：克隆指定分支/标签（仅远程）
    #[serde(default)]
    pub branch: String,
    /// 可选：覆盖链接名
    #[serde(default)]
    pub name: String,
    /// 仅远程：缓存已存在时强制 git pull
    #[serde(default)]
    pub update: bool,
}

/// ReferenceInitRequest（gui reference.go）：init 表单。
/// agent ∈ claude|codex|opencode|zcode|mimocode|none（空 = none）。
#[derive(Debug, Clone, Default, Deserialize)]
pub struct ReferenceInitRequest {
    #[serde(default)]
    pub agent: String,
}

/// ReferenceSettings（gui reference.go）：`.reference/reference.settings.json`
/// 的归一化视图（磁盘上有 agents 数组 / agent 单串两种 schema）。
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReferenceSettingsDto {
    pub initialized: bool,
    /// 归一化：agents 数组或旧版 agent 单串
    pub agents: Vec<String>,
    /// false = settings 文件不存在（视为未初始化，非错误）
    pub has_file: bool,
}

/// Repo（internal/reference types.go）：单条引用，对应 `repo list -f json`
/// 的一行。注意 Go json tag 为 snake_case（cache_path/commit_at），不走 camelCase。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Repo {
    /// "remote" | "local"
    #[serde(default, rename = "type")]
    pub ty: String,
    /// 链接名（项目内唯一）
    #[serde(default)]
    pub name: String,
    /// URL（远程）或绝对路径（本地）
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub cache_path: String,
    /// 缓存 commit 的 ISO 日期
    #[serde(default)]
    pub commit_at: String,
    #[serde(default)]
    pub branch: String,
}

/// RepoList：`repo list -f json` 的 `{ "repos": [...] }` 信封。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RepoList {
    #[serde(default)]
    pub repos: Vec<Repo>,
}

/// MapTopic（internal/reference types.go）：MapEntry.Topics 的一项。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MapTopic {
    /// e.g. "release-workflow.md"
    #[serde(default)]
    pub file: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub commit: String,
}

/// MapEntry：`.reference/reference.map.jsonl` 的一行（比 Repo 更丰富的元数据，
/// 含驱动 wiki 浏览器的 topics 索引）。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MapEntry {
    #[serde(default)]
    pub ref_name: String,
    #[serde(default, rename = "type")]
    pub ty: String,
    #[serde(default)]
    pub platform: String,
    #[serde(default)]
    pub full_name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub repo_path: String,
    #[serde(default)]
    pub wiki_path: String,
    #[serde(default)]
    pub commit: String,
    #[serde(default)]
    pub topics: Vec<MapTopic>,
}

/// SccLanguage（internal/reference types.go）：语言统计一行。
/// 注意 CLI 输出的键是 "languages"（复数）。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SccLanguage {
    #[serde(default)]
    pub languages: String,
    #[serde(default)]
    pub files: i64,
    #[serde(default)]
    pub lines: i64,
    #[serde(default)]
    pub code: i64,
    #[serde(default)]
    pub comments: i64,
    #[serde(default)]
    pub blanks: i64,
    #[serde(default)]
    pub complexity: i64,
}

/// SccTopFile：Top 文件排名一行。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SccTopFile {
    #[serde(default)]
    pub filename: String,
    #[serde(default)]
    pub language: String,
    #[serde(default)]
    pub location: String,
    #[serde(default)]
    pub code: i64,
    #[serde(default)]
    pub complexity: i64,
}

/// SccResult：`repo scc <name> -f json` 完整载荷。
/// 注意 Go tag "topFiles" 为 camelCase。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SccResult {
    #[serde(default)]
    pub repo: String,
    #[serde(default)]
    pub path: String,
    #[serde(default)]
    pub languages: Vec<SccLanguage>,
    #[serde(default, rename = "topFiles")]
    pub top_files: Vec<SccTopFile>,
}

/// DoctorCheck：`doctor -f json` checks 数组的一项。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DoctorCheck {
    #[serde(default)]
    pub name: String,
    /// "ok" | "warn" | "error"
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub details: String,
    /// "core" | "agent" | ...
    #[serde(default)]
    pub group: String,
}

/// DoctorResult：doctor 载荷（CLI 另有 summary 字段，Go DTO 不含、一并忽略）。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DoctorResult {
    #[serde(default)]
    pub project_dir: String,
    #[serde(default)]
    pub checks: Vec<DoctorCheck>,
}

/// WikiStatus（internal/reference client.go）：`reference wiki` 文本输出的
/// 结构化提取。
#[derive(Debug, Clone, Default, Serialize)]
pub struct WikiStatus {
    pub initialized: bool,
    /// 远程 URL（未设置为 ""）
    pub remote: String,
    /// 工作区是否干净
    pub clean: bool,
    /// 原始文本（前端兜底展示）
    #[serde(rename = "rawOutput")]
    pub raw_output: String,
}

/// WikiTrashEntry：`wiki trash -f json` 的一行。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WikiTrashEntry {
    #[serde(default)]
    pub path: String,
    #[serde(default)]
    pub commit: String,
    #[serde(default)]
    pub date: String,
    #[serde(default)]
    pub message: String,
}

/// FileNode（gui core.FileNode；repo/reference/backup 浏览器共用，与 repo.rs 同款）。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileNode {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub size: i64,
}

/// FileResult（gui core.FileResult；与 repo.rs / secret.rs 同款）。
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileResult {
    pub content: String,
    pub lines: i64,
    pub binary: bool,
    pub not_found: bool,
}

// ---------------------------------------------------------------------------
// exe 发现（对齐 Go internal/reference/discover*.go）
// ---------------------------------------------------------------------------

fn reference_check_cache() -> &'static Mutex<Option<ReferenceCheckResult>> {
    static CACHE: OnceLock<Mutex<Option<ReferenceCheckResult>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(None))
}

/// 解析 reference 可执行文件路径（对齐 Go reference.FindExe）：
/// 1. config.yaml `reference.exe_path`（须能响应 `reference version`）
/// 2. PATH 查找（PATHEXT 感知，Windows 下 .exe/.bat/.cmd）
/// 3. Windows 回退探测 %USERPROFILE%\go\bin 与 %GOPATH%\bin
fn find_reference_exe() -> Result<String, String> {
    // Go：cfg, _ := config.LoadSettings() —— 加载失败静默视为未配置
    let user_path = Settings::load()
        .map(|s| s.reference.exe_path)
        .unwrap_or_default()
        .trim()
        .to_string();
    if !user_path.is_empty() {
        if reference_version(&user_path).is_ok() {
            return Ok(user_path);
        }
        return Err(format!("指定的 reference 路径无效或不可执行: {user_path}"));
    }
    if let Some(p) = look_path("reference") {
        return Ok(p.to_string_lossy().into_owned());
    }
    #[cfg(windows)]
    if let Some(p) = probe_windows_default_bin() {
        return Ok(p.to_string_lossy().into_owned());
    }
    Err(concat!(
        "未找到 reference 可执行文件\n",
        "安装方式:\n",
        "  go install github.com/cicbyte/reference@latest\n",
        "  或从内部 release 下载 reference.exe 并加入 PATH"
    )
    .into())
}

/// PATH 查找（secret.rs look_path 同款：直接命中 + PATHEXT 扩展探测）。
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

/// Windows 回退探测（对齐 Go probeWindowsDefaultBin）：GUI 启动常不带完整
/// shell 环境，`go install` 默认输出的 ~/go/bin 可能不在 PATH 上。
#[cfg(windows)]
fn probe_windows_default_bin() -> Option<PathBuf> {
    let home = std::env::var_os("USERPROFILE").map(PathBuf::from)?;
    let gopath = std::env::var_os("GOPATH")
        .map(PathBuf::from)
        .map(|p| if p.is_absolute() { p } else { home.join(p) });
    // Go 对未设置 GOPATH 的情况会得到 home\bin\reference.exe，一并镜像
    let candidates = [
        Some(home.join("go").join("bin").join("reference.exe")),
        Some(gopath.unwrap_or_else(|| home.clone()).join("bin").join("reference.exe")),
    ];
    candidates.into_iter().flatten().find(|p| p.is_file())
}

/// 运行 `<exe> version`，返回组合输出的首行（对齐 Go reference.Version：
/// CLI 打印两行，首行整体保留展示；"reference dev"）。
fn reference_version(exe: &str) -> Result<String, String> {
    let mut cmd = Command::new(exe);
    cmd.arg("version");
    hide_console(&mut cmd);
    let out = cmd.output().map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err(format!("exit status {:?}", out.status.code()));
    }
    let mut combined = String::new();
    combined.push_str(&String::from_utf8_lossy(&out.stdout));
    combined.push_str(&String::from_utf8_lossy(&out.stderr));
    Ok(combined.trim().lines().next().unwrap_or("").trim().to_string())
}

// ---------------------------------------------------------------------------
// 子进程执行（对齐 Go internal/reference 的 runRaw / runJSON / runJSONLoose；
// gitops.rs git_run 同款：隐藏窗口 + 超时 + stdout/stderr 分离捕获 + 独立线程
// 读管道防写满阻塞）
// ---------------------------------------------------------------------------

/// reference 子进程执行结果。
struct RefProc {
    /// 退出码是否为 0。
    success: bool,
    /// 是否因超时被强制终止。
    timed_out: bool,
    /// 是否被前端 CancelTask 取消。
    cancelled: bool,
    /// 进程退出码（被杀死时为 None）。
    code: Option<i32>,
    stdout: String,
    stderr: String,
}

impl RefProc {
    fn cancelled() -> Self {
        Self { success: false, timed_out: false, cancelled: true, code: None, stdout: String::new(), stderr: String::new() }
    }
    fn timeout() -> Self {
        Self { success: false, timed_out: true, cancelled: false, code: None, stdout: String::new(), stderr: String::new() }
    }
    fn failed(msg: String) -> Self {
        Self { success: false, timed_out: false, cancelled: false, code: None, stdout: String::new(), stderr: msg }
    }
}

/// 执行一条 reference 命令（cwd=dir）。cancellable=true 时注册到全局任务表
/// （对应 Go TaskContext：前端 CancelTask 置标志并 kill 子进程）。
/// 启动失败返回 Err（对应 Go 的非 ExitError 分支）。
fn ref_exec(exe: &str, dir: &Path, args: &[String], timeout: Duration, cancellable: bool) -> Result<RefProc, String> {
    fn drain<R: std::io::Read>(mut r: R) -> Vec<u8> {
        let mut buf = Vec::new();
        let _ = std::io::Read::read_to_end(&mut r, &mut buf);
        buf
    }

    let mut cmd = Command::new(exe);
    cmd.args(args)
        .current_dir(dir)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    hide_console(&mut cmd);

    let mut child = cmd.spawn().map_err(|e| format!("执行 reference 失败: {e}"))?;

    // 独立线程持续读管道，避免大输出写满管道导致子进程阻塞
    let out_handle = child.stdout.take().map(|p| std::thread::spawn(move || drain(p)));
    let err_handle = child.stderr.take().map(|p| std::thread::spawn(move || drain(p)));

    // 可取消任务：注册 kill 回调（tasks.rs 单槽注册表；本域仅 update 接入，
    // 避免短命令顶替其它长任务的注册槽位）
    let child = Arc::new(Mutex::new(child));
    let registered = if cancellable {
        let pid = child.lock().unwrap().id();
        let killer = child.clone();
        crate::tasks::reset();
        crate::tasks::register(pid, Box::new(move || {
            if let Ok(mut c) = killer.lock() {
                let _ = c.kill();
            }
        }));
        true
    } else {
        false
    };

    let wait = move || -> RefProc {
        let deadline = Instant::now() + timeout;
        loop {
            // 取消判定优先：CancelTask 已置标志（并可能已 kill 子进程）
            if cancellable && crate::tasks::is_cancelled() {
                if let Ok(mut c) = child.lock() {
                    let _ = c.kill();
                    let _ = c.wait();
                }
                return RefProc::cancelled();
            }
            match child.lock().unwrap().try_wait() {
                Ok(Some(status)) => {
                    let stdout = out_handle.and_then(|h| h.join().ok()).unwrap_or_default();
                    let stderr = err_handle.and_then(|h| h.join().ok()).unwrap_or_default();
                    return RefProc {
                        success: status.success(),
                        timed_out: false,
                        cancelled: false,
                        code: status.code(),
                        stdout: String::from_utf8_lossy(&stdout).into_owned(),
                        stderr: String::from_utf8_lossy(&stderr).into_owned(),
                    };
                }
                Ok(None) => {
                    if Instant::now() >= deadline {
                        if let Ok(mut c) = child.lock() {
                            let _ = c.kill();
                            let _ = c.wait();
                        }
                        return RefProc::timeout();
                    }
                    std::thread::sleep(Duration::from_millis(15));
                }
                Err(e) => return RefProc::failed(format!("等待 reference 进程失败: {e}")),
            }
        }
    };
    let result = wait();
    if registered {
        crate::tasks::release();
    }
    Ok(result)
}

/// 运行并取 trimmed stdout（对齐 Go runRawCtx 的错误映射）：
/// 超时 → "reference 执行超时"；取消 → "reference 操作已取消"；
/// 非零退出 → "reference 失败 (exit N): stderr"。
fn run_raw(exe: &str, dir: &Path, args: &[String], timeout: Duration, cancellable: bool) -> Result<String, String> {
    let p = ref_exec(exe, dir, args, timeout, cancellable)?;
    if p.cancelled {
        return Err("reference 操作已取消".into());
    }
    if p.timed_out {
        return Err("reference 执行超时".into());
    }
    if !p.success {
        if let Some(code) = p.code {
            return Err(format!("reference 失败 (exit {code}): {}", p.stderr.trim()));
        }
        return Err(format!("执行 reference 失败: {}", p.stderr.trim()));
    }
    Ok(p.stdout.trim().to_string())
}

/// 运行并把 stdout 解析为 JSON（对齐 Go runJSONArgs 的容错）：
/// 空输出 / 非 `{`/`[` 开头的人类可读提示（如"当前项目暂无引用仓库…"）
/// → 零值；解析失败报错并附 300 字节原文。
fn run_json<T: DeserializeOwned + Default>(
    exe: &str,
    dir: &Path,
    args: &[String],
    timeout: Duration,
) -> Result<T, String> {
    let stdout = run_raw(exe, dir, args, timeout, false)?;
    parse_reference_json(&stdout)
}

/// 运行并解析 JSON，忽略退出码（对齐 Go runJSONArgsLoose：`doctor` 对告警
/// 返回非零但仍输出有效 JSON）。stdout 全空才视为错误（附 stderr）。
fn run_json_loose<T: DeserializeOwned + Default>(
    exe: &str,
    dir: &Path,
    args: &[String],
    timeout: Duration,
) -> Result<T, String> {
    // Go：`_ = cmd.Run()` —— 启动/退出码失败都不作为错误，只看 stdout
    let p = match ref_exec(exe, dir, args, timeout, false) {
        Ok(p) => p,
        Err(e) => return Err(format!("reference 未返回任何输出: {e}")),
    };
    let trimmed = p.stdout.trim().to_string();
    if trimmed.is_empty() {
        return Err(format!("reference 未返回任何输出: {}", p.stderr.trim()));
    }
    parse_reference_json(&trimmed)
}

/// Go runJSONArgs 同款 JSON 容错解析。
fn parse_reference_json<T: DeserializeOwned + Default>(stdout: &str) -> Result<T, String> {
    let trimmed = stdout.trim();
    if trimmed.is_empty() {
        return Ok(T::default());
    }
    let first = trimmed.as_bytes()[0];
    if first != b'{' && first != b'[' {
        return Ok(T::default());
    }
    serde_json::from_str(trimmed)
        .map_err(|e| format!("解析 reference 输出失败: {e} (原始: {})", truncate_str(trimmed, 300)))
}

/// Go truncate(s, n) 等价（字节截断 + "..."，按 UTF-8 字符边界回退）。
fn truncate_str(s: &str, n: usize) -> String {
    if s.len() <= n {
        return s.to_string();
    }
    let mut end = n;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}...", &s[..end])
}

// ---------------------------------------------------------------------------
// 私有辅助
// ---------------------------------------------------------------------------

/// &[&str] → Vec<String>（命令参数构造）。
fn str_args(items: &[&str]) -> Vec<String> {
    items.iter().map(|s| s.to_string()).collect()
}

/// 按名查找当前项目的一条引用（对齐 Go findRefRepo：先 repo list 再匹配，
/// 缺失时给出明确的"未找到引用"而非下游路径解析失败）。
fn find_ref_repo(exe: &str, dir: &Path, name: &str) -> Result<Repo, String> {
    let list = run_json::<RepoList>(exe, dir, &str_args(&["repo", "list", "-f", "json"]), SHORT_TIMEOUT)?;
    for r in &list.repos {
        if r.name == name {
            return Ok(r.clone());
        }
    }
    Err(format!("未找到引用: {name}"))
}

/// 引用的磁盘根（对齐 Go refRootByType）：
/// remote → cache_path（全局缓存克隆）；local → source（用户原仓库）。
fn ref_root_by_type(r: &Repo) -> String {
    if r.ty == "local" {
        r.source.clone()
    } else {
        r.cache_path.clone()
    }
}

/// 相对路径是否越出根（repo.rs 同款，等价 Go Clean+HasPrefix 守卫的意图）：
/// rel 内的 ".." 回退不能低于根；rel 不能携带盘符/根（会替换掉根）。
fn rel_escapes(rel: &str) -> bool {
    use std::path::Component;
    for c in Path::new(rel).components() {
        match c {
            Component::Prefix(_) | Component::RootDir => return true,
            _ => {}
        }
    }
    let mut depth: isize = 0;
    for c in Path::new(rel).components() {
        match c {
            Component::Normal(_) => depth += 1,
            Component::ParentDir => {
                depth -= 1;
                if depth < 0 {
                    return true;
                }
            }
            _ => {}
        }
    }
    false
}

/// 从 URL/路径猜链接名（对齐 Go deriveName）：
/// "https://github.com/owner/repo" → "repo"；"/path/to/foo" → "foo"。
fn derive_name(target: &str) -> String {
    let mut t = target.trim().to_string();
    if t.ends_with('/') {
        t.pop();
    }
    if let Some(stripped) = t.strip_suffix(".git") {
        t = stripped.to_string();
    }
    if let Some(i) = t.rfind('/') {
        t = t[i + 1..].to_string();
    }
    t
}

/// 递归走目录树搜文件名（repo.rs search_walk 同款，对齐 Go WalkDir：
/// 词典序遍历、错误忽略、剪枝 .git/node_modules/.DS_Store、封顶 500 即停、
/// 路径归一为 / 分隔）。
fn ref_search_walk(dir: &Path, q: &str, root: &Path, out: &mut Vec<FileNode>) -> bool {
    const MAX_RESULTS: usize = 500;

    fn walk(dir: &Path, q: &str, root: &Path, out: &mut Vec<FileNode>) -> bool {
        let Ok(rd) = std::fs::read_dir(dir) else { return false };
        let mut entries: Vec<_> = rd.flatten().collect();
        entries.sort_by_key(|e| e.file_name());
        for e in entries {
            if out.len() >= MAX_RESULTS {
                return true;
            }
            let name = e.file_name().to_string_lossy().into_owned();
            let is_dir = e.file_type().map(|t| t.is_dir()).unwrap_or(false);
            if is_dir {
                if name == ".git" || name == "node_modules" || name == ".DS_Store" {
                    continue;
                }
                if walk(&e.path(), q, root, out) {
                    return true;
                }
            } else {
                if !name.to_lowercase().contains(q) {
                    continue;
                }
                let rel = e
                    .path()
                    .strip_prefix(root)
                    .map(|p| p.to_string_lossy().replace('\\', "/"))
                    .unwrap_or_default();
                let size = e.metadata().map(|m| m.len()).unwrap_or(0) as i64;
                out.push(FileNode { name, path: rel, is_dir: false, size });
            }
        }
        false
    }

    walk(dir, q, root, out)
}

// ---------------------------------------------------------------------------
// 命令（Go ReferenceModule 绑定，23 个）
// ---------------------------------------------------------------------------

/// ReferenceCheck 检测 reference CLI 是否就绪（可用性结果进程内缓存，
/// 对应 Go refCheckCache；Go 由设置保存时的 ReferenceClearCache 失效——
/// 该方法不在本域清单且 Rust settings_save 无清缓存钩子，缓存存活到进程退出）。
/// 应用级探测，不使用项目目录；repo 仅为适配层统一注入。
#[tauri::command]
pub fn reference_check(repo: Option<String>) -> Result<ReferenceCheckResult, String> {
    let _ = repo;
    let cell = reference_check_cache();
    let mut guard = cell.lock().unwrap();
    if let Some(cached) = guard.clone() {
        return Ok(cached);
    }
    let res = match find_reference_exe() {
        Err(e) => ReferenceCheckResult { message: e, ..Default::default() },
        Ok(exe_path) => match reference_version(&exe_path) {
            Err(e) => ReferenceCheckResult {
                exe_path,
                message: format!("无法获取版本: {e}"),
                ..Default::default()
            },
            Ok(version) => ReferenceCheckResult {
                available: true,
                exe_path,
                version,
                message: String::new(),
            },
        },
    };
    *guard = Some(res.clone());
    Ok(res)
}

/// ReferenceInit 在当前项目运行 `reference init [--agent X]`。
/// 幂等：重复执行安全，（重）建 .reference/、settings、map、gitignore。
#[tauri::command]
pub fn reference_init(req: ReferenceInitRequest, repo: Option<String>) -> Result<(), String> {
    let dir = repo_path(repo);
    let exe = find_reference_exe()?;
    let mut args: Vec<String> = vec!["init".into()];
    if !req.agent.is_empty() {
        args.push("--agent".into());
        args.push(req.agent.clone());
    }
    run_raw(&exe, &dir, &args, INIT_TIMEOUT, false).map(|_| ())
}

/// ReferenceList 当前项目的全部引用（`repo list -f json`）。
#[tauri::command]
pub fn reference_list(repo: Option<String>) -> Result<RepoList, String> {
    let dir = repo_path(repo);
    let exe = find_reference_exe()?;
    run_json(&exe, &dir, &str_args(&["repo", "list", "-f", "json"]), SHORT_TIMEOUT)
}

/// ReferenceMap 直读 `.reference/reference.map.jsonl`（含 topics 索引的富
/// map）。未初始化返回空列表（非错误）。
#[tauri::command]
pub fn reference_map(repo: Option<String>) -> Result<Vec<MapEntry>, String> {
    let dir = repo_path(repo);
    let path = dir.join(".reference").join("reference.map.jsonl");
    let data = match std::fs::read_to_string(&path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e.to_string()),
        Ok(d) => d,
    };
    let mut entries = Vec::new();
    for line in data.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        // 对齐 Go：跳过格式错误的行，不中断
        if let Ok(e) = serde_json::from_str::<MapEntry>(line) {
            entries.push(e);
        }
    }
    Ok(entries)
}

/// ReferenceRemove 删除一条引用（`repo remove <name> --yes`）；
/// purge=true 时同时清除全局缓存（仅远程有效）。
#[tauri::command]
pub async fn reference_remove(name: String, purge: bool, repo: Option<String>) -> Result<(), String> {
    let dir = repo_path(repo);
    let exe = find_reference_exe()?;
    if name.is_empty() {
        return Err("name 不能为空".into());
    }
    run_blocking(move || {
        let mut args = str_args(&["repo", "remove", &name, "--yes"]);
        if purge {
            args.push("--purge".into());
        }
        run_raw(&exe, &dir, &args, REMOVE_TIMEOUT, false).map(|_| ())
    })?
}

/// ReferenceUpdate 对缓存的远程引用执行 git pull（`repo update [name]`，
/// 空 name = 更新全部远程引用）。10 分钟超时；经 tasks.rs 注册可被前端
/// CancelTask 取消（对应 Go TaskContext 的取消链）。
#[tauri::command]
pub async fn reference_update(name: String, repo: Option<String>) -> Result<(), String> {
    let dir = repo_path(repo);
    let exe = find_reference_exe()?;
    run_blocking(move || {
        let mut args = str_args(&["repo", "update"]);
        if !name.is_empty() {
            args.push(name.clone());
        }
        run_raw(&exe, &dir, &args, UPDATE_TIMEOUT, true).map(|_| ())
    })?
}

/// ReferenceAdd 添加远程/本地引用（`repo add`，大仓库可达数十秒以上）。
/// add 默认不返回 JSON：成功后重新 `repo list` 匹配记录回填完整字段
/// （list 失败/匹配不到均为 best-effort，返回零值 Repo，不报错——对齐 Go）。
#[tauri::command]
pub async fn reference_add(req: ReferenceAddRequest, repo: Option<String>) -> Result<Repo, String> {
    let dir = repo_path(repo);
    let exe = find_reference_exe()?;
    run_blocking(move || {
        if req.target.is_empty() {
            return Err("target 不能为空".into());
        }
        let mut args: Vec<String> = vec!["repo".into(), "add".into()];
        if req.local {
            args.push("--local".into());
        }
        if !req.branch.is_empty() {
            args.push("-b".into());
            args.push(req.branch.clone());
        }
        if !req.name.is_empty() {
            args.push("-n".into());
            args.push(req.name.clone());
        }
        if req.update {
            args.push("-u".into());
        }
        args.push(req.target.clone());
        run_raw(&exe, &dir, &args, ADD_TIMEOUT, false)?;

        // 回查列表匹配新增记录
        let Ok(list) = run_json::<RepoList>(&exe, &dir, &str_args(&["repo", "list", "-f", "json"]), SHORT_TIMEOUT)
        else {
            return Ok(Repo::default());
        };
        let target = if req.name.trim().is_empty() {
            derive_name(&req.target)
        } else {
            req.name.trim().to_string()
        };
        for r in &list.repos {
            if r.name == target {
                return Ok(r.clone());
            }
        }
        Ok(Repo::default())
    })?
}

/// ReferenceDoctor 健康检查（symlink/junction/db 一致性等，`doctor -f json`）。
/// doctor 对告警返回非零退出码但 JSON 有效——忽略退出码只解析输出。
#[tauri::command]
pub fn reference_doctor(repo: Option<String>) -> Result<DoctorResult, String> {
    let dir = repo_path(repo);
    let exe = find_reference_exe()?;
    run_json_loose(&exe, &dir, &str_args(&["doctor", "-f", "json"]), SHORT_TIMEOUT)
}

/// ReferenceSettings 直读 `.reference/reference.settings.json`（init UI 展示
/// 当前初始化状态）。文件缺失返回 hasFile=false（非错误）；文件存在但解析
/// 失败返回 hasFile=true 的零值；兼容 agents 数组与旧版 agent 单串两种
/// schema，归一化进 agents。
#[tauri::command]
pub fn reference_settings(repo: Option<String>) -> Result<ReferenceSettingsDto, String> {
    let dir = repo_path(repo);
    let path = dir.join(".reference").join("reference.settings.json");
    let data = match std::fs::read_to_string(&path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Ok(ReferenceSettingsDto { has_file: false, ..Default::default() });
        }
        Err(e) => return Err(format!("读取 settings 失败: {e}")),
        Ok(d) => d,
    };
    // 宽松解析：容忍两种 schema，不做严格结构校验
    let Ok(raw) = serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(&data) else {
        return Ok(ReferenceSettingsDto { has_file: true, ..Default::default() });
    };
    let mut out = ReferenceSettingsDto { has_file: true, ..Default::default() };
    if let Some(v) = raw.get("initialized").and_then(|v| v.as_bool()) {
        out.initialized = v;
    }
    // 优先 "agents"（数组），回退旧版 "agent"（单串）
    if let Some(arr) = raw.get("agents").and_then(|v| v.as_array()) {
        for item in arr {
            if let Some(s) = item.as_str() {
                if !s.is_empty() {
                    out.agents.push(s.to_string());
                }
            }
        }
    } else if let Some(s) = raw.get("agent").and_then(|v| v.as_str()) {
        if !s.is_empty() {
            out.agents = vec![s.to_string()];
        }
    }
    Ok(out)
}

/// ReferenceScc 单个引用的代码统计（`repo scc <name> -f json -n 15`，
/// topN 固定 15 对齐 Go）。大仓库统计较慢，放后台线程。
#[tauri::command]
pub async fn reference_scc(name: String, repo: Option<String>) -> Result<SccResult, String> {
    let dir = repo_path(repo);
    let exe = find_reference_exe()?;
    if name.is_empty() {
        return Err("name 不能为空".into());
    }
    run_blocking(move || {
        let args = str_args(&["repo", "scc", &name, "-f", "json", "-n", "15"]);
        run_json(&exe, &dir, &args, SCC_TIMEOUT)
    })?
}

/// ReferenceCachePath 引用的磁盘根绝对路径（filePath 非空时为 root/filePath，
/// 供"复制路径/在资源管理器中显示"）。remote → cache_path，local → source。
#[tauri::command]
pub fn reference_cache_path(name: String, file_path: String, repo: Option<String>) -> Result<String, String> {
    let dir = repo_path(repo);
    let exe = find_reference_exe()?;
    let r = find_ref_repo(&exe, &dir, &name)?;
    let root = ref_root_by_type(&r);
    if root.is_empty() {
        return Err(format!("引用 {name} 的缓存路径为空"));
    }
    if file_path.is_empty() {
        return Ok(root);
    }
    // 防 ../ 越界：清理后的完整路径必须仍在根下（Go Clean+HasPrefix 守卫）
    if rel_escapes(&file_path) {
        return Err("路径越界".into());
    }
    // Go filepath.FromSlash：前端路径约定 / 分隔，转本机分隔符后拼接
    let native = PathBuf::from(file_path.replace('/', std::path::MAIN_SEPARATOR_STR));
    Ok(Path::new(&root).join(&native).display().to_string())
}

/// ReferenceFileList 列出引用缓存内某目录的直接子项（subPath 空串 = 根）。
/// 目录懒加载（前端点击展开）；根级跳过 .git，任意层级跳过
/// node_modules/.DS_Store；目录在前、名称不区分大小写排序。
#[tauri::command]
pub fn reference_file_list(name: String, sub_path: String, repo: Option<String>) -> Result<Vec<FileNode>, String> {
    let dir = repo_path(repo);
    let exe = find_reference_exe()?;
    let r = find_ref_repo(&exe, &dir, &name)?;
    let root = ref_root_by_type(&r);
    if root.is_empty() {
        return Err(format!("引用 {name} 的缓存路径为空"));
    }
    if rel_escapes(&sub_path) {
        return Err("路径越界".into());
    }
    let root = PathBuf::from(&root);
    let target = if sub_path.is_empty() { root.clone() } else { root.join(&sub_path) };

    let entries = match std::fs::read_dir(&target) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(format!("读取目录失败: {e}")),
        Ok(rd) => rd,
    };

    let mut nodes: Vec<FileNode> = Vec::new();
    for e in entries.flatten() {
        let nm = e.file_name().to_string_lossy().into_owned();
        // VCS 内部 + 常见噪声（对齐 Go：仅根级跳 .git）
        if sub_path.is_empty() && nm == ".git" {
            continue;
        }
        if nm == "node_modules" || nm == ".DS_Store" {
            continue;
        }
        let is_dir = e.file_type().map(|t| t.is_dir()).unwrap_or(false);
        let size = e.metadata().map(|m| m.len()).unwrap_or(0) as i64;
        let rel = if sub_path.is_empty() { nm.clone() } else { format!("{sub_path}/{nm}") };
        nodes.push(FileNode { name: nm, path: rel, is_dir, size });
    }
    // VSCode 风格：目录在前，名称小写序
    nodes.sort_by(|a, b| match (a.is_dir, b.is_dir) {
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
    });
    Ok(nodes)
}

/// ReferenceFileRead 读取引用缓存内文件（二进制检测 + 1MB 截断，
/// FullBackupFileRead/SecretFileContent 同款；目录/缺失统一 NotFound）。
#[tauri::command]
pub fn reference_file_read(name: String, file_path: String, repo: Option<String>) -> Result<FileResult, String> {
    let dir = repo_path(repo);
    let exe = find_reference_exe()?;
    let r = find_ref_repo(&exe, &dir, &name)?;
    let root = ref_root_by_type(&r);
    if root.is_empty() {
        return Err(format!("引用 {name} 的缓存路径为空"));
    }
    if rel_escapes(&file_path) {
        return Err("路径越界".into());
    }
    let full = Path::new(&root).join(&file_path);
    // 目录显式按 NotFound 返回（对齐 Go：Unix 下读目录会"成功"但无意义）
    if full.is_dir() {
        return Ok(FileResult { not_found: true, ..Default::default() });
    }
    let data = match std::fs::read(&full) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Ok(FileResult { not_found: true, ..Default::default() });
        }
        Err(e) => return Err(format!("读取文件失败: {e}")),
        Ok(d) => d,
    };
    // 二进制检测：前 8KB 出现 NUL 即视为二进制
    let scan = &data[..data.len().min(8192)];
    if scan.contains(&0) {
        return Ok(FileResult { binary: true, ..Default::default() });
    }
    let mut content = String::from_utf8_lossy(&data).into_owned();
    let mut truncated = false;
    if content.len() > MAX_FILE_SIZE {
        let mut end = MAX_FILE_SIZE;
        while end > 0 && !content.is_char_boundary(end) {
            end -= 1;
        }
        content.truncate(end);
        truncated = true;
    }
    // 对齐 Go：行数在追加截断提示前统计（提示行不计入）
    let lines = content.matches('\n').count() as i64 + 1;
    if truncated {
        content.push_str("\n\n…（文件超过 1MB，已截断显示）");
    }
    Ok(FileResult { content, lines, ..Default::default() })
}

/// ReferenceFileSearch 递归搜索引用缓存内文件名包含 query（不区分大小写）
/// 的文件，上限 500 条；.git/node_modules/.DS_Store 全层级剪枝；
/// 路径归一为 / 分隔。大目录走查放后台线程。
#[tauri::command]
pub async fn reference_file_search(name: String, query: String, repo: Option<String>) -> Result<Vec<FileNode>, String> {
    let dir = repo_path(repo);
    let exe = find_reference_exe()?;
    run_blocking(move || {
        let r = find_ref_repo(&exe, &dir, &name)?;
        let root = ref_root_by_type(&r);
        if root.is_empty() {
            return Err(format!("引用 {name} 的缓存路径为空"));
        }
        let q = query.trim().to_lowercase();
        if q.is_empty() {
            return Ok(Vec::new());
        }
        let root = PathBuf::from(&root);
        let mut matches: Vec<FileNode> = Vec::new();
        ref_search_walk(&root, &q, &root, &mut matches);
        // 路径小写序，展示稳定
        matches.sort_by(|a, b| a.path.to_lowercase().cmp(&b.path.to_lowercase()));
        Ok(matches)
    })?
}

/// ReferenceWikiList 枚举引用 wiki 目录下的 .md 文件（map 失步时的兜底）。
/// 返回文件名（basename）。偏差：Go 注释称按修改时间排序但实现未排序
/// （os.ReadDir 实为字典序），此处显式字典序对齐 Go 实际行为。
#[tauri::command]
pub fn reference_wiki_list(repo_name: String, repo: Option<String>) -> Result<Vec<String>, String> {
    if repo_name.contains('/') || repo_name.contains('\\') {
        return Err(format!("非法的仓库名: {repo_name}"));
    }
    let dir = repo_path(repo).join(".reference").join("wiki").join(&repo_name);
    let entries = match std::fs::read_dir(&dir) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e.to_string()),
        Ok(rd) => rd,
    };
    let mut out: Vec<String> = entries
        .flatten()
        .filter(|e| !e.file_type().map(|t| t.is_dir()).unwrap_or(false))
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.ends_with(".md"))
        .collect();
    out.sort();
    Ok(out)
}

/// ReferenceWikiFile 读单个 wiki markdown 文件（file 为 basename，服务端
/// 守卫禁止路径分隔符与 ".."）；超过 1MB 追加截断提示。
#[tauri::command]
pub fn reference_wiki_file(repo_name: String, file: String, repo: Option<String>) -> Result<String, String> {
    if file.contains('/') || file.contains('\\') || file == ".." || file.contains("..") {
        return Err(format!("非法的 wiki 文件名: {file}"));
    }
    if repo_name.contains('/') || repo_name.contains('\\') || repo_name == ".." {
        return Err(format!("非法的仓库名: {repo_name}"));
    }
    let path = repo_path(repo).join(".reference").join("wiki").join(&repo_name).join(&file);
    let data = std::fs::read(&path).map_err(|e| e.to_string())?;
    if data.len() > MAX_FILE_SIZE {
        let mut capped = data[..MAX_FILE_SIZE].to_vec();
        capped.extend_from_slice("\n\n…（文件过大，已截断显示）".as_bytes());
        return Ok(String::from_utf8_lossy(&capped).into_owned());
    }
    Ok(String::from_utf8_lossy(&data).into_owned())
}

/// ReferenceWikiSync 知识库同步（`wiki sync` = pull + 自动提交 + push），
/// 返回原始文本输出（CLI 无 JSON 模式）。含网络操作，放后台线程。
#[tauri::command]
pub async fn reference_wiki_sync(repo: Option<String>) -> Result<String, String> {
    let dir = repo_path(repo);
    let exe = find_reference_exe()?;
    run_blocking(move || run_raw(&exe, &dir, &str_args(&["wiki", "sync"]), WIKI_SYNC_TIMEOUT, false))?
}

/// ReferenceWikiStatus 知识库状态（`reference wiki` 文本输出解析为
/// init/remote/工作区 三态；rawOutput 保留原文供前端兜底展示）。
#[tauri::command]
pub fn reference_wiki_status(repo: Option<String>) -> Result<WikiStatus, String> {
    let dir = repo_path(repo);
    let exe = find_reference_exe()?;
    let out = run_raw(&exe, &dir, &str_args(&["wiki"]), WIKI_SHORT_TIMEOUT, false)?;
    let mut s = WikiStatus { raw_output: out.clone(), ..Default::default() };
    for line in out.lines() {
        let line = line.trim();
        if let Some(v) = line.strip_prefix("wiki 仓库:") {
            let v = v.trim();
            s.initialized = v.contains("已初始化") || v.contains("initialized");
        } else if let Some(v) = line.strip_prefix("远程仓库:") {
            let v = v.trim();
            if !v.contains("未设置") && !v.contains("not set") {
                s.remote = v.to_string();
            }
        } else if let Some(v) = line.strip_prefix("工作区:") {
            let v = v.trim();
            // 除"干净/clean"之外的措辞均视为脏工作区
            s.clean = v.contains("干净")
                || v.contains("clean")
                || v.contains("无更改")
                || v.contains("up to date");
        }
    }
    Ok(s)
}

/// ReferenceWikiRemoteGet 读知识库远程 URL（`wiki remote` 文本解析；
/// 未设置返回空串）。
#[tauri::command]
pub fn reference_wiki_remote_get(repo: Option<String>) -> Result<String, String> {
    let dir = repo_path(repo);
    let exe = find_reference_exe()?;
    let out = run_raw(&exe, &dir, &str_args(&["wiki", "remote"]), WIKI_SHORT_TIMEOUT, false)?;
    // 输出为单行 "  远程仓库: <url>" 或 "(未设置)"
    for line in out.lines() {
        let line = line.trim();
        if let Some(v) = line.strip_prefix("远程仓库:") {
            let v = v.trim();
            if !v.contains("未设置") {
                return Ok(v.to_string());
            }
        }
    }
    Ok(String::new())
}

/// ReferenceWikiRemoteSet 设置知识库远程 URL（`wiki remote <url>`）。
#[tauri::command]
pub fn reference_wiki_remote_set(url: String, repo: Option<String>) -> Result<(), String> {
    let dir = repo_path(repo);
    let exe = find_reference_exe()?;
    if url.is_empty() {
        return Err("remote URL 不能为空".into());
    }
    run_raw(&exe, &dir, &str_args(&["wiki", "remote", &url]), WIKI_SET_TIMEOUT, false).map(|_| ())
}

/// ReferenceWikiRestore 从 Git 历史恢复一个被软删除的知识文件
/// （`wiki restore <path>`；path 须来自 WikiTrash 的条目）。
#[tauri::command]
pub fn reference_wiki_restore(path: String, repo: Option<String>) -> Result<(), String> {
    let dir = repo_path(repo);
    let exe = find_reference_exe()?;
    if path.is_empty() {
        return Err("path 不能为空".into());
    }
    run_raw(&exe, &dir, &str_args(&["wiki", "restore", &path]), WIKI_TRASH_TIMEOUT, false).map(|_| ())
}

/// ReferenceWikiTrash 列出被软删除的知识文件（`wiki trash -f json -n 50`，
/// limit 固定 50 对齐 Go）。
#[tauri::command]
pub fn reference_wiki_trash(repo: Option<String>) -> Result<Vec<WikiTrashEntry>, String> {
    let dir = repo_path(repo);
    let exe = find_reference_exe()?;
    let stdout = run_raw(&exe, &dir, &str_args(&["wiki", "trash", "-f", "json", "-n", "50"]), WIKI_TRASH_TIMEOUT, false)?;
    let trimmed = stdout.trim();
    if trimmed.is_empty() || trimmed == "[]" {
        return Ok(Vec::new());
    }
    serde_json::from_str(trimmed).map_err(|e| format!("解析 wiki trash 输出失败: {e}"))
}

/// ReferenceWikiCommit 提交知识库本地待提交更改（不 push，`wiki commit`），
/// 返回原始文本输出。放后台线程。
#[tauri::command]
pub async fn reference_wiki_commit(repo: Option<String>) -> Result<String, String> {
    let dir = repo_path(repo);
    let exe = find_reference_exe()?;
    run_blocking(move || run_raw(&exe, &dir, &str_args(&["wiki", "commit"]), WIKI_COMMIT_TIMEOUT, false))?
}
