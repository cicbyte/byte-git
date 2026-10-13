//! AI 命令：提交信息生成 + 历史批量改写。

use byte_git_core::aicommit::{self, CommitOptions};
use byte_git_core::airewrite;
use byte_git_core::config::Settings;
use clap::{Args, Subcommand};
use std::path::Path;

#[derive(Args, Debug)]
pub struct AiCmd {
    #[command(subcommand)]
    pub cmd: AiSub,
}

#[derive(Subcommand, Debug)]
pub enum AiSub {
    /// AI 生成 commit message 并提交（暂存区 diff → LLM）
    Commit {
        /// AI 模型名称
        #[arg(long)]
        model: Option<String>,
        /// API Base URL
        #[arg(long)]
        base_url: Option<String>,
        /// API Key（Ollama 无需填写）
        #[arg(long)]
        api_key: Option<String>,
        /// 描述语言 (zh/en)
        #[arg(long)]
        lang: Option<String>,
        /// diff 部分 token 上限
        #[arg(long)]
        max_tokens: Option<i64>,
        /// 仅预览生成结果，不提交
        #[arg(long)]
        dry_run: bool,
        /// 跳过 Git hooks
        #[arg(long)]
        no_verify: bool,
    },
    /// 提交历史 AI 改写（规则检测 → 批量生成 → 应用）
    Rewrite {
        #[command(subcommand)]
        cmd: AiRewriteSub,
    },
}

#[derive(Subcommand, Debug)]
pub enum AiRewriteSub {
    /// 导出待改写提交清单（JSON）
    Gen {
        /// 最近 N 条
        #[arg(long)]
        last: Option<usize>,
        /// 提交区间（A..B）
        #[arg(long)]
        range: Option<String>,
        /// 仅未推送提交
        #[arg(long)]
        unpushed: bool,
        /// 按规则自动标记不规范提交
        #[arg(long)]
        auto_detect: bool,
        /// 按正则标记
        #[arg(long)]
        pattern: Option<String>,
        /// 输出文件（默认 .git/byte-git/rewrite/ai-rewrite.json）
        #[arg(short = 'o', long)]
        output: Option<String>,
    },
    /// 批量调用 AI 生成改写建议
    Run {
        /// 输入/输出 JSON 文件
        #[arg(short = 'f', long)]
        file: Option<String>,
        /// 重试失败条目
        #[arg(long)]
        retry: bool,
        /// 仅预览（不调用 AI）
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        model: Option<String>,
        #[arg(long)]
        base_url: Option<String>,
        #[arg(long)]
        api_key: Option<String>,
        #[arg(long)]
        lang: Option<String>,
        /// 单条 prompt 字符上限
        #[arg(long, default_value = "12000")]
        max_chars: usize,
    },
    /// 批准并应用到历史（filter-branch）
    Apply {
        /// JSON 文件
        #[arg(short = 'f', long)]
        file: Option<String>,
        /// 目标分支（默认 HEAD）
        #[arg(long)]
        branch: Option<String>,
        /// 仅预览级联影响，不执行
        #[arg(long)]
        dry_run: bool,
        /// 跳过确认直接执行（危险）
        #[arg(long)]
        yes: bool,
    },
    /// 查看/管理检测规则
    Rules {
        /// 列出规则（默认）
        #[arg(long)]
        list: bool,
    },
}

pub fn run(repo: &Path, c: AiCmd) -> anyhow::Result<()> {
    match c.cmd {
        AiSub::Commit { model, base_url, api_key, lang, max_tokens, dry_run, no_verify } => {
            // 配置回退链：命令行参数 > config.yaml > 内置默认（对齐 git-mate）
            let s = Settings::load().unwrap_or_default();
            let opts = CommitOptions {
                repo: repo.display().to_string(),
                model: model.unwrap_or_else(|| s.ai.model.clone()),
                base_url: base_url.unwrap_or_else(|| s.ai.base_url.clone()),
                api_key: api_key.unwrap_or_else(|| s.ai.api_key.clone()),
                lang: lang.unwrap_or_else(|| s.ai.lang.clone()),
                max_tokens: max_tokens.unwrap_or(s.ai.max_tokens),
                dry_run,
                no_verify,
            };
            byte_git_core::ai::init_prompts()?;
            aicommit::run_commit_flow(&opts)
        }
        AiSub::Rewrite { cmd } => run_rewrite(repo, cmd),
    }
}

fn run_rewrite(repo: &Path, cmd: AiRewriteSub) -> anyhow::Result<()> {
    match cmd {
        AiRewriteSub::Gen { last, range, unpushed, auto_detect, pattern, output } => {
            airewrite::gen(&airewrite::GenOptions {
                repo: repo.display().to_string(),
                all: false,
                range: range.unwrap_or_default(),
                last: last.unwrap_or(0),
                unpushed,
                auto_detect,
                pattern: pattern.unwrap_or_default(),
                output: output.unwrap_or_default(),
            })
        }
        AiRewriteSub::Run { file, retry, dry_run, model, base_url, api_key, lang, max_chars } => {
            let s = Settings::load().unwrap_or_default();
            let file = file.map(|f| Ok(f)).unwrap_or_else(|| airewrite::default_rewrite_file(repo).map(|p| p.display().to_string()))?;
            airewrite::run(&airewrite::RunOptions {
                repo: repo.display().to_string(),
                file,
                model: model.unwrap_or_else(|| s.ai.model.clone()),
                base_url: base_url.unwrap_or_else(|| s.ai.base_url.clone()),
                api_key: api_key.unwrap_or_else(|| s.ai.api_key.clone()),
                lang: lang.unwrap_or_else(|| s.ai.lang.clone()),
                max_chars,
                retry,
                dry_run,
            })
        }
        AiRewriteSub::Apply { file, branch, dry_run, yes } => {
            let file = file.map(|f| Ok(f)).unwrap_or_else(|| airewrite::default_rewrite_file(repo).map(|p| p.display().to_string()))?;
            if !dry_run && !yes {
                println!("⚠ filter-branch 将改写提交历史（建议先 byte-git backup create 备份）");
                print!("确认继续? [y/N]: ");
                use std::io::Write;
                std::io::stdout().flush()?;
                let mut input = String::new();
                std::io::stdin().read_line(&mut input)?;
                if !input.trim().eq_ignore_ascii_case("y") {
                    println!("已取消");
                    return Ok(());
                }
            }
            airewrite::apply(&airewrite::ApplyOptions {
                repo: repo.display().to_string(),
                branch: branch.unwrap_or_default(),
                file,
                dry_run,
            })
        }
        AiRewriteSub::Rules { .. } => {
            let rules = airewrite::load_rules()?;
            println!("检测规则（{}）：~/.cicbyte/apps/byte-git/airewrite/rules.json", rules.len());
            for r in &rules {
                println!("  {:<10} /{}/  {}", r.name, r.pattern, r.desc);
            }
            Ok(())
        }
    }
}
