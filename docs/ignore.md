# ignore — .gitignore 管理

管理项目的 `.gitignore` 文件，支持从模板添加规则、检查文件是否被忽略、AI 整理。

## 基本用法

```bash
byte-git ignore <子命令>
```

## 子命令

### add — 添加模板规则

```bash
byte-git ignore add go python node       # 添加多种语言模板
byte-git ignore add go -t github         # 从 GitHub 官方模板库获取
byte-git ignore add go -t ai             # AI 生成模板
byte-git ignore add go --dry-run         # 只看 diff 不写入
byte-git ignore add go --yes             # 跳过确认直接写入
```

从模板源获取指定语言的忽略规则，去重合并后写入 `.gitignore`。

| 参数 | 类型 | 默认值 | 说明 |
|---|---|---|---|
| `[lang...]` | 位置参数 | （必填） | 语言名（至少一个，如 `go python node`） |
| `-t, --type` | string | `local` | 模板源：`local`（内置）/ `github`（官方库）/ `ai`（AI 生成） |
| `--yes` | bool | `false` | 跳过确认直接写入 |
| `--dry-run` | bool | `false` | 只显示 diff 不写入 |
| `--repo` | string | `.` | 仓库路径 |

**模板源说明**：

- **local**：内置常用模板（go / python / node / macos / vscode 等）
- **github**：从 [github/gitignore](https://github.com/github/gitignore) 官方库克隆获取
- **ai**：调用 AI 流式生成（需要配置 AI 模型）

### check — 检查文件是否被忽略

```bash
byte-git ignore check main.go            # 检查文件是否被 .gitignore 忽略
byte-git ignore check build/output.log --repo /path/to/repo
```

调用 `git check-ignore -v` 判断。被忽略时显示匹配规则与来源（文件:行号）；未被忽略时退出码为 1。

| 参数 | 类型 | 默认值 | 说明 |
|---|---|---|---|
| `<file>` | 位置参数 | （必填） | 要检查的文件路径 |
| `--repo` | string | `.` | 仓库路径 |

### refactor — AI 整理 .gitignore

```bash
byte-git ignore refactor                 # 整理现有 .gitignore
byte-git ignore refactor --dry-run       # 只看 diff
byte-git ignore refactor --yes           # 跳过确认
```

整理现有 `.gitignore`：去重、分组、格式化，**不增删规则**。空文件或已是最佳状态时直接返回。

| 参数 | 类型 | 默认值 | 说明 |
|---|---|---|---|
| `--yes` | bool | `false` | 跳过确认直接写入 |
| `--dry-run` | bool | `false` | 只显示 diff 不写入 |
| `--repo` | string | `.` | 仓库路径 |
