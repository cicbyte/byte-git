# rewrite — 改写提交元数据

通过 JSON 文件改写 Git 提交的日期、作者、邮箱等元数据。

两步操作：导出 JSON → 编辑 → 应用。

## 基本用法

```bash
# 1. 导出
byte-git rewrite export [选项]

# 2. 编辑导出的 JSON 文件

# 3. 应用改写
byte-git rewrite apply [选项]
```

## export — 导出提交信息

```bash
# 按数量
byte-git rewrite export --last 20

# 按推送状态
byte-git rewrite export --unpushed

# 按范围
byte-git rewrite export --range abc123..def456

# 导出全部提交（默认行为）
byte-git rewrite export

# 指定输出文件
byte-git rewrite export --last 20 -o my-rewrite.json
```

### 规则导出

```bash
byte-git rewrite export --last 10 \
  --rule "author:=新作者名" \
  --rule "email:旧邮箱=新邮箱" \
  --rule "date-shift:+2h"
```

### 日期重映射

```bash
byte-git rewrite export --last 10 \
  --remap-dates "2024-01-01,2024-06-30" \
  --date-mode random \
  --active-hours 9-22 \
  --daily-limit 30
```

## apply — 应用改写

```bash
# 使用默认路径
byte-git rewrite apply

# 指定文件，同步远程 ref
byte-git rewrite apply -f custom.json --sync-refs

# 仅预览
byte-git rewrite apply --dry-run
```

## 选项

### export

| 选项 | 说明 | 默认值 |
|---|---|---|
| `--last` | 导出最近 N 条提交 | `0` |
| `--all` | 导出所有提交 | `true` |
| `--unpushed` | 仅未推送的提交 | `false` |
| `--range` | 指定 commit 范围（如 `abc123..def456`） | `""` |
| `-o, --output` | 输出文件路径 | `.git/byte-git/rewrite/rewrite.json` |
| `--rule` | 应用规则（可多次使用） | — |
| `--remap-dates` | 日期重映射范围（如 `2024-01-01,2024-06-30`） | `""` |
| `--date-mode` | 分布方式：`random`/`spread` | `random` |
| `--active-hours` | 活跃时间段，如 `9-22` | `""` |
| `--daily-limit` | 每天最大提交数（`0` 不限制） | `0` |
| `--repo` | 目标仓库路径 | `.` |

### apply

| 选项 | 说明 | 默认值 |
|---|---|---|
| `-f, --file` | JSON 文件路径 | `.git/byte-git/rewrite/rewrite.json` |
| `--branch` | 目标分支 | `HEAD` |
| `--sync-refs` | 改写后同步远程追踪 ref | `false` |
| `--dry-run` | 仅预览，不执行 | `false` |
| `--repo` | 目标仓库路径 | `.` |
