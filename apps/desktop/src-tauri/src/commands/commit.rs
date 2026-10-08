//! commit 域命令：提交台（状态分组 / 暂存 / 提交 / 推送）与历史操作
//! （cherry-pick / revert / reset / 从提交建分支）。
//!
//! 移植自 git-mate `gui/internal/commit`（commit.go + commit_actions.go）。
//! git 子进程统一走本文件的 `git_exec`（隐藏窗口 + 超时 + 合并输出），
//! 对应 Go 侧的 core.HideExec / sysproc.CommandContext + CombinedOutput；
//! 纯计算逻辑优先复用 byte-git-core（aicommit / config / ai）。
//!
//! 命令均为 async：Tauri v2 同步命令在主线程执行，git/AI/网络等待会冻结
//! 窗口；Go Wails 侧每个绑定都跑在自己的 goroutine 上，这里用 async 对齐。

use byte_git_core::ai::ChatClient;
use byte_git_core::aicommit;
use byte_git_core::config::Settings;
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};
use tauri::Emitter;

/// Go 侧未设超时的命令：这里给一个宽松上限，防止异常悬挂。
const LONG_TIMEOUT: Duration = Duration::from_secs(300);
/// 提交可能触发 pre-commit 钩子，放宽到 10 分钟。
const COMMIT_TIMEOUT: Duration = Duration::from_secs(600);

// ---------------------------------------------------------------------------
// 基础设施
// ---------------------------------------------------------------------------

/// 仓库路径解析：空串回落到 "."（对应 Go 的 CurrentRepoOrDot 回落语义）。
fn repo_path(repo: &str) -> PathBuf {
    if repo.is_empty() {
        PathBuf::from(".")
    } else {
        PathBuf::from(repo)
    }
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

/// git_exec 的执行结果。
struct GitOutput {
    /// 退出码是否为 0。
    success: bool,
    /// 是否因超时被强制终止。
    timed_out: bool,
    /// 合并后的 stdout + stderr（对应 Go 的 CombinedOutput）。
    output: String,
}

/// 在仓库目录执行 `git -C <repo> <args...>`：隐藏窗口 + 超时 + 合并输出。
/// 对应 Go 的 core.HideExec(...).CombinedOutput() / sysproc.CommandContext。
fn git_exec(repo: &Path, args: &[&str], timeout: Duration) -> GitOutput {
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
            return GitOutput {
                success: false,
                timed_out: false,
                output: format!("启动 git 失败: {e}"),
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
                let output = format!(
                    "{}{}",
                    String::from_utf8_lossy(&stdout),
                    String::from_utf8_lossy(&stderr)
                );
                return GitOutput { success: status.success(), timed_out: false, output };
            }
            Ok(None) => {
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    return GitOutput {
                        success: false,
                        timed_out: true,
                        output: format!("命令超时（{} 秒）: git {}", timeout.as_secs(), args.join(" ")),
                    };
                }
                std::thread::sleep(Duration::from_millis(15));
            }
            Err(e) => {
                return GitOutput {
                    success: false,
                    timed_out: false,
                    output: format!("等待 git 进程失败: {e}"),
                }
            }
        }
    }
}

/// 在独立线程执行阻塞任务（LLM 流式请求 / 网络推送）。
/// reqwest::blocking 不能在 tokio 异步上下文里运行（会 panic），
/// 因此把阻塞工作挪到专用 std 线程，命令本身保持 async 不冻结 UI。
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
// DTO（json tag 与 Go 结构体对齐）
// ---------------------------------------------------------------------------

/// FileEntry is a single file with a human-readable status label.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileEntry {
    pub path: String,
    /// modified/added/deleted/renamed/untracked
    pub status: String,
}

/// CommitStatusResult groups files by staged / unstaged / untracked.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitStatusResult {
    pub staged: Vec<FileEntry>,
    pub unstaged: Vec<FileEntry>,
    pub untracked: Vec<FileEntry>,
}

/// PushStatusResult describes the current branch's push-readiness for the UI.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PushStatusResult {
    /// any remote configured
    pub has_remote: bool,
    /// branch has @{upstream}
    pub has_upstream: bool,
    /// first remote name (e.g. "origin"), "" if none
    pub remote: String,
    /// current branch name
    pub branch: String,
    /// commits ahead of upstream (0 if no upstream)
    pub ahead: i64,
}

/// ResetPreviewItem is one commit that will be discarded by a reset.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResetPreviewItem {
    pub hash: String,
    pub subject: String,
    pub author: String,
}

