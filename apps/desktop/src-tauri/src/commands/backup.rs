//! backup 域命令：整库克隆备份 + 备份文件浏览、仓库打包归档，以及
//! log 域（提交列表 / 统计 / 时间机器文件浏览）。
//!
//! 移植自 git-mate `gui/internal/backup`（backup.go + archive.go）与
//! `gui/internal/log`（log.go）。git 子进程统一走本文件的 `git_exec` /
//! `git_run`（隐藏窗口 + 超时 + stdout/stderr 分离捕获）；归档目录沿用
//! byte-git 应用目录布局（AppPaths 的 downloads/backups，对应 Go 的
//! config.GetDownloadDir/GetAppDir）。
//!
//! Go 侧 backup 绑定方法无 repo 参数（尾部追加 `repo: Option<String>`，
//! 经 crate::state::resolve 解析）；log 域 LogFetch/LogFetchByDate/LogDiff
//! 在 Go 侧首参即 repo，Rust 侧保持首参 `repo: String`（空串回落当前仓库）。

use byte_git_core::util::AppPaths;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// Go 侧未设超时的命令：这里给一个宽松上限，防止异常悬挂。
const LONG_TIMEOUT: Duration = Duration::from_secs(300);
/// log 域单次 git 查询超时（对应 Go 的 10s context）。
const LOG_TIMEOUT: Duration = Duration::from_secs(10);
/// git archive 上限（对应 Go 的 10 分钟 context）。
const ARCHIVE_TIMEOUT: Duration = Duration::from_secs(600);
/// 整库克隆上限（对应 Go 的 30 分钟 context）。
const CLONE_TIMEOUT: Duration = Duration::from_secs(1800);

/// 送给前端的内容上限（对应 Go core.MaxSecretFileSize）。
const MAX_TEXT_CONTENT: usize = 1 << 20; // 1 MB

/// 与 Go GUI 一致：无仓库上下文的命令统一接收可选 repo 参数；
/// 缺省回落到当前仓库状态（state.rs，对应 Core.CurrentRepo），最终 "."。
fn repo_path(repo: Option<String>) -> PathBuf {
    crate::state::resolve(repo)
}

// ---------------------------------------------------------------------------
// 基础设施（同 gitops.rs：隐藏窗口 + 超时 + stdout/stderr 分离捕获，
// 覆盖 Go 的 Output() / CombinedOutput() 及 stderr 文案判别用法）
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

/// git 子进程执行结果。
struct GitOutput {
    /// 退出码是否为 0。
    success: bool,
    /// 是否因超时被强制终止。
    timed_out: bool,
    stdout: String,
    stderr: String,
}

impl GitOutput {
    /// 合并输出（对应 Go 的 CombinedOutput）。
    fn combined(&self) -> String {
        format!("{}{}", self.stdout, self.stderr)
    }
}

/// 执行任意 `git <args...>`（不带 -C；clone 等全局命令用）：隐藏窗口 + 超时。
fn git_run(args: &[&str], timeout: Duration) -> GitOutput {
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
            return GitOutput {
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
                return GitOutput {
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
                    return GitOutput {
                        success: false,
                        timed_out: true,
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
                return GitOutput {
                    success: false,
                    timed_out: false,
                    stdout: String::new(),
                    stderr: format!("等待 git 进程失败: {e}"),
                }
            }
        }
    }
}

/// 在仓库目录执行 `git -C <repo> <args...>`。
fn git_exec(repo: &Path, args: &[&str], timeout: Duration) -> GitOutput {
    let repo_str = repo.to_string_lossy();
    let mut full: Vec<&str> = Vec::with_capacity(args.len() + 2);
    full.push("-C");
    full.push(repo_str.as_ref());
    full.extend_from_slice(args);
    git_run(&full, timeout)
}

/// 在独立线程执行阻塞任务（git clone / git archive），命令保持 async 不冻结 UI。
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
// 共享 DTO（json tag 与 Go 结构体逐一对齐）
// ---------------------------------------------------------------------------

/// archive.Options 的 GUI 镜像。
#[derive(Debug, Clone, Default, Deserialize)]
pub struct ArchiveOptionsDto {
    /// 归档 ref（默认 HEAD）
    #[serde(rename = "ref", default)]
    pub ref_: String,
    /// 可选路径过滤；空 = 全部
    #[serde(default)]
    pub paths: Vec<String>,
    /// "tar.gz" | "zip" | "tar"（默认 tar.gz）
    #[serde(default)]
    pub format: String,
    /// 输出文件；空 = 默认路径
    #[serde(default)]
    pub output: String,
    /// "standard" | "full"（默认 standard）
    #[serde(default)]
    pub mode: String,
}

/// archive.Result 的 GUI 镜像。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArchiveResultDto {
    pub output_path: String,
    pub size: i64,
    pub file_count: i64,
    pub mode: String,
}

/// archive.Entry 的 GUI 镜像（一个归档文件）。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArchiveEntryDto {
    pub name: String,
    pub size: i64,
    /// "YYYY-MM-DD HH:MM:SS"
    pub mod_time: String,
    pub path: String,
}

/// backup.BackupEntry 的 GUI 镜像（一个整库克隆备份目录）。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupEntryDto {
    /// 目录名 "20260624-150000-myrepo"
    pub name: String,
    /// "2026-06-24 15:00:00"
    pub timestamp: String,
    pub path: String,
    /// 递归总字节数
    pub size: i64,
}

/// core.FileNode 的 GUI 镜像（备份浏览树节点）。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileNodeDto {
    pub name: String,
    /// 相对备份根的 "/" 分隔路径
    pub path: String,
    pub is_dir: bool,
    pub size: i64,
}

/// core.FileResult 的 GUI 镜像（历史 / 备份文件内容）。
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileResultDto {
    pub content: String,
    pub lines: i64,
    pub binary: bool,
    pub not_found: bool,
    /// 图片内联 data URL（前端直接 <img> 展示；仅图片填）
    #[serde(default)]
    pub data_url: String,
}

impl FileResultDto {
    fn not_found() -> Self {
        Self { not_found: true, ..Default::default() }
    }
}

/// LogCommit：GUI 日志条目（含 message 正文与 conventional type/scope）。
#[derive(Debug, Clone, Serialize)]
pub struct LogCommitDto {
    pub hash: String,
    pub author: String,
    /// RFC3339
    pub date: String,
    pub subject: String,
    /// 提交信息正文（subject 行之后；无为空）
    pub body: String,
    #[serde(rename = "type")]
    pub ty: String,
    pub scope: String,
}

/// CommitFileTreeEntry：某提交下的一个文件路径（扁平列表，前端自建树）。
#[derive(Debug, Clone, Serialize)]
pub struct CommitFileTreeEntryDto {
    pub path: String,
}

/// CommitStatEntry：提交中变更的一个文件及其行数增删。
#[derive(Debug, Clone, Serialize)]
pub struct CommitStatEntryDto {
    pub file: String,
    pub added: i64,
    pub deleted: i64,
    /// A/M/D
    pub status: String,
}

