# ai rewrite — AI 改写提交信息

通过 AI 将不规范的 commit message 改写为 conventional commit 格式。

三步操作：导出 → AI 生成 → 应用。

## 基本用法

```bash
# 1. 导出
byte-git ai rewrite export [选项]

# 2. AI 生成建议消息
byte-git ai rewrite generate [选项]

# 3. 应用改写
byte-git ai rewrite apply [选项]
```

## 示例

```bash
# 导出最近 10 条提交
byte-git ai rewrite export --last 10

# 自动标记不规范的消息
byte-git ai rewrite export --last 20 --auto-detect

# 指定输出文件
byte-git ai rewrite export --last 10 -o my-rewrite.json

# AI 生成建议（使用 DeepSeek，并发 3）
byte-git ai rewrite generate \
  --base-url https://api.deepseek.com/v1 \
  --model deepseek-chat \
  --concurrency 3

# 仅预览，不调用 AI
byte-git ai rewrite generate --dry-run

# 仅重新处理上次失败的条目
byte-git ai rewrite generate --retry

# 应用改写
byte-git ai rewrite apply
```

## 工作流程

1. **export**：导出提交信息到 JSON 文件
2. **编辑 JSON**：标记需要 AI 处理的条目（`need_ai: true`），或用 `--auto-detect` 自动标记
3. **generate**：AI 为标记的条目生成建议消息
4. **审核 JSON**：检查 `suggested` 字段内容
5. **apply**：应用改写到 git 历史

## AI 缓存

已生成的内容会缓存到 `.git/byte-git/ai-cache/`，重复执行不会重复调用 AI。

## 选项

### export

| 选项 | 说明 | 默认值 |
|---|---|---|
| `--last` | 导出最近 N 条提交 | `0` |
| `--all` | 导出所有提交 | `true` |
| `--unpushed` | 仅未推送的提交 | `false` |
| `--range` | 指定 commit 范围 | `""` |
| `--auto-detect` | 自动标记不规范的 commit message | `false` |
| `--pattern` | 自定义不规范匹配正则（配合 `--auto-detect`） | `""` |
| `-o, --output` | 输出文件路径 | `ai-rewrite.json` |
| `--repo` | 目标仓库路径 | `.` |

### generate

| 选项 | 说明 | 默认值 |
|---|---|---|
| `-f, --file` | JSON 文件路径 | `.git/byte-git/rewrite/ai-rewrite.json` |
| `--model` | AI 模型名称 | `gemma4:e4b` |
| `--base-url` | API 地址 | `http://localhost:11434` |
| `--lang` | 描述语言（`zh` / `en`） | `zh` |
| `--concurrency` | 并发数 | `1` |
| `--max-chars` | prompt 最大字符数 | `8000` |
| `--dry-run` | 仅预览，不调用 AI | `false` |
| `--retry` | 仅重新处理失败条目 | `false` |
| `--repo` | 目标仓库路径 | `.` |

> **注意**：`generate` 没有 `--api-key` 参数，API Key 通过 `byte-git config set ai.api_key` 全局配置。

### apply

| 选项 | 说明 | 默认值 |
|---|---|---|
| `-f, --file` | JSON 文件路径 | `.git/byte-git/rewrite/ai-rewrite.json` |
| `--branch` | 目标分支 | `HEAD` |
| `--dry-run` | 仅预览，不执行 | `false` |
| `--repo` | 目标仓库路径 | `.` |
