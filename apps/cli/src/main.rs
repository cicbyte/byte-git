//! byte-git CLI——Git 增强工具集。
//!
//! 功能规格移植自 git-mate（Go），Rust 实现参照 byte-writer 的 CLI 结构。
//! Phase 1 已实现核心命令；AI 改写/审查/密钥扫描等重型模块在后续阶段接入。

mod commands;

use clap::Parser;

/// byte-git —— Git 增强工具集（分支治理 / stash / 统计 / 备份 / 许可证 / gitignore）
#[derive(Parser, Debug)]
#[command(name = "byte-git", version, about, long_about = None)]
struct Cli {
    /// 目标仓库路径（默认当前目录）
    #[arg(long, global = true)]
    repo: Option<String>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(clap::Subcommand, Debug)]
enum Commands {
    /// 查看版本信息
    Version,
    /// 管理全局配置（~/.cicbyte/apps/byte-git/config/config.yaml）
    Config(commands::config::ConfigCmd),
    /// 批量清理已合并分支、列出孤立分支
    Branch(commands::branch::BranchCmd),
    /// 带名称保存/弹出 stash，支持搜索
    Stash(commands::stash::StashCmd),
    /// 仓库概览与健康检查
    Info(commands::info::InfoCmd),
    /// 多仓库开发统计（提交/时段/连续天数/语言分布）
    Stats(commands::stats::StatsCmd),
    /// 完整仓库备份（git clone --mirror）
    Backup(commands::backup::BackupCmd),
    /// 开源许可证创建与检测
    License(commands::license::LicenseCmd),
    /// .gitignore 模板管理
    Ignore(commands::ignore::IgnoreCmd),
    /// 代理加速克隆 Git 仓库（Lua 规则引擎）
    Clone(commands::clone::CloneCmd),
    /// 下载 GitHub Release 资产
    Download(commands::download::DownloadCmd),
    /// 归档仓库快照（tar.gz/zip/tar）
    Archive(commands::archive::ArchiveCmd),
    /// AI 提交信息生成与历史改写
    Ai(commands::ai::AiCmd),
    /// 提交历史元数据改写（export/apply）
    Rewrite(commands::rewrite::RewriteCmd),
    /// AI 代码审查（集成 OpenCodeReview）
    Review(commands::review::ReviewCmd),
    /// 密钥泄露扫描（集成 gitleaks）
    Secret(commands::secret::SecretCmd),
    /// Git 钩子管理（shebang 脚本：shell/python/node）
    Hook(commands::hook::HookCmd),
    /// Rebase 预览与自动 squash
    Rebase(commands::rebase::RebaseCmd),
    /// 安全撤销（上次提交 / rebase / merge）
    Undo(commands::undo::UndoCmd),
    /// Git 日志 TUI 浏览
    Logview(commands::logview::LogviewCmd),
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let repo = cli.repo.unwrap_or_else(|| ".".into());
    let repo_path = std::path::PathBuf::from(&repo);

    match cli.command {
        Commands::Version => {
            println!("byte-git {} (Rust, phase 1)", env!("CARGO_PKG_VERSION"));
        }
        Commands::Config(c) => commands::config::run(c)?,
        Commands::Branch(c) => commands::branch::run(&repo_path, c)?,
        Commands::Stash(c) => commands::stash::run(&repo_path, c)?,
        Commands::Info(c) => commands::info::run(&repo_path, c)?,
        Commands::Stats(c) => commands::stats::run(c)?,
        Commands::Backup(c) => commands::backup::run(c)?,
        Commands::License(c) => commands::license::run(&repo_path, c)?,
        Commands::Ignore(c) => commands::ignore::run(&repo_path, c)?,
        Commands::Clone(c) => commands::clone::run(c)?,
        Commands::Download(c) => commands::download::run(c)?,
        Commands::Archive(c) => commands::archive::run(&repo_path, c)?,
        Commands::Ai(c) => commands::ai::run(&repo_path, c)?,
        Commands::Rewrite(c) => commands::rewrite::run(&repo_path, c)?,
        Commands::Review(c) => commands::review::run(c)?,
        Commands::Secret(c) => commands::secret::run(c)?,
        Commands::Hook(c) => commands::hook::run(&repo_path, c)?,
        Commands::Rebase(c) => commands::rebase::run(&repo_path, c)?,
        Commands::Undo(c) => commands::undo::run(&repo_path, c)?,
        Commands::Logview(c) => commands::logview::run(&repo_path, c)?,
    }
    Ok(())
}
