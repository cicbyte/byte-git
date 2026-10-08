//! repo 域命令：当前仓库状态与最近仓库列表（切换/移除/读取）、
//! 仓库文件浏览器（列目录/搜索/读文件/读图片/README）、系统打开。
//!
//! 移植自 git-mate `gui/internal/repo`（repo.go + browser.go + repos.go）。
//! 当前仓库状态与最近列表走 crate::state（recent-repos.json 持久化），
//! 对应 Go 的 Core.CurrentRepo 与 repos.json。
//!
//! 已知偏差（详见各命令注释）：
//! - PickRepoFolder：tauri-plugin-dialog 原生目录选择框（取消返回空串）；
//! - OpenInExplorer：按约定用 `explorer <path>` 直接打开（Go 为 /select 定位）；
//! - 最近仓库不持久化时间戳（state 只存路径），lastOpened 恒为空串。

use serde::Serialize;
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// git 子进程统一超时（对齐 Go 侧无超时的宽松上限）。
const GIT_TIMEOUT: Duration = Duration::from_secs(60);
/// 前端文件读取上限（对齐 Go MaxSecretFileSize = 1MB）。
const MAX_SECRET_FILE_SIZE: usize = 1 << 20;
/// 前端图片读取上限（对齐 Go MaxImageSize = 5MB）。
const MAX_IMAGE_SIZE: usize = 5 << 20;

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
    success: bool,
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
                    stdout: String::new(),
                    stderr: format!("等待 git 进程失败: {e}"),
                }
            }
        }
    }
}

/// 在独立线程执行阻塞任务（commit.rs 同款：大目录树的递归搜索不冻结窗口）。
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

/// RecentRepo（gui repo/repos.go）。state 只持久化路径，lastOpened 恒为空串。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecentRepo {
    pub path: String,
    pub name: String,
    /// RFC3339 时间戳；偏差：state 未持久化时间戳，恒为 ""。
    pub last_opened: String,
}

/// SwitchResult（SwitchRepo 返回：解析后的绝对 toplevel + 展示信息 + 最近列表）。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SwitchResult {
    pub path: String,
    pub name: String,
    pub branch: String,
    pub recent: Vec<RecentRepo>,
}

/// RepoInfo（页脚展示的当前仓库信息）。
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepoInfo {
    pub path: String,
    pub name: String,
    pub branch: String,
}

/// FileNode（文件树节点，repo/reference/backup 浏览器共用）。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileNode {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub size: i64,
}

/// FileResult（文件内容读取结果：内容/二进制/未找到）。
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileResult {
    pub content: String,
    pub lines: i64,
    pub binary: bool,
    pub not_found: bool,
}

/// ImageResult（本地图片内联为 base64 data URL；标志位说明未内联的原因）。
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageResult {
    pub data_url: String,
    pub not_found: bool,
    pub too_large: bool,
    pub unsupported: bool,
}

// ---------------------------------------------------------------------------
// 私有辅助
// ---------------------------------------------------------------------------

/// 相对路径绝对化（对齐 Go filepath.Abs：相对路径拼进程 CWD，不解析符号链接）。
fn abs_path(p: &Path) -> std::io::Result<PathBuf> {
    if p.is_absolute() {
        return Ok(p.to_path_buf());
    }
    let cwd = std::env::current_dir()?;
    Ok(cwd.join(p))
}

/// 路径最后一段（对齐 repoBaseName：Clean 后取 Base，取不到回退原路径）。
fn repo_base_name(path: &str) -> String {
    match Path::new(path).file_name() {
        Some(n) if !n.is_empty() => n.to_string_lossy().into_owned(),
        _ => path.to_string(),
    }
}

/// 校验目录是否为 git 工作树，返回绝对 toplevel（对齐 resolveGitRepo）。
fn resolve_git_repo(dir: &str) -> Result<String, String> {
    let abs = abs_path(Path::new(dir)).map_err(|e| format!("无法解析路径: {e}"))?;
    let res = git_exec(&abs, &["rev-parse", "--show-toplevel"], GIT_TIMEOUT);
    if !res.success {
        return Err(format!("不是 git 仓库: {}", abs.display()));
    }
    Ok(res.stdout.trim().to_string())
}

