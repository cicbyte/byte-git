//! ignore 域命令：.gitignore 模板管理（内置/GitHub/AI 三源）、
//! 合并预览与应用、文件忽略状态、分类规则与模板 CRUD。
//!
//! 模板目录在 `~/.cicbyte/apps/byte-git/gitignore/`：
//! - template/          本地模板（首次由内置模板播种，之后尊重用户改动）
//! - template_github/   `github/gitignore` 仓库的 depth-1 克隆
//! - categories.json    用户自定义分类规则
//!
//! 内置模板与 `crates/core/assets/gitignore` 同源，编译期经 include_str!
//! 嵌入（core::ignore 未暴露模板内容，GUI 侧需要文件目录形态的模板源）。

use byte_git_core::ai::{ChatClient, ChatMessage};
use byte_git_core::config::Settings;
use byte_git_core::util::AppPaths;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

/// 与 Go GUI 一致：无仓库上下文的命令统一接收可选 repo 参数；
/// 缺省回落到当前仓库状态（state.rs，对应 Core.CurrentRepo），最终 "."。
fn repo_path(repo: Option<String>) -> std::path::PathBuf { crate::state::resolve(repo) }

/// 子进程隐藏控制台窗口（sysproc.HideExec 的 Windows 等价物）。
#[cfg(windows)]
fn hide_window(cmd: &mut std::process::Command) {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    cmd.creation_flags(CREATE_NO_WINDOW);
}
#[cfg(not(windows))]
fn hide_window(_cmd: &mut std::process::Command) {}

// =====================================================================
// DTO（json tag 对齐 Go 结构体）
// =====================================================================

/// 合并/整理结果（原内容、合并后内容与 diff，供前端并排预览）。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IgnoreAddResult {
    pub added: i64,
    pub diff: String,
    /// 当前 .gitignore 内容（并排预览用）。
    pub original: String,
    /// 合并/整理后的内容（并排预览用）。
    pub updated: String,
    pub applied: bool,
}

/// 单文件忽略检查结果。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IgnoreCheckResult {
    pub ignored: bool,
    pub rule: String,
    pub source: String,
}

/// 批量忽略检查条目。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IgnoreCheckBatchItem {
    pub file: String,
    pub ignored: bool,
    pub rule: String,
    pub source: String,
}

/// 工作区文件 git 追踪/忽略状态。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileStatusEntry {
    pub path: String,
    pub is_dir: bool,
    /// "tracked" | "ignored" | "untracked"
    pub status: String,
    /// 命中的忽略规则（如有）。
    pub rule: String,
    /// 如 ".gitignore:12"（如有）。
    pub source: String,
}

/// 模板源状态。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IgnoreTemplateSource {
    /// "local" | "github"
    pub name: String,
    pub count: i64,
    pub ready: bool,
    /// github 源：仓库是否已克隆。
    pub cloned: bool,
    pub local_path: String,
}

/// 分类规则（镜像 core 侧 CategoryRule，供前端编辑）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IgnoreCategoryRule {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub keywords: Vec<String>,
}

// =====================================================================
// 模板管理器（TemplateManager 等价物）
// =====================================================================

/// 内置模板（与 crates/core/assets/gitignore 同源，编译期嵌入）。
const BUNDLED_TEMPLATES: &[(&str, &str)] = &[
    ("go", include_str!("../../../../../crates/core/assets/gitignore/go.gitignore")),
    ("macos", include_str!("../../../../../crates/core/assets/gitignore/macos.gitignore")),
    ("node", include_str!("../../../../../crates/core/assets/gitignore/node.gitignore")),
    ("python", include_str!("../../../../../crates/core/assets/gitignore/python.gitignore")),
    ("vscode", include_str!("../../../../../crates/core/assets/gitignore/vscode.gitignore")),
];

struct TemplateManager {
    /// ~/.cicbyte/apps/byte-git/gitignore/template
    local_dir: PathBuf,
    /// ~/.cicbyte/apps/byte-git/gitignore/template_github
    remote_dir: PathBuf,
}

impl TemplateManager {
    fn new() -> Result<Self, String> {
        let base = AppPaths::new().map_err(|e| e.to_string())?.root().join("gitignore");
        Ok(Self {
            local_dir: base.join("template"),
            remote_dir: base.join("template_github"),
        })
    }
}

