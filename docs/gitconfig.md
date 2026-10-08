# gitconfig — Git 配置管理

管理 Git 自身的配置（`user.name` / `user.email` 等），区分 `local`（当前仓库 `.git/config`）与 `global`（`~/.gitconfig`，默认）两个层级。

> **与 `config` 命令的区别**：`gitconfig` 管理 Git 本身的配置，`config` 管理 byte-git 的配置（AI 模型、代理、Token 等）。

## 基本用法

```bash
byte-git gitconfig <子命令>
```

## 子命令

### list — 列出配置

```bash
byte-git gitconfig list                 # 列出常用 Git 配置及来源
```

以表格形式打印 KEY / SOURCE（`local` / `global` / `system`）/ VALUE。

| 参数 | 类型 | 默认值 | 说明 |
|---|---|---|---|
| `--repo` | string | `.` | 目标仓库路径 |

### get — 查看单项配置

```bash
byte-git gitconfig get user.name        # 查看 user.name 的生效值
```

打印指定 key 的：生效值、来源（local/global）、local 值、global 值。空值显示 `(未设置)`。

| 参数 | 类型 | 默认值 | 说明 |
|---|---|---|---|
| `<key>` | 位置参数 | （必填） | 配置键名，如 `user.name` |
| `--repo` | string | `.` | 目标仓库路径 |

### set — 设置配置

```bash
byte-git gitconfig set user.name "张三"              # 设置全局配置（默认）
byte-git gitconfig set user.email "a@b.com" --scope local  # 设置当前仓库配置
```

| 参数 | 类型 | 默认值 | 说明 |
|---|---|---|---|
| `<key>` | 位置参数 | （必填） | 配置键名 |
| `<value>` | 位置参数 | （必填） | 配置值 |
| `--scope` | string | `global` | 配置层级：`local`（当前仓库）/ `global`（全局） |
| `--repo` | string | `.` | 目标仓库路径 |

### unset — 删除配置

```bash
byte-git gitconfig unset user.email --scope local   # 删除当前仓库的配置
byte-git gitconfig unset user.name                   # 删除全局配置
```

| 参数 | 类型 | 默认值 | 说明 |
|---|---|---|---|
| `<key>` | 位置参数 | （必填） | 配置键名 |
| `--scope` | string | `global` | 配置层级：`local` / `global` |
| `--repo` | string | `.` | 目标仓库路径 |

## 配置层级说明

Git 配置有三个层级，优先级从高到低：

| 层级 | 文件位置 | `--scope` 参数 |
|---|---|---|
| **local** | `<repo>/.git/config` | `local` |
| **global** | `~/.gitconfig` | `global`（默认） |
| **system** | `/etc/gitconfig` | （只读，不可通过本命令设置） |

local 配置会覆盖 global，global 覆盖 system。
