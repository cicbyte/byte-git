//! gitops 域命令：Git 配置（user.name/email 等常用项）、远程 / 分支 / 标签
//! 管理、Lua 钩子与 stash。
//!
//! 移植自 git-mate `gui/internal/gitops`（gitconfig.go + gitmanage.go +
//! hook.go + stash.go）。git 子进程统一走本文件的 `git_exec` / `git_run`
//! （隐藏窗口 + 超时 + stdout/stderr 分离捕获），对应 Go 侧的
//! core.HideExec / sysproc.Command(Context)；钩子安装 / 执行 / 读脚本复用
//! byte-git-core::hook。
//!
//! Go 侧绑定方法均无 repo 参数（用 Core.CurrentRepo），Rust 侧统一在尾部
//! 追加 `repo: Option<String>`，经 crate::state::resolve 解析（当前仓库
//! 状态 → "."）。命名 stash 前缀沿用 byte-git 全仓统一的
//! "byte-git-stash:"（Go 侧为 "git-mate-stash:"），保证 GUI 与 CLI 互认。

use byte_git_core::hook::{self as core_hook};
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// Go 侧未设超时的命令：这里给一个宽松上限，防止异常悬挂。
const LONG_TIMEOUT: Duration = Duration::from_secs(300);
/// stash push / pop（对应 Go 30s 超时）。
const STASH_TIMEOUT: Duration = Duration::from_secs(30);
/// stash drop（对应 Go 15s 超时）。
const STASH_DROP_TIMEOUT: Duration = Duration::from_secs(15);

/// 命名 stash 的消息前缀（core::stash 同款；Go 侧为 git-mate-stash:）。
const STASH_PREFIX: &str = "byte-git-stash:";

/// 与 Go GUI 一致：无仓库上下文的命令统一接收可选 repo 参数；
/// 缺省回落到当前仓库状态（state.rs，对应 Core.CurrentRepo），最终 "."。
fn repo_path(repo: Option<String>) -> PathBuf {
    crate::state::resolve(repo)
}

// ---------------------------------------------------------------------------
// 基础设施（拷贝自 commit.rs，stdout/stderr 分离捕获以覆盖 Go 的
// Output() / CombinedOutput() 两种用法及退出码特判）
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
    /// 进程退出码（被杀死 / 启动失败时为 None）。
    code: Option<i32>,
    stdout: String,
    stderr: String,
}

impl GitOutput {
    /// 合并输出（对应 Go 的 CombinedOutput）。
    fn combined(&self) -> String {
        format!("{}{}", self.stdout, self.stderr)
    }
}

/// 执行任意 `git <args...>`（不带 -C）：隐藏窗口 + 超时 + 捕获输出。
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
                code: None,
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
                    code: status.code(),
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
                        code: None,
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
                    code: None,
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

/// 在独立线程执行阻塞任务（git clone / 网络推送），命令保持 async 不冻结 UI。
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

/// GitConfigEntryDTO mirrors gitconfig.Entry（跨 scope 合并视图）。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitConfigEntryDto {
    pub key: String,
    /// 合并值（local 优先于 global）
    pub effective_value: String,
    /// .git/config 显式设置值
    pub local_value: String,
    /// ~/.gitconfig 显式设置值
    pub global_value: String,
    pub has_local: bool,
    pub has_global: bool,
}

/// RemoteDTO describes one configured remote.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteDto {
    /// e.g. origin
    pub name: String,
    /// fetch url
    pub url: String,
    /// push url（常与 url 相同）
    pub push_url: String,
    /// ["fetch","push"]
    pub types: Vec<String>,
}

/// BranchDTO describes a local or remote-tracking branch.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BranchDto {
    pub name: String,
    pub current: bool,
    pub is_remote: bool,
    /// e.g. "origin/main"；空 = 无上游
    pub upstream: String,
    /// 上游 ref 已不存在
    pub gone: bool,
}

/// TagDTO describes a tag.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TagDto {
    pub name: String,
    pub date: String,
    pub message: String,
    pub is_annotated: bool,
    pub commit: String,
}

