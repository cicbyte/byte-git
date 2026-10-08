# license — 开源许可证管理

创建 LICENSE 文件、检测许可证类型、审查依赖库的许可证合规性。

## 基本用法

```bash
byte-git license <子命令>
```

## 子命令

### create — 创建 LICENSE 文件

```bash
byte-git license create mit                     # MIT 许可证
byte-git license create apache-2.0              # Apache 2.0
byte-git license create gpl-3.0                 # GPL 3.0
byte-git license create mit --author "张三"      # 指定版权持有者
byte-git license create mit --year 2024         # 指定年份
```

| 参数 | 类型 | 默认值 | 说明 |
|---|---|---|---|
| `<type>` | 位置参数 | （必填） | 许可证类型（不区分大小写，如 `mit` / `MIT`） |
| `--author` | string | `""` | 版权持有者名称（默认从 `git config user.name` 读取） |
| `--year` | string | `""` | 版权年份（默认当前年份） |
| `--repo` | string | `.` | 仓库路径 |

**常用许可证类型**：

`mit`、`apache-2.0`、`gpl-3.0`、`agpl-3.0`、`bsd-3-clause`、`mpl-2.0`、`unlicense`

完整列表可用 `byte-git license list` 查看。

### detect — 检测许可证类型

```bash
byte-git license detect                 # 检测当前仓库的 LICENSE
byte-git license detect --repo /path    # 检测指定仓库
```

读取 LICENSE 文件，识别 SPDX 类型与置信度，打印权限（允许/条件/限制）。无法识别时提示警告。

| 参数 | 类型 | 默认值 | 说明 |
|---|---|---|---|
| `--repo` | string | `.` | 仓库路径 |

### list — 列出所有许可证模板

```bash
byte-git license list
```

按类别分组打印所有内置许可证模板：

- **permissive**（宽松）：MIT、Apache-2.0、BSD-3-Clause...
- **weak-copyleft**（弱传染）：MPL-2.0、LGPL...
- **strong-copyleft**（强传染）：GPL-3.0、AGPL-3.0...
- **public-domain**（公共领域）：Unlicense...

Featured 项标 ★，末尾汇总模板总数。

### audit — 审查依赖许可证

```bash
byte-git license audit                  # 审查当前仓库的依赖
byte-git license audit --scan-dir vendor # 扫描指定目录
```

扫描指定目录下的依赖库 LICENSE 文件，按风险等级汇总：

| 等级 | 标记 | 含义 |
|---|---|---|
| low | 🟢 | 低风险（宽松许可证） |
| medium | 🟡 | 中风险（需注意条件） |
| high | 🔴 | 高风险（传染性许可证，可能影响自有代码） |
| unknown | ⚪ | 未知（无 LICENSE 文件） |

| 参数 | 类型 | 默认值 | 说明 |
|---|---|---|---|
| `--repo` | string | `.` | 仓库路径 |
| `--scan-dir` | string | `""` | 扫描目录（默认同 `--repo`） |
