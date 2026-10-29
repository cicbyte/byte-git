//! Git 钩子管理：shebang 脚本钩子（shell/python/node，git 原生机制直执行）。
//!
//! - 21 个固定钩子槽位（自定义槽位已移除）
//! - local/global 双作用域（core.hooksPath 配置感知）
//! - Exec 统一直接执行 hook 文件（shebang），无内嵌语言运行时
//! - 配置落盘 `.git/byte-git/config.json`（仓库级）与 AppDir `hooks.json`（全局）
//! - clone 代理规则的 Lua 引擎不受影响（mlua 仍由 clone 域使用）

use crate::util::{git_dir, repo_data_dir, run_capture, AppPaths};
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// git 内置钩子槽位（上游固定 21 个）。
pub const HOOK_NAMES: &[&str] = &[
    "applypatch-msg", "commit-msg", "fsmonitor-watchman",
    "post-applypatch", "post-checkout", "post-commit",
    "post-merge", "post-receive", "post-rewrite",
    "post-update", "pre-applypatch", "pre-auto-gc",
    "pre-commit", "pre-merge-commit", "pre-push",
    "pre-rebase", "pre-receive", "prepare-commit-msg",
    "push-to-checkout", "sendemail-validate",
    "update",
];

// hook type 常量（对齐上游收敛后的字面量）
pub const TYPE_SHELL: &str = "shell";
pub const TYPE_PYTHON: &str = "python";
pub const TYPE_NODE: &str = "node";
pub const TYPE_SAMPLE: &str = "sample";
pub const TYPE_SCRIPT: &str = "script";

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Hook {
    pub name: String,
    #[serde(default)]
    pub enabled: bool,
    #[serde(rename = "type", default)]
    pub ty: String,
    #[serde(default)]
    pub source: String,
    /// "local" | "global"
    #[serde(default)]
    pub scope: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct HookEntry {
    #[serde(rename = "type")]
    ty: String,
    #[serde(default)]
    source: String,
    /// 脚本语言（shell/python/node）
    #[serde(default)]
    lang: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct HookConfig {
    #[serde(default)]
    hooks: std::collections::HashMap<String, HookEntry>,
}

// ---------------------------------------------------------------------------
// 钩子名称校验（Windows 共享 hooks 目录的文件名安全）
// ---------------------------------------------------------------------------

/// 校验钩子名可安全用作 hooks 目录下的文件名（非空、无路径分隔、仅字母数字-_\.）。
pub fn validate_hook_name(name: &str) -> Result<()> {
    if name.is_empty() {
        bail!("hook 名称为空");
    }
    if name.contains('/') || name.contains('\\') || name.contains("..") {
        bail!("hook 名称含有非法字符（不允许 / \\ 或 ..）");
    }
    if !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.') {
        bail!("hook 名称含非法字符（仅允许字母、数字、- _ .）");
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// hooks 目录解析（core.hooksPath 感知）
// ---------------------------------------------------------------------------

/// git config 单层读取：`git config --<layer> --get <key>`；未设置返回 None。
fn config_get_layered(layer: &str, repo: &Path, key: &str) -> Option<String> {
    let args: Vec<String> = vec![
        "config".into(), format!("--{layer}"), "--get".into(), key.into(),
    ];
    let refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    run_capture(repo, refs).ok().map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}

/// 绝对化：相对路径 local 按 repo、global 按 $HOME 解析（对齐上游）。
fn absolutize(hp: String, base: &Path) -> PathBuf {
    let p = PathBuf::from(&hp);
    if p.is_absolute() { p } else { base.join(&hp) }
}

/// git 实际使用的 hooks 目录 + 来源作用域：
/// local core.hooksPath > global core.hooksPath > `<gitdir>/hooks`（local）。
pub fn hooks_dir_with_scope(repo: &Path) -> Result<(PathBuf, String)> {
    if let Some(hp) = config_get_layered("local", repo, "core.hooksPath") {
        return Ok((absolutize(hp, repo), "local".into()));
    }
    if let Some(hp) = config_get_layered("global", repo, "core.hooksPath") {
        let home = std::env::var("USERPROFILE")
            .or_else(|_| std::env::var("HOME"))
            .unwrap_or_default();
        return Ok((absolutize(hp, Path::new(&home)), "global".into()));
    }
    Ok((git_dir(repo)?.join("hooks"), "local".into()))
}

/// 忽略作用域的兼容包装（CLI 合并视图用）。
pub fn hooks_dir(repo: &Path) -> Result<PathBuf> {
    Ok(hooks_dir_with_scope(repo)?.0)
}

/// global 层 hooks 目录（未设置返回空路径）。
fn global_hooks_dir(repo: &Path) -> PathBuf {
    match config_get_layered("global", repo, "core.hooksPath") {
        Some(hp) => {
            let home = std::env::var("USERPROFILE")
                .or_else(|_| std::env::var("HOME"))
                .unwrap_or_default();
            absolutize(hp, Path::new(&home))
        }
        None => PathBuf::new(),
    }
}

/// local 层 hooks 目录（local core.hooksPath 或默认 `<gitdir>/hooks`）。
fn local_hooks_dir(repo: &Path) -> PathBuf {
    match config_get_layered("local", repo, "core.hooksPath") {
        Some(hp) => absolutize(hp, repo),
        None => git_dir(repo).map(|gd| gd.join("hooks")).unwrap_or_default(),
    }
}

/// 按作用域取 hooks 目录。global 未配置时报错（前端引导去配置）。
pub fn hooks_dir_for_scope(repo: &Path, scope: &str) -> Result<PathBuf> {
    if scope == "global" {
        let dir = global_hooks_dir(repo);
        if dir.as_os_str().is_empty() {
            bail!("全局 core.hooksPath 未设置，请在「Git 配置 → Hook」页配置");
        }
        return Ok(dir);
    }
    let dir = local_hooks_dir(repo);
    if dir.as_os_str().is_empty() {
        bail!("无法确定仓库 hooks 目录");
    }
    Ok(dir)
}

// ---------------------------------------------------------------------------
// 配置存取（仓库级 config.json / 全局 hooks.json）
// ---------------------------------------------------------------------------

fn config_path(repo: &Path) -> Result<PathBuf> {
    Ok(repo_data_dir(repo)?.join("config.json"))
}

fn global_config_path() -> Result<PathBuf> {
    Ok(AppPaths::new().context("定位应用目录失败")?.root().join("hooks.json"))
}

fn parse_config(data: &str) -> HookConfig {
    serde_json::from_str(data).unwrap_or_default()
}

fn load_config(repo: &Path) -> Result<HookConfig> {
    let path = config_path(repo)?;
    match std::fs::read_to_string(&path) {
        Ok(data) => Ok(parse_config(&data)),
        Err(_) => Ok(HookConfig::default()),
    }
}

fn save_config(repo: &Path, cfg: &HookConfig) -> Result<()> {
    let path = config_path(repo)?;
    AppPaths::ensure_dir(path.parent().unwrap())?;
    std::fs::write(&path, serde_json::to_string_pretty(cfg)?)
        .with_context(|| format!("写入失败: {}", path.display()))
}

fn load_global_config() -> HookConfig {
    let Ok(path) = global_config_path() else { return HookConfig::default() };
    match std::fs::read_to_string(&path) {
        Ok(data) => parse_config(&data),
        Err(_) => HookConfig::default(),
    }
}

fn save_global_config(cfg: &HookConfig) -> Result<()> {
    let path = global_config_path()?;
    if let Some(parent) = path.parent() {
        AppPaths::ensure_dir(parent)?;
    }
    std::fs::write(&path, serde_json::to_string_pretty(cfg)?)
        .with_context(|| format!("写入失败: {}", path.display()))
}

// ---------------------------------------------------------------------------
// 列表（CLI 合并视图 + GUI 双作用域视图）
// ---------------------------------------------------------------------------

/// CLI：按 git 实际生效目录列出（上游 List 语义）。
pub fn list(repo: &Path) -> Result<Vec<Hook>> {
    let dir = hooks_dir(repo)?;
    let cfg = load_config(repo).unwrap_or_default();
    let mut hooks = Vec::new();
    for name in HOOK_NAMES {
        let hook_path = dir.join(name);
        let sample = dir.join(format!("{name}.sample"));
        let mut h = Hook { name: name.to_string(), ..Default::default() };
        if let Some(entry) = cfg.hooks.get(*name) {
            h.enabled = true;
            h.ty = entry.ty.clone();
            h.source = entry.source.clone();
        } else if hook_path.is_file() {
            h.enabled = true;
            h.ty = TYPE_SCRIPT.into();
        } else if sample.is_file() {
            h.ty = TYPE_SAMPLE.into();
        } else {
            continue;
        }
        hooks.push(h);
    }
    Ok(hooks)
}

pub fn print_list(repo: &Path) -> Result<()> {
    let hooks = list(repo)?;
    if hooks.is_empty() {
        println!("没有钩子");
        return Ok(());
    }
    println!("{:<22} {:<6} {}", "钩子", "状态", "类型");
    println!("{}", "-".repeat(40));
    for h in &hooks {
        println!("{:<22} {:<6} {}", h.name, if h.enabled { "启用" } else { "禁用" }, h.ty);
    }
    Ok(())
}

/// GUI：按作用域列出全部 21 个槽位（含未启用空槽）。
/// 作用域未配置（如 global hooksPath 未设置）返回空列表，前端引导配置。
pub fn list_hooks(repo: &Path, scope: &str) -> Result<Vec<Hook>> {
    let scope = if scope.is_empty() { "local" } else { scope };
    let dir = match hooks_dir_for_scope(repo, scope) {
        Ok(d) => d,
        Err(_) => return Ok(Vec::new()),
    };
    let cfg_hooks = if scope == "global" {
        load_global_config().hooks
    } else {
        load_config(repo).map(|c| c.hooks).unwrap_or_default()
    };

    let mut hooks = Vec::new();
    for name in HOOK_NAMES {
        let hook_path = dir.join(name);
        let sample = dir.join(format!("{name}.sample"));
        let mut h = Hook { name: name.to_string(), scope: scope.into(), ..Default::default() };
        if let Some(entry) = cfg_hooks.get(*name) {
            h.enabled = true;
            h.ty = entry.ty.clone();
            h.source = entry.source.clone();
        } else if hook_path.is_file() {
            h.enabled = true;
            h.ty = detect_lang(&hook_path);
        } else if sample.is_file() {
            h.ty = TYPE_SAMPLE.into();
        }
        hooks.push(h);
    }
    Ok(hooks)
}

// ---------------------------------------------------------------------------
// 安装/移除/读取/执行
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
pub struct InstallOptions {
    pub hook_name: String,
    /// 脚本文件路径（shebang 驱动：shell/python/node）
    pub script: String,
}

/// CLI 安装：从脚本文件复制到 hooks 目录（上游 Install 语义）。
pub fn install(repo: &Path, opts: &InstallOptions) -> Result<()> {
    if !HOOK_NAMES.contains(&opts.hook_name.as_str()) {
        bail!("未知钩子: {}（见 hook list）", opts.hook_name);
    }
    let dir = hooks_dir(repo)?;
    let hook_path = dir.join(&opts.hook_name);
    if hook_path.is_file() {
        bail!("钩子 {} 已存在，请先 hook remove", opts.hook_name);
    }
    AppPaths::ensure_dir(&dir)?;
    if !opts.script.is_empty() {
        let src = std::fs::read(&opts.script).with_context(|| format!("读取脚本失败: {}", opts.script))?;
        std::fs::write(&hook_path, src).context("安装钩子失败")?;
        println!("已安装脚本钩子: {}", opts.hook_name);
        return Ok(());
    }
    bail!("请指定 --script <file>")
}

/// 按作用域移除钩子（文件 + 对应配置项）。
pub fn remove(repo: &Path, name: &str, scope: &str) -> Result<()> {
    let dir = hooks_dir_for_scope(repo, scope)?;
    let hook_path = dir.join(name);
    if !hook_path.exists() {
        bail!("钩子 {name} 不存在");
    }
    std::fs::remove_file(&hook_path).context("删除钩子失败")?;
    if scope == "global" {
        let mut g = load_global_config();
        g.hooks.remove(name);
        save_global_config(&g)?;
    } else {
        let mut cfg = load_config(repo)?;
        cfg.hooks.remove(name);
        save_config(repo, &cfg)?;
    }
    println!("已移除钩子: {name}");
    Ok(())
}

/// 读首行 shebang 判定脚本语言；无识别 shebang 回落 "script"。
pub fn detect_lang(path: &Path) -> String {
    let Ok(mut f) = std::fs::File::open(path) else { return TYPE_SCRIPT.into() };
    use std::io::Read;
    let mut buf = [0u8; 256];
    let n = f.read(&mut buf).unwrap_or(0);
    let first_line = String::from_utf8_lossy(&buf[..n]);
    let first_line = first_line.split('\n').next().unwrap_or("");
    if first_line.contains("python") {
        TYPE_PYTHON.into()
    } else if first_line.contains("node") {
        TYPE_NODE.into()
    } else if first_line.contains("/sh") || first_line.contains("/bash") || first_line.contains("/zsh") {
        TYPE_SHELL.into()
    } else {
        TYPE_SCRIPT.into()
    }
}

/// GUI 安装：按语言直接写内容到作用域 hooks 目录（git 经 shebang 执行）。
pub fn install_script(repo: &Path, hook_name: &str, lang: &str, content: &str, scope: &str) -> Result<()> {
    let scope = if scope.is_empty() { "local" } else { scope };
    if !matches!(lang, TYPE_SHELL | TYPE_PYTHON | TYPE_NODE) {
        bail!("不支持的语言: {lang}（允许 shell/python/node）");
    }
    let dir = hooks_dir_for_scope(repo, scope)?;
    AppPaths::ensure_dir(&dir)?;
    std::fs::write(dir.join(hook_name), content).context("写入 hook 失败")?;
    let entry = HookEntry { ty: lang.into(), source: String::new(), lang: lang.into() };
    if scope == "global" {
        let mut g = load_global_config();
        g.hooks.insert(hook_name.into(), entry);
        return save_global_config(&g);
    }
    let mut cfg = load_config(repo)?;
    cfg.hooks.insert(hook_name.into(), entry);
    save_config(repo, &cfg)
}

/// 读取钩子脚本内容（作用域目录未配置或文件不存在 → 空串，前端回落模板）。
pub fn read_script(repo: &Path, hook_name: &str, scope: &str) -> String {
    let scope = if scope.is_empty() { "local" } else { scope };
    let Ok(dir) = hooks_dir_for_scope(repo, scope) else { return String::new() };
    std::fs::read_to_string(dir.join(hook_name)).unwrap_or_default()
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct ExecResult {
    pub exit_code: i32,
    pub output: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub error: String,
}

/// 执行钩子：直接 spawn hook 文件（shebang 原生机制），cwd=仓库。
/// 上游 GUI 测试执行语义：失败不返回 Err，以 ExecResult 反映。
pub fn exec(repo: &Path, hook_name: &str, scope: &str, args: &[String]) -> Result<ExecResult> {
    let dir = hooks_dir_for_scope(repo, scope)?;
    let hook_path = dir.join(hook_name);
    if !hook_path.exists() {
        bail!("hook {hook_name:?} 未安装");
    }
    let mut cmd = std::process::Command::new(&hook_path);
    cmd.args(args).current_dir(repo);
    match cmd.output() {
        Ok(out) => {
            let mut combined = String::from_utf8_lossy(&out.stdout).into_owned();
            combined.push_str(&String::from_utf8_lossy(&out.stderr));
            let code = out.status.code().unwrap_or(1);
            Ok(ExecResult {
                exit_code: code,
                output: combined,
                error: if out.status.success() { String::new() } else { format!("exit {code}") },
            })
        }
        Err(e) => Ok(ExecResult {
            exit_code: 1,
            output: String::new(),
            error: e.to_string(),
        }),
    }
}

// ---------------------------------------------------------------------------
// core.hooksPath 配置面板（GUI HookConfigInfo / HookSetHooksPath）
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default, Serialize)]
pub struct HookConfigInfo {
    #[serde(skip_serializing_if = "String::is_empty")]
    pub hooks_path_local: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub hooks_path_global: String,
    pub hooks_path_effective: String,
    pub default_hooks_dir: String,
    pub has_local_hooks_path: bool,
    pub has_global_hooks_path: bool,
}

/// 当前 core.hooksPath 配置全貌（local/global/effective + 默认目录）。
pub fn config_info(repo: &Path) -> HookConfigInfo {
    let mut info = HookConfigInfo {
        hooks_path_effective: config_get_layered("", repo, "core.hooksPath").unwrap_or_default(),
        ..Default::default()
    };
    if let Some(v) = config_get_layered("local", repo, "core.hooksPath") {
        info.hooks_path_local = v;
        info.has_local_hooks_path = true;
    }
    if let Some(v) = config_get_layered("global", repo, "core.hooksPath") {
        info.hooks_path_global = v;
        info.has_global_hooks_path = true;
    }
    if let Ok((dir, _)) = hooks_dir_with_scope(repo) {
        info.default_hooks_dir = dir.display().to_string();
    }
    info
}

/// 设置（空值=清除）core.hooksPath。scope = "local" | "global"。
pub fn set_hooks_path(repo: &Path, scope: &str, value: &str) -> Result<()> {
    let layer = if scope == "global" { "--global" } else { "--local" };
    let args: Vec<String> = if value.is_empty() {
        vec!["config".into(), layer.into(), "--unset".into(), "core.hooksPath".into()]
    } else {
        vec!["config".into(), layer.into(), "core.hooksPath".into(), value.into()]
    };
    let refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    // --unset 在未设置时 exit 5 —— 幂等视为成功
    if run_capture(repo, refs).is_err() && !value.is_empty() {
        bail!("设置 core.hooksPath 失败");
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// hook run（run.go：husky 式入口——钩子脚本内调 `byte-git hook run commit-msg --ai`）
// ---------------------------------------------------------------------------

/// `hook run` 选项（对齐上游 RunOptions）。
#[derive(Debug, Clone, Default)]
pub struct HookRunOptions {
    pub repo: String,
    pub hook_name: String,
    pub ai: bool,
    pub model: String,
    pub base_url: String,
    pub lang: String,
}

/// `hook run` 入口：目前仅支持 commit-msg（其余钩子上游同样未支持）。
pub fn run_hook(opts: &HookRunOptions) -> Result<()> {
    match opts.hook_name.as_str() {
        "commit-msg" => run_commit_msg(opts),
        other => bail!("hook run 暂不支持 {other:?}，目前支持: commit-msg"),
    }
}

/// commit-msg 钩子：非 AI 模式打印消息；AI 模式改写后写回（保留 # 注释块）。
/// 消息文件路径：env BYTE_GIT_COMMIT_MSG_FILE / GIT_MATE_COMMIT_MSG_FILE >
/// 进程尾参（git 调用钩子时传入，CLI 以 trailing arg 透传）。
fn run_commit_msg(opts: &HookRunOptions) -> Result<()> {
    let mut commit_msg_file = std::env::var("BYTE_GIT_COMMIT_MSG_FILE")
        .or_else(|_| std::env::var("GIT_MATE_COMMIT_MSG_FILE"))
        .unwrap_or_default();
    if commit_msg_file.is_empty() {
        let args: Vec<String> = std::env::args().collect();
        if args.len() > 1 {
            commit_msg_file = args[args.len() - 1].clone();
        }
    }
    if commit_msg_file.is_empty() {
        bail!("无法获取 commit-msg 文件路径");
    }

    let data = std::fs::read_to_string(&commit_msg_file)
        .with_context(|| format!("读取 commit message 文件失败: {commit_msg_file}"))?;
    let original = data.trim().to_string();
    let comment_start = original.find('#');
    let msg = match comment_start {
        Some(i) => original[..i].trim().to_string(),
        None => original.clone(),
    };
    if msg.is_empty() {
        return Ok(());
    }

    if !opts.ai {
        println!("{msg}");
        return Ok(());
    }

    let repo = Path::new(if opts.repo.is_empty() { "." } else { opts.repo.as_str() });
    let diff = staged_diff(repo).unwrap_or_else(|e| {
        eprintln!("警告: 获取 diff 失败: {e:#}");
        String::new()
    });
    let files = staged_files(repo);

    let entry = crate::airewrite::AICommitEntry {
        original: msg.clone(),
        files: files.clone(),
        diff_summary: format!("{} files changed", files.len()),
        diff,
        ..Default::default()
    };
    let lang = if opts.lang.is_empty() { "zh".into() } else { opts.lang.clone() };
    let prompt = crate::airewrite::build_prompt_for_hook(&entry, &entry.diff, &lang, 8000);
    let model = if opts.model.is_empty() { "gemma4:e4b".into() } else { opts.model.clone() };
    let base_url = if opts.base_url.is_empty() { "http://localhost:11434".into() } else { opts.base_url.clone() };
    let client = crate::ai::ChatClient::new(&base_url, "", &model);
    let suggested = match client.chat(&[crate::ai::ChatMessage::user(prompt)]) {
        Ok(s) => s.trim().to_string(),
        Err(e) => {
            eprintln!("AI 改写失败，保留原始消息: {e:#}");
            return Ok(());
        }
    };
    if suggested.is_empty() {
        return Ok(());
    }

    let comment_block = match comment_start {
        Some(i) => format!("\n{}", &original[i..]),
        None => String::new(),
    };
    let new_content = format!("{suggested}{comment_block}\n");
    std::fs::write(&commit_msg_file, new_content).context("写入 commit message 失败")?;
    eprintln!("AI 改写: {msg} → {suggested}");
    Ok(())
}

fn staged_diff(repo: &Path) -> Result<String> {
    let stat = run_capture(repo, ["diff", "--cached", "--stat"])?;
    let full = run_capture(repo, ["diff", "--cached"])?;
    Ok(format!("{stat}\n{full}"))
}

fn staged_files(repo: &Path) -> Vec<String> {
    run_capture(repo, ["diff", "--cached", "--name-only"])
        .map(|o| o.lines().map(|l| l.to_string()).filter(|l| !l.is_empty()).collect())
        .unwrap_or_default()
}
