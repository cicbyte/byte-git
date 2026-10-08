# clone — 代理加速克隆

通过配置的代理镜像加速 `git clone`，支持自定义代理、快捷预设以及克隆后还原原始 URL。

## 基本用法

```bash
byte-git clone -u <仓库URL> [选项]
```

## 示例

```bash
# 通过默认代理克隆
byte-git clone -u https://github.com/user/repo.git

# 克隆后还原为原始仓库 URL
byte-git clone -u https://github.com/user/repo.git -r

# 指定本地代理端口
byte-git clone -u https://github.com/user/repo.git --local-proxy 7890

# 使用代理预设
byte-git clone -u https://github.com/user/repo.git --proxy-preset clash
```

## 选项

| 选项 | 说明 | 默认值 |
|---|---|---|
| `-u, --repo-url` | 仓库 URL（必填） | — |
| `-r, --restore-url` | 克隆后还原为原始仓库 URL | `false` |
| `-n, --proxy-name` | 代理名称 | `ghp` |
| `--proxy-type` | 代理类型 | `http` |
| `--local-proxy` | 本地代理端口（如 1080、7890） | — |
| `--http-proxy` | HTTP 代理服务器 | — |
| `--https-proxy` | HTTPS 代理服务器 | — |
| `--proxy-preset` | 代理预设（shadowsocks/v2ray/clash/surge） | — |
| `--no-proxy` | 不使用代理的主机列表 | — |

## 代理预设

内置常用代理工具的默认端口和协议：

| 预设名 | 代理类型 | 默认端口 |
|---|---|---|
| `shadowsocks` / `ss` | socks5 | 1080 |
| `v2ray` | http | 1080 |
| `clash` | http | 7890 |
| `surge` | http | 6152 |
