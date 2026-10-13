//! Git 钩子管理命令（shebang 脚本）。
//!
//! 对齐上游 03915e0：移除 Lua 路径与 exec 子命令，统一 shebang 直执行；
//! remove 支持 local/global 作用域（core.hooksPath 感知）；
//! run 为 husky 式入口（commit-msg，支持 --ai 改写）。

use byte_git_core::hook as core_hook;
use clap::{Args, Subcommand};
use std::path::Path;

#[derive(Args, Debug)]
pub struct HookCmd {
    #[command(subcommand)]
    pub cmd: HookSub,
}

#[derive(Subcommand, Debug)]
pub enum HookSub {
    /// 列出钩子槽位状态（按 git 实际生效的 hooks 目录）
    List,
    /// 安装钩子（shebang 脚本：shell/python/node）
    Install {
        /// 钩子名（如 pre-commit）
        name: String,
        /// 脚本文件路径
        #[arg(long)]
        script: Option<String>,
    },
    /// 移除钩子
    Remove {
        /// 钩子名
        name: String,
        /// 作用域：local（默认）/ global
        #[arg(long, default_value = "local")]
        scope: String,
    },
    /// 执行 Git 钩子（husky 式入口，目前支持 commit-msg）
    Run {
        /// 钩子名（commit-msg）
        name: String,
        /// 使用 AI 改写当前 commit message
        #[arg(long)]
        ai: bool,
        /// AI 模型名称
        #[arg(long, default_value = "gemma4:e4b")]
        model: String,
        /// Ollama API 地址
        #[arg(long, default_value = "http://localhost:11434")]
        base_url: String,
        /// 输出语言 (zh/en)
        #[arg(long, default_value = "zh")]
        lang: String,
        /// commit-msg 文件路径（git 调用钩子时以尾参传入，通常无需手填）
        #[arg(trailing_var_arg = true, hide = true)]
        file: Vec<String>,
    },
}

pub fn run(repo: &Path, c: HookCmd) -> anyhow::Result<()> {
    match c.cmd {
        HookSub::List => core_hook::print_list(repo),
        HookSub::Install { name, script } => core_hook::install(repo, &core_hook::InstallOptions {
            hook_name: name,
            script: script.unwrap_or_default(),
        }),
        HookSub::Remove { name, scope } => core_hook::remove(repo, &name, &scope),
        HookSub::Run { name, ai, model, base_url, lang, file } => {
            // git 调用钩子脚本时把 COMMIT_EDITMSG 作为尾参传入——
            // env 未设时以进程尾参解析（对齐上游 os.Args[len-1] 语义）
            if std::env::var("BYTE_GIT_COMMIT_MSG_FILE")
                .or_else(|_| std::env::var("GIT_MATE_COMMIT_MSG_FILE"))
                .is_err()
            {
                if let Some(last) = file.last() {
                    std::env::set_var("BYTE_GIT_COMMIT_MSG_FILE", last);
                }
            }
            core_hook::run_hook(&core_hook::HookRunOptions {
                repo: repo.display().to_string(),
                hook_name: name,
                ai,
                model,
                base_url,
                lang,
            })
        }
    }
}