/// FileDiffResult：某文件在提交前 / 后的内容（并排 diff 视图）。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileDiffResultDto {
    /// 父版本内容（新增文件为空）
    pub before: String,
    /// 当前版本内容（删除文件为空）
    pub after: String,
    /// A/M/D
    pub status: String,
    pub binary: bool,
    /// 父版本不存在（纯新增）
    pub not_found: bool,
}

// ---------------------------------------------------------------------------
// 私有辅助
// ---------------------------------------------------------------------------

/// filepath.Abs 的词法等价：相对路径拼当前目录（不归一符号链接，避免
/// Windows canonicalize 产生 \\?\ 前缀）。
fn abs_path(p: &Path) -> PathBuf {
    if p.is_absolute() {
        p.to_path_buf()
    } else {
        std::env::current_dir()
            .map(|cwd| cwd.join(p))
            .unwrap_or_else(|_| p.to_path_buf())
    }
}

/// 仓库末段目录名（对应 Go repoName：先取绝对路径再 Base）。
fn repo_base_name(repo: &Path) -> String {
    abs_path(repo)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "repo".to_string())
}

/// filepath.Clean 的词法等价：折叠 "." / ".."（不触碰文件系统）。
fn clean_path(p: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for comp in p.components() {
        match comp {
            std::path::Component::ParentDir => {
                // 已到根（盘符前缀等）无法继续弹出时保留 ".."
                if !out.pop() {
                    out.push("..");
                }
            }
            std::path::Component::CurDir => {}
            other => out.push(other.as_os_str()),
        }
    }
    if out.as_os_str().is_empty() {
        out.push(".");
    }
    out
}

/// 越界防护：target 必须位于 root 内（含 root 本身）。
/// Go 用 HasPrefix(Clean(target), Clean(root))（存在 "root-xxx" 前缀误判），
/// 这里按目录组件边界判断，合法输入行为一致且更严。
fn ensure_within(root: &Path, target: &Path) -> Result<(), String> {
    let root_c = clean_path(root);
    let target_c = clean_path(target);
    if target_c == root_c || target_c.starts_with(&root_c) {
        return Ok(());
    }
    Err("路径越界".into())
}

/// 递归统计目录总大小（尽力而为，错误忽略；对应 Go dirSize）。
fn dir_size(path: &Path) -> i64 {
    fn walk(p: &Path, total: &mut i64) {
        if let Ok(rd) = std::fs::read_dir(p) {
            for e in rd.flatten() {
                match e.file_type() {
                    Ok(ft) if ft.is_dir() => walk(&e.path(), total),
                    Ok(_) => {
                        if let Ok(md) = e.metadata() {
                            *total += md.len() as i64;
                        }
                    }
                    Err(_) => {}
                }
            }
        }
    }
    let mut total = 0;
    walk(path, &mut total);
    total
}

/// SystemTime → "YYYY-MM-DD HH:MM:SS"（本地时区，Go 的 "2006-01-02 15:04:05"）。
fn format_system_time(t: Option<std::time::SystemTime>) -> String {
    match t {
        Some(t) => {
            chrono::DateTime::<chrono::Local>::from(t).format("%Y-%m-%d %H:%M:%S").to_string()
        }
        None => String::new(),
    }
}

/// formatBackupName："20260624-150000-myrepo" → "2026-06-24 15:00:00"；
/// 解析失败原样返回。
fn format_backup_name(name: &str) -> String {
    let parts: Vec<&str> = name.splitn(3, '-').collect();
    if parts.len() < 2 || parts[0].len() != 8 || parts[1].len() != 6 {
        return name.to_string();
    }
    let (d, t) = (parts[0], parts[1]);
    format!(
        "{}-{}-{} {}:{}:{}",
        &d[0..4], &d[4..6], &d[6..8],
        &t[0..2], &t[2..4], &t[4..6]
    )
}

/// 共享的内容规整：二进制探测（前 8192 字节 NUL）+ 1MB 截断 + 行数。
/// 对应 Go ReadFileAtCommit / ReadWorkingTreeFile / ShowFileAt 的尾部逻辑。
fn file_result_from_bytes(raw: &[u8]) -> FileResultDto {
    let scan = &raw[..raw.len().min(8192)];
    if scan.contains(&0) {
        return FileResultDto { binary: true, ..Default::default() };
    }
    let mut content = String::from_utf8_lossy(raw).into_owned();
    let mut truncated = false;
    if content.len() > MAX_TEXT_CONTENT {
        // 按字节截断（对齐 Go），再以 lossy 修复可能切断的 UTF-8 边界
        content = String::from_utf8_lossy(&content.as_bytes()[..MAX_TEXT_CONTENT]).into_owned();
        truncated = true;
    }
    let lines = content.matches('\n').count() as i64 + 1;
    if truncated {
        content.push_str("\n\n…（文件超过 1MB，已截断显示）");
    }
    FileResultDto { content, lines, ..Default::default() }
}

/// numstat 数值解析："-"/非数字 → 0（对应 Go Sscanf 容错）。
fn parse_num(s: &str) -> i64 {
    if s == "-" {
        return 0;
    }
    s.trim().parse::<i64>().unwrap_or(0)
}

// ===========================================================================
// Archive（archive.go）
// ===========================================================================

fn ext_for(format: &str) -> &'static str {
    match format {
        "zip" => ".zip",
        "tar" => ".tar",
        _ => ".tar.gz",
    }
}

/// DefaultArchivePath：downloads/archive/<repo>/<ts>-<ref><ext>
/// （时间戳前缀，重复归档同一 ref/mode 不互相覆盖）。
fn default_archive_path(repo: &Path, ref_: &str, format: &str) -> Result<PathBuf, String> {
    let name = repo_base_name(repo);
    let safe_ref = ref_.replace('/', "_");
    let dir = AppPaths::new()
        .map_err(|e| e.to_string())?
        .downloads_dir()
        .join("archive")
        .join(&name);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let ts = chrono::Local::now().format("%Y%m%dT%H%M%S");
    Ok(dir.join(format!("{ts}-{safe_ref}{}", ext_for(format))))
}

/// ArchiveRun executes an archive operation for the current repo.
/// （长阻塞：git archive 上限 10 分钟，async + std 线程避免冻结 UI。）
#[tauri::command]
pub async fn archive_run(
    dto: ArchiveOptionsDto,
    repo: Option<String>,
) -> Result<ArchiveResultDto, String> {
    let repo = repo_path(repo);
    run_blocking(move || archive_run_impl(&repo, dto))?
}

