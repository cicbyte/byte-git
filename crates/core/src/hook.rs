//! Git 钩子管理：Lua 脚本钩子（wrapper 指向本程序 `hook exec`）与普通脚本钩子。
//!
//! 移植自 git-mate `internal/hook`（hook.go + exec.go）。
//! 配置落盘 `.git/byte-git/config.json`。

use crate::util::{git_dir, repo_data_dir};
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

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

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Hook {
    pub name: String,
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub r#type: String,
    #[serde(default)]
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct HookEntry {
    #[serde(rename = "type")]
    ty: String,
    source: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct HookConfig {
    #[serde(default)]
    hooks: std::collections::HashMap<String, HookEntry>,
}

fn hooks_dir(repo: &Path) -> Result<PathBuf> {
    Ok(git_dir(repo)?.join("hooks"))
}

fn config_path(repo: &Path) -> Result<PathBuf> {
    Ok(repo_data_dir(repo)?.join("config.json"))
}

fn load_config(repo: &Path) -> Result<HookConfig> {
    let path = config_path(repo)?;
    match std::fs::read_to_string(&path) {
        Ok(data) => serde_json::from_str(&data).with_context(|| format!("解析失败: {}", path.display())),
        Err(_) => Ok(HookConfig::default()),
    }
}

fn save_config(repo: &Path, cfg: &HookConfig) -> Result<()> {
    let path = config_path(repo)?;
    crate::util::AppPaths::ensure_dir(path.parent().unwrap())?;
    std::fs::write(&path, serde_json::to_string_pretty(cfg)?)
        .with_context(|| format!("写入失败: {}", path.display()))?;
    Ok(())
}

/// 列出所有钩子槽位状态（启用/禁用/样例）。
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
            h.r#type = entry.ty.clone();
            h.source = entry.source.clone();
        } else if hook_path.is_file() {
            h.enabled = true;
            h.r#type = "script".into();
        } else if sample.is_file() {
            h.r#type = "sample".into();
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
        println!("{:<22} {:<6} {}", h.name, if h.enabled { "启用" } else { "禁用" }, h.r#type);
    }
    Ok(())
}

#[derive(Debug, Clone, Default)]
pub struct InstallOptions {
    pub hook_name: String,
    pub lua_file: String,
    pub script: String,
}

pub fn install(repo: &Path, opts: &InstallOptions) -> Result<()> {
    if !HOOK_NAMES.contains(&opts.hook_name.as_str()) {
        bail!("未知钩子: {}（见 hook list）", opts.hook_name);
    }
    let dir = hooks_dir(repo)?;
    let hook_path = dir.join(&opts.hook_name);
    if hook_path.is_file() {
        bail!("钩子 {} 已存在，请先 hook remove", opts.hook_name);
    }
    crate::util::AppPaths::ensure_dir(&dir)?;

    if !opts.lua_file.is_empty() {
        let abs = std::fs::canonicalize(&opts.lua_file)
            .map_err(|_| anyhow::anyhow!("Lua 文件不存在: {}", opts.lua_file))?;
        let exe = std::env::current_exe()
            .map(|p| p.to_string_lossy().replace('/', "\\"))
            .context("获取可执行文件路径失败")?;

        // wrapper: .git/hooks/<name> → exec <本程序> hook exec <name> "$@"
        let content = format!("#!/bin/sh\nexec \"{exe}\" hook exec {} \"$@\"\n", opts.hook_name);
        std::fs::write(&hook_path, content).context("创建钩子失败")?;

        let mut cfg = load_config(repo).unwrap_or_default();
        cfg.hooks.insert(opts.hook_name.clone(), HookEntry {
            ty: "lua".into(),
            source: abs.to_string_lossy().into_owned(),
        });
        save_config(repo, &cfg)?;
        println!("已安装 Lua 钩子: {} → {}", opts.hook_name, abs.display());
        Ok(())
    } else if !opts.script.is_empty() {
        let src = std::fs::read(&opts.script).with_context(|| format!("读取脚本失败: {}", opts.script))?;
        std::fs::write(&hook_path, src).context("安装钩子失败")?;
        println!("已安装脚本钩子: {}", opts.hook_name);
        Ok(())
    } else {
        bail!("请指定 --lua <file> 或 --script <file>")
    }
}

pub fn remove(repo: &Path, name: &str) -> Result<()> {
    let hook_path = hooks_dir(repo)?.join(name);
    if !hook_path.exists() {
        bail!("钩子 {name} 不存在");
    }
    std::fs::remove_file(&hook_path).context("删除钩子失败")?;
    let mut cfg = load_config(repo).unwrap_or_default();
    cfg.hooks.remove(name);
    save_config(repo, &cfg)?;
    println!("已移除钩子: {name}");
    Ok(())
}

// ---------------------------------------------------------------------------
// exec：执行 Lua 钩子
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, serde::Serialize)]
pub struct ExecResult {
    pub exit_code: i32,
    pub output: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub error: String,
}