/// ResetPreview describes what a reset to `hash` would affect.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResetPreview {
    /// commits that will be discarded
    pub commits: Vec<ResetPreviewItem>,
    /// number of commits
    pub count: i64,
    /// is the working tree clean?
    pub work_clean: bool,
    /// risk warnings
    pub warnings: Vec<String>,
}

// ---------------------------------------------------------------------------
// 私有辅助（对应 Go 模块私有方法 / aicommit 内部函数）
// ---------------------------------------------------------------------------

/// statusChar maps a porcelain XY status char to a readable label.
fn status_label(c: u8) -> &'static str {
    match c {
        b'M' => "modified",
        b'A' => "added",
        b'D' => "deleted",
        b'R' => "renamed",
        b'C' => "copied",
        b'?' => "untracked",
        _ => "modified",
    }
}

/// branchUpstream returns ("origin/main", true) or ("", false).
fn branch_upstream(repo: &Path) -> Option<String> {
    let res = git_exec(
        repo,
        &["rev-parse", "--abbrev-ref", "--symbolic-full-name", "@{upstream}"],
        LONG_TIMEOUT,
    );
    if !res.success {
        return None;
    }
    let s = res.output.trim();
    if s.is_empty() { None } else { Some(s.to_string()) }
}

/// firstRemote returns the first remote name (usually "origin"), or "".
fn first_remote(repo: &Path) -> String {
    let res = git_exec(repo, &["remote"], LONG_TIMEOUT);
    if !res.success {
        return String::new();
    }
    for l in res.output.split('\n') {
        let l = l.trim();
        if !l.is_empty() {
            return l.to_string();
        }
    }
    String::new()
}

/// currentBranchName returns the current branch (or "HEAD" if detached).
fn current_branch_name(repo: &Path) -> String {
    let res = git_exec(repo, &["rev-parse", "--abbrev-ref", "HEAD"], LONG_TIMEOUT);
    if !res.success {
        return String::new();
    }
    res.output.trim().to_string()
}

/// aheadCount returns commits HEAD is ahead of @{upstream}.
fn ahead_count(repo: &Path) -> i64 {
    let res = git_exec(repo, &["rev-list", "--count", "@{upstream}..HEAD"], LONG_TIMEOUT);
    if !res.success {
        return 0;
    }
    res.output.trim().parse::<i64>().unwrap_or(0)
}

/// 暂存区是否有改动（对应 aicommit.HasStagedChanges：exit != 0 即有改动）。
fn has_staged_changes(repo: &Path) -> bool {
    let res = git_exec(repo, &["diff", "--cached", "--quiet"], LONG_TIMEOUT);
    !res.success
}

/// 暂存区完整 diff（对应 aicommit.GetStagedDiff）。
fn staged_diff(repo: &Path) -> Result<String, String> {
    let res = git_exec(repo, &["diff", "--cached"], LONG_TIMEOUT);
    if !res.success {
        return Err(format!("获取暂存区 diff 失败: {}", res.output.trim()));
    }
    Ok(res.output)
}

/// 暂存文件列表（对应 aicommit.GetStagedFiles）。
fn staged_files(repo: &Path) -> Result<Vec<String>, String> {
    let res = git_exec(repo, &["diff", "--cached", "--name-only"], LONG_TIMEOUT);
    if !res.success {
        return Err(format!("获取暂存文件列表失败: {}", res.output.trim()));
    }
    Ok(res
        .output
        .trim()
        .split('\n')
        .map(|f| f.trim().to_string())
        .filter(|f| !f.is_empty())
        .collect())
}

/// 暂存区短统计（对应 aicommit.GetStagedStat，git-mate 用 --shortstat）。
fn staged_shortstat(repo: &Path) -> String {
    let res = git_exec(repo, &["diff", "--cached", "--shortstat"], LONG_TIMEOUT);
    if !res.success {
        return String::new();
    }
    res.output.trim().to_string()
}

// ---------------------------------------------------------------------------
// 命令：提交台（commit.go）
// ---------------------------------------------------------------------------

