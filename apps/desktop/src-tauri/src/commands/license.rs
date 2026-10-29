//! license 域命令：开源许可证模板 / 创建 / 检测 / 读写 / 依赖审计。
//!
//! 模板元数据（标题/分类/权限/条件/
//! 限制/推荐位）静态内嵌，与 core assets/licenses 的 choosealicense.com
//! 模板 frontmatter 同源；正文生成复用 byte-git-core::license。

use byte_git_core::license as core_license;
use serde::Serialize;
use std::path::{Path, PathBuf};

fn repo_path(repo: Option<String>) -> PathBuf {
    crate::state::resolve(repo)
}

/// 前端文件读取上限（对齐 Go MaxSecretFileSize = 1MB）。
const MAX_SECRET_FILE_SIZE: usize = 1 << 20;

// ---------------------------------------------------------------------------
// DTO
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LicenseTemplateDto {
    pub spdx_id: String,
    pub title: String,
    pub category: String,
    pub permissions: Vec<String>,
    pub conditions: Vec<String>,
    pub limitations: Vec<String>,
    pub featured: bool,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LicenseDetectResult {
    pub spdx_id: String,
    pub confidence: f64,
    pub is_valid: bool,
    pub permissions: Vec<String>,
    pub conditions: Vec<String>,
    pub limitations: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LicenseDepEntry {
    pub path: String,
    pub spdx_id: String,
    pub confidence: f64,
    pub risk_level: String,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileResult {
    pub content: String,
    pub lines: i64,
    pub binary: bool,
    pub not_found: bool,
    /// 图片内联 data URL（wire 对齐 core.FileResult；本读者恒为空）
    #[serde(default)]
    pub data_url: String,
}

// ---------------------------------------------------------------------------
// 模板元数据表（同源 core assets/licenses frontmatter + Go categoryMap）
// ---------------------------------------------------------------------------

struct TemplateMeta {
    spdx: &'static str,
    title: &'static str,
    category: &'static str,
    featured: bool,
    permissions: &'static [&'static str],
    conditions: &'static [&'static str],
    limitations: &'static [&'static str],
}

