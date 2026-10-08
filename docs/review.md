# review — AI 代码审查

集成 [OpenCodeReview](https://github.com/qianc62/opencodereview)（`ocr`）进行 AI 代码审查，自动分析 diff 并给出改进建议。

## 前置要求

需要安装 `ocr` 可执行文件。可执行文件查找顺序：

1. `--ocr-path` 参数指定的路径
2. 配置文件中的 `OCR.ExePath`
3. 系统 `PATH` 中的 `ocr`

## 基本用法

```bash
byte-git review                        # 审查当前工作区未提交的改动
byte-git review --from main --to HEAD  # 审查 main..HEAD 的所有改动
byte-git review -c abc1234             # 审查单次 commit
byte-git review -b "实现用户登录"       # 附带业务上下文
byte-git review --model gpt-4o         # 覆盖本次审查模型
```

## 参数

| 参数 | 类型 | 默认值 | 说明 |
|---|---|---|---|
| `[repo]` | 位置参数 | `.` | 仓库路径（可选） |
| `--from` | string | `""` | range 起点 ref |
| `--to` | string | `""` | range 终点 ref |
| `-c, --commit` | string | `""` | 单次 commit hash |
| `--model` | string | `""` | 覆盖本次审查模型 |
| `-b, --background` | string | `""` | 需求/业务上下文（帮助 AI 理解改动意图） |
| `--ocr-path` | string | `""` | ocr 可执行文件路径（覆盖配置） |

## 审查范围

未指定 `--from`/`--to`/`--commit` 时，默认审查工作区未提交的改动。三种范围互斥使用：

- **工作区变更**（默认）：审查 `git diff` 的内容
- **区间**（`--from` + `--to`）：审查两个 ref 之间的所有提交
- **单次提交**（`-c`）：审查单个 commit 引入的改动
