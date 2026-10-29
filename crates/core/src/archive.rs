//! 仓库快照归档：`git archive` → tar.gz/zip/tar。
//!
//! 标准模式归档（完整模式见 full_backup，由 backup 模块承担，
//! 此处的 archive 即标准模式 + 导出目录约定）。

use crate::util::{run_capture, AppPaths};
use anyhow::{bail, Context, Result};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct Options {
    pub repo: String,
    pub ref_: String,
    pub paths: Vec<String>,
    pub format: String,
    pub output: String,
}

impl Default for Options {
    fn default() -> Self {
        Self { repo: ".".into(), ref_: "HEAD".into(), paths: vec![], format: "tar.gz".into(), output: String::new() }
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct Result_ {
    pub output_path: String,
    pub size: u64,
    pub file_count: usize,
}

fn ext_for(format: &str) -> &'static str {
    match format {
        "zip" => ".zip",
        "tar" => ".tar",
        _ => ".tar.gz",
    }
}

fn git_format(format: &str) -> &'static str {
    if format == "zip" { "zip" } else { "tar" }
}

/// 默认输出：`downloads/archive/<repo>/<时间戳>-<ref><ext>`（重复归档不覆盖）。
pub fn default_archive_path(repo: &Path, ref_: &str, format: &str) -> Result<PathBuf> {
    // 先解析绝对路径再取末段目录名（repo 可能传 "."）
    let abs = std::fs::canonicalize(repo).unwrap_or_else(|_| repo.to_path_buf());
    let name = abs
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "repo".into());
    let safe_ref = ref_.replace('/', "_");
    let dir = AppPaths::new()?.downloads_dir().join("archive").join(&name);
    AppPaths::ensure_dir(&dir)?;
    let ts = chrono::Local::now().format("%Y%m%dT%H%M%S");
    Ok(dir.join(format!("{ts}-{safe_ref}{}", ext_for(format))))
}

/// 归档主入口。
pub fn archive(opts: &Options) -> Result<Result_> {
    let repo = Path::new(&opts.repo);
    if opts.ref_.is_empty() {
        bail!("ref 不能为空");
    }
    let out = if opts.output.is_empty() {
        default_archive_path(repo, &opts.ref_, &opts.format)?
    } else {
        PathBuf::from(&opts.output)
    };
    if let Some(dir) = out.parent() {
        AppPaths::ensure_dir(dir)?;
    }

    if opts.format == "tar.gz" {
        archive_tar_gz(repo, &opts.ref_, &opts.paths, &out)
    } else {
        archive_direct(repo, &opts.ref_, &opts.paths, &out, &opts.format)
    }
}

/// zip / tar：git archive 直接写文件。
fn archive_direct(repo: &Path, ref_: &str, paths: &[String], out: &Path, format: &str) -> Result<Result_> {
    let output_arg = format!("--output={}", out.display());
    let format_arg = format!("--format={}", git_format(format));
    let mut args: Vec<String> = vec!["archive".into(), format_arg, output_arg, ref_.into()];
    args.extend(paths.iter().cloned());
    run_capture(repo, args.iter().map(String::as_str).collect::<Vec<_>>().as_slice())
        .context("git archive 失败")?;
    let size = std::fs::metadata(out).map(|m| m.len()).unwrap_or(0);
    Ok(Result_ { output_path: out.display().to_string(), size, file_count: 0 })
}

/// tar.gz：git archive 输出 tar 到管道，边读边 gzip（不落裸 tar 中间文件）。
fn archive_tar_gz(repo: &Path, ref_: &str, paths: &[String], out: &Path) -> Result<Result_> {
    let mut args: Vec<String> = vec!["archive".into(), "--format=tar".into(), ref_.into()];
    args.extend(paths.iter().cloned());

    // 启动 git archive，stdout 接管道
    let mut child = std::process::Command::new("git")
        .arg("-C").arg(repo)
        .args(&args)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .context("启动 git archive 失败")?;
    let stdout = child.stdout.take().expect("piped stdout");

    let out_file = std::fs::File::create(out).with_context(|| format!("创建文件失败: {}", out.display()))?;
    let mut writer = flate2::write::GzEncoder::new(std::io::BufWriter::new(out_file), flate2::Compression::default());
    let mut reader = std::io::BufReader::new(stdout);
    let count = std::io::copy(&mut reader, &mut writer).context("压缩写入失败")?;
    writer.finish().context("完成 gzip 失败")?;

    let status = child.wait().context("等待 git archive 失败")?;
    if !status.success() {
        let _ = std::fs::remove_file(out);
        let err = child.stderr
            .take()
            .map(|mut s| { use std::io::Read; let mut buf = String::new(); let _ = s.read_to_string(&mut buf); buf })
            .unwrap_or_default();
        bail!("git archive 失败: {err}");
    }

    let size = std::fs::metadata(out).map(|m| m.len()).unwrap_or(0);
    // tar 每 512 字节一个条目头，粗略计数仅作展示（Go 版输出文件数需要解 tar，此处对齐直接模式）
    let file_count = (count / 512).max(1) as usize / 3;
    Ok(Result_ { output_path: out.display().to_string(), size, file_count })
}

/// 列出已有归档（downloads/archive/<repo>/ 下按时间序）。
pub fn list(repo: &Path) -> Result<Vec<String>> {
    let abs = std::fs::canonicalize(repo).unwrap_or_else(|_| repo.to_path_buf());
    let name = abs.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "repo".into());
    let dir = AppPaths::new()?.downloads_dir().join("archive").join(&name);
    if !dir.exists() {
        return Ok(vec![]);
    }
    let mut items: Vec<String> = std::fs::read_dir(&dir)
        .context("读取归档目录失败")?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_file())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    items.sort();
    Ok(items)
}