/// 释放内置模板到用户目录（目录为空时播种，之后尊重用户改动）。
fn init_bundled_templates(dir: &Path) -> Result<(), String> {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for e in entries.flatten() {
            if e.file_name().to_string_lossy().ends_with(".gitignore") {
                return Ok(()); // 已有模板，不覆盖
            }
        }
    }
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    for (name, content) in BUNDLED_TEMPLATES {
        std::fs::write(dir.join(format!("{name}.gitignore")), content)
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// 列出目录下的模板语言名（目录不存在返回 None，与 Go 一致不算错误）。
fn list_templates(dir: &Path) -> Option<Vec<String>> {
    let entries = std::fs::read_dir(dir).ok()?;
    let mut names = Vec::new();
    for e in entries.flatten() {
        if e.path().is_dir() {
            continue;
        }
        let name = e.file_name().to_string_lossy().into_owned();
        if name.to_lowercase().ends_with(".gitignore") {
            names.push(name.strip_suffix(".gitignore").unwrap_or(&name).to_string());
        }
    }
    Some(names)
}

/// 在目录下查找 <lang>.gitignore（大小写不敏感）并读取内容。
fn read_template_file(lang: &str, dir: &Path) -> Result<String, String> {
    let entries = std::fs::read_dir(dir).map_err(|e| format!("模板目录不可读: {e}"))?;
    let target = format!("{}.gitignore", lang.to_lowercase());
    for e in entries.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        if !e.path().is_dir() && name.to_lowercase() == target {
            return std::fs::read_to_string(e.path()).map_err(|e| e.to_string());
        }
    }
    Err(format!("未找到语言 '{lang}' 的模板（在 {}）", dir.display()))
}

// =====================================================================
// git 子进程辅助（clone/pull/check-ignore/ls-files）
// =====================================================================

fn git_output(dir: Option<&Path>, args: &[&str]) -> Result<std::process::Output, String> {
    let mut cmd = std::process::Command::new("git");
    if let Some(d) = dir {
        cmd.arg("-C").arg(d);
    }
    cmd.args(args);
    hide_window(&mut cmd);
    cmd.output().map_err(|e| format!("启动 git 失败: {e}"))
}

fn combined(out: &std::process::Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    )
}

/// `git clone --depth 1 <url> <dest>`。
fn git_clone(url: &str, dest: &Path) -> Result<(), String> {
    let mut cmd = std::process::Command::new("git");
    cmd.args(["clone", "--depth", "1", url]).arg(dest);
    hide_window(&mut cmd);
    let out = cmd.output().map_err(|e| format!("git clone 失败: {e}"))?;
    if !out.status.success() {
        return Err(format!("git clone 失败: {}: {}", out.status, combined(&out).trim()));
    }
    Ok(())
}

/// `git -C <dir> pull --ff-only`。
fn git_pull(dir: &Path) -> Result<(), String> {
    let out = git_output(Some(dir), &["pull", "--ff-only"])?;
    if !out.status.success() {
        return Err(format!("git pull 失败: {}: {}", out.status, combined(&out).trim()));
    }
    Ok(())
}

