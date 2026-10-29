//! .gitignore 管理：内嵌常用模板 + 追加写入。
//!

use anyhow::{bail, Result};
use include_dir::{include_dir, Dir};
use std::path::Path;

static TEMPLATES: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/assets/gitignore");

/// 可用模板列表（不含 .gitignore 后缀）。
pub fn list() -> Vec<String> {
    let mut names: Vec<String> = TEMPLATES
        .files()
        .map(|f| {
            f.path()
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default()
        })
        .filter(|n| !n.is_empty())
        .collect();
    names.sort();
    names
}

/// 将模板内容追加到仓库 .gitignore（带分段注释，幂等：同名模板不重复追加）。
pub fn add(repo: &Path, template: &str) -> Result<()> {
    let file_name = format!("{template}.gitignore");
    let Some(tpl) = TEMPLATES.get_file(&file_name) else {
        bail!("未知模板: {template}（ignore list 查看可用列表）");
    };
    let content = std::str::from_utf8(tpl.contents())
        .map_err(|e| anyhow::anyhow!("模板编码错误: {e}"))?;

    let marker = format!("# --- byte-git: {template} ---");
    let dest = repo.join(".gitignore");
    let existing = std::fs::read_to_string(&dest).unwrap_or_default();
    if existing.contains(&marker) {
        println!("模板 {template} 已存在于 .gitignore，跳过");
        return Ok(());
    }

    let block = format!("\n{marker}\n{content}\n");
    std::fs::write(&dest, existing + &block)?;
    println!("已追加 {template} 模板到 .gitignore");
    Ok(())
}

/// 通用忽略规则追加（任意若干条目）。
pub fn add_rules(repo: &Path, rules: &[String]) -> Result<()> {
    if rules.is_empty() {
        bail!("请至少提供一条规则");
    }
    let dest = repo.join(".gitignore");
    let existing = std::fs::read_to_string(&dest).unwrap_or_default();
    let block = format!("\n# --- byte-git: custom ---\n{}\n", rules.join("\n"));
    std::fs::write(&dest, existing + &block)?;
    println!("已追加 {} 条规则", rules.len());
    Ok(())
}

// =====================================================================
// 忽略文件视图辅助（check-ignore 输出解析、误忽略风险判定）
// =====================================================================

/// `git check-ignore -v` 单条命中结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckIgnoreMatch {
    /// 规则来源文件（如 ".gitignore"、"docs/.gitignore"，全局忽略文件为绝对路径）。
    pub source: String,
    /// 规则所在行号（1-based）。
    pub line: u32,
    /// 命中的模式（保留写法原样，如 "lib/"）。
    pub pattern: String,
}

/// 解析 `git check-ignore -v` 输出行：`<source>:<line>:<pattern>\t<path>`。
///
/// source 可能含盘符冒号（Windows 全局忽略文件），pattern 也可能含冒号，
/// 故 info 段从右侧按冒号切三段。
pub fn parse_check_ignore_line(raw: &str) -> Option<(String, CheckIgnoreMatch)> {
    let (info, path) = raw.split_once('\t')?;
    let mut parts = info.rsplitn(3, ':');
    let pattern = parts.next()?.to_string();
    let line = parts.next()?.trim().parse::<u32>().ok()?;
    let source = parts.next()?.to_string();
    if source.is_empty() || path.is_empty() {
        return None;
    }
    Some((
        path.to_string(),
        CheckIgnoreMatch { source, line, pattern },
    ))
}

/// 疑似源码/配置类扩展名——这类文件被忽略大概率是误伤（如 lib/ 规则吞掉前端源码）。
const SOURCE_EXTS: &[&str] = &[
    "rs", "go", "py", "js", "jsx", "mjs", "cjs", "ts", "tsx", "mts", "cts", "vue", "svelte",
    "astro", "java", "kt", "kts", "c", "cc", "cpp", "cxx", "h", "hpp", "hh", "cs", "php", "rb",
    "swift", "m", "mm", "scala", "sh", "bash", "ps1", "bat", "cmd", "sql", "proto", "gradle",
    "dart", "lua", "r", "pl", "ex", "exs", "erl", "hs", "clj", "cljs", "zig", "nim", "html",
    "htm", "css", "scss", "sass", "less", "json", "yaml", "yml", "toml", "xml", "md", "mdx",
];

/// 路径是否为源码/配置类文件（按扩展名判断，忽略大小写）。
pub fn is_source_like(path: &str) -> bool {
    let name = path
        .rsplit(|c| c == '/' || c == '\\')
        .next()
        .unwrap_or(path);
    let Some((stem, ext)) = name.rsplit_once('.') else {
        return false;
    };
    if stem.is_empty() {
        return false;
    }
    SOURCE_EXTS.contains(&ext.to_lowercase().as_str())
}

