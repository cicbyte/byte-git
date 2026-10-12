//! Git 钩子管理命令（shebang 脚本）。
//!
//! 对齐上游 03915e0：移除 Lua 路径与 exec 子命令，统一 shebang 直执行；
//! remove 支持 local/global 作用域（core.hooksPath 感知）。

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
}

pub fn run(repo: &Path, c: HookCmd) -> anyhow::Result<()> {
    match c.cmd {
        HookSub::List => core_hook::print_list(repo),
        HookSub::Install { name, script } => core_hook::install(repo, &core_hook::InstallOptions {
            hook_name: name,
            script: script.unwrap_or_default(),
        }),
        HookSub::Remove { name, scope } => core_hook::remove(repo, &name, &scope),
    }
}