/// 当前分支名（对齐 currentBranch：symbolic-ref 失败回退短哈希，再失败空串）。
fn current_branch(repo: &str) -> String {
    let p = Path::new(repo);
    let sym = git_exec(p, &["symbolic-ref", "--short", "HEAD"], GIT_TIMEOUT);
    if sym.success {
        let s = sym.stdout.trim();
        if !s.is_empty() {
            return s.to_string();
        }
    }
    // 分离 HEAD → 短哈希
    let rev = git_exec(p, &["rev-parse", "--short", "HEAD"], GIT_TIMEOUT);
    if rev.success {
        let s = rev.stdout.trim();
        if !s.is_empty() {
            return s.to_string();
        }
    }
    String::new()
}

/// 解析"当前仓库"（对齐 Go GetCurrentRepo 的优先级）：
/// 显式参数 > 已切换状态（state）> 进程 CWD 的 git toplevel（失败给 ""）。
/// Go 侧无显式参数；Rust 侧尾参用于命令级的仓库覆盖。
fn resolve_current(explicit: Option<&str>) -> String {
    if let Some(p) = explicit.filter(|s| !s.is_empty()) {
        return p.to_string();
    }
    let cur = crate::state::current_or_dot();
    if cur != "." {
        return cur;
    }
    // 回退：进程 CWD 不是仓库时返回 ""（对齐 Go）
    let res = git_exec(Path::new("."), &["rev-parse", "--show-toplevel"], GIT_TIMEOUT);
    if res.success {
        let s = res.stdout.trim();
        if !s.is_empty() {
            return s.to_string();
        }
    }
    String::new()
}

/// state 的路径列表 → 前端 RecentRepo DTO（name 取路径最后一段）。
fn load_recent_dtos() -> Vec<RecentRepo> {
    crate::state::load_recent()
        .unwrap_or_default()
        .into_iter()
        .map(|p| RecentRepo {
            name: repo_base_name(&p),
            path: p,
            last_opened: String::new(),
        })
        .collect()
}

/// 相对路径是否越出仓库根（对齐 Go Clean+HasPrefix 守卫的意图）：
/// rel 内的 ".." 回退不能低于根；rel 不能携带盘符/根（会替换掉根）。
fn rel_escapes(rel: &str) -> bool {
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

/// 图片扩展名 → MIME 类型（对齐 imageMIME；非图片返回 None → unsupported）。
fn image_mime(path: &str) -> Option<&'static str> {
    match ext_lower(path).as_str() {
        ".png" => Some("image/png"),
        ".jpg" | ".jpeg" => Some("image/jpeg"),
        ".gif" => Some("image/gif"),
        ".webp" => Some("image/webp"),
        ".bmp" => Some("image/bmp"),
        ".svg" => Some("image/svg+xml"),
        ".ico" => Some("image/x-icon"),
        _ => None,
    }
}

/// 小写扩展名（含点；无扩展名为空串）。
fn ext_lower(path: &str) -> String {
    Path::new(path)
        .extension()
        .map(|e| format!(".{}", e.to_string_lossy().to_lowercase()))
        .unwrap_or_default()
}

/// 标准 base64 编码（含 padding；无第三方 crate，对齐 Go base64.StdEncoding）。
fn base64_encode(data: &[u8]) -> String {
    const TBL: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = *chunk.get(1).unwrap_or(&0) as u32;
        let b2 = *chunk.get(2).unwrap_or(&0) as u32;
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(TBL[(n >> 18) as usize & 0x3f] as char);
        out.push(TBL[(n >> 12) as usize & 0x3f] as char);
        out.push(if chunk.len() > 1 {
            TBL[(n >> 6) as usize & 0x3f] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            TBL[n as usize & 0x3f] as char
        } else {
            '='
        });
    }
    out
}

