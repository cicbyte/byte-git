# hook — Git 钩子管理

管理 Git 钩子的安装、查看和移除。支持安装 Shell/Python/Node 脚本作为钩子（靠 shebang 执行，git 原生机制）。

## 基本用法

```bash
byte-git hook <子命令> [选项]
```

## 子命令

### list — 列出钩子

```bash
byte-git hook list
```

显示所有 Git 钩子及其安装状态（按 git 实际生效的 hooks 目录，感知 `core.hooksPath`）。

### install — 安装钩子

```bash
byte-git hook install pre-commit --script ./hooks/pre.sh
```

将指定脚本安装为 Git 钩子（复制到 hooks 目录，git 经 shebang 直接执行）。

| 参数 | 类型 | 默认值 | 说明 |
|---|---|---|---|
| `<hook-name>` | 位置参数 | （必填） | 钩子类型，如 `pre-commit`、`commit-msg`、`pre-push` |
| `--script` | string | `""` | 脚本文件路径（首行 shebang 决定解释器） |
| `--repo` | string | `.` | 目标仓库路径 |

### remove — 移除钩子

```bash
byte-git hook remove <hook-name>                 # 仓库级（默认）
byte-git hook remove <hook-name> --scope global  # 全局 hooksPath 目录
```

| 参数 | 类型 | 默认值 | 说明 |
|---|---|---|---|
| `<hook-name>` | 位置参数 | （必填） | 要移除的钩子名 |
| `--scope` | string | `local` | 作用域：`local` / `global`（对应 `core.hooksPath` 层级） |
| `--repo` | string | `.` | 目标仓库路径 |

### run — 运行钩子（husky 式入口）

```bash
byte-git hook run <hook-name>                          # 执行钩子
byte-git hook run commit-msg --ai                      # AI 改写当前 commit message
byte-git hook run commit-msg --ai --model gemma4:12b   # 指定 AI 模型
```

目前支持 `commit-msg`：非 AI 模式打印消息；AI 模式基于 staged diff 改写后写回消息文件（保留 `#` 注释块），AI 失败保留原文。消息文件路径取 `BYTE_GIT_COMMIT_MSG_FILE` 环境变量或进程尾参（git 调用钩子时自动传入）。可在钩子脚本中调用实现 husky 式体验。

| 参数 | 类型 | 默认值 | 说明 |
|---|---|---|---|
| `<hook-name>` | 位置参数 | （必填） | 要执行的钩子名（`commit-msg`） |
| `--ai` | bool | `false` | 使用 AI 改写 commit message |
| `--model` | string | `gemma4:e4b` | AI 模型名称 |
| `--base-url` | string | `http://localhost:11434` | Ollama API 地址 |
| `--lang` | string | `zh` | 输出语言（`zh` / `en`） |
| `--repo` | string | `.` | 目标仓库路径 |

## 脚本支持

| 类型 | shebang | 说明 |
|---|---|---|
| Shell | `#!/bin/sh` / `#!/bin/bash` | 推荐（git 自带 sh） |
| Python | `#!/usr/bin/env python` | |
| Node | `#!/usr/bin/env node` | |

## hooks 目录解析

优先级：仓库级 `core.hooksPath` > 全局 `core.hooksPath` > `<gitdir>/hooks`。GUI 的「Git 配置 → Hook」页可直接配置。

## 配置存储

仓库级钩子配置存储在 `.git/byte-git/config.json`；全局在 `~/.cicbyte/apps/byte-git/hooks.json`。