/// 执行钩子的 Lua 脚本：脚本须定义 `main(hook_name, ...args)`，
/// 返回 0/nil/true = 成功，非零/false = 阻断。
pub fn exec(repo: &Path, hook_name: &str, args: &[String]) -> Result<ExecResult> {
    let cfg = load_config(repo).context("加载 hook 配置失败")?;
    let Some(entry) = cfg.hooks.get(hook_name) else {
        bail!("hook {hook_name:?} 未安装 Lua 脚本");
    };
    if entry.ty != "lua" || entry.source.is_empty() {
        bail!("hook {hook_name:?} 未安装 Lua 脚本");
    }
    let source = Path::new(&entry.source);
    if !source.exists() {
        bail!("Lua 脚本文件不存在: {}", entry.source);
    }

    let lua = mlua::Lua::new();
    let code = std::fs::read_to_string(source).with_context(|| format!("读取 Lua 失败: {}", source.display()))?;
    if let Err(e) = lua.load(code).set_name(&format!("{hook_name}.lua")).exec() {
        return Ok(ExecResult {
            exit_code: 1,
            output: String::new(),
            error: format!("Lua 脚本加载失败: {e}"),
        });
    }

    let main: Option<mlua::Function> = lua.globals().get("main").ok();
    let Some(main) = main else {
        return Ok(ExecResult {
            exit_code: 1,
            output: String::new(),
            error: "Lua 脚本未定义 main 函数".into(),
        });
    };

    // main(hook_name, ...args) —— 展开为多值参数（Vec 会被编组成单个 table）
    let mk_str = |s: &str| -> mlua::Value {
        lua.create_string(s)
            .map(mlua::Value::String)
            .unwrap_or(mlua::Value::Nil)
    };
    let mut multi = mlua::MultiValue::new();
    multi.push_front(mk_str(hook_name));
    for a in args {
        multi.push_back(mk_str(a));
    }
    match main.call::<mlua::Value>(multi) {
        Ok(mlua::Value::Integer(n)) => Ok(ExecResult {
            exit_code: n as i32,
            output: format!("hook {hook_name} 执行完成 (exit {n})"),
            error: String::new(),
        }),
        Ok(mlua::Value::Boolean(ok)) => {
            let code = if ok { 0 } else { 1 };
            Ok(ExecResult { exit_code: code, output: format!("hook {hook_name} 执行完成 (exit {code})"), error: String::new() })
        }
        // nil / 其它返回值 = 成功
        Ok(_) => Ok(ExecResult {
            exit_code: 0,
            output: format!("hook {hook_name} 执行完成 (exit 0)"),
            error: String::new(),
        }),
        Err(e) => Ok(ExecResult {
            exit_code: 1,
            output: String::new(),
            error: format!("Lua main() 执行失败: {e}"),
        }),
    }
}

/// 把 Lua 脚本保存到 `.git/byte-git/hook-scripts/<name>.lua` 并返回路径。
pub fn save_lua_script(repo: &Path, hook_name: &str, content: &str) -> Result<PathBuf> {
    let dir = repo_data_dir(repo)?.join("hook-scripts");
    crate::util::AppPaths::ensure_dir(&dir)?;
    let path = dir.join(format!("{hook_name}.lua"));
    std::fs::write(&path, content).context("写入脚本失败")?;
    Ok(path)
}

/// 读取已安装钩子的 Lua 源码。
pub fn read_lua_script(repo: &Path, hook_name: &str) -> Result<String> {
    let cfg = load_config(repo)?;
    match cfg.hooks.get(hook_name) {
        Some(e) if !e.source.is_empty() => Ok(std::fs::read_to_string(&e.source)?),
        _ => Ok(String::new()),
    }
}
