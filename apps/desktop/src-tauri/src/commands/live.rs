//! live 域命令：实时变更（Live Changes）——监视工作区相对 HEAD 的未提交
//! 改动，外加 Git 备忘录（cheat）内容仓管理（Go 侧挂在 system 模块下，
//! 按文件分工约定随 live 域一起落地）。
//!
//! 实现说明：
//! - `gui/internal/live/live.go`：Go fsnotify → notify 8 recommended_watcher。
//!   手工递归添加目录（跳过 node_modules/dist/.idea/vendor；.git 只监视
//!   index/HEAD/refs 三个入口——commit/stage/checkout 不触碰工作区文件，
//!   少了它们 Navbar 徽标会失活），事件 300ms 防抖 + porcelain 签名去抖
//!   后 emit "live:changed"（payload null，对齐 Go EventsEmit(ctx, ..., nil)）。
//! - `gui/internal/system/cheat.go` + `internal/cheat/cheat.go`：内容仓位于
//!   `~/.cicbyte/data/git-cheat-content`（AppPaths::cheat_dir，对应 Go
//!   config.GetCheatDir 的共享数据目录）。CheatEnsure 浅克隆 / CheatPull
//!   快进更新 / CheatList 扫描 docs/zh/<分类>/*.md / CheatRead 读正文
//!   （剥掉 YAML frontmatter）。
//!
//! Go 侧 live/cheat 绑定方法均无 repo 参数（尾部追加 `repo: Option<String>`，
//! 经 crate::state::resolve 解析）。npm 域前端未调用（apps/desktop/frontend/src
//! 无 NpmAvailable 引用），未实现。
//!
//! 事件清单：
//! - "live:changed"：payload null。防抖 + 签名去抖后由 watcher 线程发出。
//!
//! 与 Go 的已知偏差：
//! - 防抖实现为「单 deadline + 事件重置」，fire() 串行执行；Go 每个事件各起
//!   一个 timer goroutine，忙树时可能并发 fire（其注释本意就是要避免）。
//!   用户可见行为一致：仅当 porcelain 签名变化时 emit。
//! - cheat frontmatter 用逐行 `key: value` 解析（无 serde_yaml 依赖），
//!   不支持多行/嵌套 YAML；title/description/command 均为纯标量，实际够用。

use crate::commands::backup::{CommitStatEntryDto, FileDiffResultDto};
use byte_git_core::util::AppPaths;
use notify::{RecursiveMode, Watcher};
use serde::Serialize;
use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri::Emitter;

/// porcelain / numstat / show 单次 git 查询超时（对应 Go 的 10s context）。
const STAT_TIMEOUT: Duration = Duration::from_secs(10);
/// computeSig 的 git status 超时（对应 Go 的 5s context）。
const SIG_TIMEOUT: Duration = Duration::from_secs(5);
/// 防抖窗口（对应 Go time.NewTimer(300ms)）。
const DEBOUNCE: Duration = Duration::from_millis(300);
/// git clone --depth 1（Go 无超时；按约定给网络操作 10 分钟上限）。
const CLONE_TIMEOUT: Duration = Duration::from_secs(600);
/// git pull --ff-only（Go 无超时；同上）。
const PULL_TIMEOUT: Duration = Duration::from_secs(300);
/// 送给前端的内容上限（对应 Go core.MaxSecretFileSize = 1MB）。
const MAX_TEXT_CONTENT: usize = 1 << 20;

/// cheat 内容源仓库（对应 Go cheat.RepoURL）。
const CHEAT_REPO_URL: &str = "https://github.com/cicbyte/git-cheat-content";
/// 仓库内存放本地化 .md 树的子目录（对应 Go cheat.docsRoot）。
const CHEAT_DOCS_ROOT: &str = "docs/zh";

/// 与 Go GUI 一致：无仓库上下文的命令统一接收可选 repo 参数；
/// 缺省回落到当前仓库状态（state.rs，对应 Core.CurrentRepo），最终 "."。
fn repo_path(repo: Option<String>) -> PathBuf {
    crate::state::resolve(repo)
}

