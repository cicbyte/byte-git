//! 代理加速克隆命令（`byte-git clone -u <url>`）。

use byte_git_core::clone as core_clone;
use clap::Args;

#[derive(Args, Debug)]
pub struct CloneCmd {
    /// 仓库 URL（--list 时可省略）
    #[arg(short = 'u', long)]
    pub repo_url: Option<String>,

    /// 代理名称（默认 ghp）
    #[arg(short = 'n', long)]
    pub proxy_name: Option<String>,

    /// 克隆后还原为原始仓库 URL
    #[arg(short = 'r', long)]
    pub restore_url: bool,

    /// HTTP 代理服务器
    #[arg(long)]
    pub http_proxy: Option<String>,

    /// HTTPS 代理服务器
    #[arg(long)]
    pub https_proxy: Option<String>,

    /// 不使用代理的主机列表
    #[arg(long)]
    pub no_proxy: Option<String>,

    /// 本地代理端口（如 1080、7890）
    #[arg(long)]
    pub local_proxy: Option<String>,

    /// 代理类型（http / socks5）
    #[arg(long, default_value = "http")]
    pub proxy_type: String,

    /// 代理预设（shadowsocks / v2ray / clash / surge）
    #[arg(long)]
    pub proxy_preset: Option<String>,

    /// 列出可用代理（读取 rules.lua get_proxies()）
    #[arg(long)]
    pub list: bool,
}

pub fn run(c: CloneCmd) -> anyhow::Result<()> {
    if c.list {
        let proxies = core_clone::get_proxy_list()?;
        println!("{:<12} {:<40} {}", "名称", "URL", "默认");
        for p in &proxies {
            println!("{:<12} {:<40} {}", p.name, p.url, if p.is_default { "*" } else { "" });
        }
        return Ok(());
    }

    let Some(repo_url) = c.repo_url.clone() else {
        anyhow::bail!("请用 -u 指定仓库 URL（或 --list 查看代理列表）");
    };

    core_clone::clone(core_clone::CloneOptions {
        proxy_name: c.proxy_name.clone().unwrap_or_default(),
        repo_url,
        restore_url: c.restore_url,
        http_proxy: c.http_proxy.clone().unwrap_or_default(),
        https_proxy: c.https_proxy.clone().unwrap_or_default(),
        no_proxy: c.no_proxy.clone().unwrap_or_default(),
        local_proxy: c.local_proxy.clone().unwrap_or_default(),
        proxy_type: c.proxy_type.clone(),
        proxy_preset: c.proxy_preset.clone().unwrap_or_default(),
    })
}