fn archive_run_impl(repo: &Path, dto: ArchiveOptionsDto) -> Result<ArchiveResultDto, String> {
    let mode = dto.mode.as_str();
    if !(mode.is_empty() || mode == "standard" || mode == "full") {
        return Err(format!("未知模式: {mode} (standard/full)"));
    }
    if mode == "full" {
        return archive_full_impl(repo, dto);
    }

    let ref_ = if dto.ref_.is_empty() { "HEAD" } else { dto.ref_.as_str() };
    let format = if dto.format.is_empty() { "tar.gz" } else { dto.format.as_str() };

    let out = if dto.output.is_empty() {
        default_archive_path(repo, ref_, format)?
    } else {
        PathBuf::from(&dto.output)
    };
    if let Some(parent) = out.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
    }

    // git archive 直接产出 tar.gz/zip/tar（Go 经管道 + gzip.Writer，产物等价）
    let format_arg = match format {
        "zip" => "--format=zip",
        "tar" => "--format=tar",
        _ => "--format=tar.gz",
    };
    let output_arg = format!("--output={}", out.display());
    let mut args: Vec<String> = vec!["archive".into(), format_arg.into(), output_arg, ref_.into()];
    args.extend(dto.paths.iter().cloned());
    let refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    let res = git_exec(repo, &refs, ARCHIVE_TIMEOUT);
    if res.timed_out {
        return Err(format!("打包操作超时 (超过 10 分钟): {}", res.combined().trim()));
    }
    if !res.success {
        return Err(format!("git archive 失败: {}", res.combined().trim()));
    }
    // statResult：大小（尽力而为）；文件数 Go 侧恒为 0
    let size = std::fs::metadata(&out).map(|m| m.len() as i64).unwrap_or(0);
    Ok(ArchiveResultDto {
        output_path: out.display().to_string(),
        size,
        file_count: 0,
        mode: "standard".into(),
    })
}

/// full 模式（对齐 Go archiveFull）：tar/zip 直接打包整个工作区（含 .git），
/// 不依赖 git archive；尊重 paths 过滤器（相对路径前缀匹配）。
/// zip 支持见 zip_write。
fn archive_full_impl(repo: &Path, dto: ArchiveOptionsDto) -> Result<ArchiveResultDto, String> {
    use std::io::Write;

    let format = if dto.format.is_empty() { "tar.gz" } else { dto.format.as_str() };
    let ref_ = if dto.ref_.is_empty() { "HEAD" } else { dto.ref_.as_str() };
    let out = if dto.output.is_empty() {
        default_archive_path(repo, &format!("{ref_}-full"), format)?
    } else {
        PathBuf::from(&dto.output)
    };
    if let Some(parent) = out.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
    }
    let abs = abs_path(repo);

    // 收集条目（含 .git；paths 过滤器 = 相对路径前缀匹配）
    let mut entries: Vec<(PathBuf, PathBuf)> = Vec::new(); // (绝对路径, 相对路径)
    collect_files(&abs, &abs, &dto.paths, &mut entries);

    let mut file_count: i64 = 0;
    if format == "zip" {
        let mut f = std::fs::File::create(&out).map_err(|e| format!("创建归档文件失败: {e}"))?;
        file_count = zip_write(&mut f, &entries)?;
    } else {
        let f = std::fs::File::create(&out).map_err(|e| format!("创建归档文件失败: {e}"))?;
        let mut build = |w: Box<dyn Write>| -> Result<(), String> {
        let mut tar = tar::Builder::new(w);
        for (abs_p, rel_p) in &entries {
            let meta = std::fs::metadata(abs_p).map_err(|e| format!("读取 {}: {e}", rel_p.display()))?;
            let mtime = meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
                .unwrap_or(0);
            let mut header = tar::Header::new_gnu();
            header.set_size(meta.len());
            header.set_mode(if meta.is_dir() { 0o755 } else { 0o644 });
            header.set_mtime(mtime);
            header.set_cksum();
            let rel_str = rel_p.display().to_string().replace('\\', "/");
            if meta.is_dir() {
                header.set_entry_type(tar::EntryType::Directory);
                tar.append_data(&mut header, format!("{rel_str}/"), std::io::empty())
            } else {
                header.set_entry_type(tar::EntryType::Regular);
                let src = std::fs::File::open(abs_p).map_err(|e| format!("读取 {}: {e}", rel_p.display()))?;
                tar.append_data(&mut header, rel_str, src)
            }
            .map_err(|e| format!("写入归档失败: {e}"))?;
            if meta.is_file() {
                file_count += 1;
            }
        }
        tar.finish().map_err(|e| format!("归档收尾失败: {e}"))
    };

    match format {
        "tar" => build(Box::new(f))?,
        _ => build(Box::new(flate2::write::GzEncoder::new(f, flate2::Compression::default())))?,
    }
    }

    let size = std::fs::metadata(&out).map(|m| m.len() as i64).unwrap_or(0);
    Ok(ArchiveResultDto {
        output_path: out.display().to_string(),
        size,
        file_count,
        mode: "full".into(),
    })
}

