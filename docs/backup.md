# backup — 完整仓库备份

通过 `git clone` 创建完整仓库备份，方便手动回滚。

备份存储在 `~/.cicbyte/byte-git/backups/<repo-name>/` 下，不占用项目空间。

## 基本用法

```bash
byte-git backup <子命令>
```

## 子命令

### create — 创建备份

```bash
byte-git backup create
```

在备份目录创建一份完整的仓库 clone。

### ls — 列出备份

```bash
byte-git backup ls
```

显示当前仓库的所有备份。

### rm — 删除备份

```bash
byte-git backup rm <name>
```

删除指定名称的备份。

## 存储位置

```
~/.cicbyte/byte-git/backups/
  <repo-name>/
    <timestamp>/      # 每次备份一个独立目录
```
