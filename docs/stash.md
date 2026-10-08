# stash — Stash 增强

提供带名称保存/弹出、搜索、差异查看的 stash 管理功能。

## 基本用法

```bash
byte-git stash <子命令> [选项]
```

## 子命令

### save — 保存 stash

```bash
byte-git stash save --name my-work
```

| 选项 | 说明 |
|---|---|
| `--name` | stash 名称（必填） |

### pop — 弹出 stash

```bash
byte-git stash pop --name my-work
```

按名称查找并弹出 stash。

| 选项 | 说明 |
|---|---|
| `--name` | stash 名称（必填） |

### list — 列出 stash

```bash
# 列出所有 stash
byte-git stash list

# 搜索 stash
byte-git stash list --search pattern
```

| 选项 | 说明 |
|---|---|
| `--search` | 搜索关键词 |

### show — 查看 stash

```bash
byte-git stash show stash@{0}
byte-git stash show --diff stash@{0}
```

| 选项 | 说明 |
|---|---|
| `--diff` | 显示完整差异 |