/// 手写 zip 写入器：local file header + 数据 + central directory + EOCD。
/// 文件走 DEFLATE（flate2，方法 8），目录走 STORED（方法 0，零长度）；
/// 名称置 UTF-8 位（0x0800），路径 `/` 分隔，目录名尾 `/`。
/// 绕开 zip crate 2.3/2.4 的 deflate-flate2 feature 打包缺陷。
/// 返回文件数（目录不计）。
fn zip_write<W: std::io::Write>(w: &mut W, entries: &[(PathBuf, PathBuf)]) -> Result<i64, String> {
    use std::io::Write as _;

    use chrono::{Datelike, Timelike};
    let now = chrono::Local::now();
    let year = now.year().max(1980);
    let dos_time = ((now.hour() as u16) << 11) | ((now.minute() as u16) << 5) | (now.second() as u16 / 2);
    let dos_date = (((year - 1980) as u16) << 9) | ((now.month() as u16) << 5) | now.day() as u16;

    let mut offset: u32 = 0;
    let mut central: Vec<u8> = Vec::new();
    let mut total_entries: u16 = 0;
    let mut file_count: i64 = 0;

    let wr = |w: &mut W, buf: &[u8]| -> Result<(), String> {
        w.write_all(buf).map_err(|e| format!("写入归档失败: {e}"))
    };

    for (abs_p, rel_p) in entries {
        let meta = std::fs::metadata(abs_p).map_err(|e| format!("读取 {}: {e}", rel_p.display()))?;
        let is_dir = meta.is_dir();
        let mut name = rel_p.display().to_string().replace('\\', "/");
        let (method, payload, crc32v, uncomp) = if is_dir {
            name.push('/');
            (0u16, Vec::new(), 0u32, 0u32)
        } else {
            let raw = std::fs::read(abs_p).map_err(|e| format!("读取 {}: {e}", rel_p.display()))?;
            let mut crc = flate2::Crc::new();
            crc.update(&raw);
            let mut enc = flate2::write::DeflateEncoder::new(
                Vec::new(),
                flate2::Compression::default(),
            );
            enc.write_all(&raw).map_err(|e| format!("压缩失败: {e}"))?;
            let comp = enc.finish().map_err(|e| format!("压缩失败: {e}"))?;
            let uncomp = raw.len() as u32;
            (8u16, comp, crc.sum(), uncomp)
        };
        let comp_len = payload.len() as u32;

        // ---- local file header（PK\x03\x04）----
        let lfh_off = offset;
        let mut lfh: Vec<u8> = Vec::with_capacity(30 + name.len());
        lfh.extend_from_slice(&0x0403_4b50u32.to_le_bytes()); // 签名
        lfh.extend_from_slice(&20u16.to_le_bytes()); // 版本需求 2.0
        lfh.extend_from_slice(&0x0800u16.to_le_bytes()); // UTF-8 名称
        lfh.extend_from_slice(&method.to_le_bytes());
        lfh.extend_from_slice(&dos_time.to_le_bytes());
        lfh.extend_from_slice(&dos_date.to_le_bytes());
        lfh.extend_from_slice(&crc32v.to_le_bytes());
        lfh.extend_from_slice(&comp_len.to_le_bytes());
        lfh.extend_from_slice(&uncomp.to_le_bytes());
        lfh.extend_from_slice(&(name.len() as u16).to_le_bytes());
        lfh.extend_from_slice(&0u16.to_le_bytes()); // extra 长度
        lfh.extend_from_slice(name.as_bytes());
        wr(w, &lfh)?;
        wr(w, &payload)?;
        offset += (lfh.len() as u32).wrapping_add(comp_len);

        // ---- central directory 记录（PK\x01\x02，先攒后写）----
        let ext_attrs: u32 = if is_dir { 0x10 } else { 0o100_644 << 16 };
        let mut cd: Vec<u8> = Vec::with_capacity(46 + name.len());
        cd.extend_from_slice(&0x0201_4b50u32.to_le_bytes());
        cd.extend_from_slice(&20u16.to_le_bytes()); // 制作版本
        cd.extend_from_slice(&20u16.to_le_bytes()); // 需求版本
        cd.extend_from_slice(&0x0800u16.to_le_bytes());
        cd.extend_from_slice(&method.to_le_bytes());
        cd.extend_from_slice(&dos_time.to_le_bytes());
        cd.extend_from_slice(&dos_date.to_le_bytes());
        cd.extend_from_slice(&crc32v.to_le_bytes());
        cd.extend_from_slice(&comp_len.to_le_bytes());
        cd.extend_from_slice(&uncomp.to_le_bytes());
        cd.extend_from_slice(&(name.len() as u16).to_le_bytes());
        cd.extend_from_slice(&0u16.to_le_bytes()); // extra
        cd.extend_from_slice(&0u16.to_le_bytes()); // comment
        cd.extend_from_slice(&0u16.to_le_bytes()); // 起始盘号
        cd.extend_from_slice(&0u16.to_le_bytes()); // 内部属性
        cd.extend_from_slice(&ext_attrs.to_le_bytes());
        cd.extend_from_slice(&lfh_off.to_le_bytes());
        cd.extend_from_slice(name.as_bytes());
        central.extend_from_slice(&cd);

        total_entries = total_entries.wrapping_add(1);
        if !is_dir {
            file_count += 1;
        }
    }

    // ---- central directory + EOCD（PK\x05\x06）----
    let cd_off = offset;
    wr(w, &central)?;
    let mut eocd: Vec<u8> = Vec::with_capacity(22);
    eocd.extend_from_slice(&0x0605_4b50u32.to_le_bytes());
    eocd.extend_from_slice(&0u16.to_le_bytes()); // 当前盘
    eocd.extend_from_slice(&0u16.to_le_bytes()); // central 目录起始盘
    eocd.extend_from_slice(&total_entries.to_le_bytes());
    eocd.extend_from_slice(&total_entries.to_le_bytes());
    eocd.extend_from_slice(&(central.len() as u32).to_le_bytes());
    eocd.extend_from_slice(&cd_off.to_le_bytes());
    eocd.extend_from_slice(&0u16.to_le_bytes()); // comment 长度
    wr(w, &eocd)?;
    Ok(file_count)
}

/// 递归收集（含隐藏目录与 .git；paths 空 = 全收；否则按相对路径前缀过滤）。
fn collect_files(root: &Path, dir: &Path, filter: &[String], out: &mut Vec<(PathBuf, PathBuf)>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let abs_p = e.path();
        let Ok(rel) = abs_p.strip_prefix(root) else { continue };
        let rel_str = rel.display().to_string().replace('\\', "/");
        if !filter.is_empty() && !filter.iter().any(|p| rel_str.starts_with(&p.replace('\\', "/"))) {
            // 不在过滤器内：文件与目录整枝跳过（对齐 Go pathfilter 前缀语义）
            continue;
        }
        out.push((abs_p.clone(), rel.to_path_buf()));
        if abs_p.is_dir() {
            collect_files(root, &abs_p, filter, out);
        }
    }
}

/// ArchiveExportToDir：把 ref 处的完整源码树导出到 targetDir（不压缩、
/// 保留目录结构；git archive --format=tar 流式解包）。
/// （长阻塞：上限 10 分钟，async + std 线程。）
#[tauri::command]
pub async fn archive_export_to_dir(
    ref_: String,
    target_dir: String,
    repo: Option<String>,
) -> Result<ArchiveResultDto, String> {
    let repo = repo_path(repo);
    run_blocking(move || {
        let ref_ = if ref_.is_empty() { "HEAD".to_string() } else { ref_ };
        let target = PathBuf::from(&target_dir);
        std::fs::create_dir_all(&target).map_err(|e| format!("创建导出目录失败: {e}"))?;

        // git archive --format=tar <ref>，stdout 为 tar 流
        let mut cmd = std::process::Command::new("git");
        cmd.args(["archive", "--format=tar", ref_.as_str()])
            .current_dir(&repo)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());
        hide_console(&mut cmd);
        let mut child = cmd.spawn().map_err(|e| format!("启动 git archive 失败: {e}"))?;
        let stdout = child.stdout.take().unwrap();
        let target_disp = target.display().to_string();
        let target_in = target.clone();
        let unpack = std::thread::spawn(move || -> Result<i64, String> {
            let mut ar = tar::Archive::new(stdout);
            let mut count = 0i64;
            for entry in ar.entries().map_err(|e| format!("解包失败: {e}"))? {
                let mut entry = entry.map_err(|e| format!("解包失败: {e}"))?;
                if entry.header().entry_type().is_file() {
                    count += 1;
                }
                entry.unpack_in(&target_in).map_err(|e| format!("解包失败: {e}"))?;
            }
            Ok(count)
        });
        let status = child.wait().map_err(|e| format!("等待 git archive 失败: {e}"))?;
        let count = unpack.join().map_err(|_| "解包线程异常".to_string())??;
        if !status.success() {
            return Err("git archive 失败（ref 无效或仓库为空）".into());
        }
        let size = dir_size(&target);
        Ok(ArchiveResultDto {
            output_path: target_disp,
            size,
            file_count: count,
            mode: "standard".into(),
        })
    })?
}