// ---------------------------------------------------------------------------
// 基础设施（同 backup.rs：隐藏窗口 + 超时 + stdout/stderr 分离捕获）
// ---------------------------------------------------------------------------

/// Windows 下避免子进程闪现控制台窗口（对应 sysproc 的 CREATE_NO_WINDOW）。
#[cfg(windows)]
fn hide_console(cmd: &mut Command) {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    cmd.creation_flags(CREATE_NO_WINDOW);
}

#[cfg(not(windows))]
fn hide_console(_cmd: &mut Command) {}

/// git 子进程执行结果（stdout/stderr 分离，对齐 Go cmd.Output() 只取 stdout；
/// clone/pull 等用 combined() 拼出 CombinedOutput 语义）。
struct GitOut {
    success: bool,
    stdout: String,
    stderr: String,
}

impl GitOut {
    /// 合并输出（对应 Go 的 CombinedOutput）。
    fn combined(&self) -> String {
        format!("{}{}", self.stdout, self.stderr)
    }
}

/// 执行 `git <args...>`（含 "-C <repo>" 也由调用方拼入）：隐藏窗口 + 超时。
fn git_run(args: &[&str], timeout: Duration) -> GitOut {
    fn drain<R: std::io::Read>(mut r: R) -> Vec<u8> {
        let mut buf = Vec::new();
        let _ = std::io::Read::read_to_end(&mut r, &mut buf);
        buf
    }

    let mut cmd = Command::new("git");
    cmd.args(args)
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
                            "命令超时（{} 秒）: git {}",
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

/// 在独立线程执行阻塞任务（git 查询 / 网络克隆 / 目录树遍历）。
/// Tauri v2 同步命令在主线程执行，等待会冻结窗口，故 async + std 线程对齐。
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

/// filepath.Abs 的词法等价：相对路径拼当前目录（不归一符号链接，避免
/// Windows canonicalize 产生 \\?\ 前缀）。
fn abs_lexical(p: &Path) -> PathBuf {
    if p.is_absolute() {
        p.to_path_buf()
    } else {
        std::env::current_dir()
            .map(|cwd| cwd.join(p))
            .unwrap_or_else(|_| p.to_path_buf())
    }
}

/// 相对路径是否越出根目录（对齐 Go Clean+HasPrefix 守卫的意图）：
/// rel 内的 ".." 回退不能低于根；rel 不能携带盘符/根（会替换掉根）。
fn rel_escapes(rel: &str) -> bool {
    let mut depth = 0i32;
    for c in Path::new(rel).components() {
        match c {
            Component::Prefix(_) | Component::RootDir => return true,
            Component::ParentDir => {
                depth -= 1;
                if depth < 0 {
                    return true;
                }
            }
            Component::CurDir => {}
            _ => depth += 1,
        }
    }
    false
}

/// 工作区文件行数（尽力而为；对应 Go countFileLines：'\n' 计数 + 1）。
fn count_file_lines(p: &Path) -> i64 {
    match std::fs::read(p) {
        Ok(raw) => String::from_utf8_lossy(&raw).matches('\n').count() as i64 + 1,
        Err(_) => 0,
    }
}

// ---------------------------------------------------------------------------
// watcher 全局状态（对应 Go LiveModule 的 Watcher/WatchStop/WatchPath/LastSig）
// ---------------------------------------------------------------------------

/// watcher 线程消息：文件系统事件（内容不区分，与 Go 一样只用来触发防抖）
/// 或停止指令。
enum WatchMsg {
    Fs,
    Stop,
}

struct WatchState {
    /// 当前监视的仓库绝对路径（幂等判断用）。
    path: PathBuf,
    /// 通知 watcher 线程退出。
    stop: mpsc::Sender<WatchMsg>,
}

static WATCH: Mutex<Option<WatchState>> = Mutex::new(None);

/// 最近一次 porcelain 签名（对应 Go LiveModule.LastSig；LiveStat 成功后也会
/// 同步，避免随后的 watcher 事件对同一状态重复 emit）。
static LAST_SIG: Mutex<String> = Mutex::new(String::new());

/// computeSig：当前工作区状态的廉价签名（porcelain 原始输出）。
/// 失败返回空串（对应 Go err → ""）。
fn compute_sig() -> String {
    let repo = crate::state::current_or_dot();
    let out = git_run(&["-C", repo.as_str(), "status", "--porcelain"], SIG_TIMEOUT);
    if !out.success {
        return String::new();
    }
    out.stdout
}

/// fire：签名变化时 emit "live:changed"（对应 Go fire()，payload nil）。
fn fire(app: &tauri::AppHandle) {
    let sig = compute_sig();
    let mut last = LAST_SIG.lock().unwrap();
    let changed = sig != *last;
    *last = sig;
    drop(last);
    if changed {
        let _ = app.emit("live:changed", ());
    }
}

/// 递归添加目录监视（对应 Go 的 filepath.Walk + w.Add）：
/// 跳过 node_modules/dist/.idea/vendor；.git 只监视 index/HEAD/refs。
fn add_watches_recursive(watcher: &mut notify::RecommendedWatcher, root: &Path) {
    let _ = watcher.watch(root, RecursiveMode::NonRecursive);
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else { continue };
        for e in entries.flatten() {
            let Ok(ft) = e.file_type() else { continue };
            if !ft.is_dir() {
                continue;
            }
            let name = e.file_name().to_string_lossy().into_owned();
            if name == "node_modules" || name == "dist" || name == ".idea" || name == "vendor" {
                continue;
            }
            let path = e.path();
            if name == ".git" {
                // 不遍历整个 .git（objects/logs 噪音大）；只监视少数几个
                // 变更即代表 git 状态变化的入口。refs 用递归监视：分支切换
                // 改的是 refs/heads/*，而 Go fsnotify 的 Windows 后端本身就是
                // 递归的（w.Add(.git/refs) 实际能收到子目录事件）。
                let _ = watcher.watch(&path.join("index"), RecursiveMode::NonRecursive);
                let _ = watcher.watch(&path.join("HEAD"), RecursiveMode::NonRecursive);
                let _ = watcher.watch(&path.join("refs"), RecursiveMode::Recursive);
                continue;
            }
            let _ = watcher.watch(&path, RecursiveMode::NonRecursive);
            stack.push(path);
        }
    }
}

