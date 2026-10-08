//! 开源许可证：内嵌 47 份模板，按名称创建（占位符替换年份/作者）。
//!
//! 移植自 git-mate `internal/license`（模板同源）。

use anyhow::{bail, Result};
use chrono::Datelike;
use include_dir::{include_dir, Dir};

static LICENSES: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/assets/licenses");

/// 可用许可证列表（不含 .txt 后缀）。
pub fn list() -> Vec<String> {
    let mut names: Vec<String> = LICENSES
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

/// 创建 LICENSE 文件到 repo 根。
///
/// 模板为 choosealicense.com 格式：YAML frontmatter（--- ... ---）+ 正文；
/// 写入时剥离 frontmatter，仅保留正文（与 git-mate 行为一致）。
pub fn create(repo: &std::path::Path, name: &str, author: &str, year: Option<i32>) -> Result<()> {
    let file_name = format!("{name}.txt");
    let Some(tpl) = LICENSES.get_file(&file_name) else {
        bail!("未知许可证: {name}（license list 查看可用列表）");
    };
    let raw = std::str::from_utf8(tpl.contents())
        .map_err(|e| anyhow::anyhow!("模板编码错误: {e}"))?;

    // 剥离 YAML frontmatter：--- yaml --- body
    let content = strip_frontmatter(raw);

    let year = year.unwrap_or_else(|| chrono::Local::now().year());
    // 作者回退链：参数 > git user.name > "Your Name"（与 git-mate 一致）
    let author = if author.is_empty() {
        crate::git::config_get(Some(repo), "user.name").unwrap_or_else(|| "Your Name".into())
    } else {
        author.to_string()
    };

    // 常见占位符替换（对齐 git-mate create.go）
    let out = content
        .replace("[year]", &year.to_string())
        .replace("[yyyy]", &year.to_string())
        .replace("[fullname]", &author)
        .replace("[name of copyright owner]", &author);

    let dest = repo.join("LICENSE");
    std::fs::write(&dest, out)?;
    println!("已创建 {} -> {}", name, dest.display());
    Ok(())
}

/// 剥离 choosealicense.com 模板的 YAML frontmatter。
fn strip_frontmatter(raw: &str) -> String {
    if !raw.starts_with("---") {
        return raw.trim().to_string();
    }
    let parts: Vec<&str> = raw.splitn(3, "---").collect();
    if parts.len() < 3 {
        return raw.trim().to_string();
    }
    parts[2].trim().to_string()
}

/// 取指定模板的正文（剥 frontmatter）。
pub fn template_text(name: &str) -> Option<String> {
    let lower = name.trim().to_lowercase();
    let file = LICENSES.get_file(format!("{lower}.txt")).or_else(|| LICENSES.get_file(lower))?;
    Some(strip_frontmatter(&String::from_utf8_lossy(&file.contents())))
}

/// 文本归一：小写、非字母数字折叠为空白、分词。
fn normalize_words(text: &str) -> Vec<String> {
    let mut cur = String::new();
    let mut words = Vec::new();
    for ch in text.to_lowercase().chars() {
        if ch.is_alphanumeric() {
            cur.push(ch);
        } else if !cur.is_empty() {
            words.push(std::mem::take(&mut cur));
        }
    }
    if !cur.is_empty() {
        words.push(cur);
    }
    words
}

/// 词级 trigram 集合。
fn trigrams(words: &[String]) -> std::collections::HashSet<(String, String, String)> {
    words
        .windows(3)
        .map(|w| (w[0].clone(), w[1].clone(), w[2].clone()))
        .collect()
}

/// n-gram 检测（licensecheck 简化版）：对全部 47 个模板计算
/// 文档 trigram 被模板覆盖的比例，取最高分。
/// 返回 (SPDX id 小写, 置信度 0.0~1.0)；未检出置信度 0。
pub fn detect_confidence(content: &str) -> (String, f64) {
    let doc = trigrams(&normalize_words(content));
    if doc.is_empty() {
        return (String::new(), 0.0);
    }
    let mut best: (String, f64) = (String::new(), 0.0);
    for f in LICENSES.files() {
        let Some(name) = f.path().file_stem().and_then(|s| s.to_str()) else { continue };
        let tpl = trigrams(&normalize_words(&strip_frontmatter(&String::from_utf8_lossy(&f.contents()))));
        if tpl.is_empty() {
            continue;
        }
        let covered = doc.iter().filter(|g| tpl.contains(*g)).count();
        let score = covered as f64 / doc.len() as f64;
        if score > best.1 {
            best = (name.to_string(), score);
        }
    }
    best
}

/// 检测仓库现有 LICENSE 类型。
/// 优先 n-gram 全模板匹配（真实置信度）；兜底关键词启发式。
pub fn detect(repo: &std::path::Path) -> Result<String> {
    let content = std::fs::read_to_string(repo.join("LICENSE"))
        .or_else(|_| std::fs::read_to_string(repo.join("LICENSE.txt")))
        .or_else(|_| std::fs::read_to_string(repo.join("LICENSE.md")))?;

    let (name, score) = detect_confidence(&content);
    if score >= 0.35 {
        println!("检测到许可证: {name}（置信度 {:.0}%）", score * 100.0);
        return Ok(name);
    }

    // 兜底：关键词（检测被大幅改写的自定义文本）
    let head: String = content.chars().take(600).collect();
    let lower = head.to_lowercase();
    let rules: &[(&str, &str)] = &[
        ("apache license", "apache-2.0"),
        ("gnu affero general public license", "agpl-3.0"),
        ("gnu general public license", "gpl-3.0"),
        ("gnu lesser general public license", "lgpl-2.1"),
        ("mit license", "mit"),
        ("mozilla public license", "mpl-2.0"),
        ("bsd 3-clause", "bsd-3-clause"),
        ("bsd 2-clause", "bsd-2-clause"),
        ("unlicense", "unlicense"),
        ("islamic", "isl"),
    ];
    for (kw, name) in rules {
        if lower.contains(kw) {
            println!("检测到许可证: {name}（关键词匹配）");
            return Ok((*name).into());
        }
    }
    bail!("无法识别 LICENSE 类型（可能为自定义或未知模板）")
}
