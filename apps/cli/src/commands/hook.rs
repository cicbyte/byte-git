//! Git 钩子管理命令（Lua / 脚本）。

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
    /// 列出钩子槽位状态
    List,
    /// 安装钩子（--lua 或 --script 二选一）
    Install {
        /// 钩子名（如 pre-commit）
        name: String,
        /// Lua 脚本（须定义 main(hook_name) → 0/非零）
        #[arg(long)]
        lua: Option<String>,
        /// 普通可执行脚本
        #[arg(long)]
        script: Option<String>,
    },
    /// 移除钩子
    Remove { name: String },
    /// 直接执行某钩子的 Lua 脚本（wrapper 内部使用）
    Exec {
        /// 钩子名
        name: String,
        /// 额外参数（git 传入的文件路径等）
        #[arg(value_name = "ARG")]
        args: Vec<String>,
    },
}

pub fn run(repo: &Path, c: HookCmd) -> anyhow::Result<()> {
    match c.cmd {
        HookSub::List => core_hook::print_list(repo),
        HookSub::Install { name, lua, script } => core_hook::install(repo, &core_hook::InstallOptions {
            hook_name: name,
            lua_file: lua.unwrap_or_default(),
            script: script.unwrap_or_default(),
        }),
        HookSub::Remove { name } => core_hook::remove(repo, &name),
        HookSub::Exec { name, args } => {
            let result = core_hook::exec(repo, &name, &args)?;
            if !result.error.is_empty() {
                eprintln!("{}", result.error);
            }
            std::process::exit(result.exit_code);
        }
    }
}
