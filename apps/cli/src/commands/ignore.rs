//! ignore：.gitignore 模板管理。

use byte_git_core::ignore;
use clap::{Args, Subcommand};
use std::path::Path;

#[derive(Args, Debug)]
pub struct IgnoreCmd {
    #[command(subcommand)]
    pub cmd: IgnoreSub,
}

#[derive(Subcommand, Debug)]
pub enum IgnoreSub {
    /// 列出可用模板
    List,
    /// 追加模板到 .gitignore（幂等）
    Add {
        /// 模板名（ignore list 查看）或任意规则
        templates: Vec<String>,
    },
}

pub fn run(repo: &Path, cmd: IgnoreCmd) -> anyhow::Result<()> {
    match cmd.cmd {
        IgnoreSub::List => {
            for n in ignore::list() {
                println!("  {n}");
            }
        }
        IgnoreSub::Add { templates } => {
            let known = ignore::list();
            let (tpls, rules): (Vec<String>, Vec<String>) = templates
                .into_iter()
                .partition(|t| known.iter().any(|k| k.eq_ignore_ascii_case(t)));
            for t in &tpls {
                ignore::add(repo, t)?;
            }
            if !rules.is_empty() {
                ignore::add_rules(repo, &rules)?;
            }
        }
    }
    Ok(())
}
