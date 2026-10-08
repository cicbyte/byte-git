//! 密钥泄露扫描命令（gitleaks 集成）。

use byte_git_core::gitleaks as core_gitleaks;
use clap::Args;

#[derive(Args, Debug)]
pub struct SecretCmd {
    /// 目标仓库路径（默认当前目录）
    #[arg(value_name = "REPO")]
    pub repo: Option<String>,

    /// gitleaks 可执行文件路径（覆盖配置）
    #[arg(long)]
    pub gitleaks_path: Option<String>,

    /// 自定义 gitleaks 规则配置（.toml）
    #[arg(long)]
    pub config: Option<String>,

    /// 显示完整密钥（默认脱敏 前4+***+后4）
    #[arg(short = 'v', long)]
    pub verbose: bool,

    /// 扫描超时秒数（默认 300）
    #[arg(long)]
    pub timeout: Option<u64>,
}

pub fn run(c: SecretCmd) -> anyhow::Result<()> {
    let repo = c.repo.unwrap_or_else(|| ".".into());

    let exe = match core_gitleaks::find_exe(c.gitleaks_path.as_deref().unwrap_or("")) {
        Ok(p) => p,
        Err(e) => {
            println!("❌ {e}");
            println!();
            println!("安装方式:\n  go install github.com/zricethezav/gitleaks/v8@latest\n  或从 https://github.com/gitleaks/gitleaks/releases 下载");
            return Ok(());
        }
    };
    println!("🔍 使用 gitleaks {} ({})", core_gitleaks::version(&exe), exe);

    let res = core_gitleaks::scan(&core_gitleaks::ScanOptions {
        repo_dir: repo,
        exe_path: exe,
        config_path: c.config.unwrap_or_default(),
        verbose: c.verbose,
        timeout: c.timeout.unwrap_or(300),
    })?;
    core_gitleaks::print_result(&res);
    Ok(())
}