/// ArchivePickDir：原生目录选择框（tauri-plugin-dialog；取消返回空串）。
#[tauri::command]
pub async fn archive_pick_dir(app: tauri::AppHandle) -> Result<String, String> {
    use tauri_plugin_dialog::DialogExt;
    let (tx, rx) = std::sync::mpsc::channel::<Option<String>>();
    app.dialog().file().pick_folder(move |dir| {
        let path = dir
            .and_then(|d| d.as_path().map(|p| p.display().to_string()))
            .unwrap_or_default();
        let _ = tx.send(Some(path));
    });
    let picked = std::thread::spawn(move || rx.recv().ok().flatten())
        .join()
        .map_err(|_| "对话框线程异常".to_string())?
        .unwrap_or_default();
    Ok(picked)
}

/// ArchiveList lists all archive files for the current repo (newest first).
#[tauri::command]
pub fn archive_list(repo: Option<String>) -> Result<Vec<ArchiveEntryDto>, String> {
    let repo = repo_path(repo);
    let dir = AppPaths::new()
        .map_err(|e| e.to_string())?
        .downloads_dir()
        .join("archive")
        .join(repo_base_name(&repo));
    let entries = match std::fs::read_dir(&dir) {
        Ok(rd) => rd,
        Err(_) => return Ok(Vec::new()), // 还没有归档——不算错误
    };
    let mut result = Vec::new();
    for e in entries.flatten() {
        if e.path().is_dir() {
            continue;
        }
        let Ok(info) = e.metadata() else { continue };
        let name = e.file_name().to_string_lossy().into_owned();
        result.push(ArchiveEntryDto {
            name: name.clone(),
            size: info.len() as i64,
            mod_time: format_system_time(info.modified().ok()),
            path: dir.join(name).display().to_string(),
        });
    }
    // 新的在前（文件名时间戳前缀，字典序反转）
    result.sort_by(|a, b| a.name.cmp(&b.name));
    result.reverse();
    Ok(result)
}

/// ArchiveDelete removes one archive file by name.
#[tauri::command]
pub fn archive_delete(name: String, repo: Option<String>) -> Result<(), String> {
    let repo = repo_path(repo);
    let dir = AppPaths::new()
        .map_err(|e| e.to_string())?
        .downloads_dir()
        .join("archive")
        .join(repo_base_name(&repo));
    let target = dir.join(&name);
    if !target.exists() {
        return Err(format!("打包文件不存在: {name}"));
    }
    std::fs::remove_file(&target).map_err(|e| e.to_string())
}

// ===========================================================================
// Full Backup（backup.go）
// ===========================================================================

/// 备份根目录：AppDir/backups/<repoName>（对应 Go GetAppDir()/backups）。
fn backups_repo_dir(repo: &Path) -> Result<PathBuf, String> {
    Ok(AppPaths::new()
        .map_err(|e| e.to_string())?
        .backups_dir()
        .join(repo_base_name(repo)))
}

/// FullBackupList lists all full-clone backups for the current repo (newest first).
#[tauri::command]
pub fn full_backup_list(repo: Option<String>) -> Result<Vec<BackupEntryDto>, String> {
    let repo = repo_path(repo);
    let backup_dir = backups_repo_dir(&repo)?;
    // 旧镜像模型（backups/<repo>/ 即裸仓库）自动转换——否则 refs/objects 等会被当成备份条目
    byte_git_core::backup::migrate_legacy_mirror(&backup_dir);
    let entries = match std::fs::read_dir(&backup_dir) {
        Ok(rd) => rd,
        Err(_) => return Ok(Vec::new()), // 还没有备份——不算错误
    };
    let mut result = Vec::new();
    for e in entries.flatten() {
        if !e.path().is_dir() {
            continue;
        }
        let name = e.file_name().to_string_lossy().into_owned();
        let full = backup_dir.join(&name);
        let ts = format_backup_name(&name);
        result.push(BackupEntryDto {
            name,
            timestamp: ts,
            path: full.display().to_string(),
            size: dir_size(&full),
        });
    }
    // 新的在前（目录名时间戳前缀，字典序反转）
    result.sort_by(|a, b| a.name.cmp(&b.name));
    result.reverse();
    Ok(result)
}

/// FullBackupCreate clones the entire repo into the AppDir backup location.
/// （慢操作：git clone 上限 30 分钟，async + std 线程。）
#[tauri::command]
pub async fn full_backup_create(repo: Option<String>) -> Result<BackupEntryDto, String> {
    let repo = repo_path(repo);
    run_blocking(move || {
        let abs = abs_path(&repo);
        let repo_name = abs
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .filter(|n| !n.is_empty() && n != ".")
            .unwrap_or_else(|| "repo".to_string());
        let ts = chrono::Local::now().format("%Y%m%d-%H%M%S");
        let backup_name = format!("{ts}-{repo_name}");
        let backup_dir = AppPaths::new()
            .map_err(|e| e.to_string())?
            .backups_dir()
            .join(&repo_name);
        std::fs::create_dir_all(&backup_dir)
            .map_err(|e| format!("创建备份目录失败: {e}"))?;
        let dest = backup_dir.join(&backup_name);
        let src_arg = abs.display().to_string();
        let dst_arg = dest.display().to_string();
        let res = git_run(
            &["clone", "--no-hardlinks", src_arg.as_str(), dst_arg.as_str()],
            CLONE_TIMEOUT,
        );
        if res.timed_out {
            return Err("克隆超时（超过 30 分钟）— 仓库过大或网络异常".into());
        }
        if !res.success {
            return Err(format!("克隆失败: {}", res.combined().trim()));
        }
        Ok(BackupEntryDto {
            name: backup_name.clone(),
            timestamp: format_backup_name(&backup_name),
            path: dest.display().to_string(),
            size: dir_size(&dest),
        })
    })?
}

/// FullBackupDelete removes one full-clone backup by name.
#[tauri::command]
pub fn full_backup_delete(name: String, repo: Option<String>) -> Result<(), String> {
    let repo = repo_path(repo);
    let target = backups_repo_dir(&repo)?.join(&name);
    if !target.exists() {
        return Err(format!("备份不存在: {name}"));
    }
    std::fs::remove_dir_all(&target).map_err(|e| e.to_string())
}

/// 顶层永不出示的目录（VCS 内部数据，非用户代码）。
// （根层 VCS 目录过滤已由共享 read_dir_nodes 统一处理）

/// FullBackupFileList lists the immediate children of a directory inside a
/// full-clone backup（subPath="" 为备份根；目录懒加载，前端点击展开）。
#[tauri::command]
pub fn full_backup_file_list(
    backup_name: String,
    sub_path: String,
    repo: Option<String>,
) -> Result<Vec<FileNodeDto>, String> {
    let repo = repo_path(repo);
    let backup_root = backups_repo_dir(&repo)?.join(&backup_name);
    let target = if sub_path.is_empty() {
        backup_root.clone()
    } else {
        backup_root.join(&sub_path)
    };
    ensure_within(&backup_root, &target)?;

    // 共享 ReadDirNodes（上游统一：.git/node_modules/.DS_Store 全层过滤 + symlink 跟随）
    let nodes = super::repo::read_dir_nodes(&target, &sub_path)
        .map_err(|e| format!("读取目录失败: {e}"))?;
    Ok(nodes
        .into_iter()
        .map(|n| FileNodeDto { name: n.name, path: n.path, is_dir: n.is_dir, size: n.size })
        .collect())
}

