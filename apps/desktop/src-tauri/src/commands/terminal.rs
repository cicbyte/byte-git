//! terminal 域命令：内嵌终端（xterm.js + ConPTY/PTY）。
//!
//! 移植自 git-mate `gui/internal/system/terminal.go` 与
//! `internal/terminal/{terminal.go, terminal_windows.go}`（ConPTY 封装）。
//! Go conpty → portable-pty 0.9：CommandBuilder + openpty(PtySize) +
//! 读线程 base64 编码后 emit。
//!
//! 会话模型：单会话（对齐 Go SystemModule 只有一个 Pty 字段，TerminalStart
//! 先拆旧会话再建新会话；全局 `Mutex<Option<TermSession>>`，写/resize 与
//! stop 同锁防 TOCTOU——对应 Go 持 PtyMu 的语义）。
//!
//! 事件清单（payload 与 Go Wails 一致）：
//! - "term:data"：`{ data: <base64> }`——PTY 输出（STD_ENCODING）分块推送；
//! - "term:exit"：payload null——PTY 读到 EOF/错误（含 TerminalStop 关闭后）。
//!
//! base64：base64 0.22（标准字母表，对应 Go base64.StdEncoding）。
//! Go 侧绑定方法无 repo 参数（尾部追加 `repo: Option<String>`，
//! 仅 TerminalStart 用它做空 cwd 的回落解析，其余忽略）。

use base64::{engine::general_purpose::STANDARD, Engine as _};
use byte_git_core::config::Settings;
use portable_pty::{native_pty_system, CommandBuilder, MasterPty, PtySize};
use serde::Serialize;
use std::io::{Read, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri::Emitter;

/// 单次 git 查询超时（cwd 回落时 rev-parse --show-toplevel 用）。
const GIT_TIMEOUT: Duration = Duration::from_secs(10);
/// PTY 读缓冲（对应 Go 的 4096 字节 buf）。
const READ_BUF: usize = 4096;

// ---------------------------------------------------------------------------
// DTO（json tag 与 Go 结构体对齐）
// ---------------------------------------------------------------------------

/// term:data 事件负载（对应 Go termDataEvent）。
#[derive(Debug, Clone, Serialize)]
pub struct TermDataEvent {
    /// base64 编码的 PTY 输出
    pub data: String,
}

/// TerminalDetectResult：探测到的 shell 路径 + 是否 git-bash。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TerminalDetectResult {
    pub path: String,
    pub is_git_bash: bool,
}

// ---------------------------------------------------------------------------
// 会话管理（单会话；对应 Go SystemModule 的 Pty/PtyStop + PtyMu）
// ---------------------------------------------------------------------------

struct TermSession {
    /// 用户输入通道（take_writer 只能取一次，随会话保存）
    writer: Box<dyn Write + Send>,
    /// master 存活以支持 resize；drop 即关闭 PTY（读线程随之 EOF）
    master: Box<dyn MasterPty + Send>,
    child: Box<dyn portable_pty::Child + Send + Sync>,
}

static TERM: Mutex<Option<TermSession>> = Mutex::new(None);

/// 关闭 PTY 并结束子进程（对应 Go TeardownPtyLocked；调用方持锁）。
/// master drop 后读线程读到 EOF/错误 → emit "term:exit"（与 Go 关闭 PTY
/// 解除 Read 阻塞后由读 goroutine emit 的行为一致）。
fn teardown_locked(slot: &mut Option<TermSession>) {
    if let Some(mut s) = slot.take() {
        let _ = s.child.kill();
        let _ = s.child.wait();
        drop(s.master);
        drop(s.writer);
    }
}

// ---------------------------------------------------------------------------
// 私有辅助
// ---------------------------------------------------------------------------

/// Windows 下避免子进程闪现控制台窗口（对应 sysproc 的 CREATE_NO_WINDOW；
/// 仅用于辅助 git 查询——PTY 子进程由 portable-pty/ConPTY 自行托管）。
#[cfg(windows)]
fn hide_console(cmd: &mut Command) {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    cmd.creation_flags(CREATE_NO_WINDOW);
}

#[cfg(not(windows))]
fn hide_console(_cmd: &mut Command) {}

/// 带超时执行 git 并取 stdout（成功时返回 Some；仅 cwd 回落推导用，
/// 输出只有一行，无需独立读线程）。
fn git_stdout(args: &[&str], timeout: Duration) -> Option<String> {
    let mut cmd = Command::new("git");
    cmd.args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    hide_console(&mut cmd);
    let mut child = cmd.spawn().ok()?;
    let deadline = Instant::now() + timeout;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                if !status.success() {
                    return None;
                }
                break;
            }
            Ok(None) => {
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    return None;
                }
                std::thread::sleep(Duration::from_millis(15));
            }
            Err(_) => return None,
        }
    }
    let mut buf = Vec::new();
    if let Some(mut p) = child.stdout.take() {
        let _ = p.read_to_end(&mut buf);
    }
    Some(String::from_utf8_lossy(&buf).into_owned())
}