/// 按 Go LoadTemplates 的目录（文件名）顺序排列。
#[rustfmt::skip]
static TEMPLATES: &[TemplateMeta] = &[
    TemplateMeta { spdx: "0BSD", title: "BSD Zero Clause License", category: "permissive", featured: false,
        permissions: &["commercial-use", "distribution", "modifications", "private-use"], conditions: &[], limitations: &["liability", "warranty"] },
    TemplateMeta { spdx: "AFL-3.0", title: "Academic Free License v3.0", category: "permissive", featured: false,
        permissions: &["commercial-use", "modifications", "distribution", "private-use", "patent-use"], conditions: &["include-copyright", "document-changes"], limitations: &["trademark-use", "liability", "warranty"] },
    TemplateMeta { spdx: "AGPL-3.0", title: "GNU Affero General Public License v3.0", category: "strong-copyleft", featured: true,
        permissions: &["commercial-use", "modifications", "distribution", "patent-use", "private-use"], conditions: &["include-copyright", "document-changes", "disclose-source", "network-use-disclose", "same-license"], limitations: &["liability", "warranty"] },
    TemplateMeta { spdx: "Apache-2.0", title: "Apache License 2.0", category: "permissive", featured: true,
        permissions: &["commercial-use", "modifications", "distribution", "patent-use", "private-use"], conditions: &["include-copyright", "document-changes"], limitations: &["trademark-use", "liability", "warranty"] },
    TemplateMeta { spdx: "Artistic-2.0", title: "Artistic License 2.0", category: "permissive", featured: false,
        permissions: &["commercial-use", "modifications", "distribution", "patent-use", "private-use"], conditions: &["include-copyright", "document-changes"], limitations: &["liability", "trademark-use", "warranty"] },
    TemplateMeta { spdx: "BlueOak-1.0.0", title: "Blue Oak Model License 1.0.0", category: "permissive", featured: false,
        permissions: &["commercial-use", "modifications", "distribution", "patent-use", "private-use"], conditions: &["include-copyright"], limitations: &["liability", "warranty"] },
    TemplateMeta { spdx: "BSD-2-Clause-Patent", title: "BSD-2-Clause Plus Patent License", category: "permissive", featured: false,
        permissions: &["commercial-use", "modifications", "distribution", "patent-use", "private-use"], conditions: &["include-copyright"], limitations: &["liability", "warranty"] },
    TemplateMeta { spdx: "BSD-2-Clause", title: "BSD 2-Clause \"Simplified\" License", category: "permissive", featured: false,
        permissions: &["commercial-use", "modifications", "distribution", "private-use"], conditions: &["include-copyright"], limitations: &["liability", "warranty"] },
    TemplateMeta { spdx: "BSD-3-Clause-Clear", title: "BSD 3-Clause Clear License", category: "permissive", featured: false,
        permissions: &["commercial-use", "modifications", "distribution", "private-use"], conditions: &["include-copyright"], limitations: &["liability", "patent-use", "warranty"] },
    TemplateMeta { spdx: "BSD-3-Clause", title: "BSD 3-Clause \"New\" or \"Revised\" License", category: "permissive", featured: true,
        permissions: &["commercial-use", "modifications", "distribution", "private-use"], conditions: &["include-copyright"], limitations: &["liability", "warranty"] },
    TemplateMeta { spdx: "BSD-4-Clause", title: "BSD 4-Clause \"Original\" or \"Old\" License", category: "permissive", featured: false,
        permissions: &["commercial-use", "modifications", "distribution", "private-use"], conditions: &["include-copyright"], limitations: &["liability", "warranty"] },
    TemplateMeta { spdx: "BSL-1.0", title: "Boost Software License 1.0", category: "permissive", featured: false,
        permissions: &["commercial-use", "modifications", "distribution", "private-use"], conditions: &["include-copyright--source"], limitations: &["liability", "warranty"] },
    TemplateMeta { spdx: "CC-BY-4.0", title: "Creative Commons Attribution 4.0 International", category: "permissive", featured: false,
        permissions: &["commercial-use", "modifications", "distribution", "private-use"], conditions: &["include-copyright", "document-changes"], limitations: &["liability", "trademark-use", "patent-use", "warranty"] },
    TemplateMeta { spdx: "CC-BY-SA-4.0", title: "Creative Commons Attribution Share Alike 4.0 International", category: "permissive", featured: false,
        permissions: &["commercial-use", "modifications", "distribution", "private-use"], conditions: &["include-copyright", "document-changes", "same-license"], limitations: &["liability", "trademark-use", "patent-use", "warranty"] },
    TemplateMeta { spdx: "CC0-1.0", title: "Creative Commons Zero v1.0 Universal", category: "public-domain", featured: false,
        permissions: &["commercial-use", "modifications", "distribution", "private-use"], conditions: &[], limitations: &["liability", "trademark-use", "patent-use", "warranty"] },
    TemplateMeta { spdx: "CECILL-2.1", title: "CeCILL Free Software License Agreement v2.1", category: "permissive", featured: false,
        permissions: &["commercial-use", "modifications", "distribution", "private-use", "patent-use"], conditions: &["include-copyright", "network-use-disclose", "disclose-source", "same-license"], limitations: &["liability", "warranty"] },
    TemplateMeta { spdx: "CERN-OHL-P-2.0", title: "CERN Open Hardware Licence Version 2 - Permissive", category: "permissive", featured: false,
        permissions: &["commercial-use", "modifications", "distribution", "private-use", "patent-use"], conditions: &["include-copyright", "document-changes"], limitations: &["liability", "warranty"] },
    TemplateMeta { spdx: "CERN-OHL-S-2.0", title: "CERN Open Hardware Licence Version 2 - Strongly Reciprocal", category: "permissive", featured: false,
        permissions: &["commercial-use", "modifications", "distribution", "private-use", "patent-use"], conditions: &["include-copyright", "document-changes", "disclose-source", "same-license"], limitations: &["liability", "warranty"] },
    TemplateMeta { spdx: "CERN-OHL-W-2.0", title: "CERN Open Hardware Licence Version 2 - Weakly Reciprocal", category: "permissive", featured: false,
        permissions: &["commercial-use", "modifications", "distribution", "private-use", "patent-use"], conditions: &["include-copyright", "document-changes", "disclose-source", "same-license--library"], limitations: &["liability", "warranty"] },
    TemplateMeta { spdx: "ECL-2.0", title: "Educational Community License v2.0", category: "permissive", featured: false,
        permissions: &["commercial-use", "modifications", "distribution", "patent-use", "private-use"], conditions: &["include-copyright", "document-changes"], limitations: &["trademark-use", "liability", "warranty"] },
    TemplateMeta { spdx: "EPL-1.0", title: "Eclipse Public License 1.0", category: "permissive", featured: false,
        permissions: &["commercial-use", "distribution", "modifications", "patent-use", "private-use"], conditions: &["disclose-source", "include-copyright", "same-license"], limitations: &["liability", "warranty"] },
    TemplateMeta { spdx: "EPL-2.0", title: "Eclipse Public License 2.0", category: "weak-copyleft", featured: false,
        permissions: &["commercial-use", "distribution", "modifications", "patent-use", "private-use"], conditions: &["disclose-source", "include-copyright", "same-license"], limitations: &["liability", "warranty"] },
    TemplateMeta { spdx: "EUPL-1.1", title: "European Union Public License 1.1", category: "permissive", featured: false,
        permissions: &["commercial-use", "modifications", "distribution", "patent-use", "private-use"], conditions: &["include-copyright", "disclose-source", "document-changes", "network-use-disclose", "same-license"], limitations: &["liability", "trademark-use", "warranty"] },
    TemplateMeta { spdx: "EUPL-1.2", title: "European Union Public License 1.2", category: "permissive", featured: false,
        permissions: &["commercial-use", "modifications", "distribution", "patent-use", "private-use"], conditions: &["include-copyright", "disclose-source", "document-changes", "network-use-disclose", "same-license"], limitations: &["liability", "trademark-use", "warranty"] },
    TemplateMeta { spdx: "GFDL-1.3", title: "GNU Free Documentation License v1.3", category: "permissive", featured: false,
        permissions: &["commercial-use", "modifications", "distribution", "private-use"], conditions: &["include-copyright", "disclose-source", "same-license", "document-changes"], limitations: &["liability", "warranty"] },
    TemplateMeta { spdx: "GPL-2.0", title: "GNU General Public License v2.0", category: "strong-copyleft", featured: true,
        permissions: &["commercial-use", "modifications", "distribution", "private-use"], conditions: &["include-copyright", "document-changes", "disclose-source", "same-license"], limitations: &["liability", "warranty"] },
    TemplateMeta { spdx: "GPL-3.0", title: "GNU General Public License v3.0", category: "strong-copyleft", featured: true,
        permissions: &["commercial-use", "modifications", "distribution", "patent-use", "private-use"], conditions: &["include-copyright", "document-changes", "disclose-source", "same-license"], limitations: &["liability", "warranty"] },
    TemplateMeta { spdx: "ISC", title: "ISC License", category: "permissive", featured: false,
        permissions: &["commercial-use", "distribution", "modifications", "private-use"], conditions: &["include-copyright"], limitations: &["liability", "warranty"] },
    TemplateMeta { spdx: "LGPL-2.1", title: "GNU Lesser General Public License v2.1", category: "weak-copyleft", featured: false,
        permissions: &["commercial-use", "modifications", "distribution", "private-use"], conditions: &["include-copyright", "disclose-source", "document-changes", "same-license--library"], limitations: &["liability", "warranty"] },
    TemplateMeta { spdx: "LGPL-3.0", title: "GNU Lesser General Public License v3.0", category: "weak-copyleft", featured: false,
        permissions: &["commercial-use", "modifications", "distribution", "patent-use", "private-use"], conditions: &["include-copyright", "disclose-source", "document-changes", "same-license--library"], limitations: &["liability", "warranty"] },
    TemplateMeta { spdx: "LPPL-1.3c", title: "LaTeX Project Public License v1.3c", category: "permissive", featured: false,
        permissions: &["commercial-use", "modifications", "distribution", "private-use"], conditions: &["include-copyright", "document-changes", "disclose-source"], limitations: &["liability", "warranty"] },
    TemplateMeta { spdx: "MIT-0", title: "MIT No Attribution", category: "permissive", featured: false,
        permissions: &["commercial-use", "modifications", "distribution", "private-use"], conditions: &[], limitations: &["liability", "warranty"] },
    TemplateMeta { spdx: "MIT", title: "MIT License", category: "permissive", featured: true,
        permissions: &["commercial-use", "modifications", "distribution", "private-use"], conditions: &["include-copyright"], limitations: &["liability", "warranty"] },
    TemplateMeta { spdx: "MPL-2.0", title: "Mozilla Public License 2.0", category: "weak-copyleft", featured: true,
        permissions: &["commercial-use", "modifications", "distribution", "patent-use", "private-use"], conditions: &["disclose-source", "include-copyright", "same-license--file"], limitations: &["liability", "trademark-use", "warranty"] },
    TemplateMeta { spdx: "MS-PL", title: "Microsoft Public License", category: "permissive", featured: false,
        permissions: &["commercial-use", "modifications", "distribution", "patent-use", "private-use"], conditions: &["include-copyright"], limitations: &["warranty", "trademark-use"] },
    TemplateMeta { spdx: "MS-RL", title: "Microsoft Reciprocal License", category: "permissive", featured: false,
        permissions: &["commercial-use", "modifications", "distribution", "patent-use", "private-use"], conditions: &["disclose-source", "include-copyright", "same-license--file"], limitations: &["warranty", "trademark-use"] },
    TemplateMeta { spdx: "MulanPSL-2.0", title: "Mulan Permissive Software License, Version 2", category: "permissive", featured: false,
        permissions: &["commercial-use", "modifications", "distribution", "patent-use", "private-use"], conditions: &["include-copyright"], limitations: &["liability", "trademark-use", "warranty"] },
    TemplateMeta { spdx: "NCSA", title: "University of Illinois/NCSA Open Source License", category: "permissive", featured: false,
        permissions: &["commercial-use", "modifications", "distribution", "private-use"], conditions: &["include-copyright"], limitations: &["liability", "warranty"] },
    TemplateMeta { spdx: "ODbL-1.0", title: "Open Data Commons Open Database License v1.0", category: "permissive", featured: false,
        permissions: &["commercial-use", "distribution", "modifications", "private-use"], conditions: &["disclose-source", "include-copyright", "same-license"], limitations: &["liability", "patent-use", "trademark-use", "warranty"] },
    TemplateMeta { spdx: "OFL-1.1", title: "SIL Open Font License 1.1", category: "permissive", featured: false,
        permissions: &["private-use", "commercial-use", "modifications", "distribution"], conditions: &["include-copyright", "same-license"], limitations: &["liability", "warranty"] },
    TemplateMeta { spdx: "OSL-3.0", title: "Open Software License 3.0", category: "permissive", featured: false,
        permissions: &["commercial-use", "distribution", "modifications", "patent-use", "private-use"], conditions: &["include-copyright", "disclose-source", "document-changes", "network-use-disclose", "same-license"], limitations: &["trademark-use", "liability", "warranty"] },
    TemplateMeta { spdx: "PostgreSQL", title: "PostgreSQL License", category: "permissive", featured: false,
        permissions: &["commercial-use", "modifications", "distribution", "private-use"], conditions: &["include-copyright"], limitations: &["liability", "warranty"] },
    TemplateMeta { spdx: "Unlicense", title: "The Unlicense", category: "public-domain", featured: true,
        permissions: &["private-use", "commercial-use", "modifications", "distribution"], conditions: &[], limitations: &["liability", "warranty"] },
    TemplateMeta { spdx: "UPL-1.0", title: "Universal Permissive License v1.0", category: "permissive", featured: false,
        permissions: &["commercial-use", "modifications", "distribution", "patent-use", "private-use"], conditions: &["include-copyright"], limitations: &["liability", "warranty"] },
    TemplateMeta { spdx: "Vim", title: "Vim License", category: "permissive", featured: false,
        permissions: &["commercial-use", "modifications", "distribution", "private-use"], conditions: &["include-copyright", "document-changes", "disclose-source", "same-license"], limitations: &[] },
    TemplateMeta { spdx: "WTFPL", title: "Do What The F*ck You Want To Public License", category: "permissive", featured: false,
        permissions: &["commercial-use", "modifications", "distribution", "private-use"], conditions: &[], limitations: &[] },
    TemplateMeta { spdx: "Zlib", title: "zlib License", category: "permissive", featured: false,
        permissions: &["commercial-use", "modifications", "distribution", "private-use"], conditions: &["include-copyright--source", "document-changes"], limitations: &["liability", "warranty"] },
];

