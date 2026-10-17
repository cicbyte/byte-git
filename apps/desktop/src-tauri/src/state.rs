//! 当前仓库状态 + 最近仓库列表（对应 Go gui/internal/core Core.CurrentRepo
//! 与 RepoModule 的 recent repos）。
//!
//! Go 侧 Wails 绑定多数方法无 repo 参数，靠 Core 内部状态；
//! Rust 侧用全局 Mutex 状态 + `~/.cicbyte/apps/byte-git/recent-repos.json` 持久化。
//! 前端 SwitchRepo(path) 切换时更新；无状态时回落 "."。

use byte_git_core::util::AppPaths;
use std::path::PathBuf;
use std::sync::Mutex;

static CURRENT: Mutex<Option<String>> = Mutex::new(None);

/// 设置当前仓库（SwitchRepo 调用）。
pub fn set_current(path: &str) {
    let mut g = CURRENT.lock().unwrap();
    *g = Some(path.to_string());
}

/// 取当前仓库（无则 "."，对应 Go CurrentRepoOrDot）。
pub fn current_or_dot() -> String {
    CURRENT
        .lock()
        .unwrap()
        .clone()
        .unwrap_or_else(|| ".".to_string())
}

/// 命令参数解析：显式 repo 优先，否则当前仓库状态，最后 "."。
pub fn resolve(repo: Option<String>) -> PathBuf {
    let p = repo.filter(|s| !s.is_empty()).unwrap_or_else(current_or_dot);
    PathBuf::from(p)
}

fn recent_file() -> anyhow::Result<PathBuf> {
    Ok(AppPaths::new()?.root().join("recent-repos.json"))
}

/// 记录最近仓库（最新在前，去重，上限 30）。
/// 上游 maxRecent=10；byte-git 提高到 30——项目栏可滚动（rail-items
/// overflow-y:auto），10 个对多仓库切换场景偏紧。有意偏差。
pub fn record_recent(path: &str) -> anyhow::Result<()> {
    let file = recent_file()?;
    let mut list: Vec<String> = std::fs::read_to_string(&file)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();
    list.retain(|p| p != path);
    list.insert(0, path.to_string());
    list.truncate(30);
    if let Some(parent) = file.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&file, serde_json::to_string_pretty(&list)?)?;
    Ok(())
}

/// 读最近仓库列表。
pub fn load_recent() -> anyhow::Result<Vec<String>> {
    let file = recent_file()?;
    Ok(std::fs::read_to_string(&file)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default())
}

/// 移除最近仓库记录。
pub fn remove_recent(path: &str) -> anyhow::Result<()> {
    let file = recent_file()?;
    let mut list: Vec<String> = std::fs::read_to_string(&file)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();
    list.retain(|p| p != path);
    std::fs::write(&file, serde_json::to_string_pretty(&list)?)?;
    Ok(())
}
