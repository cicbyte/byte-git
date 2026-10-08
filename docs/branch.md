# branch — 分支管理

提供分支批量清理和查询功能。

## 基本用法

```bash
byte-git branch <子命令> [选项]
```

## 子命令

### clean — 清理已合并分支

```bash
# 预览待删除分支
byte-git branch clean --dry-run

# 删除已合并分支
byte-git branch clean

# 指定基准分支
byte-git branch clean --merged-into main
```

| 选项 | 说明 | 默认值 |
|---|---|---|
| `--dry-run` | 仅预览，不删除 | `false` |
| `--merged-into` | 基准分支名 | 当前分支 |

### list — 列出分支

```bash
# 列出所有分支
byte-git branch list

# 列出孤立分支（无追踪远程的分支）
byte-git branch list --orphan
```

| 选项 | 说明 | 默认值 |
|---|---|---|
| `--orphan` | 仅显示孤立分支 | `false` |
