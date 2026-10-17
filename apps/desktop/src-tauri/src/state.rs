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

/// recent-repos.json 结构：recent（按时间倒序）+ pinned（置顶，按置顶时间倒序）。
/// 旧格式为纯路径数组（迁移见 load_recent_file）。
#[derive(Default, serde::Serialize, serde::Deserialize)]
pub struct RecentFile {
    #[serde(default)]
    pub recent: Vec<String>,
    #[serde(default)]
    pub pinned: Vec<String>,
}

/// 读取（兼容旧格式：纯路径数组 → 全部视作 recent，无置顶）。
fn load_recent_file() -> RecentFile {
    let Ok(s) = std::fs::read_to_string(recent_file().unwrap_or_default()) else {
        return RecentFile::default();
    };
    if let Ok(rf) = serde_json::from_str::<RecentFile>(&s) {
        return rf;
    }
    // 旧格式：["pathA", "pathB"]
    if let Ok(list) = serde_json::from_str::<Vec<String>>(&s) {
        return RecentFile { recent: list, pinned: Vec::new() };
    }
    RecentFile::default()
}

fn save_recent_file(rf: &RecentFile) -> anyhow::Result<()> {
    let file = recent_file()?;
    if let Some(parent) = file.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&file, serde_json::to_string_pretty(rf)?)?;
    Ok(())
}

impl RecentFile {
    /// 从 recent 中摘出 path 并插到最前（取消置顶时回到队首）。
    fn retain_front(&mut self, path: &str) {
        self.recent.retain(|p| p != path);
        self.recent.insert(0, path.to_string());
    }
}

/// 记录最近仓库（最新在前，去重，无上限——byte-git 有意偏差上游 maxRecent=10）。
/// 置顶项位置稳定：打开置顶项目不改变其置顶序，也不进入 recent。
pub fn record_recent(path: &str) -> anyhow::Result<()> {
    let mut rf = load_recent_file();
    if rf.pinned.iter().any(|p| p == path) {
        return Ok(()); // 已置顶：保持不动
    }
    rf.recent.retain(|p| p != path);
    rf.recent.insert(0, path.to_string());
    save_recent_file(&rf)
}

/// 读最近仓库（recent + pinned 两个列表）。
pub fn load_recent() -> anyhow::Result<RecentFile> {
    Ok(load_recent_file())
}

/// 置顶/取消置顶：置顶 = recent→pinned 头部；取消 = pinned→recent 头部。
pub fn set_recent_pinned(path: &str, pinned: bool) -> anyhow::Result<()> {
    let mut rf = load_recent_file();
    if pinned {
        rf.recent.retain(|p| p != path);
        rf.pinned.retain(|p| p != path);
        rf.pinned.insert(0, path.to_string());
    } else {
        rf.retain_front(path);
    }
    save_recent_file(&rf)
}

/// 移除最近仓库记录（两列表都清）。
pub fn remove_recent(path: &str) -> anyhow::Result<()> {
    let mut rf = load_recent_file();
    rf.recent.retain(|p| p != path);
    rf.pinned.retain(|p| p != path);
    save_recent_file(&rf)
}