/// LiveStartWatch 主体：同路径幂等；不同路径先拆旧 watcher 再重建。
fn start_watch_impl(repo: PathBuf, app: tauri::AppHandle) -> Result<(), String> {
    let abs = abs_lexical(&repo);
    let mut guard = WATCH.lock().unwrap();

    // already watching this exact path → nothing to do
    if let Some(w) = guard.as_ref() {
        if w.path == abs {
            return Ok(());
        }
    }

    // tear down any previous watcher
    if let Some(w) = guard.take() {
        let _ = w.stop.send(WatchMsg::Stop);
    }

    let (tx, rx) = mpsc::channel::<WatchMsg>();
    let evt_tx = tx.clone();
    let mut watcher = notify::recommended_watcher(
        move |res: Result<notify::Event, notify::Error>| {
            // Go 忽略 w.Errors：仅事件触发防抖
            if res.is_ok() {
                let _ = evt_tx.send(WatchMsg::Fs);
            }
        },
    )
    .map_err(|e| format!("创建文件监视器失败: {e}"))?;

    add_watches_recursive(&mut watcher, &abs);

    *guard = Some(WatchState { path: abs, stop: tx });

    // 防抖线程：编辑器/工具会连发大量事件，合并为一个信号；且仅当 git
    // status 实际变化（签名不同）时才 emit，噪音目录不刷前端。
    std::thread::spawn(move || {
        let _watcher = watcher; // 持有 watcher；线程退出即停止监视
        let mut deadline: Option<Instant> = None;
        loop {
            // 无 pending 时长驻等待（Stop 消息会立即唤醒；1 小时兜底重算）
            let wait = deadline
                .map(|d| d.saturating_duration_since(Instant::now()))
                .unwrap_or(Duration::from_secs(3600));
            match rx.recv_timeout(wait) {
                Ok(WatchMsg::Stop) => return,
                Ok(WatchMsg::Fs) => deadline = Some(Instant::now() + DEBOUNCE),
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    deadline = None;
                    fire(&app);
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => return,
            }
        }
    });
    Ok(())
}