/// `git rev-parse --git-dir` 验证目录是有效仓库。
fn is_valid_git_repo(dir: &Path) -> bool {
    git_output(Some(dir), &["rev-parse", "--git-dir"])
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// 确保 GitHub 模板已克隆：有效仓库则 pull 更新，否则（重）克隆。
fn ensure_github_cloned(tm: &TemplateManager) -> Result<(), String> {
    let git_dir = tm.remote_dir.join(".git");
    if git_dir.is_dir() {
        if is_valid_git_repo(&tm.remote_dir) {
            return git_pull(&tm.remote_dir);
        }
        // .git 存在但无效——清掉重新克隆
        let _ = std::fs::remove_dir_all(&tm.remote_dir);
    }
    if let Some(parent) = tm.remote_dir.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    git_clone("https://github.com/github/gitignore.git", &tm.remote_dir)
}

/// `git ls-files` 类调用：成功返回去空行后的行列表。
fn run_lines(repo: &Path, args: &[&str]) -> Result<Vec<String>, String> {
    let out = git_output(Some(repo), args)?;
    if !out.status.success() {
        return Err(combined(&out).trim().to_string());
    }
    Ok(String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect())
}

// =====================================================================
// .gitignore 更新器（GitignoreUpdater/MergeContent/PlainDiff/ExtractRules）
// =====================================================================

struct GitignoreUpdater {
    path: PathBuf,
}

impl GitignoreUpdater {
    fn new(path: PathBuf) -> Self {
        Self { path }
    }

    /// 读取内容；文件不存在返回空串而非错误。
    fn read_content(&self) -> Result<String, String> {
        match std::fs::read_to_string(&self.path) {
            Ok(s) => Ok(s),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
            Err(e) => Err(e.to_string()),
        }
    }

    /// 合并新内容（双去重：对已有规则 + 本批内重复），返回合并后完整内容与新增数。
    fn merge_content(&self, new_content: &str) -> Result<(String, i64), String> {
        let original = self.read_content()?;
        let mut existing = extract_rules(&original);

        let mut new_rules = String::new();
        let mut new_count: i64 = 0;
        let mut last_line_empty = false;
        let mut seen_in_new_batch: HashSet<String> = HashSet::new();
        for line in new_content.lines() {
            let trimmed = line.trim();
            if !trimmed.is_empty() && !existing.contains(trimmed) && !seen_in_new_batch.contains(trimmed) {
                existing.insert(trimmed.to_string());
                seen_in_new_batch.insert(trimmed.to_string());
                new_rules.push_str(line);
                new_rules.push('\n');
                new_count += 1;
                last_line_empty = false;
            } else if trimmed.is_empty() && !last_line_empty {
                new_rules.push('\n');
                last_line_empty = true;
            }
        }

        let mut final_content = original.clone();
        // 原始内容不以空行结尾时，先补空行分隔
        if !original.is_empty() && !original.ends_with("\n\n") {
            if !original.ends_with('\n') {
                final_content.push('\n');
            }
            final_content.push('\n');
        }
        final_content.push_str(&new_rules);
        Ok((final_content, new_count))
    }

    /// 写入 .gitignore。
    fn apply_changes(&self, content: &str) -> Result<(), String> {
        std::fs::write(&self.path, content).map_err(|e| e.to_string())
    }
}

/// 提取有效规则集合（非空行且非注释）。
fn extract_rules(content: &str) -> HashSet<String> {
    content
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(|l| l.to_string())
        .collect()
}

/// 纯文本行级 diff：仅输出 +/- 前缀的增删行（供 GUI 渲染）。
fn plain_diff(original: &str, updated: &str) -> String {
    let orig_lines: Vec<&str> = original.split('\n').collect();
    let new_lines: Vec<&str> = updated.split('\n').collect();
    let orig_set: HashSet<&str> = orig_lines.iter().copied().collect();
    let new_set: HashSet<&str> = new_lines.iter().copied().collect();
    let mut b = String::new();
    for l in &new_lines {
        if !orig_set.contains(l) {
            b.push_str("+ ");
            b.push_str(l);
            b.push('\n');
        }
    }
    for l in &orig_lines {
        if !new_set.contains(l) {
            b.push_str("- ");
            b.push_str(l);
            b.push('\n');
        }
    }
    b
}

// =====================================================================
// 分类规则（categories.json）
// =====================================================================

/// 内置默认分类（对齐 Go DefaultCategories）。
fn default_categories() -> Vec<IgnoreCategoryRule> {
    let mk = |name: &str, keywords: &[&str]| IgnoreCategoryRule {
        name: name.to_string(),
        keywords: keywords.iter().map(|s| s.to_string()).collect(),
    };
    vec![
        mk("构建产物", &["build/", "dist/", "bin/", "out/", "target/", "*.exe", "*.exe~", "*.dll", "*.so", "*.dylib", "*.test", "*.out", "*.class", "*.pyc", "*.o", "*.obj", "*.tgz", ".next", ".nuxt", "*.tsbuildinfo"]),
        mk("依赖", &["node_modules", "vendor/", ".env", "package-lock", "yarn.lock", "pnpm-lock", "go.sum", "__pycache__", ".venv", "venv/", ".gradle", ".m2"]),
        mk("IDE/编辑器", &[".vscode", ".idea", ".ds_store", "*.swp", "*.swo", "thumbs.db", ".project", ".settings/", "*.sublime", ".atom"]),
        mk("日志", &["*.log", "logs/", "npm-debug", "yarn-debug", "*.pid", "*.seed"]),
        mk("密钥/凭证", &["*.key", "*.pem", "*.cert", "secret", ".ssh", "credentials", "*.token", "*.p12"]),
        mk("测试覆盖率", &["coverage/", ".nyc_output", "*.lcov", ".pytest_cache"]),
        mk("系统文件", &[".ds_store", "thumbs.db", "desktop.ini", "$recycle.bin", "*.tmp", "*.bak"]),
    ]
}

fn categories_path() -> Result<PathBuf, String> {
    Ok(AppPaths::new().map_err(|e| e.to_string())?.root().join("gitignore").join("categories.json"))
}

/// 读取 categories.json（缺失/损坏回退默认值；首次缺失时播种默认值）。
fn load_categories() -> Result<Vec<IgnoreCategoryRule>, String> {
    let path = categories_path()?;
    match std::fs::read_to_string(&path) {
        Ok(data) => {
            let cats: Vec<IgnoreCategoryRule> =
                serde_json::from_str(&data).unwrap_or_default();
            if cats.is_empty() {
                Ok(default_categories())
            } else {
                Ok(cats)
            }
        }
        Err(e) => {
            if e.kind() == std::io::ErrorKind::NotFound {
                let _ = save_categories(&default_categories());
            }
            Ok(default_categories())
        }
    }
}

/// 写入 categories.json。
fn save_categories(cats: &[IgnoreCategoryRule]) -> Result<(), String> {
    let path = categories_path()?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let data = serde_json::to_string_pretty(cats).map_err(|e| e.to_string())?;
    std::fs::write(&path, data).map_err(|e| e.to_string())
}

/// 本地确定性整理：去重 + 按分类分组 + 注释标题 + 去多余空行（不调 AI）。
fn local_refactor(content: &str, cats: &[IgnoreCategoryRule]) -> String {
    let category_of = |pattern: &str| -> String {
        let p = pattern.to_lowercase();
        for cat in cats {
            for kw in &cat.keywords {
                if p.contains(&kw.to_lowercase()) {
                    return cat.name.clone();
                }
            }
        }
        "其他".to_string()
    };

    // 保持配置中的分类顺序，"其他" 恒排最后
    let mut category_order: Vec<String> = cats.iter().map(|c| c.name.clone()).collect();
    category_order.push("其他".to_string());

    let mut seen: HashSet<String> = HashSet::new();
    let mut groups: HashMap<String, Vec<String>> = HashMap::new();
    let mut header_comments: Vec<String> = Vec::new();

    for line in content.split('\n') {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed.starts_with('#') {
            if groups.is_empty() && header_comments.len() < 10 {
                header_comments.push(trimmed.to_string());
            } else {
                groups.entry("其他".to_string()).or_default().push(trimmed.to_string());
            }
            continue;
        }
        if seen.contains(trimmed) {
            continue;
        }
        seen.insert(trimmed.to_string());
        let cat = category_of(trimmed);
        groups.entry(cat).or_default().push(trimmed.to_string());
    }

    let mut b = String::new();
    if !header_comments.is_empty() {
        for c in &header_comments {
            b.push_str(c);
            b.push('\n');
        }
        b.push('\n');
    }
    let mut first = true;
    for cat in &category_order {
        let Some(rules) = groups.get(cat) else { continue };
        if !first {
            b.push('\n');
        }
        first = false;
        b.push_str("# === ");
        b.push_str(cat);
        b.push_str(" ===\n");
        for r in rules {
            b.push_str(r);
            b.push('\n');
        }
    }
    format!("{}\n", b.trim_end_matches('\n'))
}

// =====================================================================
// AI 生成（source = "ai"）
// =====================================================================

/// add.md 提示词模板（%s 为语言列表）。
const AI_ADD_PROMPT: &str = "您是一位专门生成高质量 .gitignore 文件的专家助手。
您的任务是根据提供的语言和框架为项目创建一个 .gitignore 文件。

<instructions>
1.  **内容**: 文件必须仅包含以下语言/框架的规则：%s。
2.  **结构**: 将规则组织成逻辑类别（例如，\"依赖项\"、\"构建输出\"、\"日志\"、\"IDE 文件\"）。
3.  **标题**: 每个类别必须以 '# === 类别名称 ===' 格式的标题开头，后跟一个空行。
4.  **注释**: 为不明显的规则或规则组添加简洁的注释（以 '#' 开头）以解释其用途。
5.  **排他性**: 不要包含任何未指定的其他语言、框架或工具的规则。
6.  **输出格式**: 您的响应必须仅为 .gitignore 文件的原始文本内容。不要包含任何介绍性文字、解释或 markdown 代码块。
</instructions>
";

/// AI 生成指定语言的 .gitignore 模板（OpenAI 兼容非流式调用）。
fn ai_generate(langs: &[String]) -> Result<String, String> {
    let cfg = Settings::load().map_err(|e| format!("加载配置失败: {e}"))?;
    if cfg.ai.api_key.is_empty() {
        return Err("未配置 AI API Key，请先在设置中配置".into());
    }
    let prompt = AI_ADD_PROMPT.replacen("%s", &langs.join(", "), 1);
    let client = ChatClient::new(&cfg.ai.base_url, &cfg.ai.api_key, &cfg.ai.model);
    let result = client
        .chat(&[ChatMessage::user(prompt)])
        .map_err(|e| e.to_string())?;
    Ok(cleanup_ai_output(&result))
}

/// 清理 AI 输出：去掉首行 ```lang 围栏与结尾 ```。
fn cleanup_ai_output(raw: &str) -> String {
    let mut s = raw.trim().to_string();
    if s.starts_with("```") {
        if let Some(idx) = s.find('\n') {
            if idx > 0 {
                s = s[idx + 1..].to_string();
            }
        }
    }
    if s.ends_with("```") {
        s.truncate(s.len() - 3);
    }
    s.trim().to_string()
}

// =====================================================================
// 模板内容获取（local / github / ai 三源）
// =====================================================================

/// 按源拼接多个语言的模板内容（单语言缺失时跳过，与 Go 一致）。
fn fetch_template_content(langs: &[String], source: &str) -> Result<String, String> {
    let tm = TemplateManager::new()?;
    match source {
        "local" => {
            init_bundled_templates(&tm.local_dir)?;
            collect_templates(langs, &tm.local_dir)
        }
        "github" => {
            // 仅在缺失时克隆；平时不自动拉取，用户经 IgnoreGithubSync 手动同步
            if !tm.remote_dir.join(".git").exists() {
                ensure_github_cloned(&tm)?;
            }
            collect_templates(langs, &tm.remote_dir)
        }
        "ai" => ai_generate(langs),
        _ => Err(format!("未知模板源: {source}")),
    }
}

fn collect_templates(langs: &[String], dir: &Path) -> Result<String, String> {
    let mut sb = String::new();
    for lang in langs {
        if let Ok(c) = read_template_file(lang, dir) {
            sb.push_str(&c);
            if !c.ends_with('\n') {
                sb.push('\n');
            }
        }
    }
    Ok(sb)
}

/// 生成合并结果（preview 不写盘 / apply 写盘）。
fn build_add_result(
    repo: Option<String>,
    langs: &[String],
    source: &str,
    apply: bool,
) -> Result<IgnoreAddResult, String> {
    let path = repo_path(repo);
    let content = fetch_template_content(langs, source)?;
    let updater = GitignoreUpdater::new(path.join(".gitignore"));
    let original = updater.read_content()?;
    let (merged, added) = updater.merge_content(&content)?;
    if apply {
        updater.apply_changes(&merged)?;
    }
    Ok(IgnoreAddResult {
        added,
        diff: plain_diff(&original, &merged),
        original,
        updated: merged,
        applied: apply,
    })
}

// =====================================================================
// 命令
// =====================================================================

/// IgnoreAddPreview 生成合并预览（diff），不写盘。source: "local"|"github"|"ai"。
#[tauri::command]
pub fn ignore_add_preview(repo: Option<String>, langs: Vec<String>, source: String) -> Result<IgnoreAddResult, String> {
    build_add_result(repo, &langs, &source, false)
}

/// IgnoreAddApply 用户确认预览后写入合并内容。
#[tauri::command]
pub fn ignore_add_apply(repo: Option<String>, langs: Vec<String>, source: String) -> Result<IgnoreAddResult, String> {
    build_add_result(repo, &langs, &source, true)
}

/// IgnoreWrite 用给定内容整体覆写仓库 .gitignore（GUI 编辑器 Ctrl+S 直存）。
#[tauri::command]
pub fn ignore_write(repo: Option<String>, content: String) -> Result<(), String> {
    let path = repo_path(repo.clone()).join(".gitignore");
    std::fs::write(&path, content).map_err(|e| e.to_string())
}

/// IgnoreListTemplates 列出本地可用模板语言名（首次自动播种内置模板）。
#[tauri::command]
pub fn ignore_list_templates(repo: Option<String>) -> Result<Vec<String>, String> {
    let _ = repo_path(repo.clone());
    let tm = TemplateManager::new()?;
    init_bundled_templates(&tm.local_dir)?;
    Ok(list_templates(&tm.local_dir).unwrap_or_default())
}

/// IgnoreListGithubTemplates 列出 GitHub 源模板（需先 IgnoreGithubSync）。
#[tauri::command]
pub fn ignore_list_github_templates(repo: Option<String>) -> Result<Vec<String>, String> {
    let _ = repo_path(repo.clone());
    let tm = TemplateManager::new()?;
    Ok(list_templates(&tm.remote_dir).unwrap_or_default())
}

/// IgnoreSourcesStatus 报告各模板源状态（可用数量/是否就绪/本地路径）。
#[tauri::command]
pub fn ignore_sources_status(repo: Option<String>) -> Result<Vec<IgnoreTemplateSource>, String> {
    let _ = repo_path(repo.clone());
    let tm = TemplateManager::new()?;
    init_bundled_templates(&tm.local_dir)?;

    let local_count = list_templates(&tm.local_dir).map(|l| l.len()).unwrap_or(0);
    let github_count = list_templates(&tm.remote_dir).map(|l| l.len()).unwrap_or(0);
    let cloned = tm.remote_dir.join(".git").is_dir();

    Ok(vec![
        IgnoreTemplateSource {
            name: "local".into(),
            count: local_count as i64,
            ready: true,
            cloned: false,
            local_path: tm.local_dir.display().to_string(),
        },
        IgnoreTemplateSource {
            name: "github".into(),
            count: github_count as i64,
            ready: cloned,
            cloned,
            local_path: tm.remote_dir.display().to_string(),
        },
    ])
}

/// IgnoreGithubSync 克隆或拉取 GitHub 官方 gitignore 模板仓库。
#[tauri::command]
pub fn ignore_github_sync(repo: Option<String>) -> Result<IgnoreTemplateSource, String> {
    let _ = repo_path(repo.clone());
    let tm = TemplateManager::new()?;
    ensure_github_cloned(&tm)?;
    let count = list_templates(&tm.remote_dir).map(|l| l.len()).unwrap_or(0);
    Ok(IgnoreTemplateSource {
        name: "github".into(),
        count: count as i64,
        ready: true,
        cloned: true,
        local_path: tm.remote_dir.display().to_string(),
    })
}

/// IgnoreGithubReset 删除并重新克隆 GitHub 模板仓库。
#[tauri::command]
pub fn ignore_github_reset(repo: Option<String>) -> Result<IgnoreTemplateSource, String> {
    let _ = repo_path(repo.clone());
    let tm = TemplateManager::new()?;
    if tm.remote_dir.exists() {
        std::fs::remove_dir_all(&tm.remote_dir).map_err(|e| format!("清理失败: {e}"))?;
    }
    ensure_github_cloned(&tm)?;
    let count = list_templates(&tm.remote_dir).map(|l| l.len()).unwrap_or(0);
    Ok(IgnoreTemplateSource {
        name: "github".into(),
        count: count as i64,
        ready: true,
        cloned: true,
        local_path: tm.remote_dir.display().to_string(),
    })
}

/// IgnoreTemplateView 按名称 + 源读取模板内容。
#[tauri::command]
pub fn ignore_template_view(repo: Option<String>, name: String, source: String) -> Result<String, String> {
    let _ = repo_path(repo.clone());
    let tm = TemplateManager::new()?;
    let dir = if source == "github" { &tm.remote_dir } else { &tm.local_dir };
    if source == "local" {
        init_bundled_templates(&tm.local_dir)?;
    }
    read_template_file(&name, dir)
}

/// IgnoreTemplateDelete 删除一个本地模板。
#[tauri::command]
pub fn ignore_template_delete(repo: Option<String>, name: String) -> Result<(), String> {
    let _ = repo_path(repo.clone());
    let tm = TemplateManager::new()?;
    let target = tm.local_dir.join(format!("{name}.gitignore"));
    if !target.exists() {
        return Err(format!("模板不存在: {name}"));
    }
    std::fs::remove_file(&target).map_err(|e| e.to_string())
}

/// IgnoreTemplateSave 保存自定义模板（创建或覆盖）。
#[tauri::command]
pub fn ignore_template_save(repo: Option<String>, name: String, content: String) -> Result<(), String> {
    let _ = repo_path(repo.clone());
    let tm = TemplateManager::new()?;
    std::fs::create_dir_all(&tm.local_dir).map_err(|e| e.to_string())?;
    let target = tm.local_dir.join(format!("{name}.gitignore"));
    std::fs::write(&target, content).map_err(|e| e.to_string())
}

/// IgnoreTemplateCopy 将 GitHub 模板复制到本地（供自定义编辑）。
#[tauri::command]
pub fn ignore_template_copy(repo: Option<String>, name: String) -> Result<(), String> {
    let _ = repo_path(repo.clone());
    let tm = TemplateManager::new()?;
    let content = read_template_file(&name, &tm.remote_dir)
        .map_err(|e| format!("读取 GitHub 模板失败: {e}"))?;
    std::fs::create_dir_all(&tm.local_dir).map_err(|e| e.to_string())?;
    let target = tm.local_dir.join(format!("{name}.gitignore"));
    std::fs::write(&target, content).map_err(|e| e.to_string())
}

/// IgnoreLoadCategories 读取用户分类规则（缺失时播种默认值）。
#[tauri::command]
pub fn ignore_load_categories(repo: Option<String>) -> Result<Vec<IgnoreCategoryRule>, String> {
    let _ = repo_path(repo.clone());
    load_categories()
}

/// IgnoreSaveCategories 保存用户分类规则。
#[tauri::command]
pub fn ignore_save_categories(
    repo: Option<String>,
    cats: Vec<IgnoreCategoryRule>,
) -> Result<(), String> {
    let _ = repo;
    save_categories(&cats)
}

/// IgnoreResetCategories 恢复默认分类规则并返回。
#[tauri::command]
pub fn ignore_reset_categories(repo: Option<String>) -> Result<Vec<IgnoreCategoryRule>, String> {
    let _ = repo_path(repo.clone());
    let defaults = default_categories();
    save_categories(&defaults)?;
    Ok(defaults)
}

/// IgnoreRefactorPreview 本地规则引擎整理 .gitignore（去重 + 分类分组）预览。
#[tauri::command]
pub fn ignore_refactor_preview(repo: Option<String>) -> Result<IgnoreAddResult, String> {
    let path = repo_path(repo.clone());
    let updater = GitignoreUpdater::new(path.join(".gitignore"));
    let original = updater.read_content()?;
    let cats = load_categories()?;
    let refactored = local_refactor(&original, &cats);
    Ok(IgnoreAddResult {
        added: 0,
        original: original.clone(),
        diff: plain_diff(&original, &refactored),
        updated: refactored,
        applied: false,
    })
}

/// IgnoreRefactorApply 用户确认后写入本地整理结果。
#[tauri::command]
pub fn ignore_refactor_apply(repo: Option<String>) -> Result<IgnoreAddResult, String> {
    let path = repo_path(repo.clone());
    let updater = GitignoreUpdater::new(path.join(".gitignore"));
    let original = updater.read_content()?;
    let cats = load_categories()?;
    let refactored = local_refactor(&original, &cats);
    updater.apply_changes(&refactored)?;
    Ok(IgnoreAddResult {
        added: 0,
        original: original.clone(),
        diff: plain_diff(&original, &refactored),
        updated: refactored,
        applied: true,
    })
}

/// IgnoreCheck 经 `git check-ignore -v` 检查单文件是否被忽略及命中的规则。
#[tauri::command]
pub fn ignore_check(repo: Option<String>, file_path: String) -> Result<IgnoreCheckResult, String> {
    let path = repo_path(repo.clone());
    let out = git_output(Some(path.as_path()), &["check-ignore", "-v", &file_path])
        .map_err(|e| format!("git check-ignore 失败: {e}"))?;
    match out.status.code() {
        Some(0) => {}
        Some(1) => return Ok(IgnoreCheckResult { ignored: false, rule: String::new(), source: String::new() }),
        _ => {
            return Err(format!(
                "git check-ignore 失败: {}",
                String::from_utf8_lossy(&out.stderr).trim()
            ))
        }
    }
    // 解析 ".gitignore:12:*.key\tfile"
    let output = String::from_utf8_lossy(&out.stdout);
    let first = output.trim().split('\t').next().unwrap_or("");
    let info: Vec<&str> = first.splitn(3, ':').collect();
    let mut res = IgnoreCheckResult { ignored: true, rule: String::new(), source: String::new() };
    if info.len() >= 3 {
        res.rule = info[2].to_string();
        res.source = format!("{}:{}", info[0], info[1]);
    }
    Ok(res)
}

/// IgnoreCheckBatch 批量检查多个文件（单文件失败按“未忽略”计，与 Go 一致）。
#[tauri::command]
pub fn ignore_check_batch(repo: Option<String>, files: Vec<String>) -> Result<Vec<IgnoreCheckBatchItem>, String> {
    let path = repo_path(repo.clone());
    let mut results = Vec::with_capacity(files.len());
    for f in &files {
        let out = match git_output(Some(path.as_path()), &["check-ignore", "-v", f]) {
            Ok(o) => o,
            Err(_) => {
                // git 启动失败等：按未忽略记录（与 Go 的容错分支一致）
                results.push(IgnoreCheckBatchItem { file: f.clone(), ignored: false, rule: String::new(), source: String::new() });
                continue;
            }
        };
        if out.status.code() != Some(0) {
            // 退出码 1 = 未忽略；其他失败同样按未忽略记录
            results.push(IgnoreCheckBatchItem { file: f.clone(), ignored: false, rule: String::new(), source: String::new() });
            continue;
        }
        let output = String::from_utf8_lossy(&out.stdout);
        let first = output.trim().split('\t').next().unwrap_or("");
        let info: Vec<&str> = first.splitn(3, ':').collect();
        let mut item = IgnoreCheckBatchItem { file: f.clone(), ignored: true, rule: String::new(), source: String::new() };
        if info.len() >= 3 {
            item.rule = info[2].to_string();
            item.source = format!("{}:{}", info[0], info[1]);
        }
        results.push(item);
    }
    Ok(results)
}

/// IgnoreListFiles 列出仓库全部已追踪文件（git ls-files，供检查文件选择器）。
#[tauri::command]
pub fn ignore_list_files(repo: Option<String>) -> Result<Vec<String>, String> {
    let path = repo_path(repo.clone());
    run_lines(&path, &["ls-files"]).map_err(|e| format!("git ls-files 失败: {e}"))
}

/// IgnoreFileStatuses 三次批量 `git ls-files` 拿到全工作区文件的
/// tracked / ignored / untracked 状态（无逐文件 fork；--directory 折叠整目录）。
#[tauri::command]
pub fn ignore_file_statuses(repo: Option<String>) -> Result<Vec<FileStatusEntry>, String> {
    let path = repo_path(repo.clone());

    // 1. 已追踪文件
    let tracked = run_lines(&path, &["ls-files"])
        .map_err(|e| format!("git ls-files 失败: {e}"))?;
    // 2. 未追踪且未忽略
    let untracked = run_lines(&path, &["ls-files", "--others", "--exclude-standard"])
        .map_err(|e| format!("git ls-files --others 失败: {e}"))?;
    // 3. 被忽略的文件/目录（--directory 折叠整目录，--no-empty-directory 去空目录）
    let ignored = run_lines(
        &path,
        &["ls-files", "--others", "--ignored", "--exclude-standard", "--directory", "--no-empty-directory"],
    )
    .map_err(|e| format!("git ls-files --ignored 失败: {e}"))?;

    // 路径 → 状态（每个路径只出现一次）
    let mut by_path: HashMap<String, FileStatusEntry> = HashMap::new();
    for p in tracked {
        by_path.insert(p.clone(), FileStatusEntry { path: p, is_dir: false, status: "tracked".into(), rule: String::new(), source: String::new() });
    }
    for p in untracked {
        by_path.insert(p.clone(), FileStatusEntry { path: p, is_dir: false, status: "untracked".into(), rule: String::new(), source: String::new() });
    }
    for p in ignored {
        // --directory 输出的目录带尾斜杠：标记 isDir 并去掉斜杠保持显示一致
        let (p, is_dir) = match p.strip_suffix('/') {
            Some(stripped) => (stripped.to_string(), true),
            None => (p, false),
        };
        by_path.insert(p.clone(), FileStatusEntry { path: p, is_dir, status: "ignored".into(), rule: String::new(), source: String::new() });
    }

    let mut result: Vec<FileStatusEntry> = by_path.into_values().collect();
    result.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(result)
}
