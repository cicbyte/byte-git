# secret — 密钥泄露扫描

集成 [gitleaks](https://github.com/gitleaks/gitleaks) 扫描 Git 全历史中的密钥和凭证泄露（API Key、Token、私钥等）。

## 前置要求

需要安装 `gitleaks` 可执行文件。查找顺序：

1. `--gitleaks-path` 参数指定的路径
2. 配置文件中的 `Gitleaks.ExePath`
3. 系统 `PATH` 中的 `gitleaks`

## 基本用法

```bash
byte-git secret                        # 扫描当前仓库全历史
byte-git secret /path/to/repo          # 扫描指定仓库
byte-git secret --staged               # 仅扫描暂存区（适合 pre-commit 钩子）
byte-git secret -v                     # 显示完整密钥（默认脱敏）
byte-git secret --config rules.toml    # 自定义 gitleaks 规则
byte-git secret --timeout 600          # 超时 600 秒
```

## 参数

| 参数 | 类型 | 默认值 | 说明 |
|---|---|---|---|
| `[repo]` | 位置参数 | `.` | 仓库路径（可选） |
| `--gitleaks-path` | string | `""` | gitleaks 可执行文件路径（覆盖配置） |
| `--config` | string | `""` | 自定义 gitleaks 规则配置文件（`.toml`） |
| `-v, --verbose` | bool | `false` | 显示完整密钥（默认脱敏） |
| `--timeout` | int | `300` | 扫描超时秒数（`0` = 不限） |

## 脱敏规则

默认输出脱敏密钥，格式为「前 4 位 + `***` + 后 4 位」，例如：

```
AKIA•••••••••••WJ2Q
```

使用 `-v` / `--verbose` 可显示完整密钥内容（注意安全风险）。

## 输出内容

发现泄露时，每条记录包含：

- **文件路径 : 行号**
- **匹配规则**（如 `aws-access-token`、`private-key`）
- **作者** / **commit hash**
- **密钥**（脱敏或完整）
- **指纹**（用于去重和追踪）