fn template_meta(spdx: &str) -> Option<&'static TemplateMeta> {
    TEMPLATES.iter().find(|t| t.spdx.eq_ignore_ascii_case(spdx))
}

// ---------------------------------------------------------------------------
// 检测（关键词启发式，与 core license::detect 同一规则集）
// ---------------------------------------------------------------------------

/// 检测：返回 (SPDXID, 置信度, 是否有效)。
/// 词级 trigram 全模板覆盖匹配（core::license::detect_confidence，
/// licensecheck n-gram 的简化版）给出真实置信度；关键词表兜底（置信度 0.5）。
fn detect_content(content: &str) -> (String, f64, bool) {
    let (name, score) = core_license::detect_confidence(content);
    if score >= 0.35 {
        let spdx = template_meta(&name)
            .map(|t| t.spdx.to_string())
            .unwrap_or_else(|| name.to_uppercase());
        return (spdx, score, true);
    }
    // 关键词兜底：文本被大幅改写但保留特征词
    let head: String = content.chars().take(600).collect();
    let lower = head.to_lowercase();
    let rules: &[(&str, &str)] = &[
        ("apache license", "Apache-2.0"),
        ("gnu affero general public license", "AGPL-3.0"),
        ("gnu general public license", "GPL-3.0"),
        ("gnu lesser general public license", "LGPL-2.1"),
        ("mit license", "MIT"),
        ("mozilla public license", "MPL-2.0"),
        ("bsd 3-clause", "BSD-3-Clause"),
        ("bsd 2-clause", "BSD-2-Clause"),
        ("unlicense", "Unlicense"),
        ("islamic", "ISL"),
    ];
    for (kw, name) in rules {
        if lower.contains(kw) {
            return ((*name).to_string(), 0.5, true);
        }
    }
    (String::new(), 0.0, false)
}

