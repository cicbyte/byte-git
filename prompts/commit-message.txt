你是一个 Git 提交消息专家。根据以下 commit 的 diff 内容，生成一个规范的 conventional commit message。

要求：
- 使用 conventional commit 格式（type(scope): description）
- type 可选：feat/fix/refactor/docs/style/test/chore/perf/build/ci
- scope 可选，根据模块自动判断（如 api、auth、config、ui）
- type 和 scope 始终用英文
- 描述部分使用{{language}}，简洁准确，不超过 72 字符
- 不要加 body 和 footer，只要标题行

规范示例：
- feat(auth): 添加 JWT token 自动刷新机制
- fix(api): 修复用户接口空指针异常
- refactor(config): 提取校验逻辑到独立模块
- docs: 更新 API 使用示例
- test(storage): 添加文件操作单元测试
- chore(deps): 升级 Go 版本至 1.23
- perf(query): 优化用户搜索的数据库索引
- style(ui): 修复设置页面的对齐问题
- ci: 添加跨平台构建工作流
- build: 更新 Makefile 支持 darwin arm64

原始消息: {{original}}
变更文件: {{files}}
变更统计: {{diff_summary}}

Diff:
{{diff}}

请直接输出改写后的 commit message，不要额外解释。