// ---------------------------------------------------------------------------
// 命令：Live（live.go）
// ---------------------------------------------------------------------------

/// LiveStartWatch 开始监视当前仓库的文件系统变更，防抖后 emit "live:changed"。
/// 重复调用安全：同一路径为 no-op，不同路径自动重启 watcher。
/// （递归遍历大仓库可能耗时，async + run_blocking 避免冻结 UI。）
#[tauri::command]
pub async fn live_start_watch(repo: Option<String>, app: tauri::AppHandle) -> Result<(), String> {
    let repo = repo_path(repo);
    run_blocking(move || start_watch_impl(repo, app))?
}

/// LiveStopWatch 停止 live watcher（视图卸载时调用；对应 Go 无返回值）。
#[tauri::command]
pub fn live_stop_watch(repo: Option<String>) {
    let _ = repo_path(repo); // 适配层统一注入 repo；停止动作忽略（对齐 Go）
    let mut guard = WATCH.lock().unwrap();
    if let Some(w) = guard.take() {
        let _ = w.stop.send(WatchMsg::Stop);
    }
}

/// LiveStat 返回相对 HEAD 的全部未提交改动（含未跟踪 ??）。
/// 合并 `git status --porcelain`（状态 + 未跟踪集）与 `git diff --numstat
/// HEAD`（行数增删）；未跟踪文件整文件计入新增行数。
/// （两次 git 查询各 10s 超时，async + run_blocking 避免冻结 UI。）
#[tauri::command]
pub async fn live_stat(repo: Option<String>) -> Result<Vec<CommitStatEntryDto>, String> {
    let repo = repo_path(repo);
    run_blocking(move || live_stat_impl(&repo))?
}

fn live_stat_impl(repo: &Path) -> Result<Vec<CommitStatEntryDto>, String> {
    let repo_str = repo.display().to_string();

    // 1. porcelain：每个变更路径 + XY 状态（覆盖未跟踪 ??）
    let status = git_run(&["-C", repo_str.as_str(), "status", "--porcelain"], STAT_TIMEOUT);
    if !status.success {
        return Err(format!("git status 失败: {}", status.combined().trim()));
    }
    let status_str = status.stdout.replace("\r\n", "\n");
    let mut path_stat: HashMap<String, &str> = HashMap::new();
    let mut ordered: Vec<String> = Vec::new(); // 保持 porcelain 顺序
    for line in status_str.split('\n') {
        let b = line.as_bytes();
        if b.len() < 4 {
            continue;
        }
        let (x, y) = (b[0], b[1]);
        // porcelain 路径可能带引号；XY 与空格均为 ASCII，切片安全
        let file = line[3..].trim().trim_matches('"');
        if file.is_empty() {
            continue;
        }
        let st = if x == b'?' && y == b'?' {
            "A" // untracked → 相对 HEAD 视为新增
        } else if x == b'A' || y == b'A' {
            "A"
        } else if x == b'D' || y == b'D' {
            "D"
        } else {
            "M"
        };
        // 去重：porcelain 中一个路径至多一行
        if !path_stat.contains_key(file) {
            path_stat.insert(file.to_string(), st);
            ordered.push(file.to_string());
        }
    }

    // 2. numstat：已跟踪变更的行数增减（未跟踪文件不输出，单独处理）
    let num = git_run(&["-C", repo_str.as_str(), "diff", "--numstat", "HEAD"], STAT_TIMEOUT);
    let mut line_delta: HashMap<String, (i64, i64)> = HashMap::new();
    if num.success {
        let num_str = num.stdout.replace("\r\n", "\n");
        for line in num_str.split('\n') {
            let line = line.trim();
            if line.is_empty() || line.starts_with("warning:") {
                continue;
            }
            let parts: Vec<&str> = line.splitn(3, '\t').collect();
            if parts.len() < 3 {
                continue;
            }
            let added = if parts[0] == "-" { 0 } else { parts[0].trim().parse::<i64>().unwrap_or(0) };
            let deleted = if parts[1] == "-" { 0 } else { parts[1].trim().parse::<i64>().unwrap_or(0) };
            line_delta.insert(parts[2].to_string(), (added, deleted));
        }
    }

    // 3. 合并（空 Vec 而非 null，避免前端 .map 报错——对齐 Go 注释意图）
    let mut entries = Vec::with_capacity(ordered.len());
    for file in &ordered {
        let st = path_stat[file];
        let (mut added, deleted) = line_delta.get(file).copied().unwrap_or((0, 0));
        // 未跟踪文件 numstat 无记录：整文件行数计入新增（仅对 HEAD 无版本的
        // 文件近似，已跟踪的 M 文件不做此近似）
        if st == "A" && added == 0 && deleted == 0 {
            let n = count_file_lines(&repo.join(file));
            if n > 0 {
                added = n;
            }
        }
        entries.push(CommitStatEntryDto {
            file: file.clone(),
            added,
            deleted,
            status: st.to_string(),
        });
    }

    // 同步签名：随后的 watcher 事件若无实际变化不再 emit
    *LAST_SIG.lock().unwrap() = status.stdout.clone();
    Ok(entries)
}