fn assess_risk(spdx: &str) -> &'static str {
    match template_meta(spdx) {
        None => "unknown",
        Some(t) => match t.category {
            "strong-copyleft" => "high",    // GPL/AGPL —— 传染性 copyleft
            "weak-copyleft" => "medium",    // LGPL/MPL —— 修改部分须开源
            "permissive" | "public-domain" => "low",
            _ => "unknown",
        },
    }
}

// ---------------------------------------------------------------------------
// 命令
// ---------------------------------------------------------------------------

/// LicenseListTemplates：返回全部内嵌许可证模板元数据。
#[tauri::command]
pub fn license_list_templates() -> Result<Vec<LicenseTemplateDto>, String> {
    Ok(TEMPLATES
        .iter()
        .map(|t| LicenseTemplateDto {
            spdx_id: t.spdx.to_string(),
            title: t.title.to_string(),
            category: t.category.to_string(),
            permissions: t.permissions.iter().map(|s| s.to_string()).collect(),
            conditions: t.conditions.iter().map(|s| s.to_string()).collect(),
            limitations: t.limitations.iter().map(|s| s.to_string()).collect(),
            featured: t.featured,
        })
        .collect())
}

/// LicenseCreate：按模板 + 作者 + 年份生成 LICENSE 内容（返回文本，不落盘；
/// Go 版 Create 本身不写文件，写盘由 LicenseWrite 完成）。
#[tauri::command]
pub fn license_create(repo: Option<String>, spdx_id: String, author: String, year: String) -> Result<String, String> {
    let repo = repo_path(repo);
    // core 模板按文件名（小写 SPDX id）索引，Go 为大小写不敏感匹配
    let name = spdx_id.trim().to_lowercase();
    if template_meta(&spdx_id).is_none() {
        return Err(format!("未找到许可证模板: {spdx_id}"));
    }
    let year_num: Option<i32> = year.trim().parse().ok();

    // core::license::create 会直接写 LICENSE；为对齐 Go「只生成不落盘」语义，
    // 生成后读回内容并恢复原状态
    let license_file = repo.join("LICENSE");
    let old: Option<Vec<u8>> = std::fs::read(&license_file).ok();
    core_license::create(&repo, &name, author.trim(), year_num).map_err(|e| format!("{e:#}"))?;
    let content = std::fs::read_to_string(&license_file)
        .map_err(|e| format!("读取生成结果失败: {e}"))?;
    match old {
        Some(b) => {
            let _ = std::fs::write(&license_file, b);
        }
        None => {
            let _ = std::fs::remove_file(&license_file);
        }
    }
    Ok(content)
}

