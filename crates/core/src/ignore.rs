//! .gitignore 管理：内嵌常用模板 + 追加写入。
//!

use anyhow::{bail, Result};
use include_dir::{include_dir, Dir};
use std::path::Path;

static TEMPLATES: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/assets/gitignore");

/// 可用模板列表（不含 .gitignore 后缀）。
pub fn list() -> Vec<String> {
    let mut names: Vec<String> = TEMPLATES
        .files()
        .map(|f| {
            f.path()
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default()
        })
        .filter(|n| !n.is_empty())
        .collect();
    names.sort();
    names
}

/// 将模板内容追加到仓库 .gitignore（带分段注释，幂等：同名模板不重复追加）。
pub fn add(repo: &Path, template: &str) -> Result<()> {
    let file_name = format!("{template}.gitignore");
    let Some(tpl) = TEMPLATES.get_file(&file_name) else {
        bail!("未知模板: {template}（ignore list 查看可用列表）");
    };
    let content = std::str::from_utf8(tpl.contents())
        .map_err(|e| anyhow::anyhow!("模板编码错误: {e}"))?;

    let marker = format!("# --- byte-git: {template} ---");
    let dest = repo.join(".gitignore");
    let existing = std::fs::read_to_string(&dest).unwrap_or_default();
    if existing.contains(&marker) {
        println!("模板 {template} 已存在于 .gitignore，跳过");
        return Ok(());
    }

    let block = format!("\n{marker}\n{content}\n");
    std::fs::write(&dest, existing + &block)?;
    println!("已追加 {template} 模板到 .gitignore");
    Ok(())
}

/// 通用忽略规则追加（任意若干条目）。
pub fn add_rules(repo: &Path, rules: &[String]) -> Result<()> {
    if rules.is_empty() {
        bail!("请至少提供一条规则");
    }
    let dest = repo.join(".gitignore");
    let existing = std::fs::read_to_string(&dest).unwrap_or_default();
    let block = format!("\n# --- byte-git: custom ---\n{}\n", rules.join("\n"));
    std::fs::write(&dest, existing + &block)?;
    println!("已追加 {} 条规则", rules.len());
    Ok(())
}