/// 单文件状态（porcelain XY → A/M/D；查询失败按默认 M，对齐 Go）。
fn single_file_status(repo_str: &str, file_path: &str) -> String {
    let out = git_run(
        &["-C", repo_str, "status", "--porcelain", "--", file_path],
        STAT_TIMEOUT,
    );
    if !out.success {
        return "M".into();
    }
    let s = out.stdout.trim();
    let b = s.as_bytes();
    if b.len() >= 2 {
        let (x, y) = (b[0], b[1]);
        if x == b'?' && y == b'?' {
            return "A".into();
        } else if x == b'D' || y == b'D' {
            return "D".into();
        } else if x == b'A' || y == b'A' {
            return "A".into();
        }
    }
    "M".into()
}

/// ShowFileAt 的返回：ok=false 表示 ref:path 在该版本不存在（对应 Go
/// LogModule.ShowFileAt；ref 传 "" 得 ":path"，即暂存区 stage 0 blob）。
struct FileAt {
    content: String,
    binary: bool,
    ok: bool,
}

fn show_file_at(repo_str: &str, commit_ref: &str, file_path: &str) -> FileAt {
    let spec = format!("{commit_ref}:{file_path}");
    let res = git_run(&["-C", repo_str, "show", spec.as_str()], STAT_TIMEOUT);
    if !res.success {
        return FileAt { content: String::new(), binary: false, ok: false };
    }
    // 二进制探测：前 8192 字节含 NUL
    let scan = &res.stdout.as_bytes()[..res.stdout.len().min(8192)];
    if scan.contains(&0) {
        return FileAt { content: String::new(), binary: true, ok: true };
    }
    let mut c = res.stdout;
    if c.len() > MAX_TEXT_CONTENT {
        // 按字节截断（对齐 Go），再以 lossy 修复可能切断的 UTF-8 边界
        c = String::from_utf8_lossy(&c.as_bytes()[..MAX_TEXT_CONTENT]).into_owned();
        c.push_str("\n\n…（文件超过 1MB，已截断显示）");
    }
    FileAt { content: c, binary: false, ok: true }
}

/// LiveFileDiff 返回单个文件的 HEAD 版本（before）与工作区版本（after），
/// 供并排实时 diff 视图。
#[tauri::command]
pub async fn live_file_diff(
    file_path: String,
    repo: Option<String>,
) -> Result<FileDiffResultDto, String> {
    let repo = repo_path(repo);
    run_blocking(move || live_file_diff_impl(&file_path, &repo))?
}