/// 当前仓库目录（对应 Go Core.GetCurrentRepo）：
/// 显式 repo 参数 > 内存当前仓库 > 进程 CWD 的 git toplevel；均无 → ""。
fn current_repo_dir(repo: Option<String>) -> String {
    if let Some(p) = repo.filter(|s| !s.is_empty()) {
        return p;
    }
    let cur = crate::state::current_or_dot();
    if cur != "." {
        return cur;
    }
    if let Some(out) = git_stdout(&["rev-parse", "--show-toplevel"], GIT_TIMEOUT) {
        let s = out.trim();
        if !s.is_empty() {
            return s.to_string();
        }
    }
    String::new()
}

/// os.UserHomeDir 等价：Windows %USERPROFILE%，unix $HOME。
fn home_dir() -> String {
    std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .and_then(|s| s.to_str().map(|s| s.to_string()))
        .filter(|s| !s.is_empty())
        .unwrap_or_default()
}

/// filepath.Abs 的词法等价（ConPTY 需要绝对工作目录）。
fn abs_lexical(p: &str) -> String {
    let path = Path::new(p);
    if path.is_absolute() {
        p.to_string()
    } else {
        std::env::current_dir()
            .map(|cwd| cwd.join(path).display().to_string())
            .unwrap_or_else(|_| p.to_string())
    }
}

/// isGitBash：文件名为 bash.exe 且路径含 "git"（忽略大小写）。
fn is_git_bash(path: &str) -> bool {
    let low = path.to_lowercase();
    let base = Path::new(&low)
        .file_name()
        .map(|f| f.to_string_lossy().into_owned())
        .unwrap_or_default();
    base == "bash.exe" && low.contains("git")
}

/// exec.LookPath 等价：沿 PATH 查找可执行文件（Windows 无扩展名时补 .exe）。
fn look_path(name: &str) -> Option<String> {
    #[cfg(windows)]
    let exe = if name.contains('.') { name.to_string() } else { format!("{name}.exe") };
    #[cfg(not(windows))]
    let exe = name.to_string();
    if let Some(path) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&path) {
            let cand = dir.join(&exe);
            if cand.is_file() {
                return Some(cand.display().to_string());
            }
        }
    }
    None
}

/// 自动探测最佳可用 shell（对应 Go detectShell）。
#[cfg(windows)]
fn detect_shell() -> (String, bool) {
    let mut candidates: Vec<String> = vec![
        r"C:\Program Files\Git\bin\bash.exe".into(),
        r"C:\Program Files\Git\usr\bin\bash.exe".into(),
        r"C:\Program Files (x86)\Git\bin\bash.exe".into(),
    ];
    // LOCALAPPDATA 下也可能装了 Git
    if let Ok(local) = std::env::var("LOCALAPPDATA") {
        if !local.is_empty() {
            candidates.push(format!(
                r"{}\Programs\Git\bin\bash.exe",
                local.trim_end_matches('\\')
            ));
        }
    }
    for c in &candidates {
        if Path::new(c).exists() {
            return (c.clone(), true);
        }
    }
    // 回退：wsl
    if let Some(p) = look_path("wsl.exe") {
        return (p, false);
    }
    // 回退：cmd
    if let Some(p) = look_path("cmd.exe") {
        return (p, false);
    }
    (String::new(), false)
}

/// unix：$SHELL > bash > sh（对应 Go detectShell 非 Windows 分支）。
#[cfg(not(windows))]
fn detect_shell() -> (String, bool) {
    if let Ok(s) = std::env::var("SHELL") {
        if !s.is_empty() {
            return (s, false);
        }
    }
    if let Some(p) = look_path("bash") {
        return (p, false);
    }
    if let Some(p) = look_path("sh") {
        return (p, false);
    }
    (String::new(), false)
}

/// 选定 shell：设置 terminal.shell_path（存在时）> 自动探测 > 兜底。
fn resolve_shell() -> String {
    if let Ok(cfg) = Settings::load() {
        let sp = cfg.terminal.shell_path;
        if !sp.is_empty() && Path::new(&sp).exists() {
            return sp;
        }
    }
    let (path, _) = detect_shell();
    if path.is_empty() {
        // last resort
        return if cfg!(windows) { "cmd.exe".into() } else { "/bin/sh".into() };
    }
    path
}

// ---------------------------------------------------------------------------
// 命令（Go SystemModule 绑定）
// ---------------------------------------------------------------------------