/// Hook 槽位状态（对应 Go hook.Hook；source omitempty）。
#[derive(Debug, Clone, Serialize)]
pub struct HookDto {
    pub name: String,
    pub enabled: bool,
    /// shell / python / node / script / sample（完全空置的槽位为 ""）
    #[serde(rename = "type")]
    pub ty: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub source: String,
    /// local / global
    #[serde(default)]
    pub scope: String,
}

/// Hook 执行结果（对应 Go hook.ExecResult）。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HookExecResultDto {
    pub exit_code: i32,
    pub output: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub error: String,
}

/// StashEntry is a frontend-friendly stash item.
#[derive(Debug, Clone, Serialize)]
pub struct StashEntryDto {
    pub index: i64,
    pub message: String,
    /// 自定义名（命名前缀已剥离；非命名 stash 为空）
    pub name: String,
    /// e.g. stash@{0}
    #[serde(rename = "ref")]
    pub ref_: String,
}

// ---------------------------------------------------------------------------
// Git Config（gitconfig.go）
// ---------------------------------------------------------------------------

/// gitconfig.CommonKeys：UI 呈现的高频配置键。
const COMMON_CONFIG_KEYS: &[&str] = &[
    "user.name", "user.email",
    "core.editor", "core.autocrlf", "core.quotepath",
    "pull.rebase", "init.defaultBranch",
    "commit.gpgsign", "http.proxy",
];

/// git config 读取：成功返回 trim 后的 stdout；退出码 1 = 键未设置
/// （返回 ""，对应 Go run 对 ExitError{code 1} 的特判）；其他失败为错误。
fn config_get(repo: &Path, args: &[&str]) -> Result<String, String> {
    let res = git_exec(repo, args, LONG_TIMEOUT);
    if res.success {
        return Ok(res.stdout.trim().to_string());
    }
    if res.code == Some(1) {
        return Ok(String::new());
    }
    Err(res.combined().trim().to_string())
}

/// GitConfigList returns the merged view of common keys for the current repo.
#[tauri::command]
pub fn git_config_list(repo: Option<String>) -> Result<Vec<GitConfigEntryDto>, String> {
    let repo = repo_path(repo);
    let mut entries = Vec::with_capacity(COMMON_CONFIG_KEYS.len());
    for &key in COMMON_CONFIG_KEYS {
        // 有效值 = local > global > system 的合并结果；读取失败为致命错误
        let effective = config_get(&repo, &["config", "--get", key])?;
        // 分层读取失败按“未设置”处理（Go 侧忽略 scopedGet 的错误）
        let local = config_get(&repo, &["config", "--local", "--get", key]).unwrap_or_default();
        let global = config_get(&repo, &["config", "--global", "--get", key]).unwrap_or_default();
        entries.push(GitConfigEntryDto {
            key: key.to_string(),
            effective_value: effective,
            has_local: !local.is_empty(),
            local_value: local,
            has_global: !global.is_empty(),
            global_value: global,
        });
    }
    Ok(entries)
}

/// GitConfigSet writes a key at the given scope ("local" or "global").
#[tauri::command]
pub fn git_config_set(
    scope: String,
    key: String,
    value: String,
    repo: Option<String>,
) -> Result<(), String> {
    if scope != "local" && scope != "global" {
        return Err(format!("无效 scope: {scope} (local/global)"));
    }
    let repo = repo_path(repo);
    let scope_arg = format!("--{scope}");
    let res = git_exec(&repo, &["config", scope_arg.as_str(), key.as_str(), value.as_str()], LONG_TIMEOUT);
    if !res.success {
        return Err(format!("git config --{scope} {key} 失败: {}", res.combined().trim()));
    }
    Ok(())
}

