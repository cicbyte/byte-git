# info — 仓库洞察

9 项分析命令，全面了解仓库状态。

## 命令列表

| 子命令 | 说明 |
|---|---|
| `activity` | 每日提交活动柱状图 |
| `authors` | 作者贡献统计 |
| `todos` | 搜索 TODO/FIXME/HACK/XXX 标记 |
| `health` | 仓库健康检查 |
| `branches` | 分支概览 |
| `size` | 仓库体积分析 |
| `summary` | 仓库概览 |
| `changelog` | 生成变更日志 |
| `hotspots` | 变更热点分析 |

## activity — 提交活动

```bash
byte-git info activity              # 全部提交
byte-git info activity --days 30    # 最近 30 天
```

输出每日 commit 数量的文本柱状图。

## authors — 作者统计

```bash
byte-git info authors                # 按提交数排序
byte-git info authors --days 30      # 最近 30 天
```

## todos — TODO 标记搜索

```bash
byte-git info todos                  # 文本输出
byte-git info todos --format json    # JSON 输出
```

从代码中提取 TODO、FIXME、HACK、XXX 标记，按文件分组彩色输出。

## health — 仓库健康检查

```bash
byte-git info health
```

检查项：

| 检查项 | 说明 | 级别 |
|---|---|---|
| 未提交改动 | `git status --porcelain` | ⚠ warn |
| 大文件 (>10MB) | `git ls-files` + 文件大小 | ⚠ warn |
| 过期分支 (>30天) | `git for-each-ref` | ⚠ warn |
| 已合并未删除分支 | `git branch --merged` | ℹ info |
| Stash 堆积 | `git stash list` | ℹ info |
| 远程未同步 | HEAD vs @{u} | ⚠ warn |

输出 A-C 评分。

## branches — 分支概览

```bash
byte-git info branches
```

分类展示：当前分支、活跃分支（最近 14 天）、已合并分支、过期分支（>30 天），含 ahead/behind 信息。

## size — 仓库体积分析

```bash
byte-git info size
```

按文件类型（Top 10）、按目录统计体积，列出大文件（>5MB）和二进制文件。

## summary — 仓库概览

```bash
byte-git info summary
```

一览仓库：文件数、体积、提交数、贡献者、分支、标签、时间线、语言分布、贡献者排名。

## changelog — 变更日志

```bash
byte-git info changelog                        # 文本输出
byte-git info changelog --format markdown      # Markdown 输出
byte-git info changelog --from v1.0.0          # 指定起始版本
byte-git info changelog --from abc123..def456  # 指定范围
```

按 conventional commits 类型分组（feat > fix > refactor > ...），彩色输出。

## hotspots — 变更热点

```bash
byte-git info hotspots                      # 全部，默认 15 条
byte-git info hotspots --days 30            # 最近 30 天
byte-git info hotspots --limit 10           # 显示前 10 个
```

按文件统计变更次数，展示柱状图和增删行数，快速定位高风险文件。

## 通用选项

所有 info 子命令均支持：

| 选项 | 说明 | 默认值 |
|---|---|---|
| `--repo` | 目标仓库路径 | `.` |