/// TerminalStart 在给定工作目录打开运行 shell 的 PTY。
/// shell 为空时用配置或自动探测的 shell；cwd 为空时回落当前仓库、再回落
/// 用户主目录；cols/rows 非法（<=0）时回落 80x24。已有会话先拆除。
#[tauri::command]
pub fn terminal_start(
    shell: String,
    cwd: String,
    cols: i32,
    rows: i32,
    repo: Option<String>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    let mut guard = TERM.lock().unwrap();

    // 拆除既有会话（单会话模型，对齐 Go TeardownPtyLocked）
    teardown_locked(&mut guard);

    // 解析 shell
    let shell = if shell.is_empty() { resolve_shell() } else { shell };

    // 解析 cwd：当前仓库 > 主目录（对应 Go GetCurrentRepo → UserHomeDir）
    let cwd = if cwd.is_empty() {
        let r = current_repo_dir(repo);
        if r.is_empty() {
            let h = home_dir();
            if h.is_empty() {
                std::env::current_dir().map(|c| c.display().to_string()).unwrap_or_default()
            } else {
                h
            }
        } else {
            r
        }
    } else {
        cwd
    };
    let cwd = abs_lexical(&cwd);

    let cols = if cols <= 0 { 80 } else { cols };
    let rows = if rows <= 0 { 24 } else { rows };
    let size = PtySize {
        rows: rows as u16,
        cols: cols as u16,
        pixel_width: 0,
        pixel_height: 0,
    };

    let pty_system = native_pty_system();
    let pair = pty_system.openpty(size).map_err(|e| format!("启动终端失败: {e}"))?;
    let mut cmd = CommandBuilder::new(&shell);
    cmd.cwd(&cwd);
    let child = pair.slave.spawn_command(cmd).map_err(|e| format!("启动终端失败: {e}"))?;
    let reader = pair.master.try_clone_reader().map_err(|e| format!("启动终端失败: {e}"))?;
    let writer = pair.master.take_writer().map_err(|e| format!("启动终端失败: {e}"))?;
    drop(pair.slave); // 关闭父进程持有的 slave 端

    // 读线程：PTY 输出 → base64 → 前端
    let emitter = app.clone();
    std::thread::spawn(move || {
        let mut reader = reader;
        let mut buf = [0u8; READ_BUF];
        loop {
            match reader.read(&mut buf) {
                Ok(n) if n > 0 => {
                    let b64 = STANDARD.encode(&buf[..n]);
                    let _ = emitter.emit("term:data", TermDataEvent { data: b64 });
                }
                // EOF 或错误 → 会话结束（含 TerminalStop 关闭后的解除阻塞）
                _ => break,
            }
        }
        let _ = emitter.emit("term:exit", ());
    });

    *guard = Some(TermSession { writer, master: pair.master, child });
    Ok(())
}

/// TerminalInput 将 base64 编码的用户输入写入 PTY。
/// 持锁执行写入，防止并发的 TerminalStop 在判空与写入之间关闭 PTY
/// （对应 Go 的 TOCTOU 注释）。
#[tauri::command]
pub fn terminal_input(b64: String) -> Result<(), String> {
    let data = STANDARD.decode(&b64).map_err(|e| format!("base64 解码失败: {e}"))?;
    let mut guard = TERM.lock().unwrap();
    let s = guard.as_mut().ok_or("终端未启动")?;
    s.writer.write_all(&data).map_err(|e| e.to_string())
}

/// TerminalResize 更新 PTY 视口尺寸（xterm 的 rows/cols）。
/// 与 TerminalInput 同锁（同 TOCTOU 原因）。
#[tauri::command]
pub fn terminal_resize(cols: i32, rows: i32) -> Result<(), String> {
    let guard = TERM.lock().unwrap();
    let s = guard.as_ref().ok_or("终端未启动")?;
    s.master
        .resize(PtySize {
            rows: rows.max(0) as u16,
            cols: cols.max(0) as u16,
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|e| e.to_string())
}

/// TerminalStop 关闭 PTY 并结束子进程（master 关闭后读线程会再发出
/// "term:exit"）。
#[tauri::command]
pub fn terminal_stop() -> Result<(), String> {
    let mut guard = TERM.lock().unwrap();
    teardown_locked(&mut guard);
    Ok(())
}

/// TerminalDetectShell 自动探测最佳可用 shell：优先用户设置
/// （terminal.shell_path，存在时），否则按 git-bash → wsl → cmd 探测。
#[tauri::command]
pub fn terminal_detect_shell(repo: Option<String>) -> Result<TerminalDetectResult, String> {
    let _ = crate::state::resolve(repo); // 应用级命令，repo 仅为适配层统一注入
    // 用户配置优先
    if let Ok(cfg) = Settings::load() {
        let sp = cfg.terminal.shell_path;
        if !sp.is_empty() && Path::new(&sp).exists() {
            let is_bash = is_git_bash(&sp);
            return Ok(TerminalDetectResult { path: sp, is_git_bash: is_bash });
        }
    }
    let (path, is_bash) = detect_shell();
    if path.is_empty() {
        return Err("未找到可用的 shell，请在设置中手动指定".into());
    }
    Ok(TerminalDetectResult { path, is_git_bash: is_bash })
}