fn live_file_diff_impl(file_path: &str, repo: &Path) -> Result<FileDiffResultDto, String> {
    let repo_str = repo.display().to_string();
    let mut res = FileDiffResultDto {
        before: String::new(),
        after: String::new(),
        status: single_file_status(&repo_str, file_path),
        binary: false,
        not_found: false,
    };

    // before = HEAD 版本（纯新增文件不存在）
    let before = show_file_at(&repo_str, "HEAD", file_path);
    if before.binary {
        res.binary = true;
        return Ok(res);
    }
    if before.ok {
        res.before = before.content;
    } else {
        res.not_found = true;
    }

    // after = 工作区文件（直接读盘，不经 git）；路径守卫防越界
    if rel_escapes(file_path) {
        return Err("路径越界".into());
    }
    let abs = repo.join(file_path);
    let raw = match std::fs::read(&abs) {
        Ok(r) => r,
        Err(_) => return Ok(res), // 工作区已删除 → after 留空
    };
    let scan = &raw[..raw.len().min(8192)];
    if scan.contains(&0) {
        res.binary = true;
        return Ok(res);
    }
    let mut c = String::from_utf8_lossy(&raw).into_owned();
    if c.len() > MAX_TEXT_CONTENT {
        c = String::from_utf8_lossy(&c.as_bytes()[..MAX_TEXT_CONTENT]).into_owned();
        c.push_str("\n\n…（文件超过 1MB，已截断显示）");
    }
    res.after = c;
    Ok(res)
}

/// LiveStagedFileDiff 返回单个文件的 HEAD 版本（before）与暂存区版本
/// （after，`git show :<path>`），供提交台的"暂存区"标签预览将提交的内容。
#[tauri::command]
pub async fn live_staged_file_diff(
    file_path: String,
    repo: Option<String>,
) -> Result<FileDiffResultDto, String> {
    let repo = repo_path(repo);
    run_blocking(move || live_staged_file_diff_impl(&file_path, &repo))?
}

fn live_staged_file_diff_impl(file_path: &str, repo: &Path) -> Result<FileDiffResultDto, String> {
    let repo_str = repo.display().to_string();
    let mut res = FileDiffResultDto {
        before: String::new(),
        after: String::new(),
        status: single_file_status(&repo_str, file_path),
        binary: false,
        not_found: false,
    };

    // before = HEAD 版本（纯新增文件不存在）
    let before = show_file_at(&repo_str, "HEAD", file_path);
    if before.binary {
        res.binary = true;
        return Ok(res);
    }
    if before.ok {
        res.before = before.content;
    } else {
        res.not_found = true;
    }

    // after = 暂存区版本（stage 0 blob；ref="" → ":path"）
    let after = show_file_at(&repo_str, "", file_path);
    if after.binary {
        res.binary = true;
        return Ok(res);
    }
    if after.ok {
        res.after = after.content;
    }
    // after.ok=false → 文件未暂存（或暂存了删除、无 blob）；After 留空
    Ok(res)
}

// ===========================================================================
// Cheat（备忘录；Go gui/internal/system/cheat.go + internal/cheat/cheat.go）
// ===========================================================================

/// Entry：一张备忘卡片（一个 .md 文件）。
#[derive(Debug, Clone, Serialize)]
pub struct CheatEntry {
    /// 父目录名
    pub category: String,
    /// frontmatter.title（缺省回退文件名）
    pub title: String,
    /// 仓库相对路径，如 docs/zh/创建/克隆远程仓库.md
    pub file: String,
    /// frontmatter.description
    pub description: String,
    /// frontmatter.command
    pub command: String,
}

/// Category：按父目录分组的一组条目。
#[derive(Debug, Clone, Serialize)]
pub struct CheatCategory {
    pub name: String,
    pub entries: Vec<CheatEntry>,
}

/// frontmatter 顶部 YAML 块的三个标量字段。
#[derive(Default)]
struct Frontmatter {
    description: String,
    command: String,
    title: String,
}

/// 内容仓克隆目录（~/.cicbyte/data/git-cheat-content）。
fn cheat_dir() -> Result<PathBuf, String> {
    AppPaths::new()
        .map_err(|e| e.to_string())?
        .cheat_dir()
        .map_err(|e| e.to_string())
}

