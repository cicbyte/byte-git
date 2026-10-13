# config — 配置管理

管理 byte-git 的全局配置。配置存储在 `~/.cicbyte/apps/byte-git/config.yaml`。

## 基本用法

```bash
byte-git config <子命令>
```

## 子命令

### list — 列出所有配置

```bash
byte-git config list
```

### get — 查看单个配置

```bash
byte-git config get ai.model
byte-git config get ai.api_key --show    # 显示敏感字段的明文（默认脱敏）
```

| 参数 | 类型 | 默认值 | 说明 |
|---|---|---|---|
| `<key>` | 位置参数 | （必填） | 配置键名 |
| `--show` | bool | `false` | 显示敏感字段（如 API Key）的明文 |

### set — 设置配置

```bash
# 直接设置
byte-git config set ai.model gpt-4o

# 交互输入（不回显，适合设置 API Key）
byte-git config set ai.api_key
```

## 配置项

| 配置项 | 说明 | 敏感 |
|---|---|---|
| `ai.model` | AI 模型名称 | 否 |
| `ai.base_url` | AI API 地址 | 否 |
| `ai.api_key` | API Key | 是 |
| `ai.lang` | AI 输出语言（`zh` / `en`） | 否 |
| `ai.max_tokens` | 最大 token 数 | 否 |
| `github.token` | GitHub Token | 是 |

## 配置文件位置

```
~/.cicbyte/apps/byte-git/config.yaml
```

## AI 提示词自定义

AI 提示词模板存储在 `~/.cicbyte/apps/byte-git/prompts/` 下，可直接编辑：

| 文件 | 用途 |
|---|---|
| `aicommit.md` | `ai commit` 的提示词模板 |
| `airewrite.md` | `ai rewrite` 的提示词模板 |

支持模板变量：`{{language}}`、`{{original}}`、`{{files}}`、`{{diff_summary}}`、`{{diff}}`。