/// LicenseWrite：写入仓库根 LICENSE 文件。
#[tauri::command]
pub fn license_write(repo: Option<String>, content: String) -> Result<(), String> {
    let repo = repo_path(repo);
    std::fs::write(repo.join("LICENSE"), content).map_err(|e| format!("{e}"))
}

/// LicenseDetect：读取并检测仓库现有 LICENSE；无 LICENSE 时返回空结果（非错误）。
#[tauri::command]
pub fn license_detect(repo: Option<String>) -> Result<LicenseDetectResult, String> {
    let repo = repo_path(repo);
    let data = match std::fs::read(repo.join("LICENSE")) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            // 没有 LICENSE 是正常结果（无可检测），其余读取错误才上抛
            return Ok(LicenseDetectResult::default());
        }
        Err(e) => return Err(format!("读取 LICENSE 失败: {e}")),
        Ok(d) => d,
    };
    let content = String::from_utf8_lossy(&data);
    let (spdx_id, confidence, is_valid) = detect_content(&content);
    let mut res = LicenseDetectResult {
        spdx_id: spdx_id.clone(),
        confidence,
        is_valid,
        ..Default::default()
    };
    if let Some(t) = template_meta(&spdx_id) {
        res.permissions = t.permissions.iter().map(|s| s.to_string()).collect();
        res.conditions = t.conditions.iter().map(|s| s.to_string()).collect();
        res.limitations = t.limitations.iter().map(|s| s.to_string()).collect();
    }
    Ok(res)
}