/// 是否已克隆（目录 + docs 子树存在）。
fn cheat_is_cloned() -> bool {
    cheat_dir()
        .map(|d| d.join(CHEAT_DOCS_ROOT).exists())
        .unwrap_or(false)
}

/// CheatEnsure 首次使用时克隆内容仓，已存在则 no-op。返回克隆目录路径。
/// （网络克隆，async + run_blocking 避免冻结 UI。）
#[tauri::command]
pub async fn cheat_ensure(repo: Option<String>) -> Result<String, String> {
    let _ = repo_path(repo); // 应用级命令，repo 仅为适配层统一注入
    run_blocking(cheat_ensure_impl)?
}

fn cheat_ensure_impl() -> Result<String, String> {
    let dir = cheat_dir()?;
    if cheat_is_cloned() {
        return Ok(dir.display().to_string());
    }
    // 父目录必须存在
    if let Some(parent) = dir.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("创建目录失败: {e}"))?;
    }
    // clone --depth 1（浅克隆，快）
    let dir_str = dir.display().to_string();
    let out = git_run(
        &["clone", "--depth", "1", CHEAT_REPO_URL, dir_str.as_str()],
        CLONE_TIMEOUT,
    );
    if !out.success {
        return Err(format!("克隆失败: {}", out.combined().trim()));
    }
    Ok(dir_str)
}

/// CheatPull 快进更新内容仓（前端容忍失败并回落本地副本）。
#[tauri::command]
pub async fn cheat_pull(repo: Option<String>) -> Result<(), String> {
    let _ = repo_path(repo);
    run_blocking(cheat_pull_impl)?
}

fn cheat_pull_impl() -> Result<(), String> {
    let dir = cheat_dir()?;
    if !cheat_is_cloned() {
        return Err("仓库未克隆".into());
    }
    let dir_str = dir.display().to_string();
    let out = git_run(&["-C", dir_str.as_str(), "pull", "--ff-only"], PULL_TIMEOUT);
    if !out.success {
        return Err(format!("更新失败: {}", out.combined().trim()));
    }
    Ok(())
}

/// CheatList 扫描 docs/zh/<分类>/*.md，返回分类（名称排序）及其条目
/// （标题排序）。坏分类跳过而非整体失败（对齐 Go）。
#[tauri::command]
pub fn cheat_list(repo: Option<String>) -> Result<Vec<CheatCategory>, String> {
    let _ = repo_path(repo);
    cheat_list_impl()
}

fn cheat_list_impl() -> Result<Vec<CheatCategory>, String> {
    let root = cheat_dir()?.join(CHEAT_DOCS_ROOT);
    let entries = std::fs::read_dir(&root).map_err(|e| format!("读取分类目录失败: {e}"))?;
    let mut cats: Vec<CheatCategory> = Vec::new();
    for e in entries.flatten() {
        let Ok(ft) = e.file_type() else { continue };
        if !ft.is_dir() {
            continue;
        }
        let cat_name = e.file_name().to_string_lossy().into_owned();
        let Ok(cat_entries) = scan_category(&root, &cat_name) else { continue };
        if cat_entries.is_empty() {
            continue;
        }
        cats.push(CheatCategory { name: cat_name, entries: cat_entries });
    }
    cats.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(cats)
}

/// 扫描一个分类目录下的全部 .md（标题排序）。
fn scan_category(root: &Path, cat_name: &str) -> std::io::Result<Vec<CheatEntry>> {
    let cat_dir = root.join(cat_name);
    let files = std::fs::read_dir(&cat_dir)?;
    let mut out = Vec::new();
    for f in files.flatten() {
        let Ok(ft) = f.file_type() else { continue };
        if ft.is_dir() {
            continue;
        }
        let fname = f.file_name().to_string_lossy().into_owned();
        if !fname.ends_with(".md") {
            continue;
        }
        // 仓库相对路径统一正斜杠（对应 Go filepath.ToSlash）
        let rel_path = format!("{CHEAT_DOCS_ROOT}/{cat_name}/{fname}");
        if let Ok(entry) = read_entry(&cat_dir, &fname, cat_name, &rel_path) {
            out.push(entry);
        }
    }
    out.sort_by(|a, b| a.title.cmp(&b.title));
    Ok(out)
}

