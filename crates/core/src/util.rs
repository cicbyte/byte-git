//! 基础设施：应用目录布局 + 子进程命令执行。
//!
//! 目录契约与命令执行工具。

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::process::Command;

/// 应用目录布局（惰性解析，`BYTE_GIT_HOME` 可覆盖根目录）。
///
/// 布局契约（对齐上游 cef8f58 的 apps 层级）：
/// - `~/.cicbyte/apps/byte-git/`            应用根
/// - `~/.cicbyte/apps/byte-git/config/`     全局配置 config.yaml
/// - `~/.cicbyte/apps/byte-git/backups/`    完整仓库备份
/// - `~/.cicbyte/apps/byte-git/prompts/`    AI 提示词模板
/// - `~/.cicbyte/apps/byte-git/downloads/`  下载归档
/// - `~/.cicbyte/apps/byte-git/rules/`      Lua 代理规则
/// - `~/.cicbyte/data/`                     跨项目共享数据（所有 cicbyte 工具复用）
///
/// 旧布局 `~/.cicbyte/byte-git/` 在首次解析时自动迁移（原子 rename）。
#[derive(Debug, Clone)]
pub struct AppPaths {
    root: PathBuf,
}

impl AppPaths {
    /// 解析应用根目录：`BYTE_GIT_HOME` > `~/.cicbyte/apps/byte-git`。
    /// 旧根 `~/.cicbyte/byte-git` 存在且新根不存在时整体迁移。
    pub fn new() -> Result<Self> {
        if let Ok(custom) = std::env::var("BYTE_GIT_HOME") {
            if !custom.is_empty() {
                return Ok(Self { root: PathBuf::from(custom) });
            }
        }
        let home = dirs::home_dir().context("无法定位用户主目录")?;
        let cicbyte = home.join(".cicbyte");
        let root = cicbyte.join("apps").join("byte-git");
        let legacy = cicbyte.join("byte-git");
        if !root.exists() && legacy.exists() {
            // 一次性迁移：同卷 rename 原子完成；失败（文件被占用等）保留旧目录，
            // 继续用新路径（数据仍在旧目录，用户可手动搬移）。
            if let Err(e) = std::fs::rename(&legacy, &root) {
                eprintln!("迁移旧数据目录 {} → {} 失败: {e}", legacy.display(), root.display());
            }
        }
        Ok(Self { root })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn config_dir(&self) -> PathBuf {
        self.root.join("config")
    }

    pub fn settings_file(&self) -> PathBuf {
        self.config_dir().join("config.yaml")
    }

    pub fn backups_dir(&self) -> PathBuf {
        self.root.join("backups")
    }

    pub fn prompts_dir(&self) -> PathBuf {
        self.root.join("prompts")
    }

    pub fn downloads_dir(&self) -> PathBuf {
        self.root.join("downloads")
    }

    pub fn rules_dir(&self) -> PathBuf {
        self.root.join("rules")
    }

    pub fn rules_file(&self) -> PathBuf {
        self.rules_dir().join("rules.lua")
    }

    pub fn logs_dir(&self) -> PathBuf {
        self.root.join("logs")
    }

    /// 跨项目共享数据目录 `~/.cicbyte/data`。
    pub fn shared_data_dir(&self) -> Result<PathBuf> {
        let home = dirs::home_dir().context("无法定位用户主目录")?;
        Ok(home.join(".cicbyte").join("data"))
    }

    /// Git 备忘录内容仓库（共享数据下的 git-cheat-content）。
    pub fn cheat_dir(&self) -> Result<PathBuf> {
        Ok(self.shared_data_dir()?.join("git-cheat-content"))
    }

    /// 确保目录存在（幂等）。
    pub fn ensure_dir(path: &Path) -> Result<()> {
        std::fs::create_dir_all(path)
            .with_context(|| format!("创建目录失败: {}", path.display()))
    }
}

impl Default for AppPaths {
    fn default() -> Self {
        Self::new().expect("无法解析应用目录（BYTE_GIT_HOME/home 均不可用）")
    }
}

/// 仓库内的 byte-git 数据目录：`<git-dir>/byte-git/`。
///
/// 仓库级数据目录。
pub fn repo_data_dir(repo: &Path) -> Result<PathBuf> {
    let git_dir = git_dir(repo)?;
    let dir = git_dir.join("byte-git");
    std::fs::create_dir_all(&dir)
        .with_context(|| format!("创建项目数据目录失败: {}", dir.display()))?;
    Ok(dir)
}

/// 解析仓库的 `.git` 目录（支持 worktree/子模块的相对 git-dir）。
pub fn git_dir(repo: &Path) -> Result<PathBuf> {
    let out = run_capture(repo, ["rev-parse", "--git-dir"])
        .context("不在 git 仓库中")?;
    let p = PathBuf::from(out.trim());
    if p.is_absolute() {
        Ok(p)
    } else {
        Ok(repo.join(p))
    }
}

// ---------------------------------------------------------------------------
// 子进程执行（sysproc 等价物）
// ---------------------------------------------------------------------------

/// Windows 下隐藏子进程控制台窗口：GUI 进程 spawn git 等控制台程序时，
/// 缺省会为每个子进程新建并闪现一个 CMD 窗口。仅用于输出全捕获的
/// 非交互命令（管道不依赖控制台）；交互/透传命令不适用。
#[cfg(windows)]
fn hide_console(cmd: &mut Command) {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    cmd.creation_flags(CREATE_NO_WINDOW);
}

#[cfg(not(windows))]
fn hide_console(_cmd: &mut Command) {}

/// 在仓库目录执行 git 命令并捕获 stdout（失败返回带 stderr 的错误）。
pub fn run_capture<I, S>(repo: &Path, args: I) -> Result<String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let args: Vec<String> = args.into_iter().map(|s| s.as_ref().to_string()).collect();
    let mut cmd = Command::new("git");
    cmd.arg("-C").arg(repo).args(&args);
    hide_console(&mut cmd);
    let out = cmd
        .output()
        .with_context(|| format!("启动 git 失败: git {}", args.join(" ")))?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
        anyhow::bail!("git {} 失败: {}", args.join(" "), stderr);
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// 在仓库目录执行 git 命令（stdout 直接透传到终端，适合交互式命令）。
pub fn run_inherit<I, S>(repo: &Path, args: I) -> Result<()>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let args: Vec<String> = args.into_iter().map(|s| s.as_ref().to_string()).collect();
    let status = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(&args)
        .status()
        .with_context(|| format!("启动 git 失败: git {}", args.join(" ")))?;
    if !status.success() {
        anyhow::bail!("git {} 失败（退出码 {:?}）", args.join(" "), status.code());
    }
    Ok(())
}

