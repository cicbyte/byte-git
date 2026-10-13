//! Tauri 命令层根模块。
//!
//! Phase 1 的少量根命令（version/config/git_status/branch_clean/ignore_add）
//! 保留；其余域命令在 commands/ 子目录（commit/gitops/backup/info/repo/
//! rewrite/secret/license/system/ignore），对应 git-mate gui/internal 布局。

use byte_git_core::branch::CleanOptions;
use byte_git_core::config::Settings;
use byte_git_core::{git, util};
use byte_git_core::ignore as core_ignore;
use serde_json::Value;
use std::path::PathBuf;

pub mod backup;
pub mod commit;
pub mod gitops;
pub mod github;
pub mod ignore;
pub mod info;
pub mod license;
pub mod live;
pub mod reference;
pub mod repo;
pub mod rewrite;
pub mod secret;
pub mod system;
pub mod terminal;
pub mod wallpaper;

fn repo_path(repo: &str) -> PathBuf {
    PathBuf::from(if repo.is_empty() { "." } else { repo })
}

#[tauri::command]
pub fn version() -> String {
    format!("byte-git {} (Rust)", env!("CARGO_PKG_VERSION"))
}

/// 取消当前长任务（对应 Go core.CancelTask）。
#[tauri::command]
pub fn cancel_task() -> bool {
    crate::tasks::cancel()
}

#[tauri::command]
pub fn git_status(repo: String) -> Result<Value, String> {
    let s = git::status(&repo_path(&repo)).map_err(|e| e.to_string())?;
    serde_json::to_value(s).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn branch_clean(repo: String, dry_run: bool) -> Result<Value, String> {
    let (candidates, deleted) = byte_git_core::branch::clean(
        &repo_path(&repo),
        &CleanOptions { dry_run, ..Default::default() },
    )
    .map_err(|e| e.to_string())?;
    Ok(serde_json::json!({ "candidates": candidates, "deleted": deleted }))
}

#[tauri::command]
pub fn ignore_list() -> Vec<String> {
    core_ignore::list()
}

#[tauri::command]
pub fn ignore_add(repo: String, template: String) -> Result<(), String> {
    core_ignore::add(&repo_path(&repo), &template).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn config_get(key: String) -> Result<Option<String>, String> {
    let s = Settings::load().map_err(|e| e.to_string())?;
    Ok(s.get(&key))
}

#[tauri::command]
pub fn config_set(key: String, value: String) -> Result<(), String> {
    // 复用 CLI 同款受控写入逻辑：经 JSON 序列化回写
    let mut s = Settings::load().map_err(|e| e.to_string())?;
    let mut json = serde_json::to_value(&s).map_err(|e| e.to_string())?;
    let obj = json.as_object_mut().ok_or("配置序列化异常")?;
    let (group, field) = key.split_once('.').ok_or("键格式应为 <分组>.<字段>")?;
    let section = obj
        .get_mut(group)
        .and_then(|v| v.as_object_mut())
        .ok_or(format!("未知配置分组: {group}"))?;
    if !section.contains_key(field) {
        return Err(format!("未知配置字段: {key}"));
    }
    let parsed: Value = value.parse::<i64>().map(Into::into)
        .or_else(|_| value.parse::<bool>().map(Into::into))
        .unwrap_or_else(|_| value.clone().into());
    section.insert(field.to_string(), parsed);
    s = serde_json::from_value(json).map_err(|e| e.to_string())?;
    s.save().map_err(|e| e.to_string())
}

/// 预留：应用目录信息（供前端设置页展示）。
#[allow(dead_code)]
fn app_root_info() -> Result<String, String> {
    Ok(util::AppPaths::new().map_err(|e| e.to_string())?.root().display().to_string())
}
