//! 代理加速克隆：快捷预设/本地代理环境注入 + Lua 规则引擎（加速 URL 生成）。
//!
//! 移植自 git-mate `cmd/clone.go` + `internal/git/git.go` + rules.lua。

use crate::util::{exec_capture, AppPaths};
use anyhow::{bail, Context, Result};
use mlua::{Function, Lua};
use std::path::Path;

#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct ProxyInfo {
    pub name: String,
    pub url: String,
    #[serde(rename = "default")]
    pub is_default: bool,
}

#[derive(Debug, Clone, Default)]
pub struct CloneOptions {
    pub proxy_name: String,
    pub repo_url: String,
    pub restore_url: bool,
    pub http_proxy: String,
    pub https_proxy: String,
    pub no_proxy: String,
    pub local_proxy: String,
    pub proxy_type: String,
    pub proxy_preset: String,
}

/// 预设 → (端口, 类型)。移植 processCloneShortcutProxy。
fn apply_preset(opts: &mut CloneOptions) {
    if !opts.proxy_preset.is_empty() {
        match opts.proxy_preset.as_str() {
            "shadowsocks" | "ss" => {
                opts.local_proxy = "1080".into();
                opts.proxy_type = "socks5".into();
                println!("使用 Shadowsocks 预设 (SOCKS5:1080)");
            }
            "v2ray" => {
                opts.local_proxy = "1080".into();
                opts.proxy_type = "http".into();
                println!("使用 V2Ray 预设 (HTTP:1080)");
            }
            "clash" => {
                opts.local_proxy = "7890".into();
                opts.proxy_type = "http".into();
                println!("使用 Clash 预设 (HTTP:7890)");
            }
            "surge" => {
                opts.local_proxy = "6152".into();
                opts.proxy_type = "http".into();
                println!("使用 Surge 预设 (HTTP:6152)");
            }
            other => println!("未知的代理预设: {other}"),
        }
    }

    if !opts.local_proxy.is_empty() {
        let proxy_url = if opts.proxy_type == "socks5" {
            format!("socks5://127.0.0.1:{}", opts.local_proxy)
        } else {
            format!("http://127.0.0.1:{}", opts.local_proxy)
        };
        if opts.http_proxy.is_empty() && opts.https_proxy.is_empty() {
            opts.http_proxy = proxy_url.clone();
            opts.https_proxy = proxy_url.clone();
            println!("快捷代理: {proxy_url}");
        }
        if opts.no_proxy.is_empty() {
            opts.no_proxy = "localhost,127.0.0.1,0.0.0.0,::1,*.local".into();
        }
    }
}

/// 注入代理环境变量与 git 全局代理配置。
fn setup_proxy(opts: &mut CloneOptions) {
    apply_preset(opts);
    if !opts.http_proxy.is_empty() {
        set_proxy_env("HTTP_PROXY", "http_proxy", &opts.http_proxy);
        println!("已设置 HTTP 代理: {}", opts.http_proxy);
    }
    if !opts.https_proxy.is_empty() {
        set_proxy_env("HTTPS_PROXY", "https_proxy", &opts.https_proxy);
        println!("已设置 HTTPS 代理: {}", opts.https_proxy);
    }
    if !opts.no_proxy.is_empty() {
        std::env::set_var("NO_PROXY", &opts.no_proxy);
        std::env::set_var("no_proxy", &opts.no_proxy);
        println!("已设置代理排除列表: {}", opts.no_proxy);
    }
}

fn set_proxy_env(upper: &str, lower: &str, url: &str) {
    std::env::set_var(upper, url);
    std::env::set_var(lower, url);
    // 与 git-mate 一致：同步写入 git 全局配置（git 自身走 http.proxy）
    let which = if upper == "HTTP_PROXY" { "http.proxy" } else { "https.proxy" };
    let _ = exec_capture("git", &["config", "--global", which, url]);
}

/// 规则文件（缺失时从内嵌默认落盘——默认规则与仓库 rules/rules.lua 同源）。
pub fn ensure_rules_file() -> Result<std::path::PathBuf> {
    let paths = AppPaths::new()?;
    AppPaths::ensure_dir(&paths.rules_dir())?;
    let file = paths.rules_file();
    if !file.exists() {
        let default = include_str!("../assets/rules.lua");
        std::fs::write(&file, default).with_context(|| format!("写入默认规则失败: {}", file.display()))?;
        println!("已初始化默认代理规则: {}", file.display());
    }
    Ok(file)
}