/// FullBackupFileRead reads a file's content from a full-clone backup.
/// （二进制探测 + 1MB 截断，目录节点统一按 NotFound 返回。）
#[tauri::command]
pub fn full_backup_file_read(
    backup_name: String,
    file_path: String,
    repo: Option<String>,
) -> Result<FileResultDto, String> {
    let repo = repo_path(repo);
    let backup_root = backups_repo_dir(&repo)?.join(&backup_name);
    let full = backup_root.join(&file_path);
    ensure_within(&backup_root, &full)?;

    let data = match std::fs::read(&full) {
        Ok(d) => d,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Ok(FileResultDto::not_found());
        }
        Err(e) => return Err(format!("读取文件失败: {e}")),
    };
    // 目录节点也会走到这里（树对文件/目录都发 open）：统一按 NotFound 返回
    if full.is_dir() {
        return Ok(FileResultDto::not_found());
    }
    Ok(file_result_from_bytes(&data))
}

// ===========================================================================
// Log（log.go）
// ===========================================================================

/// 日志格式：hash NUL 日期 NUL 作者 NUL 完整消息 NUL，记录以 NUL+\n 分隔
/// （与 Go FetchLogCommits 一致，%B 支持多行消息）。
const LOG_FORMAT: &str = "%h%x00%ai%x00%an%x00%B%x00";

/// git 日期（%ai："2006-01-02 15:04:05 -0700"）→ RFC3339；
/// 无时区布局按 UTC 处理；解析失败返回 Go 零值时间的格式化结果。
fn to_rfc3339(date_str: &str) -> String {
    if let Ok(dt) = chrono::DateTime::parse_from_str(date_str, "%Y-%m-%d %H:%M:%S %z") {
        return dt.to_rfc3339();
    }
    if let Ok(naive) = chrono::NaiveDateTime::parse_from_str(date_str, "%Y-%m-%d %H:%M:%S") {
        return naive.format("%Y-%m-%dT%H:%M:%SZ").to_string();
    }
    "0001-01-01T00:00:00Z".to_string()
}

/// conventional commit 解析（等价 Go convCommitRe）：
/// `^(feat|fix|refactor|perf|docs|style|test|chore|build|ci)(?:\(([^)]+)\))?(!)?:\s*(.+)$`
/// 命中返回 (type, scope, 去掉类型前缀的 subject)。
fn parse_conventional(subject: &str) -> Option<(String, String, String)> {
    const TYPES: &[&str] = &[
        "feat", "fix", "refactor", "perf", "docs", "style", "test", "chore", "build", "ci",
    ];
    let colon = subject.find(':')?;
    let mut head = &subject[..colon];
    let desc = subject[colon + 1..].trim_start();
    if desc.is_empty() {
        return None; // 正则要求 (.+) 非空
    }
    // 可选 "!"（break 标记，Go 侧仅捕获不使用）
    if let Some(h) = head.strip_suffix('!') {
        head = h;
    }
    // 可选 "(scope)"：type + '(' + [^)]+ + ')' 且必须吃满整个 head
    let (ty, scope) = if let Some(open) = head.find('(') {
        let t = &head[..open];
        let rest = &head[open + 1..];
        let close = rest.find(')')?;
        if close == 0 || !rest[close + 1..].is_empty() {
            return None; // scope 为空 或 ')' 后还有余量
        }
        (t, &rest[..close])
    } else {
        (head, "")
    };
    if !TYPES.contains(&ty) {
        return None;
    }
    Some((ty.to_string(), scope.to_string(), desc.to_string()))
}

/// 解析 git log --format=%h%x00... 输出为 GUI LogCommit 列表。
fn parse_log_output(out: &str) -> Vec<LogCommitDto> {
    let raw = out.trim_end_matches('\n');
    let mut result = Vec::new();
    for rec in raw.split("\x00\n") {
        let rec = rec.trim();
        if rec.is_empty() {
            continue;
        }
        // 每条记录：hash NUL 日期 NUL 作者 NUL 消息 NUL
        let parts: Vec<&str> = rec.split('\x00').collect();
        if parts.len() < 3 {
            continue;
        }
        let (hash, date_str, author) = (parts[0], parts[1], parts[2]);
        let full_msg = if parts.len() >= 4 {
            parts[3].trim_end_matches('\n')
        } else {
            ""
        };
        // 完整消息 → 首行 subject，其余为 body
        let (subject, body) = match full_msg.find('\n') {
            Some(idx) => (&full_msg[..idx], full_msg[idx + 1..].trim_start_matches('\n')),
            None => (full_msg, ""),
        };
        let mut e = LogCommitDto {
            hash: hash.to_string(),
            author: author.to_string(),
            date: to_rfc3339(date_str),
            subject: subject.to_string(),
            body: body.to_string(),
            ty: String::new(),
            scope: String::new(),
        };
        if let Some((ty, scope, subj)) = parse_conventional(subject) {
            e.ty = ty;
            e.scope = scope;
            e.subject = subj;
        }
        result.push(e);
    }
    result
}

/// LogFetch 分页拉取提交列表。
/// `before` 是“已加载条数”的偏移字符串 → --skip（^hash 会错误排除祖先提交）。
/// 空仓库探测：无任何提交时 HEAD 未出生，rev-parse 静默失败。
/// log 域各命令据此返回空态，避免 git 对 HEAD 的 fatal 透传到前端。
fn head_unborn(repo: &Path) -> bool {
    !git_exec(repo, &["rev-parse", "--verify", "--quiet", "HEAD"], LOG_TIMEOUT).success
}

#[tauri::command]
pub fn log_fetch(repo: String, limit: i64, before: String) -> Result<Vec<LogCommitDto>, String> {
    let repo = repo_path(Some(repo));
    if head_unborn(&repo) {
        return Ok(Vec::new());
    }
    let mut args: Vec<String> = vec!["log".into(), format!("--format={LOG_FORMAT}"), "--no-merges".into()];
    if limit > 0 {
        args.push(format!("-{limit}"));
    }
    if let Ok(n) = before.parse::<i64>() {
        if n > 0 {
            args.push(format!("--skip={n}"));
        }
    }
    let refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    let res = git_exec(&repo, &refs, LONG_TIMEOUT);
    if !res.success {
        return Err(res.combined().trim().to_string());
    }
    Ok(parse_log_output(&res.stdout))
}

/// Go LogModule.ShowFileAt 的三值返回镜像。
#[derive(Debug, Clone, Default, Serialize)]
pub struct ShowFileAtResult {
    pub content: String,
    pub binary: bool,
    /// ref:path 在该版本不存在时为 false（非错误）
    pub ok: bool,
}