/// CommitStatus parses `git status --porcelain` into three groups.
#[tauri::command]
pub async fn commit_status(repo: String) -> Result<CommitStatusResult, String> {
    let repo = repo_path(&repo);
    let res = git_exec(&repo, &["status", "--porcelain"], LONG_TIMEOUT);
    if !res.success {
        return Err(format!("git status 失败: {}", res.output.trim()));
    }
    let mut out = CommitStatusResult {
        staged: Vec::new(),
        unstaged: Vec::new(),
        untracked: Vec::new(),
    };
    for line in res.output.split('\n') {
        if line.len() < 4 {
            continue;
        }
        let bytes = line.as_bytes();
        let (x, y) = (bytes[0], bytes[1]);
        // porcelain path may be quoted; strip surrounding quotes
        let file = line[3..].trim().trim_matches('"');
        if x == b'?' && y == b'?' {
            out.untracked.push(FileEntry { path: file.to_string(), status: "untracked".into() });
            continue;
        }
        // staged: X is meaningful (not space, not ?)
        if x != b' ' && x != b'?' {
            out.staged.push(FileEntry { path: file.to_string(), status: status_label(x).into() });
        }
        // unstaged: Y is meaningful (worktree differs, not space, not ?)
        if y != b' ' && y != b'?' {
            out.unstaged.push(FileEntry { path: file.to_string(), status: status_label(y).into() });
        }
    }
    Ok(out)
}

/// CommitStage runs `git add -- <paths>` to stage files.
#[tauri::command]
pub async fn commit_stage(repo: String, paths: Vec<String>) -> Result<(), String> {
    if paths.is_empty() {
        return Err("没有选中的文件".into());
    }
    let repo = repo_path(&repo);
    let mut args: Vec<String> = vec!["add".into(), "--".into()];
    args.extend(paths.iter().cloned());
    let refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    let res = git_exec(&repo, &refs, LONG_TIMEOUT);
    if !res.success {
        return Err(format!("git add 失败: {}", res.output.trim()));
    }
    Ok(())
}

/// CommitUnstage runs `git restore --staged -- <paths>` to unstage files.
#[tauri::command]
pub async fn commit_unstage(repo: String, paths: Vec<String>) -> Result<(), String> {
    if paths.is_empty() {
        return Err("没有选中的文件".into());
    }
    let repo = repo_path(&repo);
    let mut args: Vec<String> = vec!["restore".into(), "--staged".into(), "--".into()];
    args.extend(paths.iter().cloned());
    let refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    let res = git_exec(&repo, &refs, LONG_TIMEOUT);
    if !res.success {
        return Err(format!("git restore 失败: {}", res.output.trim()));
    }
    Ok(())
}

/// CommitStageAll stages every change in the working tree (tracked + untracked),
/// equivalent to `git add -A`. Used by the SCM drawer's "stage all" action.
#[tauri::command]
pub async fn commit_stage_all(repo: String) -> Result<(), String> {
    let repo = repo_path(&repo);
    let res = git_exec(&repo, &["add", "-A"], LONG_TIMEOUT);
    if !res.success {
        return Err(format!("git add -A 失败: {}", res.output.trim()));
    }
    Ok(())
}

/// CommitUnstageAll unstages every currently-staged change, equivalent to
/// `git restore --staged .`. Used by the SCM drawer's "unstage all" action.
#[tauri::command]
pub async fn commit_unstage_all(repo: String) -> Result<(), String> {
    let repo = repo_path(&repo);
    let res = git_exec(&repo, &["restore", "--staged", "."], LONG_TIMEOUT);
    if !res.success {
        return Err(format!("git restore --staged 失败: {}", res.output.trim()));
    }
    Ok(())
}

/// CommitDiscardTracked discards working-tree modifications of already-tracked
/// files, resetting them to the index version (`git checkout -- <paths>`).
/// Staged content is left untouched; the caller should only pass unstaged paths.
#[tauri::command]
pub async fn commit_discard_tracked(repo: String, paths: Vec<String>) -> Result<(), String> {
    if paths.is_empty() {
        return Err("没有选中的文件".into());
    }
    let repo = repo_path(&repo);
    let mut args: Vec<String> = vec!["checkout".into(), "--".into()];
    args.extend(paths.iter().cloned());
    let refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    let res = git_exec(&repo, &refs, LONG_TIMEOUT);
    if !res.success {
        return Err(format!("git checkout 失败: {}", res.output.trim()));
    }
    Ok(())
}

/// CommitDiscardUntracked permanently deletes untracked files (`git clean -f --`).
/// This is irreversible; the frontend MUST confirm before calling.
#[tauri::command]
pub async fn commit_discard_untracked(repo: String, paths: Vec<String>) -> Result<(), String> {
    if paths.is_empty() {
        return Err("没有选中的文件".into());
    }
    let repo = repo_path(&repo);
    let mut args: Vec<String> = vec!["clean".into(), "-f".into(), "--".into()];
    args.extend(paths.iter().cloned());
    let refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    let res = git_exec(&repo, &refs, LONG_TIMEOUT);
    if !res.success {
        return Err(format!("git clean 失败: {}", res.output.trim()));
    }
    Ok(())
}