/// RepoReadFile 的实现（RepoReadme 复用）。
fn read_repo_file(repo: &Path, rel_path: &str) -> Result<FileResult, String> {
    if rel_escapes(rel_path) {
        return Err("路径越界".into());
    }
    let full = repo.join(rel_path);
    // 目录统一按“非文件”处理（Go 注释声明的意图；其 Stat 检查在 ReadFile 之后）
    if full.is_dir() {
        return Ok(FileResult { not_found: true, ..Default::default() });
    }
    let data = match std::fs::read(&full) {
        Ok(d) => d,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Ok(FileResult { not_found: true, ..Default::default() })
        }
        Err(e) => return Err(format!("读取文件失败: {e}")),
    };
    // 二进制检测：前 8192 字节含 NUL 即视为二进制
    let scan = &data[..data.len().min(8192)];
    if scan.contains(&0) {
        return Ok(FileResult { binary: true, ..Default::default() });
    }
    let mut content = String::from_utf8_lossy(&data).into_owned();
    if content.len() > MAX_SECRET_FILE_SIZE {
        let mut end = MAX_SECRET_FILE_SIZE;
        while end > 0 && !content.is_char_boundary(end) {
            end -= 1;
        }
        content.truncate(end);
        content.push_str("\n\n…（文件超过 1MB，已截断显示）");
    }
    let lines = content.matches('\n').count() as i64 + 1;
    Ok(FileResult { content, lines, ..Default::default() })
}

/// README 候选文件名（对齐 readmeCandidates，按序尝试）。
const README_CANDIDATES: [&str; 7] = [
    "README.md", "README.MD", "README.rst", "README.markdown",
    "README.txt", "README", "readme.md",
];

// ---------------------------------------------------------------------------
// 命令（Go RepoModule 绑定）
// ---------------------------------------------------------------------------

/// GetCurrentRepo 当前仓库绝对路径（无显式状态时从进程 CWD 推导 git toplevel，
/// 推导失败返回空串）。Go 无参；Rust 尾参 repo 用于显式覆盖。
#[tauri::command]
pub fn get_current_repo(repo: Option<String>) -> String {
    resolve_current(repo.as_deref())
}

/// GetRepoName 仓库名（路径最后一段；"."/"" 取进程 CWD 的目录名）。
#[tauri::command]
pub fn get_repo_name(repo: String) -> String {
    if repo == "." || repo.is_empty() {
        return match std::env::current_dir() {
            Ok(cwd) => repo_base_name(&cwd.display().to_string()),
            Err(_) => repo,
        };
    }
    repo_base_name(&repo)
}

/// CurrentRepoInfo 当前仓库展示信息（path/name/branch）；
/// 未切换且 CWD 不是仓库时返回全空对象（对齐 Go）。
#[tauri::command]
pub fn current_repo_info(repo: Option<String>) -> Result<RepoInfo, String> {
    let repo = resolve_current(repo.as_deref());
    if repo.is_empty() || repo == "." {
        return Ok(RepoInfo::default());
    }
    Ok(RepoInfo {
        path: repo.clone(),
        name: repo_base_name(&repo),
        branch: current_branch(&repo),
    })
}

/// SwitchRepo 切换当前仓库：校验 git 工作树 → 更新状态 → 记录最近列表。
#[tauri::command]
pub fn switch_repo(dir: String) -> Result<SwitchResult, String> {
    if dir.is_empty() {
        return Err("路径不能为空".into());
    }
    let top = resolve_git_repo(&dir)?;
    let name = repo_base_name(&top);
    let branch = current_branch(&top);

    crate::state::set_current(&top);
    // 持久化失败非致命：内存切换已成功（Go 仅打印警告）
    if let Err(e) = crate::state::record_recent(&top) {
        eprintln!("warn: 保存最近仓库列表失败: {e}");
    }

    Ok(SwitchResult { path: top, name, branch, recent: load_recent_dtos() })
}

/// ListRecentRepos 最近仓库列表（最新在前，上限 10；state 持久化）。
#[tauri::command]
pub fn list_recent_repos() -> Result<Vec<RecentRepo>, String> {
    Ok(load_recent_dtos())
}