/// ShowFileAt：读文件在某 commit（或 commit^）处的内容（Wails 绑定入口）。
/// 二进制探测/截断与 ReadFileAtCommit 同逻辑；ok=false = ref:path 不存在。
#[tauri::command]
pub fn show_file_at(commit_ref: String, file_path: String, repo: Option<String>) -> Result<ShowFileAtResult, String> {
    let repo = repo_path(repo);
    let r = file_at_content(&repo, &commit_ref, &file_path);
    Ok(ShowFileAtResult { content: r.content, binary: r.binary, ok: r.ok })
}

/// LogFetchByDate returns all commits on the given calendar day ("YYYY-MM-DD")
/// ——统计热力图按天钻取用。显式 "00:00:00" 时间戳窗口（裸日期跨 git
/// 版本/时区解析不一致，可能漏掉当天晚间的提交）。
#[tauri::command]
pub fn log_fetch_by_date(repo: String, date: String) -> Result<Vec<LogCommitDto>, String> {
    let repo = repo_path(Some(repo));
    if head_unborn(&repo) {
        return Ok(Vec::new());
    }
    let d = chrono::NaiveDate::parse_from_str(&date, "%Y-%m-%d")
        .map_err(|_| format!("invalid date {date:?} (expect YYYY-MM-DD)"))?;
    let next = (d + chrono::Duration::days(1)).format("%Y-%m-%d");
    let since = format!("--since={date} 00:00:00");
    let until = format!("--until={next} 00:00:00");
    let fmt = format!("--format={LOG_FORMAT}");
    let args: Vec<&str> = vec!["log", fmt.as_str(), "--no-merges", since.as_str(), until.as_str()];
    let res = git_exec(&repo, &args, LONG_TIMEOUT);
    if !res.success {
        return Err(res.combined().trim().to_string());
    }
    Ok(parse_log_output(&res.stdout))
}

/// LogDiff returns the diff text (stat + patch) for a commit.
#[tauri::command]
pub fn log_diff(repo: String, hash: String) -> Result<String, String> {
    let repo = repo_path(Some(repo));
    let res = git_exec(&repo, &["show", "--stat", "--patch", hash.as_str()], LONG_TIMEOUT);
    if !res.success {
        return Err(res.combined().trim().to_string());
    }
    Ok(res.stdout)
}

/// CommitTotal returns the total number of commits on the current branch,
/// matching LogFetch's --no-merges filter so numbering stays consistent.
#[tauri::command]
pub fn commit_total(repo: Option<String>) -> Result<i64, String> {
    let repo = repo_path(repo);
    if head_unborn(&repo) {
        return Ok(0);
    }
    let res = git_exec(&repo, &["rev-list", "--count", "--no-merges", "HEAD"], LONG_TIMEOUT);
    if !res.success {
        return Err(format!("git rev-list --count 失败: {}", res.combined().trim()));
    }
    Ok(res.stdout.trim().parse::<i64>().unwrap_or(0))
}

/// CommitStat summarizes what a commit changed: per-file line deltas.
/// （--numstat："added\tdeleted\tfile"；重命名显示为 "old => new"。）
#[tauri::command]
pub async fn commit_stat(commit: String, repo: Option<String>) -> Result<Vec<CommitStatEntryDto>, String> {
    let repo = repo_path(repo);
    run_blocking(move || commit_stat_impl(&repo, &commit))?
}

fn commit_stat_impl(repo: &Path, commit: &str) -> Result<Vec<CommitStatEntryDto>, String> {
    let res = git_exec(repo, &["show", "--numstat", "--format=", commit], LOG_TIMEOUT);
    if !res.success {
        return Err(format!("git show --numstat 失败: {}", res.combined().trim()));
    }
    let mut entries = Vec::new();
    for line in res.stdout.trim().split('\n') {
        if line.is_empty() {
            continue;
        }
        let parts: Vec<&str> = line.splitn(3, '\t').collect();
        if parts.len() < 3 {
            continue;
        }
        let (added, deleted) = (parse_num(parts[0]), parse_num(parts[1]));
        let status = if added > 0 && deleted == 0 {
            "A"
        } else if added == 0 && deleted > 0 {
            "D"
        } else {
            "M"
        };
        entries.push(CommitStatEntryDto {
            file: parts[2].to_string(),
            added,
            deleted,
            status: status.into(),
        });
    }
    Ok(entries)
}

/// CommitFileTree lists every file at a given commit via `git ls-tree -r`.
/// 返回扁平路径列表，前端自行重建目录树。
#[tauri::command]
pub fn commit_file_tree(commit: String, repo: Option<String>) -> Result<Vec<CommitFileTreeEntryDto>, String> {
    let repo = repo_path(repo);
    // core.quotepath=false：非 ASCII 路径（中文等）否则返回八进制转义，树名乱码
    let res = git_exec(
        &repo,
        &["-c", "core.quotepath=false", "ls-tree", "-r", "--name-only", commit.as_str()],
        LOG_TIMEOUT,
    );
    if !res.success {
        return Err(format!("git ls-tree 失败: {}", res.combined().trim()));
    }
    let mut entries = Vec::new();
    for line in res.stdout.trim().split('\n') {
        if !line.is_empty() {
            entries.push(CommitFileTreeEntryDto { path: line.to_string() });
        }
    }
    Ok(entries)
}

/// ReadFileAtCommit：读取提交时点的文件内容（git show commit:path；
/// 路径不存在 → notFound，其余失败上抛）。
fn read_file_at_commit(repo: &Path, commit: &str, file_path: &str) -> Result<FileResultDto, String> {
    let spec = format!("{commit}:{file_path}");
    let res = git_exec(repo, &["show", spec.as_str()], LOG_TIMEOUT);
    if !res.success {
        let msg = res.stderr.trim();
        if msg.contains("does not exist") || msg.contains("exists on disk") {
            return Ok(FileResultDto::not_found());
        }
        return Err(format!("git show 失败: {msg}"));
    }
    let raw = res.stdout.as_bytes();
    // 图片？内联 data URL（对齐上游 ReadFileAtCommit；超大图回落 binary）
    let url = super::repo::encode_image_data_url(file_path, raw);
    if !url.is_empty() {
        if raw.len() > super::repo::MAX_IMAGE_SIZE {
            return Ok(FileResultDto { binary: true, ..Default::default() });
        }
        return Ok(FileResultDto { data_url: url, ..Default::default() });
    }
    Ok(file_result_from_bytes(raw))
}

/// CommitReadFile reads a file's content at a given commit
/// （时间机器代码浏览器）。
#[tauri::command]
pub fn commit_read_file(commit: String, file_path: String, repo: Option<String>) -> Result<FileResultDto, String> {
    let repo = repo_path(repo);
    read_file_at_commit(&repo, &commit, &file_path)
}