/// 常见「本就该忽略」的目录名（依赖/构建产物/IDE 缓存），不做误忽略提示。
const EXPECTED_IGNORE_DIRS: &[&str] = &[
    "node_modules", "bower_components", "vendor", "target", "dist", "build", "out", "bin", "obj",
    "__pycache__", ".venv", "venv", "site-packages", "egg-info", ".tox", ".eggs", ".gradle",
    ".m2", ".next", ".nuxt", ".output", ".parcel-cache", ".pytest_cache", ".mypy_cache",
    ".ruff_cache", "coverage", ".nyc_output", ".idea", ".vscode", ".vs", ".cache", "tmp", "temp",
    "logs", "pods", "deriveddata", ".dart_tool", ".terraform",
];

/// 目录名（末段）是否属于常见可放心忽略的目录（大小写不敏感）。
pub fn is_expected_ignore_dir(path: &str) -> bool {
    let name = path
        .rsplit(|c| c == '/' || c == '\\')
        .next()
        .unwrap_or(path)
        .to_lowercase();
    EXPECTED_IGNORE_DIRS.contains(&name.as_str())
}

/// 目录内是否含有源码类文件（限预算防大目录爆走：最多看 budget 个条目）。
pub fn dir_contains_source(root: &Path, budget: usize) -> bool {
    let mut stack = vec![root.to_path_buf()];
    let mut seen = 0usize;
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for e in entries.flatten() {
            if seen >= budget {
                return false;
            }
            seen += 1;
            let p = e.path();
            if p.is_dir() {
                if e.file_name().to_string_lossy() == ".git" {
                    continue;
                }
                stack.push(p);
            } else if is_source_like(&p.to_string_lossy()) {
                return true;
            }
        }
    }
    false
}

#[cfg(test)]
mod ignored_tests {
    use super::*;

    #[test]
    fn parse_check_ignore_line_basic() {
        let (path, m) = parse_check_ignore_line(".gitignore:12:*.key\tsecrets/a.key").unwrap();
        assert_eq!(path, "secrets/a.key");
        assert_eq!(m.source, ".gitignore");
        assert_eq!(m.line, 12);
        assert_eq!(m.pattern, "*.key");
    }

    #[test]
    fn parse_check_ignore_line_windows_source() {
        let (path, m) =
            parse_check_ignore_line("C:/Users/me/.config/git/ignore:3:lib/\tfront/lib").unwrap();
        assert_eq!(path, "front/lib");
        assert_eq!(m.source, "C:/Users/me/.config/git/ignore");
        assert_eq!(m.line, 3);
        assert_eq!(m.pattern, "lib/");
    }

    #[test]
    fn parse_check_ignore_line_nested_and_garbage() {
        let (_, m) = parse_check_ignore_line("docs/.gitignore:1:node/\tdocs/node").unwrap();
        assert_eq!(m.source, "docs/.gitignore");
        assert!(parse_check_ignore_line("no-tab-here").is_none());
        assert!(parse_check_ignore_line("a:b:not-a-number\tx").is_none());
    }

    #[test]
    fn source_like_classification() {
        assert!(is_source_like("frontend/lib/index.ts"));
        assert!(is_source_like("a.VUE"));
        assert!(is_source_like("README.md"));
        assert!(!is_source_like("lib/a.dll"));
        assert!(!is_source_like("lib"));
        assert!(!is_source_like("archive.tar.gz"));
    }

    #[test]
    fn expected_ignore_dir_classification() {
        assert!(is_expected_ignore_dir("node_modules"));
        assert!(is_expected_ignore_dir("packages/foo/dist"));
        assert!(is_expected_ignore_dir("NODE_MODULES"));
        assert!(!is_expected_ignore_dir("frontend/lib"));
        assert!(!is_expected_ignore_dir("lib"));
    }

    #[test]
    fn dir_contains_source_walk() {
        let root = std::env::temp_dir().join(format!("byte-git-ig-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("lib")).unwrap();
        std::fs::write(root.join("lib/a.dll"), b"x").unwrap();
        assert!(!dir_contains_source(&root, 100));
        std::fs::write(root.join("lib/index.ts"), b"x").unwrap();
        assert!(dir_contains_source(&root, 100));
        assert!(!dir_contains_source(&root, 0));
        let _ = std::fs::remove_dir_all(&root);
    }
}