fn load_rules_lua() -> Result<Lua> {
    let file = ensure_rules_file()?;
    let lua = Lua::new();
    let src = std::fs::read_to_string(&file).with_context(|| format!("读取代理规则失败: {}", file.display()))?;
    lua.load(src)
        .exec()
        .map_err(|e| anyhow::anyhow!("加载代理规则失败（Lua 语法错误）: {e}"))?;
    Ok(lua)
}

/// 调用 get_proxies() 解析代理列表。
pub fn get_proxy_list() -> Result<Vec<ProxyInfo>> {
    let lua = load_rules_lua()?;
    let get: Function = lua
        .globals()
        .get("get_proxies")
        .map_err(|_| anyhow::anyhow!("规则文件缺少 get_proxies() 函数"))?;
    let table: mlua::Table = get
        .call(())
        .map_err(|e| anyhow::anyhow!("get_proxies() 调用失败: {e}"))?;
    let mut proxies = Vec::new();
    for item in table.sequence_values::<mlua::Table>() {
        let t = item.map_err(|e| anyhow::anyhow!("解析代理条目失败: {e}"))?;
        let name: String = t.get("name").unwrap_or_default();
        let url: String = t.get("url").unwrap_or_default();
        let is_default: bool = t.get("default").unwrap_or(false);
        if !name.is_empty() && !url.is_empty() {
            proxies.push(ProxyInfo { name, url, is_default });
        }
    }
    if proxies.is_empty() {
        bail!("no proxies configured");
    }
    Ok(proxies)
}

/// 按名称选择代理，回退 default。
pub fn select_proxy(proxies: &[ProxyInfo], name: &str) -> Result<ProxyInfo> {
    if let Some(p) = proxies.iter().find(|p| p.name == name) {
        return Ok(p.clone());
    }
    if let Some(p) = proxies.iter().find(|p| p.is_default) {
        eprintln!("Proxy {name} not found, using default: {}", p.name);
        return Ok(p.clone());
    }
    bail!("no matching proxy found for {name}")
}

/// 调用 get_accelerated_url(repo_url, proxy_name) 生成加速 URL（失败回退前缀拼接）。
pub fn generate_accelerated_url(proxy_url: &str, repo_url: &str, proxy_name: &str) -> String {
    let fallback = format!("{proxy_url}{repo_url}");
    let Ok(lua) = load_rules_lua() else { return fallback };
    let Ok(f) = lua.globals().get::<Function>("get_accelerated_url") else { return fallback };
    match f.call::<String>((repo_url, proxy_name)) {
        Ok(url) => url,
        Err(e) => {
            eprintln!("Lua acceleration failed: {e}");
            fallback
        }
    }
}

/// 完整克隆流程。
pub fn clone(mut opts: CloneOptions) -> Result<()> {
    if opts.repo_url.is_empty() {
        bail!("请用 -u 指定仓库 URL");
    }
    if opts.proxy_name.is_empty() {
        opts.proxy_name = "ghp".into();
    }
    if opts.proxy_type.is_empty() {
        opts.proxy_type = "http".into();
    }

    setup_proxy(&mut opts);

    let proxies = get_proxy_list().context("获取代理列表失败")?;
    let selected = select_proxy(&proxies, &opts.proxy_name)?;
    let accelerated = generate_accelerated_url(&selected.url, &opts.repo_url, &selected.name);

    println!("使用代理: {}", selected.name);
    println!("克隆仓库: {accelerated}");

    crate::util::exec_inherit("git", &["clone", &accelerated]).context("克隆失败")?;

    if opts.restore_url {
        let repo_dir = accelerated
            .rsplit('/')
            .next()
            .unwrap_or_default()
            .trim_end_matches(".git")
            .to_string();
        if repo_dir.is_empty() || !Path::new(&repo_dir).exists() {
            bail!("repository directory not found: {repo_dir}");
        }
        crate::util::exec_inherit("git", &["-C", &repo_dir, "remote", "set-url", "origin", &opts.repo_url])
            .context("还原 URL 失败")?;
        println!("已还原原始 URL: {}", opts.repo_url);
    }
    Ok(())
}

/// Lua 值转换辅助（供后续 hook 模块复用）。
pub fn lua_to_string(v: mlua::Value) -> String {
    match v {
        mlua::Value::String(s) => s.to_string_lossy(),
        mlua::Value::Boolean(b) => b.to_string(),
        other => format!("{other:?}"),
    }
}
