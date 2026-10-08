# hook — Git 钩子管理

管理 Git 钩子的安装、查看和移除。支持安装 Lua 脚本或 Shell 脚本作为钩子。

## 基本用法

```bash
byte-git hook <子命令> [选项]
```

## 子命令

### list — 列出钩子

```bash
byte-git hook list
```

显示所有 Git 钩子及其安装状态。

### install — 安装钩子

```bash
byte-git hook install <hook-name> --lua my-hook.lua        # 安装 Lua 脚本
byte-git hook install pre-commit --script ./hooks/pre.sh   # 安装 Shell 脚本
```

将指定脚本安装为 Git 钩子。通过 `--lua` 或 `--script` 指定脚本文件。

| 参数 | 类型 | 默认值 | 说明 |
|---|---|---|---|
| `<hook-name>` | 位置参数 | （必填） | 钩子类型，如 `pre-commit`、`commit-msg`、`pre-push` |
| `--lua` | string | `""` | Lua 脚本文件路径 |
| `--script` | string | `""` | Shell 脚本文件路径 |
| `--repo` | string | `.` | 目标仓库路径 |

> `--lua` 和 `--script` 二选一。

### remove — 移除钩子

```bash
byte-git hook remove <hook-name>
```

| 参数 | 类型 | 默认值 | 说明 |
|---|---|---|---|
| `<hook-name>` | 位置参数 | （必填） | 要移除的钩子名 |
| `--repo` | string | `.` | 目标仓库路径 |

### run — 手动运行钩子

```bash
byte-git hook run <hook-name>                          # 执行钩子
byte-git hook run commit-msg --ai                      # AI 改写当前 commit message
byte-git hook run commit-msg --ai --model gemma4:12b   # 指定 AI 模型
```

手动执行指定钩子，用于测试。支持 AI 改写 commit message。

| 参数 | 类型 | 默认值 | 说明 |
|---|---|---|---|
| `<hook-name>` | 位置参数 | （必填） | 要执行的钩子名 |
| `--ai` | bool | `false` | 使用 AI 改写 commit message |
| `--model` | string | `gemma4:e4b` | AI 模型名称 |
| `--base-url` | string | `http://localhost:11434` | Ollama API 地址 |
| `--lang` | string | `zh` | 输出语言（`zh` / `en`） |
| `--repo` | string | `.` | 目标仓库路径 |

## 脚本支持

| 类型 | 文件扩展名 | 说明 |
|---|---|---|
| Shell | `.sh` | 标准 Shell 脚本 |
| Lua | `.lua` | 内嵌 Lua 解释器执行 |

## 配置存储

钩子配置存储在 `.git/byte-git/config.json`，可纳入版本管理。