/// RemoveRecentRepo 从最近列表移除路径并持久化（不存在时为 no-op）。
#[tauri::command]
pub fn remove_recent_repo(path: String) -> Result<(), String> {
    crate::state::remove_recent(&path).map_err(|e| e.to_string())
}

/// PickRepoFolder 打开系统目录选择框（tauri-plugin-dialog 原生对话框）。
///
/// 对应 Go wails runtime.OpenDirectoryDialog：取消返回空串（Go 同语义），
/// 前端以空串判断未选择。
#[tauri::command]
pub async fn pick_repo_folder(app: tauri::AppHandle) -> Result<String, String> {
    use tauri_plugin_dialog::DialogExt;
    let (tx, rx) = std::sync::mpsc::channel::<Option<String>>();
    app.dialog().file().pick_folder(move |dir| {
        let path = dir
            .and_then(|d| d.as_path().map(|p| p.display().to_string()))
            .unwrap_or_default();
        let _ = tx.send(Some(path));
    });
    // 对话框由主线程事件循环驱动，std 线程等待回调（取消时目录为空串）
    let picked = std::thread::spawn(move || rx.recv().ok().flatten())
        .join()
        .map_err(|_| "对话框线程异常".to_string())?
        .unwrap_or_default();
    Ok(picked)
}

/// RepoFileList 列出仓库内某目录的直接子项（subPath 空串 = 仓库根）。
/// 目录懒加载由前端点击展开；跳过 .git/node_modules/.DS_Store；
/// 目录在前、名称不区分大小写排序。
#[tauri::command]
pub fn repo_file_list(sub_path: String, repo: Option<String>) -> Result<Vec<FileNode>, String> {
    let root = abs_path(&repo_path(repo)).map_err(|e| format!("解析仓库路径失败: {e}"))?;
    if rel_escapes(&sub_path) {
        return Err("路径越界".into());
    }
    let target = if sub_path.is_empty() { root.clone() } else { root.join(&sub_path) };

    let entries = match std::fs::read_dir(&target) {
        Ok(rd) => rd,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(format!("读取目录失败: {e}")),
    };

    let mut nodes: Vec<FileNode> = Vec::new();
    for e in entries.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        if name == ".git" || name == "node_modules" || name == ".DS_Store" {
            continue;
        }
        let is_dir = e.file_type().map(|t| t.is_dir()).unwrap_or(false);
        let size = e.metadata().map(|m| m.len()).unwrap_or(0) as i64;
        let rel = if sub_path.is_empty() {
            name.clone()
        } else {
            format!("{sub_path}/{name}")
        };
        nodes.push(FileNode { name, path: rel, is_dir, size });
    }
    // 目录在前，名称小写序
    nodes.sort_by(|a, b| match (a.is_dir, b.is_dir) {
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
    });
    Ok(nodes)
}

/// RepoFileSearch 递归搜索仓库名称包含 query（不区分大小写）的文件，
/// 上限 500 条，剪枝 .git/node_modules/.DS_Store，路径用 / 分隔。
#[tauri::command]
pub async fn repo_file_search(query: String, repo: Option<String>) -> Result<Vec<FileNode>, String> {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return Ok(Vec::new()); // 对齐 Go：空查询直接返回空列表
    }
    let root = abs_path(&repo_path(repo)).map_err(|e| format!("解析仓库路径失败: {e}"))?;
    run_blocking(move || Ok(search_walk(&root, &q)))?
}

/// 深度优先走目录树（对齐 filepath.WalkDir：词典序遍历、错误忽略、封顶即停）。
fn search_walk(root: &Path, q: &str) -> Vec<FileNode> {
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

    let mut matches: Vec<FileNode> = Vec::new();
    walk(root, q, root, &mut matches);
    matches.sort_by(|a, b| a.path.to_lowercase().cmp(&b.path.to_lowercase()));
    matches
}

