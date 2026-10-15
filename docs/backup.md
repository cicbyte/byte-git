# backup — 完整仓库备份

通过 `git clone --no-hardlinks` 创建完整仓库备份（带工作区，可直接浏览/拷回），方便手动回滚。

备份存储在 `~/.cicbyte/apps/byte-git/backups/<repo-name>/<时间戳>-<repo-name>/` 下，不占用项目空间。

## 基本用法

```bash
byte-git backup <子命令> --repo <路径>   # --repo 默认当前目录
```

## 子命令

### create — 创建备份

```bash
byte-git backup create
```

在备份目录创建一份完整的仓库 clone（每次全新时间戳条目，克隆上限 30 分钟）。

### ls — 列出备份

```bash
byte-git backup ls
```

显示当前仓库的所有备份（新在前，含可读时间戳）。`list` 为别名。

### rm — 删除备份

```bash
byte-git backup rm <name>
```

删除指定名称的备份。

## 存储位置

```
~/.cicbyte/apps/byte-git/backups/
  <repo-name>/
    <YYYYMMDD-HHMMSS>-<repo-name>/   # 每次备份一个独立目录（完整 clone）
```

> 旧版镜像备份（`backups/<repo-name>/` 即裸仓库）会在 list 时自动转换为标准条目，无需手动处理。