/// CommitStagedDiff returns the full text of `git diff --cached`.
#[tauri::command]
pub async fn commit_staged_diff(repo: String) -> Result<String, String> {
    let repo = repo_path(&repo);
    if !has_staged_changes(&repo) {
        return Ok(String::new());
    }
    staged_diff(&repo)
}

/// CommitGenerate streams an AI-generated commit message over the
/// "commit:token" event, then returns the final cleaned message.
#[tauri::command]
pub async fn commit_generate(repo: String, app: tauri::AppHandle) -> Result<String, String> {
    let repo = repo_path(&repo);
    if !has_staged_changes(&repo) {
        return Err("暂存区没有改动，请先暂存文件".into());
    }
    // AI config from config.yaml (shared with the CLI and Settings page)
    let cfg = Settings::load().map_err(|e| format!("读取配置失败: {e}"))?;

    run_blocking(move || -> Result<String, String> {
        let raw_diff = staged_diff(&repo)?;
        let files = staged_files(&repo)?;
        let stat = staged_shortstat(&repo);

        let messages = aicommit::build_messages(
            &raw_diff,
            &stat,
            &files,
            &cfg.ai.lang,
            cfg.ai.max_tokens,
        );
        let client = ChatClient::new(&cfg.ai.base_url, &cfg.ai.api_key, &cfg.ai.model);

        // stream tokens to the frontend
        let emitter = app.clone();
        let suggested = client
            .chat_stream(&messages, |token| {
                let _ = emitter.emit("commit:token", token);
            })
            .map_err(|e| format!("AI 生成失败: {e}"))?;
        // 对应 Go 的 CleanCommitMessage（复用 core::aicommit::clean_message）
        Ok(aicommit::clean_message(&suggested))
    })?
}

/// CommitDo runs `git commit -m <message>` (optionally --no-verify).
#[tauri::command]
pub async fn commit_do(repo: String, message: String, no_verify: bool) -> Result<(), String> {
    if message.trim().is_empty() {
        return Err("commit message 不能为空".into());
    }
    let repo = repo_path(&repo);
    let mut args: Vec<String> = vec!["commit".into(), "-m".into(), message];
    if no_verify {
        args.push("--no-verify".into());
    }
    let refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    let res = git_exec(&repo, &refs, COMMIT_TIMEOUT);
    if !res.success {
        return Err(format!("git commit 失败: {}", res.output.trim()));
    }
    Ok(())
}

/// CommitPush pushes the current branch to its upstream (@{upstream}).
/// force = true uses --force-with-lease (safer than --force: refuses if the
/// remote has new commits we haven't fetched).
/// If the branch has no upstream but remotes exist, the first remote (usually
/// "origin") is used with --set-upstream so first-time push just works.
#[tauri::command]
pub async fn commit_push(force: bool, repo: Option<String>) -> Result<(), String> {
    let repo = repo_path(repo.as_deref().unwrap_or(""));
    run_blocking(move || -> Result<(), String> {
        // does the branch have an upstream?
        let has_upstream = branch_upstream(&repo).is_some();
        let mut args: Vec<String> = vec!["push".into()];
        if force {
            args.push("--force-with-lease".into());
        }
        if !has_upstream {
            // pick the first remote (origin in most cases); error if none
            let remote = first_remote(&repo);
            if remote.is_empty() {
                return Err("此仓库没有配置远端（remote），请先 git remote add origin <url>".into());
            }
            args.push("--set-upstream".into());
            args.push(remote);
            args.push("HEAD".into());
        }
        let refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        // network push can hang on a slow/unresponsive remote; cap at 10 min so
        // the frontend loading state can recover instead of spinning forever
        let res = git_exec(&repo, &refs, Duration::from_secs(600));
        if res.timed_out {
            return Err("推送超时（10 分钟），请检查网络或远程仓库状态".into());
        }
        if !res.success {
            return Err(format!("推送失败: {}", res.output.trim()));
        }
        Ok(())
    })?
}

/// PushStatus returns the branch's push state so the UI can show appropriate
/// guidance (set-upstream prompt, first-push hint, normal push button, etc.).
#[tauri::command]
pub async fn push_status(repo: Option<String>) -> Result<PushStatusResult, String> {
    let repo = repo_path(repo.as_deref().unwrap_or(""));
    let remote = first_remote(&repo);
    let branch = current_branch_name(&repo);
    let has_upstream = branch_upstream(&repo).is_some();
    let mut ahead = 0;
    if has_upstream {
        ahead = ahead_count(&repo);
    }
    Ok(PushStatusResult {
        has_remote: !remote.is_empty(),
        has_upstream,
        remote,
        branch,
        ahead,
    })
}