/// ShowFileAt 的返回：ok=false 表示 ref:path 在该版本不存在。
struct FileAt {
    content: String,
    binary: bool,
    ok: bool,
}

/// ShowFileAt 的内部实现：读取文件在某个 ref（commit 或 commit^）的内容，
/// 复用 ReadFileAtCommit 的二进制/截断逻辑；失败仅表示不存在。
fn file_at_content(repo: &Path, commit_ref: &str, file_path: &str) -> FileAt {
    let spec = format!("{commit_ref}:{file_path}");
    let res = git_exec(repo, &["show", spec.as_str()], LOG_TIMEOUT);
    if !res.success {
        return FileAt { content: String::new(), binary: false, ok: false };
    }
    let scan = &res.stdout.as_bytes()[..res.stdout.len().min(8192)];
    if scan.contains(&0) {
        return FileAt { content: String::new(), binary: true, ok: true };
    }
    let mut c = res.stdout;
    if c.len() > MAX_TEXT_CONTENT {
        c = String::from_utf8_lossy(&c.as_bytes()[..MAX_TEXT_CONTENT]).into_owned();
        c.push_str("\n\n…（文件超过 1MB，已截断显示）");
    }
    FileAt { content: c, binary: false, ok: true }
}

/// CommitFileDiff returns the before (parent) and after (commit) content of
/// one file, for the side-by-side diff view.
#[tauri::command]
pub fn commit_file_diff(
    commit: String,
    file_path: String,
    repo: Option<String>,
) -> Result<FileDiffResultDto, String> {
    let repo = repo_path(repo);
    let mut res = FileDiffResultDto {
        before: String::new(),
        after: String::new(),
        status: String::new(),
        binary: false,
        not_found: false,
    };

    // 先用 numstat 判定该文件的 A/M/D 状态
    let stat = git_exec(
        &repo,
        &["show", "--numstat", "--format=", commit.as_str(), "--", file_path.as_str()],
        LOG_TIMEOUT,
    );
    if stat.success {
        for line in stat.stdout.trim().split('\n') {
            let parts: Vec<&str> = line.splitn(3, '\t').collect();
            if parts.len() == 3 {
                let (added, deleted) = (parse_num(parts[0]), parse_num(parts[1]));
                res.status = if added > 0 && deleted == 0 {
                    "A".into()
                } else if added == 0 && deleted > 0 {
                    "D".into()
                } else {
                    "M".into()
                };
                break;
            }
        }
    }
    if res.status.is_empty() {
        res.status = "M".into();
    }

    // after = commit 版本内容
    let after = file_at_content(&repo, &commit, &file_path);
    if after.binary {
        res.binary = true;
        return Ok(res);
    }
    if after.ok {
        res.after = after.content;
    }

    // before = commit^（父版本）内容；纯新增文件父版本不存在
    let parent = format!("{commit}^");
    let before = file_at_content(&repo, &parent, &file_path);
    if before.binary {
        res.binary = true;
        return Ok(res);
    }
    if before.ok {
        res.before = before.content;
    } else {
        res.not_found = true;
    }

    Ok(res)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_repo(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("bg-arch-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// full 模式往返：打包（含 .git 与过滤语义）→ tar 解包 → 校验内容
    #[test]
    fn archive_full_roundtrip() {
        let repo = temp_repo("full");
        std::fs::write(repo.join("README.md"), "hello full").unwrap();
        std::fs::create_dir(repo.join("sub")).unwrap();
        std::fs::write(repo.join("sub").join("a.txt"), "aaa").unwrap();
        std::fs::create_dir(repo.join(".git")).unwrap();
        std::fs::write(repo.join(".git").join("HEAD"), "ref: refs/heads/main").unwrap();

        let out = repo.parent().unwrap().join("bg-arch-full-out.tar.gz");
        let res = archive_full_impl(
            &repo,
            ArchiveOptionsDto {
                mode: "full".into(),
                format: "tar.gz".into(),
                output: out.display().to_string(),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(res.mode, "full");
        assert_eq!(res.file_count, 3); // README + sub/a.txt + .git/HEAD（目录不计）
        assert!(res.size > 0);
        assert!(out.exists());
        // tar.gz 可解且内容在
        let raw = std::fs::read(&out).unwrap();
        let dec = flate2::read::GzDecoder::new(&raw[..]);
        let mut ar = tar::Archive::new(dec);
        let mut names = Vec::new();
        for e in ar.entries().unwrap() {
            names.push(e.unwrap().path().unwrap().display().to_string());
        }
        assert!(names.iter().any(|n| n == "README.md"));
        assert!(names.iter().any(|n| n == "sub/a.txt"));
        assert!(names.iter().any(|n| n == ".git/HEAD"));

        // paths 过滤：只打包 sub
        let out2 = repo.parent().unwrap().join("bg-arch-full-sub.tar");
        let res2 = archive_full_impl(
            &repo,
            ArchiveOptionsDto {
                mode: "full".into(),
                format: "tar".into(),
                output: out2.display().to_string(),
                paths: vec!["sub".into()],
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(res2.file_count, 1);
        let _ = std::fs::remove_dir_all(&repo);
        let _ = std::fs::remove_file(&out);
        let _ = std::fs::remove_file(&out2);
    }

    /// full 模式 zip：手写写入器产物能被 Windows 原生解包（Expand-Archive）
    #[test]
    #[cfg(windows)]
    fn archive_full_zip() {
        let repo = temp_repo("zip");
        std::fs::write(repo.join("hello.txt"), "zip roundtrip 内容").unwrap();
        std::fs::create_dir(repo.join("d")).unwrap();
        std::fs::write(repo.join("d").join("b.bin"), vec![0u8; 1024]).unwrap();

        let out = repo.parent().unwrap().join("bg-arch-zip-out.zip");
        let res = archive_full_impl(
            &repo,
            ArchiveOptionsDto {
                mode: "full".into(),
                format: "zip".into(),
                output: out.display().to_string(),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(res.mode, "full");
        assert_eq!(res.file_count, 2);
        assert!(out.exists());

        // Windows 原生解包验证
        let dst = repo.parent().unwrap().join("bg-arch-zip-dst");
        let ps = format!(
            "Expand-Archive -LiteralPath '{}' -DestinationPath '{}' -Force",
            out.display(),
            dst.display()
        );
        let st = std::process::Command::new("powershell")
            .args(["-NoProfile", "-Command", &ps])
            .output()
            .expect("spawn powershell");
        assert!(st.status.success(), "Expand-Archive 失败: {}", String::from_utf8_lossy(&st.stderr));
        let txt = std::fs::read_to_string(dst.join("hello.txt")).unwrap();
        assert_eq!(txt, "zip roundtrip 内容");
        let bin = std::fs::read(dst.join("d").join("b.bin")).unwrap();
        assert_eq!(bin.len(), 1024);

        let _ = std::fs::remove_dir_all(&repo);
        let _ = std::fs::remove_file(&out);
        let _ = std::fs::remove_dir_all(&dst);
    }
}