/// RepoReadFile 按相对路径读仓库内文件（内容/二进制/未找到；>1MB 截断）。
#[tauri::command]
pub fn repo_read_file(rel_path: String, repo: Option<String>) -> Result<FileResult, String> {
    let repo = repo_path(repo);
    read_repo_file(&repo, &rel_path)
}

/// RepoReadme 定位并读取仓库 README（候选名按序尝试，均无则 notFound）。
#[tauri::command]
pub fn repo_readme(repo: Option<String>) -> Result<FileResult, String> {
    let repo = repo_path(repo);
    for name in README_CANDIDATES {
        let res = read_repo_file(&repo, name)?;
        if !res.not_found && !res.binary && !res.content.is_empty() {
            return Ok(FileResult { content: res.content, lines: res.lines, ..Default::default() });
        }
    }
    Ok(FileResult { not_found: true, ..Default::default() })
}

/// RepoReadImage 读仓库内图片并内联为 base64 data URL
/// （"data:<mime>;base64,<b64>"，标志位说明未内联的原因）。
#[tauri::command]
pub fn repo_read_image(rel_path: String, repo: Option<String>) -> Result<ImageResult, String> {
    let repo = repo_path(repo);
    if rel_escapes(&rel_path) {
        return Err("路径越界".into());
    }
    let full = repo.join(&rel_path);
    // 目录统一按“非文件”处理
    if full.is_dir() {
        return Ok(ImageResult { not_found: true, ..Default::default() });
    }
    let data = match std::fs::read(&full) {
        Ok(d) => d,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Ok(ImageResult { not_found: true, ..Default::default() })
        }
        Err(e) => return Err(format!("读取图片失败: {e}")),
    };
    if data.len() > MAX_IMAGE_SIZE {
        return Ok(ImageResult { too_large: true, ..Default::default() });
    }
    let Some(mime) = image_mime(&rel_path) else {
        return Ok(ImageResult { unsupported: true, ..Default::default() });
    };
    Ok(ImageResult {
        data_url: format!("data:{mime};base64,{}", base64_encode(&data)),
        ..Default::default()
    })
}

/// OpenInExplorer 在系统文件管理器中打开路径。
///
/// 偏差：按约定用 `explorer <path>` 直接打开（spawn 后不等待、启动失败静默，
/// 对齐 Go 吞错误的语义）；Go 用 `explorer.exe /select,<path>` 定位选中条目。
/// 路径为空直接成功（Go 同样不做事）。不做路径存在性校验（Go 未校验）。
#[tauri::command]
pub fn open_in_explorer(path: String) -> Result<(), String> {
    if path.is_empty() {
        return Ok(());
    }
    #[cfg(windows)]
    let mut cmd = {
        let mut c = Command::new("explorer");
        c.arg(&path);
        c
    };
    #[cfg(not(windows))]
    let mut cmd = {
        let mut c = Command::new("xdg-open");
        c.arg(&path);
        c
    };
    hide_console(&mut cmd);
    let _ = cmd.spawn(); // 不等待；文件管理器自身的失败不作为命令错误
    Ok(())
}

/// OpenInBrowser 用系统默认浏览器打开 URL。
///
/// 偏差：Go 走 Wails BrowserOpenURL；Tauri 侧未接 opener/shell 插件，
/// Windows 用 rundll32 url.dll,FileProtocolHandler（不经 cmd 解析，
/// 避免 URL 中 & 等元字符被拆断），其它平台 xdg-open/open，spawn 不等待。
#[tauri::command]
pub fn open_in_browser(url: String) {
    if url.is_empty() {
        return;
    }
    let mut cmd = open_url_cmd(&url);
    hide_console(&mut cmd);
    let _ = cmd.spawn();
}

#[cfg(windows)]
fn open_url_cmd(url: &str) -> Command {
    let mut c = Command::new("rundll32");
    c.arg("url.dll,FileProtocolHandler").arg(url);
    c
}

#[cfg(not(windows))]
fn open_url_cmd(url: &str) -> Command {
    let program = if cfg!(target_os = "macos") { "open" } else { "xdg-open" };
    let mut c = Command::new(program);
    c.arg(url);
    c
}
