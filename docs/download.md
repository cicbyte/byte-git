# download — 下载 GitHub Release

交互式下载 GitHub Release 资源文件，支持自动匹配当前平台架构。

两种模式：
1. **交互模式**：传入 `owner/repo`，自动检索 Release 并 TUI 选择下载
2. **直接模式**：传入 `-u URL` 直接下载指定文件

## 基本用法

```bash
byte-git download [owner/repo] [选项]
byte-git download -u <URL> -s <保存路径>
```

## 示例

```bash
# 交互式选择版本和文件（TUI 界面）
byte-git download cicbyte/ktconsole

# 直接下载最新版本（自动匹配当前平台）
byte-git download cicbyte/ktconsole --latest

# 指定下载目录
byte-git download cicbyte/ktconsole -d ./tmp

# 直接模式下载指定文件
byte-git download -u https://github.com/user/repo/releases/download/v1.0/app.zip -s ./app.zip
```

## 选项

| 选项 | 说明 | 默认值 |
|---|---|---|
| `-l, --latest` | 直接下载最新版本 | `false` |
| `-d, --dest` | 下载目录 | 当前目录 |
| `-u, --url` | 直接下载指定 URL | — |
| `-s, --save-path` | 保存路径（直接模式） | — |

## 功能特性

- **平台自动匹配**：根据当前 GOOS/GOARCH 自动选中最佳匹配的 asset
- **智能跳过**：已下载的文件（大小一致）自动跳过
- **TUI 交互**：空格选择/取消，`a` 全选/全不选，回车确认下载
- **进度显示**：实时显示下载进度
