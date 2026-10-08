//! Tauri 命令层：薄封装 byte-git-core，供前端 invoke 调用。
//!
//! Phase 1 先暴露核心命令集；其余模块（github/rewrite/secret/...）
//! 随 core 实现逐步接入。

use byte_git_core::branch::{self, CleanOptions};
use byte_git_core::config::Settings;
use byte_git_core::{git, ignore, info, license, stash, util};
use serde_json::Value;
use std::path::PathBuf;

fn repo_path(repo: &str) -> PathBuf {
    PathBuf::from(if repo.is_empty() { "." } else { repo })
}

#[tauri::command]
pub fn version() -> String {
    format!("byte-git {} (Rust, phase 1)", env!("CARGO_PKG_VERSION"))
}

#[tauri::command]
pub fn info_summary(repo: String) -> Result<String, String> {
    info::summary(&repo_path(&repo)).map_err(|e| e.to_string())?;
    Ok("ok".into())
}

#[tauri::command]
pub fn git_status(repo: String) -> Result<Value, String> {
    let s = git::status(&repo_path(&repo)).map_err(|e| e.to_string())?;
    serde_json::to_value(s).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn branch_list(repo: String) -> Result<Vec<String>, String> {
    branch::local_branches(&repo_path(&repo)).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn branch_clean(repo: String, dry_run: bool) -> Result<Value, String> {
    let (candidates, deleted) = branch::clean(
        &repo_path(&repo),
        &CleanOptions { dry_run, ..Default::default() },
    )
    .map_err(|e| e.to_string())?;
    Ok(serde_json::json!({ "candidates": candidates, "deleted": deleted }))
}

#[tauri::command]
pub fn stash_list(repo: String, search: Option<String>) -> Result<Vec<stash::StashEntry>, String> {
    stash::list(&repo_path(&repo), search.as_deref().unwrap_or("")).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn stash_save(repo: String, name: String) -> Result<(), String> {
    stash::save(&repo_path(&repo), &name).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn stash_pop(repo: String, name: String) -> Result<(), String> {
    stash::pop(&repo_path(&repo), &name).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn license_list() -> Vec<String> {
    license::list()
}

#[tauri::command]
pub fn license_create(repo: String, name: String, author: Option<String>, year: Option<i32>) -> Result<(), String> {
    license::create(&repo_path(&repo), &name, author.as_deref().unwrap_or(""), year).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn ignore_list() -> Vec<String> {
    ignore::list()
}

#[tauri::command]
pub fn ignore_add(repo: String, template: String) -> Result<(), String> {
    ignore::add(&repo_path(&repo), &template).map_err(|e| e.to_string())
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
