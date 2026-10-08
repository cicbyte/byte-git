# undo — 安全撤销

提供安全的撤销操作，避免误操作丢失工作。

## 基本用法

```bash
byte-git undo <子命令> [选项]
```

## 子命令

### last-commit — 撤销最后一次提交

```bash
# 撤销提交，保留改动（改动回到暂存区）
byte-git undo last-commit --keep-changes

# 撤销提交，丢弃改动
byte-git undo last-commit
```

| 选项 | 说明 | 默认值 |
|---|---|---|
| `--keep-changes` | 保留改动 | `false` |

### rebase — 撤销 rebase

```bash
# 终止 rebase
byte-git undo rebase --abort

# 备份后终止
byte-git undo rebase --backup
```

| 选项 | 说明 | 默认值 |
|---|---|---|
| `--abort` | 终止 rebase | `false` |
| `--backup` | 备份后终止 | `false` |

### merge — 撤销合并

```bash
# 终止合并
byte-git undo merge --abort
```

| 选项 | 说明 | 默认值 |
|---|---|---|
| `--abort` | 终止合并 | `false` |