/// 任意程序执行（捕获输出）。对应 sysproc.Command(...).Output()。
pub fn exec_capture(program: &str, args: &[&str]) -> Result<String> {
    let mut cmd = Command::new(program);
    cmd.args(args);
    hide_console(&mut cmd);
    let out = cmd
        .output()
        .with_context(|| format!("启动 {} 失败", program))?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
        anyhow::bail!("{} {} 失败: {}", program, args.join(" "), stderr);
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// 任意程序执行（stdout/stderr 透传）。对应 sysproc.Command(...).Run()。
pub fn exec_inherit(program: &str, args: &[&str]) -> Result<()> {
    let status = Command::new(program)
        .args(args)
        .status()
        .with_context(|| format!("启动 {} 失败", program))?;
    if !status.success() {
        anyhow::bail!("{} {} 失败（退出码 {:?}）", program, args.join(" "), status.code());
    }
    Ok(())
}

/// 在仓库目录执行 git 命令（透传 + 附加环境变量）。
pub fn run_inherit_env<I, S>(repo: &Path, args: I, envs: &[(&str, &str)]) -> Result<()>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let args: Vec<String> = args.into_iter().map(|s| s.as_ref().to_string()).collect();
    let mut cmd = Command::new("git");
    cmd.arg("-C").arg(repo).args(&args);
    for (k, v) in envs {
        cmd.env(k, v);
    }
    let status = cmd
        .status()
        .with_context(|| format!("启动 git 失败: git {}", args.join(" ")))?;
    if !status.success() {
        anyhow::bail!("git {} 失败（退出码 {:?}）", args.join(" "), status.code());
    }
    Ok(())
}

/// 旧镜像备份（`--mirror` 裸仓库直接放在 backups/<repo>/ 下）的内部条目名——
/// 迁移为标准条目后需清理，避免被 GUI 当成备份列表显示。
pub const BACKUP_LEGACY_BARE_ENTRIES: &[&str] = &[
    "HEAD",
    "config",
    "description",
    "packed-refs",
    "byte-git-source-path",
    "info",
    "hooks",
    "refs",
    "objects",
    "logs",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_paths_layout() {
        let p = AppPaths::new().unwrap();
        assert!(p.settings_file().ends_with("config.yaml"));
        assert!(p.settings_file().starts_with(p.root()));
    }

    #[test]
    fn repo_data_dir_in_git_repo() {
        // 本仓库自身即 git 仓库
        let dir = repo_data_dir(Path::new(".")).unwrap();
        assert!(dir.exists());
        assert!(dir.ends_with("byte-git"));
    }
}