// ---------------------------------------------------------------------------
// 命令：历史操作（commit_actions.go）
// ---------------------------------------------------------------------------

/// CommitCherryPick applies a commit onto the current branch.
#[tauri::command]
pub async fn commit_cherry_pick(hash: String, repo: Option<String>) -> Result<(), String> {
    let repo = repo_path(repo.as_deref().unwrap_or(""));
    let args = ["cherry-pick", hash.as_str()];
    let res = git_exec(&repo, &args, Duration::from_secs(15));
    if !res.success {
        return Err(format!("cherry-pick 失败: {}", res.output.trim()));
    }
    Ok(())
}

/// CommitRevert creates a revert commit for the given hash.
#[tauri::command]
pub async fn commit_revert(hash: String, repo: Option<String>) -> Result<(), String> {
    let repo = repo_path(repo.as_deref().unwrap_or(""));
    let args = ["revert", "--no-edit", hash.as_str()];
    let res = git_exec(&repo, &args, Duration::from_secs(15));
    if !res.success {
        return Err(format!("revert 失败: {}", res.output.trim()));
    }
    Ok(())
}

/// CommitResetPreview shows what a reset to the given hash would discard,
/// without executing anything. Used by the confirmation modal.
#[tauri::command]
pub async fn commit_reset_preview(hash: String, repo: Option<String>) -> Result<ResetPreview, String> {
    let repo = repo_path(repo.as_deref().unwrap_or(""));

    // commits between hash..HEAD (these will be discarded)
    let range = format!("{hash}..HEAD");
    let log_args = ["log", "--format=%h|%an|%s", range.as_str()];
    let res = git_exec(&repo, &log_args, Duration::from_secs(10));
    if !res.success {
        return Err(format!("预览失败: {}", res.output.trim()));
    }
    let mut commits: Vec<ResetPreviewItem> = Vec::new();
    for line in res.output.trim().split('\n') {
        if line.is_empty() {
            continue;
        }
        let mut parts = line.splitn(3, '|');
        if let (Some(h), Some(a), Some(s)) = (parts.next(), parts.next(), parts.next()) {
            commits.push(ResetPreviewItem {
                hash: h.to_string(),
                author: a.to_string(),
                subject: s.to_string(),
            });
        }
    }

    // working tree clean?（Go 侧忽略错误：失败时输出按空处理 → 视为干净）
    let stat = git_exec(&repo, &["status", "--porcelain"], Duration::from_secs(10));
    let work_clean = !stat.success || stat.output.trim().is_empty();

    // warnings
    let mut warnings: Vec<String> = Vec::new();
    if !commits.is_empty() {
        warnings.push(format!("将丢弃 {} 个提交", commits.len()));
    }
    if !work_clean {
        warnings.push("工作区有未提交改动，硬回滚将全部丢失".into());
    }

    let count = commits.len() as i64;
    Ok(ResetPreview { commits, count, work_clean, warnings })
}

/// CommitReset resets HEAD to the given hash.
/// mode: "soft" (keep changes staged) or "hard" (discard everything).
#[tauri::command]
pub async fn commit_reset(hash: String, mode: String, repo: Option<String>) -> Result<(), String> {
    if mode != "soft" && mode != "hard" {
        return Err(format!("无效的 reset 模式: {mode}（仅支持 soft/hard）"));
    }
    let repo = repo_path(repo.as_deref().unwrap_or(""));
    let mode_arg = format!("--{mode}");
    let args = ["reset", mode_arg.as_str(), hash.as_str()];
    let res = git_exec(&repo, &args, Duration::from_secs(15));
    if !res.success {
        return Err(format!("reset 失败: {}", res.output.trim()));
    }
    Ok(())
}

/// CommitCreateBranch creates a new branch from the given commit hash and
/// checks it out.
#[tauri::command]
pub async fn commit_create_branch(hash: String, branch_name: String, repo: Option<String>) -> Result<(), String> {
    if branch_name.trim().is_empty() {
        return Err("分支名不能为空".into());
    }
    let repo = repo_path(repo.as_deref().unwrap_or(""));
    let args = ["checkout", "-b", branch_name.as_str(), hash.as_str()];
    let res = git_exec(&repo, &args, Duration::from_secs(15));
    if !res.success {
        return Err(format!("创建分支失败: {}", res.output.trim()));
    }
    Ok(())
}
