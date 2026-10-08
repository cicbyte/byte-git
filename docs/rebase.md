# rebase — Rebase 辅助

提供 rebase 预览和自动 squash 功能。

## 基本用法

```bash
byte-git rebase <子命令> [选项]
```

## 子命令

### dry-run — 预览 rebase

```bash
# 预览最近 5 个提交的 rebase
byte-git rebase dry-run --count 5

# 预览相对于 main 的 rebase
byte-git rebase dry-run --branch main
```

| 选项 | 说明 | 默认值 |
|---|---|---|
| `--count` | 提交数量（`0` = 全部） | `0` |
| `--branch` | 参照分支（如 `main`） | `""` |
| `--repo` | 目标仓库路径 | `.` |

### auto-squash — 自动合并

```bash
# 自动合并 fixup/squash 提交
byte-git rebase auto-squash --count 10

# 仅预览，不执行
byte-git rebase auto-squash --dry-run
```

| 选项 | 说明 | 默认值 |
|---|---|---|
| `--count` | 提交数量（`0` = 全部） | `0` |
| `--branch` | 参照分支（如 `main`） | `""` |
| `--dry-run` | 仅预览，不执行 | `false` |
| `--repo` | 目标仓库路径 | `.` |