/// LicenseAuditDeps：递归扫描仓库（含 node_modules/vendor 等）中的依赖许可证文件。
#[tauri::command]
pub fn license_audit_deps(repo: Option<String>) -> Result<Vec<LicenseDepEntry>, String> {
    let repo = repo_path(repo);
    let mut results = Vec::new();
    walk_license_files(&repo, &repo, &mut results);
    Ok(results)
}

/// 依赖许可证文件名（与 Go licenseFileNames 完全一致——注意其中小写扩展名
/// 条目在 Go 的大小写不敏感比较下实际永不命中，此处保持同等行为）。
const LICENSE_FILE_NAMES: &[&str] = &[
    "LICENSE", "LICENSE.md", "LICENSE.txt", "LICENSE.MIT", "LICENSE.APACHE",
    "COPYING", "COPYING.txt", "COPYING.md",
    "NOTICE", "NOTICE.txt",
    "UNLICENSE", "UNLICENSE.txt",
];

fn walk_license_files(dir: &Path, root: &Path, out: &mut Vec<LicenseDepEntry>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    let mut entries: Vec<_> = rd.flatten().collect();
    entries.sort_by_key(|e| e.file_name());
    for entry in entries {
        let p = entry.path();
        let Ok(ft) = entry.file_type() else { continue };
        if ft.is_dir() {
            // 跳过 .git
            if entry.file_name() == ".git" {
                continue;
            }
            walk_license_files(&p, root, out);
            continue;
        }
        let upper = entry.file_name().to_string_lossy().to_uppercase();
        if !LICENSE_FILE_NAMES.iter().any(|lf| upper == *lf) {
            continue;
        }
        let Ok(data) = std::fs::read(&p) else { continue };
        let rel = p.strip_prefix(root).unwrap_or(&p).to_string_lossy().replace('\\', "/");
        let (spdx, confidence, _valid) = detect_content(&String::from_utf8_lossy(&data));
        out.push(LicenseDepEntry {
            path: rel,
            risk_level: assess_risk(&spdx).to_string(),
            spdx_id: spdx,
            confidence,
        });
    }
}

/// LicenseRead：读取仓库 LICENSE 内容（超过 1MB 截断）。
#[tauri::command]
pub fn license_read(repo: Option<String>) -> Result<FileResult, String> {
    let repo = repo_path(repo);
    let data = match std::fs::read(repo.join("LICENSE")) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Ok(FileResult { not_found: true, ..Default::default() });
        }
        Err(e) => return Err(format!("{e}")),
        Ok(d) => d,
    };
    let capped: &[u8] = if data.len() > MAX_SECRET_FILE_SIZE { &data[..MAX_SECRET_FILE_SIZE] } else { &data };
    let content = String::from_utf8_lossy(capped).into_owned();
    let lines = content.matches('\n').count() as i64 + 1;
    Ok(FileResult { content, lines, ..Default::default() })
}
