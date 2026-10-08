# log — TUI Git Log 浏览器

交互式终端 Git Log 浏览器，全屏查看提交历史。

> **注意**：命令名是 `log`（不是 `logview`）。对应的源码包为 `cmd/logview`。

## 基本用法

```bash
byte-git log                            # 浏览当前仓库
byte-git log --repo /path/to/repo       # 浏览指定仓库
```

## 参数

| 参数 | 类型 | 默认值 | 说明 |
|---|---|---|---|
| `--repo` | string | `.` | 目标仓库路径 |

## 键盘操作

| 按键 | 功能 |
|---|---|
| `↑` / `k` | 上一条提交 |
| `↓` / `j` | 下一条提交 |
| `PgUp` / `PgDn` | 翻页 |
| `Enter` | 查看当前提交的 diff |
| `/` | 搜索（关键词、commit hash、作者） |
| `q` | 退出 |

历史记录会自动分页加载，无需手动翻页。
