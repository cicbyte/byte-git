# archive — 打包压缩仓库

将仓库打包为压缩包，支持 `standard`（仅工作树，不含 `.git`）和 `full`（含完整 `.git` 目录）两种模式。

## 基本用法

```bash
byte-git archive                        # 打包当前仓库 HEAD（tar.gz）
byte-git archive /path/to/repo          # 打包指定仓库
byte-git archive -o release.zip         # 指定输出文件（按扩展名识别格式）
byte-git archive --format zip           # 指定格式
byte-git archive --ref v1.0.0           # 打包指定 ref
byte-git archive --mode full            # 完整打包（含 .git）
byte-git archive --paths src,docs       # 只包含指定子目录
```

## 参数

| 参数 | 类型 | 默认值 | 说明 |
|---|---|---|---|
| `[repo]` | 位置参数 | `.` | 仓库路径（可选） |
| `--ref` | string | `HEAD` | 打包的 Git ref（分支名 / tag / commit） |
| `--format` | string | `tar.gz` | 格式：`tar.gz` / `zip` / `tar` |
| `-o, --output` | string | `""` | 输出路径（不填则自动生成） |
| `--paths` | stringSlice | `[]` | 只包含的子目录（逗号分隔，如 `src,docs`） |
| `--mode` | string | `standard` | 模式：`standard`（git archive）/ `full`（含 `.git`） |

## 模式说明

- **standard**（默认）：调用 `git archive` 打包指定 ref 的文件树，不含 `.git` 目录，适合发布产物。
- **full**：包含完整 `.git` 目录的打包，可用于完整备份或仓库迁移。
