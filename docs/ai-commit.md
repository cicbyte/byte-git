# ai commit — AI 生成提交信息

根据暂存区的 diff 内容，通过 AI 生成 conventional commit message，确认后自动提交。

支持 OpenAI 兼容 API（Ollama、DeepSeek、OpenAI、Groq 等）。

## 基本用法

```bash
byte-git ai commit [选项]
```

## 示例

```bash
# 使用默认 Ollama
byte-git ai commit

# 仅预览，不提交
byte-git ai commit --dry-run

# 使用 DeepSeek API
byte-git ai commit --base-url https://api.deepseek.com/v1 --api-key sk-xxx

# 使用英文描述
byte-git ai commit --lang en
```

## 选项

| 选项 | 说明 | 默认值 |
|---|---|---|
| `--model` | AI 模型名称 | `gemma4:e4b` |
| `--base-url` | API Base URL | `http://localhost:11434/v1` |
| `--api-key` | API Key（Ollama 无需填写） | — |
| `--lang` | 描述语言 | `zh` |
| `--max-tokens` | diff 部分 token 上限 | `4000` |
| `--dry-run` | 仅预览，不提交 | `false` |
| `--no-verify` | 跳过 Git hooks | `false` |
| `--repo` | 目标仓库路径 | `.` |

## 工作流程

1. 读取暂存区 diff
2. 调用 AI 生成 conventional commit message
3. 交互式确认（支持编辑和重新生成）
4. 确认后执行 `git commit`