/// GitConfigUnset removes a key at the given scope.
#[tauri::command]
pub fn git_config_unset(scope: String, key: String, repo: Option<String>) -> Result<(), String> {
    if scope != "local" && scope != "global" {
        return Err(format!("无效 scope: {scope} (local/global)"));
    }
    let repo = repo_path(repo);
    let scope_arg = format!("--{scope}");
    let res = git_exec(&repo, &["config", scope_arg.as_str(), "--unset", key.as_str()], LONG_TIMEOUT);
    if !res.success {
        return Err(format!("git config --{scope} --unset {key} 失败: {}", res.combined().trim()));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Remote（gitmanage.go）
// ---------------------------------------------------------------------------

/// RemoteList lists all remotes with their fetch/push URLs.
#[tauri::command]
pub fn remote_list(repo: Option<String>) -> Result<Vec<RemoteDto>, String> {
    let repo = repo_path(repo);
    let res = git_exec(&repo, &["remote", "-v"], LONG_TIMEOUT);
    if !res.success {
        return Err(format!("git remote -v 失败: {}", res.combined().trim()));
    }
    let mut result: Vec<RemoteDto> = Vec::new();
    for line in res.stdout.split('\n') {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        // 格式："name\turl (type)"
        let Some(tab) = line.find('\t') else { continue };
        let name = &line[..tab];
        let rest = &line[tab + 1..]; // "url (type)"
        let (url, ty) = match rest.rfind('(') {
            Some(paren) => (
                rest[..paren].trim(),
                rest[paren + 1..].trim_matches(')').trim(),
            ),
            None => (rest.trim(), ""),
        };
        // 首次出现的 remote 插入列表，保持 git 输出顺序
        let remote = match result.iter_mut().find(|r| r.name == name) {
            Some(r) => r,
            None => {
                result.push(RemoteDto {
                    name: name.to_string(),
                    url: String::new(),
                    push_url: String::new(),
                    types: Vec::new(),
                });
                result.last_mut().expect("just pushed")
            }
        };
        if ty == "fetch" {
            remote.url = url.to_string();
        } else if ty == "push" {
            remote.push_url = url.to_string();
        }
        if !ty.is_empty() && !remote.types.iter().any(|t| t == ty) {
            remote.types.push(ty.to_string());
        }
    }
    // 未单独设置 push url 时回落到 fetch url
    for r in &mut result {
        if r.push_url.is_empty() {
            r.push_url = r.url.clone();
        }
    }
    Ok(result)
}

/// RemoteSetURL changes the fetch URL of a remote (also covers push in the common case).
#[tauri::command]
pub fn remote_set_url(name: String, url: String, repo: Option<String>) -> Result<(), String> {
    if name.is_empty() || url.is_empty() {
        return Err("名称和 URL 不能为空".into());
    }
    let repo = repo_path(repo);
    let res = git_exec(&repo, &["remote", "set-url", name.as_str(), url.as_str()], LONG_TIMEOUT);
    if !res.success {
        return Err(format!("修改远程地址失败: {}", res.combined().trim()));
    }
    Ok(())
}

/// RemoteAdd adds a new remote.
#[tauri::command]
pub fn remote_add(name: String, url: String, repo: Option<String>) -> Result<(), String> {
    if name.is_empty() || url.is_empty() {
        return Err("名称和 URL 不能为空".into());
    }
    let repo = repo_path(repo);
    let res = git_exec(&repo, &["remote", "add", name.as_str(), url.as_str()], LONG_TIMEOUT);
    if !res.success {
        return Err(format!("添加远程失败: {}", res.combined().trim()));
    }
    Ok(())
}

/// RemoteRemove deletes a remote.
#[tauri::command]
pub fn remote_remove(name: String, repo: Option<String>) -> Result<(), String> {
    if name.is_empty() {
        return Err("名称不能为空".into());
    }
    let repo = repo_path(repo);
    let res = git_exec(&repo, &["remote", "remove", name.as_str()], LONG_TIMEOUT);
    if !res.success {
        return Err(format!("删除远程失败: {}", res.combined().trim()));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Branch（gitmanage.go）
// ---------------------------------------------------------------------------

/// BranchList lists local branches (with upstream/gone) then remote-tracking branches.
#[tauri::command]
pub fn branch_list(repo: Option<String>) -> Result<Vec<BranchDto>, String> {
    let repo = repo_path(repo);
    // 本地分支 + upstream/gone 检测（-vv 输出 "[origin/xxx]" / "[origin/xxx: gone]"）
    let res = git_exec(&repo, &["branch", "-vv", "--no-color"], LONG_TIMEOUT);
    if !res.success {
        return Err(format!("git branch 失败: {}", res.combined().trim()));
    }
    let mut result = Vec::new();
    for line in res.stdout.split('\n') {
        if line.trim().is_empty() {
            continue;
        }
        let current = line.starts_with('*');
        let s = line.strip_prefix('*').unwrap_or(line).trim();
        // 分支名为第一个词
        let name = match s.split_whitespace().next() {
            Some(n) => n.to_string(),
            None => continue,
        };
        let mut b = BranchDto {
            name,
            current,
            is_remote: false,
            upstream: String::new(),
            gone: false,
        };
        if let (Some(lb), Some(rb)) = (s.find('['), s.find(']')) {
            if rb > lb {
                let inner = &s[lb + 1..rb];
                if let Some(stripped) = inner.strip_suffix(": gone") {
                    b.gone = true;
                    b.upstream = stripped.trim().to_string();
                } else {
                    b.upstream = inner.trim().to_string();
                }
                // upstream 前偶带 commit hash + 空格：前缀 ≥6 字节视为 hash 剥离
                if let Some(sp) = b.upstream.find(' ') {
                    if sp >= 6 {
                        b.upstream = b.upstream[sp + 1..].trim().to_string();
                    }
                }
            }
        }
        result.push(b);
    }
    // 远程跟踪分支（refs/remotes，排除 "origin/HEAD -> origin/main" 符号引用）；
    // 失败忽略（与 Go 一致：err == nil 才追加）
    let r = git_exec(&repo, &["branch", "-r", "--no-color"], LONG_TIMEOUT);
    if r.success {
        for line in r.stdout.split('\n') {
            let s = line.trim();
            if s.is_empty() || s.contains(" -> ") {
                continue;
            }
            result.push(BranchDto {
                name: s.to_string(),
                current: false,
                is_remote: true,
                upstream: String::new(),
                gone: false,
            });
        }
    }
    Ok(result)
}

/// BranchCreate creates a new local branch at HEAD.
#[tauri::command]
pub fn branch_create(name: String, repo: Option<String>) -> Result<(), String> {
    if name.is_empty() {
        return Err("分支名不能为空".into());
    }
    let repo = repo_path(repo);
    let res = git_exec(&repo, &["branch", name.as_str()], LONG_TIMEOUT);
    if !res.success {
        return Err(format!("创建分支失败: {}", res.combined().trim()));
    }
    Ok(())
}

/// BranchSwitch switches to an existing local branch.
#[tauri::command]
pub fn branch_switch(name: String, repo: Option<String>) -> Result<(), String> {
    if name.is_empty() {
        return Err("分支名不能为空".into());
    }
    let repo = repo_path(repo);
    let res = git_exec(&repo, &["switch", name.as_str()], LONG_TIMEOUT);
    if !res.success {
        return Err(format!("切换分支失败: {}", res.combined().trim()));
    }
    Ok(())
}

/// BranchDelete deletes a merged local branch (safe; git -d).
#[tauri::command]
pub fn branch_delete(name: String, repo: Option<String>) -> Result<(), String> {
    if name.is_empty() {
        return Err("分支名不能为空".into());
    }
    let repo = repo_path(repo);
    let res = git_exec(&repo, &["branch", "-d", name.as_str()], LONG_TIMEOUT);
    if !res.success {
        return Err(format!("删除分支失败（可能未合并）: {}", res.combined().trim()));
    }
    Ok(())
}

/// BranchDeleteForce force-deletes a local branch (git -D).
#[tauri::command]
pub fn branch_delete_force(name: String, repo: Option<String>) -> Result<(), String> {
    if name.is_empty() {
        return Err("分支名不能为空".into());
    }
    let repo = repo_path(repo);
    let res = git_exec(&repo, &["branch", "-D", name.as_str()], LONG_TIMEOUT);
    if !res.success {
        return Err(format!("强制删除分支失败: {}", res.combined().trim()));
    }
    Ok(())
}

/// BranchSetUpstream sets the upstream tracking branch.
#[tauri::command]
pub fn branch_set_upstream(local: String, remote: String, repo: Option<String>) -> Result<(), String> {
    if local.is_empty() || remote.is_empty() {
        return Err("本地分支与上游不能为空".into());
    }
    let repo = repo_path(repo);
    let res = git_exec(&repo, &["branch", "-u", remote.as_str(), local.as_str()], LONG_TIMEOUT);
    if !res.success {
        return Err(format!("设置上游失败: {}", res.combined().trim()));
    }
    Ok(())
}

/// BranchRename renames a local branch.
#[tauri::command]
pub fn branch_rename(old_name: String, new_name: String, repo: Option<String>) -> Result<(), String> {
    if old_name.is_empty() || new_name.is_empty() {
        return Err("分支名不能为空".into());
    }
    let repo = repo_path(repo);
    let res = git_exec(&repo, &["branch", "-m", old_name.as_str(), new_name.as_str()], LONG_TIMEOUT);
    if !res.success {
        return Err(format!("重命名分支失败: {}", res.combined().trim()));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Tag（gitmanage.go）
// ---------------------------------------------------------------------------

/// shortHashOfRef：解析 ref 的短提交 hash（尽力而为，失败返回空）。
fn short_hash_of_ref(repo: &Path, ref_name: &str) -> String {
    let res = git_exec(repo, &["rev-parse", "--short", ref_name], LONG_TIMEOUT);
    if !res.success {
        return String::new();
    }
    res.stdout.trim().to_string()
}

/// TagList lists all tags with metadata (annotated/lightweight, commit, date).
#[tauri::command]
pub fn tag_list(repo: Option<String>) -> Result<Vec<TagDto>, String> {
    let repo = repo_path(repo);
    // for-each-ref 给出 tagger date + object type；%(*objectname) 解引用到提交
    let fmt = "%(refname:short)|%(objecttype)|%(*objectname:short)|%(taggerdate:short)|%(subject)";
    let fmt_arg = format!("--format={fmt}");
    let res = git_exec(&repo, &["for-each-ref", fmt_arg.as_str(), "refs/tags"], LONG_TIMEOUT);
    if !res.success {
        return Err(format!("git for-each-ref refs/tags 失败: {}", res.combined().trim()));
    }
    let mut result = Vec::new();
    for line in res.stdout.split('\n') {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let parts: Vec<&str> = line.splitn(5, '|').collect();
        if parts.len() < 4 {
            continue;
        }
        let (name, otype, date) = (parts[0], parts[1], parts[3]);
        let msg = parts.get(4).copied().unwrap_or("");
        // 轻量标签 objecttype 即 commit，%(*objectname) 为空 → 回退 rev-parse
        let commit = if parts[2].is_empty() {
            short_hash_of_ref(&repo, &format!("refs/tags/{name}"))
        } else {
            parts[2].to_string()
        };
        result.push(TagDto {
            name: name.to_string(),
            date: date.to_string(),
            message: msg.to_string(),
            is_annotated: otype == "tag",
            commit,
        });
    }
    Ok(result)
}

/// TagCreate creates a tag at HEAD. If message is non-empty, creates an annotated tag.
#[tauri::command]
pub fn tag_create(name: String, message: String, repo: Option<String>) -> Result<(), String> {
    if name.is_empty() {
        return Err("标签名不能为空".into());
    }
    let repo = repo_path(repo);
    let res = if !message.is_empty() {
        git_exec(&repo, &["tag", "-a", name.as_str(), "-m", message.as_str()], LONG_TIMEOUT)
    } else {
        git_exec(&repo, &["tag", name.as_str()], LONG_TIMEOUT)
    };
    if !res.success {
        return Err(format!("创建标签失败: {}", res.combined().trim()));
    }
    Ok(())
}

/// TagDelete removes a tag.
#[tauri::command]
pub fn tag_delete(name: String, repo: Option<String>) -> Result<(), String> {
    if name.is_empty() {
        return Err("标签名不能为空".into());
    }
    let repo = repo_path(repo);
    let res = git_exec(&repo, &["tag", "-d", name.as_str()], LONG_TIMEOUT);
    if !res.success {
        return Err(format!("删除标签失败: {}", res.combined().trim()));
    }
    Ok(())
}

/// TagPush pushes one or more tags to a remote.
/// （网络推送可能长时间等待，async + std 线程，与 commit_push 一致给 10 分钟上限。）
#[tauri::command]
pub async fn tag_push(remote: String, names: Vec<String>, repo: Option<String>) -> Result<(), String> {
    if remote.is_empty() || names.is_empty() {
        return Err("远端和标签名不能为空".into());
    }
    let repo = repo_path(repo);
    run_blocking(move || -> Result<(), String> {
        let mut args: Vec<String> = vec!["push".into(), remote];
        args.extend(names);
        let refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        let res = git_exec(&repo, &refs, Duration::from_secs(600));
        if res.timed_out {
            return Err("推送超时（10 分钟），请检查网络或远程仓库状态".into());
        }
        if !res.success {
            return Err(format!("推送标签失败: {}", res.combined().trim()));
        }
        Ok(())
    })?
}

// ---------------------------------------------------------------------------
// Hook（上游 03915e0 后：shebang 统一，core::hook::list_hooks 等复用）
// ---------------------------------------------------------------------------

/// HookList：按作用域返回全部 21 个固定槽位（core.hooksPath 感知）。
/// global 未配置时返回空列表，前端引导配置。
#[tauri::command]
pub fn hook_list(scope: String, repo: Option<String>) -> Result<Vec<HookDto>, String> {
    let repo = repo_path(repo);
    core_hook::list_hooks(&repo, &scope)
        .map(|list| {
            list.into_iter()
                .map(|h| HookDto {
                    name: h.name,
                    enabled: h.enabled,
                    ty: h.ty,
                    source: h.source,
                    scope: h.scope,
                })
                .collect()
        })
        .map_err(|e| e.to_string())
}

/// HookInstallScript：按语言（shell/python/node）写内容到作用域 hooks 目录，
/// git 经 shebang 原生执行（上游 03915e0 后统一机制，Lua 路径已移除）。
#[tauri::command]
pub fn hook_install_script(
    name: String,
    lang: String,
    script_content: String,
    scope: String,
    repo: Option<String>,
) -> Result<(), String> {
    let repo = repo_path(repo);
    core_hook::install_script(&repo, &name, &lang, &script_content, &scope).map_err(|e| e.to_string())
}

/// HookRemove：按作用域卸载钩子（文件 + 对应配置项）。
#[tauri::command]
pub fn hook_remove(name: String, scope: String, repo: Option<String>) -> Result<(), String> {
    let repo = repo_path(repo);
    core_hook::remove(&repo, &name, &scope).map_err(|e| e.to_string())
}

/// HookReadScript：读取作用域目录内钩子脚本内容；
/// 未配置/文件不存在 → 空串（前端回落模板）。
#[tauri::command]
pub fn hook_read_script(name: String, scope: String, repo: Option<String>) -> Result<String, String> {
    let repo = repo_path(repo);
    Ok(core_hook::read_script(&repo, &name, &scope))
}

/// HookExec：测试执行钩子（直接 spawn hook 文件，shebang 原生机制）。
/// 失败不报错，以 ExecResult 反映（对齐上游）。
#[tauri::command]
pub fn hook_exec(name: String, scope: String, repo: Option<String>) -> Result<HookExecResultDto, String> {
    let repo = repo_path(repo);
    let no_args: Vec<String> = Vec::new();
    let r = core_hook::exec(&repo, &name, &scope, &no_args).map_err(|e| e.to_string())?;
    Ok(HookExecResultDto {
        exit_code: r.exit_code,
        output: r.output,
        error: r.error,
    })
}

/// HookConfigInfo 的 DTO（core.hooksPath 配置面板）。
#[derive(Debug, Clone, Default, Serialize)]
pub struct HookConfigInfoDto {
    #[serde(skip_serializing_if = "String::is_empty")]
    pub hooks_path_local: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub hooks_path_global: String,
    pub hooks_path_effective: String,
    pub default_hooks_dir: String,
    pub has_local_hooks_path: bool,
    pub has_global_hooks_path: bool,
}

/// HookConfigInfo：core.hooksPath 的 local/global/effective 全貌。
#[tauri::command]
pub fn hook_config_info(repo: Option<String>) -> Result<HookConfigInfoDto, String> {
    let repo = repo_path(repo);
    let i = core_hook::config_info(&repo);
    Ok(HookConfigInfoDto {
        hooks_path_local: i.hooks_path_local,
        hooks_path_global: i.hooks_path_global,
        hooks_path_effective: i.hooks_path_effective,
        default_hooks_dir: i.default_hooks_dir,
        has_local_hooks_path: i.has_local_hooks_path,
        has_global_hooks_path: i.has_global_hooks_path,
    })
}

/// HookSetHooksPath：设置（空值=清除）core.hooksPath。scope = local/global。
#[tauri::command]
pub fn hook_set_hooks_path(scope: String, value: String, repo: Option<String>) -> Result<(), String> {
    let repo = repo_path(repo);
    core_hook::set_hooks_path(&repo, &scope, &value).map_err(|e| e.to_string())
}

// ---------------------------------------------------------------------------
// Stash（stash.go）
// ---------------------------------------------------------------------------

/// StashList returns all stash entries for the current repo.
/// `git stash list` 无 stash 时 exit 0 + 空输出；真正的失败（仓库损坏 /
/// git 缺失 / 权限）如实上抛，而不是静默返回“没有 stash”。
#[tauri::command]
pub fn stash_list(repo: Option<String>) -> Result<Vec<StashEntryDto>, String> {
    let repo = repo_path(repo);
    let res = git_exec(&repo, &["stash", "list"], LONG_TIMEOUT);
    if !res.success {
        return Err(format!("git stash list 失败: {}", res.combined().trim()));
    }
    // 空输出 = 没有 stash；返回非 nil 空数组让 JSON 序列化为 [] 而非 null
    let mut result = Vec::new();
    for (i, line) in res.stdout.trim().split('\n').enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        // "stash@{0}: WIP on main: abc123 msg" → 取第一个 ": " 之后的全部
        let msg = match line.split_once(": ") {
            Some((_, rest)) => rest.to_string(),
            None => String::new(),
        };
        let name = msg.strip_prefix(STASH_PREFIX).unwrap_or("").to_string();
        result.push(StashEntryDto {
            index: i as i64,
            message: msg,
            name,
            ref_: format!("stash@{{{i}}}"),
        });
    }
    Ok(result)
}

/// StashSave saves current changes（空名回落 untitled，命名带统一前缀）。
#[tauri::command]
pub fn stash_save(name: String, repo: Option<String>) -> Result<(), String> {
    let repo = repo_path(repo);
    let msg = if name.is_empty() {
        format!("{STASH_PREFIX}untitled")
    } else {
        format!("{STASH_PREFIX}{name}")
    };
    let res = git_exec(&repo, &["stash", "push", "-m", msg.as_str()], STASH_TIMEOUT);
    if !res.success {
        return Err(format!("保存 stash 失败: {}", res.combined().trim()));
    }
    Ok(())
}

/// StashPop pops a stash by index.
#[tauri::command]
pub fn stash_pop(index: i64, repo: Option<String>) -> Result<(), String> {
    let repo = repo_path(repo);
    let stash_ref = format!("stash@{{{index}}}");
    let res = git_exec(&repo, &["stash", "pop", stash_ref.as_str()], STASH_TIMEOUT);
    if !res.success {
        return Err(format!("弹出 stash 失败: {}", res.combined().trim()));
    }
    Ok(())
}

/// StashDrop drops a stash by index.
#[tauri::command]
pub fn stash_drop(index: i64, repo: Option<String>) -> Result<(), String> {
    let repo = repo_path(repo);
    let stash_ref = format!("stash@{{{index}}}");
    let res = git_exec(&repo, &["stash", "drop", stash_ref.as_str()], STASH_DROP_TIMEOUT);
    if !res.success {
        return Err(format!("删除 stash 失败: {}", res.combined().trim()));
    }
    Ok(())
}

/// StashDiff returns the diff text for a stash entry.
#[tauri::command]
pub fn stash_diff(index: i64, repo: Option<String>) -> Result<String, String> {
    let repo = repo_path(repo);
    let stash_ref = format!("stash@{{{index}}}");
    let res = git_exec(&repo, &["stash", "show", "--patch", "--stat", stash_ref.as_str()], LONG_TIMEOUT);
    if !res.success {
        return Err(format!("查看 stash diff 失败: {}", res.combined().trim()));
    }
    Ok(res.stdout)
}
