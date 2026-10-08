//! AI 代码审查命令（OpenCodeReview 集成）。

use byte_git_core::config::Settings;
use byte_git_core::ocr as core_ocr;
use clap::Args;

#[derive(Args, Debug)]
pub struct ReviewCmd {
    /// 目标仓库路径（默认当前目录）
    #[arg(value_name = "REPO")]
    pub repo: Option<String>,

    /// range 起点（与 --to 搭配）
    #[arg(long)]
    pub from: Option<String>,

    /// range 终点（与 --from 搭配）
    #[arg(long)]
    pub to: Option<String>,

    /// 单次 commit（vs 其父）
    #[arg(short = 'c', long)]
    pub commit: Option<String>,

    /// 覆盖本次审查模型
    #[arg(long)]
    pub model: Option<String>,

    /// 需求/业务上下文
    #[arg(short = 'b', long)]
    pub background: Option<String>,

    /// ocr 可执行文件路径（覆盖配置）
    #[arg(long)]
    pub ocr_path: Option<String>,
}

pub fn run(c: ReviewCmd) -> anyhow::Result<()> {
    let repo = c.repo.unwrap_or_else(|| ".".into());

    let exe = match core_ocr::find_exe(c.ocr_path.as_deref().unwrap_or("")) {
        Ok(p) => p,
        Err(e) => {
            println!("❌ {e}");
            println!("安装: npm i -g @alibaba-group/open-code-review");
            return Ok(());
        }
    };
    println!("🔍 使用 OCR {} ({})", core_ocr::version(&exe), exe);

    let s = Settings::load().unwrap_or_default();
    let res = core_ocr::review(&core_ocr::ReviewOptions {
        repo_dir: repo,
        from: c.from.unwrap_or_default(),
        to: c.to.unwrap_or_default(),
        commit: c.commit.unwrap_or_default(),
        model: c.model.unwrap_or_default(),
        background: c.background.unwrap_or_default(),
        exe_path: exe,
        concurrency: s.ocr.concurrency,
        max_tools: s.ocr.max_tools,
        audience: s.ocr.audience.clone(),
        language: s.ocr.language.clone(),
    })?;
    core_ocr::print_result(&res);
    Ok(())
}