/// 解析一个 .md 的 frontmatter 为条目（读失败即整体跳过该文件）。
fn read_entry(dir: &Path, fname: &str, cat_name: &str, rel_path: &str) -> std::io::Result<CheatEntry> {
    let raw = std::fs::read(dir.join(fname))?;
    let fm = parse_frontmatter(&String::from_utf8_lossy(&raw));
    let title = if fm.title.is_empty() {
        // 回退：去扩展名的文件名
        fname.strip_suffix(".md").unwrap_or(fname).to_string()
    } else {
        fm.title
    };
    Ok(CheatEntry {
        category: cat_name.to_string(),
        title,
        file: rel_path.to_string(),
        description: fm.description,
        command: fm.command,
    })
}

/// 解析顶部 `---` 分隔的 YAML 块（简化为逐行 `key: value`；见模块注释偏差）。
fn parse_frontmatter(raw: &str) -> Frontmatter {
    // CRLF → LF，保证 Windows 克隆下分隔符搜索可靠
    let text = raw.replace("\r\n", "\n");
    let Some(rest) = text.strip_prefix("---\n") else {
        return Frontmatter::default();
    };
    // 闭合分隔符：恰为 "---" 的一行（容忍末块无尾随换行）
    let idx = rest.find("\n---\n").or_else(|| rest.find("\n---"));
    let Some(i) = idx else { return Frontmatter::default() };
    let block = &rest[..i];
    let mut fm = Frontmatter::default();
    for line in block.lines() {
        let Some((k, v)) = line.split_once(':') else { continue };
        let v = unquote(v.trim());
        match k.trim() {
            "description" => fm.description = v,
            "command" => fm.command = v,
            "title" => fm.title = v,
            _ => {}
        }
    }
    fm
}

/// 去掉值两侧配对的引号（YAML 单/双引号标量）。
fn unquote(s: &str) -> String {
    let b = s.as_bytes();
    if s.len() >= 2 {
        let (f, l) = (b[0], b[s.len() - 1]);
        if (f == b'"' && l == b'"') || (f == b'\'' && l == b'\'') {
            return s[1..s.len() - 1].to_string();
        }
    }
    s.to_string()
}

/// CheatRead 返回一个文件的 markdown 正文（剥掉 frontmatter）。
/// relPath 为仓库相对路径，如 "docs/zh/创建/克隆远程仓库.md"。
#[tauri::command]
pub fn cheat_read(rel_path: String, repo: Option<String>) -> Result<String, String> {
    let _ = repo_path(repo);
    cheat_read_impl(&rel_path)
}

fn cheat_read_impl(rel_path: &str) -> Result<String, String> {
    // 防路径穿越：解析结果必须留在克隆目录内
    let dir = cheat_dir()?;
    if rel_escapes(rel_path) {
        return Err("非法路径".into());
    }
    let full = dir.join(rel_path);
    let raw = std::fs::read(&full).map_err(|e| e.to_string())?;
    Ok(strip_frontmatter(&String::from_utf8_lossy(&raw)))
}

/// 剥掉顶部 `---\n...\n---` 块返回正文；无 frontmatter 时原样返回。
fn strip_frontmatter(text: &str) -> String {
    // CRLF → LF（同 parse_frontmatter；Go 亦返回归一化后的文本）
    let text = text.replace("\r\n", "\n");
    let Some(rest) = text.strip_prefix("---\n") else {
        return text;
    };
    let idx = rest.find("\n---\n").or_else(|| rest.find("\n---"));
    let Some(i) = idx else {
        return text; // 格式异常 → 原样返回
    };
    let mut body = &rest[i + "\n---".len()..];
    // 丢掉闭合分隔符后紧随的换行
    if let Some(stripped) = body.strip_prefix('\n') {
        body = stripped;
    }
    body.to_string()
}
