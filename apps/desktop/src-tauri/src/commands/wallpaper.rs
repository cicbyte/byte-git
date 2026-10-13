//! 壁纸文件生命周期管理（移植上游 gui/internal/wallpaper）。
//!
//! - 数据目录 `~/.cicbyte/apps/byte-git/wallpapers/`
//! - 上传 = 时间戳前缀复制（防覆盖）；列表 newest-first；删除 + 缩略图清理
//! - 前端经 base64 data URL 渲染（绕开 asset-protocol / CSP 差异）
//! - 缩略图 320px 宽 JPEG 缓存于 `.thumbs/`（原图数 MB → 缩略图 10~30KB）
//! - 文件安全：扩展名白名单 + 路径穿越防护

use base64::Engine;
use byte_git_core::util::AppPaths;
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// 接受的图片扩展名白名单（小写、无点）。
const WALLPAPER_EXTS: &[&str] = &["jpg", "jpeg", "png", "webp", "gif", "bmp", "avif"];
/// 缩略图宽度上限（不放大）。
const THUMB_MAX_WIDTH: u32 = 320;
/// 缩略图 JPEG 质量。
const THUMB_JPEG_QUALITY: u8 = 85;

fn is_wallpaper_ext(path: &str) -> bool {
    let ext = path.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
    WALLPAPER_EXTS.contains(&ext.as_str())
}

fn ext_of(path: &str) -> String {
    path.rsplit('.').next().unwrap_or("").to_ascii_lowercase()
}

fn mime_of(ext: &str) -> Option<&'static str> {
    Some(match ext {
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        "webp" => "image/webp",
        "gif" => "image/gif",
        "bmp" => "image/bmp",
        "avif" => "image/avif",
        _ => return None,
    })
}

/// 壁纸数据目录（不存在则创建）。
fn wallpaper_dir() -> Result<PathBuf, String> {
    let dir = AppPaths::new().map_err(|e| e.to_string())?.root().join("wallpapers");
    std::fs::create_dir_all(&dir).map_err(|e| format!("创建壁纸目录失败: {e}"))?;
    Ok(dir)
}

/// 文件名安全校验：非空 + 无路径分隔/穿越。
fn validate_filename(filename: &str) -> Result<(), String> {
    if filename.is_empty() {
        return Err("文件名为空".into());
    }
    if filename.contains('/') || filename.contains('\\') || filename.contains("..") {
        return Err("非法文件名".into());
    }
    Ok(())
}

fn to_data_url(mime: &str, data: &[u8]) -> String {
    format!("data:{mime};base64,{}", base64::engine::general_purpose::STANDARD.encode(data))
}

// ---------------------------------------------------------------------------
// 命令
// ---------------------------------------------------------------------------

/// WallpaperUpload：复制源图到壁纸目录，时间戳前缀防覆盖，返回新文件名。
#[tauri::command]
pub fn wallpaper_upload(src_path: String) -> Result<String, String> {
    if !is_wallpaper_ext(&src_path) {
        return Err(format!("不支持的图片格式（允许：{}）", WALLPAPER_EXTS.join(", ")));
    }
    let meta = std::fs::metadata(&src_path).map_err(|e| format!("源文件不可访问: {e}"))?;
    if meta.is_dir() {
        return Err("源路径是目录，不是文件".into());
    }
    let dir = wallpaper_dir()?;
    let stem = Path::new(&src_path)
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let ext = format!(".{}", ext_of(&stem));
    let name_only = stem.strip_suffix(&ext).unwrap_or(&stem);
    let ts = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let filename = format!("{ts}-{name_only}{ext}");
    std::fs::copy(&src_path, dir.join(&filename))
        .map_err(|e| format!("复制文件失败: {e}"))?;
    Ok(filename)
}

/// 列表条目（name + mtime，newest-first）。
#[derive(Serialize)]
struct ListEntry {
    name: String,
    modified: i64,
}

fn list_entries() -> Result<Vec<ListEntry>, String> {
    let dir = wallpaper_dir()?;
    let mut out = Vec::new();
    for entry in std::fs::read_dir(&dir).map_err(|e| e.to_string())? {
        let Ok(entry) = entry else { continue };
        let name = entry.file_name().to_string_lossy().into_owned();
        if entry.path().is_dir() || !is_wallpaper_ext(&name) {
            continue;
        }
        let Ok(meta) = entry.metadata() else { continue };
        let modified = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        out.push(ListEntry { name, modified });
    }
    out.sort_by(|a, b| b.modified.cmp(&a.modified));
    Ok(out)
}

/// WallpaperList：壁纸文件名列表（newest-first）。
#[tauri::command]
pub fn wallpaper_list() -> Result<Vec<String>, String> {
    Ok(list_entries()?.into_iter().map(|e| e.name).collect())
}

/// WallpaperDelete：删除壁纸 + 缩略图（穿越防护 + 白名单；源不存在仍清理缩略图）。
#[tauri::command]
pub fn wallpaper_delete(filename: String) -> Result<(), String> {
    validate_filename(&filename)?;
    if !is_wallpaper_ext(&filename) {
        return Err("仅可删除壁纸文件".into());
    }
    let dir = wallpaper_dir()?;
    let path = dir.join(&filename);
    match std::fs::remove_file(&path) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(e.to_string()),
    }
    // 尽力清理缩略图
    let _ = std::fs::remove_file(dir.join(".thumbs").join(format!("{filename}.jpg")));
    Ok(())
}

/// WallpaperDataDir：壁纸目录绝对路径。
#[tauri::command]
pub fn wallpaper_data_dir() -> Result<String, String> {
    Ok(wallpaper_dir()?.display().to_string())
}

/// WallpaperDataURL：原图 base64 data URL（30MB 上限，防内存失控）。
#[tauri::command]
pub fn wallpaper_data_url(filename: String) -> Result<String, String> {
    const MAX_BYTES: u64 = 30 * 1024 * 1024;
    validate_filename(&filename)?;
    let Some(mime) = mime_of(&ext_of(&filename)) else {
        return Err("仅可读取壁纸文件".into());
    };
    let path = wallpaper_dir()?.join(&filename);
    let meta = std::fs::metadata(&path).map_err(|e| e.to_string())?;
    if meta.len() > MAX_BYTES {
        return Err(format!("图片过大（{} 字节，上限 {}）", meta.len(), MAX_BYTES));
    }
    let data = std::fs::read(&path).map_err(|e| e.to_string())?;
    Ok(to_data_url(mime, &data))
}

// ---------------------------------------------------------------------------
// 缩略图（原图多 MB → 网格用 320px JPEG，首次访问生成缓存）
// ---------------------------------------------------------------------------

fn thumbs_dir(dir: &Path) -> Result<PathBuf, String> {
    let td = dir.join(".thumbs");
    std::fs::create_dir_all(&td).map_err(|e| format!("创建缩略图目录失败: {e}"))?;
    Ok(td)
}

/// 生成（或命中缓存）缩略图，返回路径。解码失败（avif 等）返回 Err 由调用方回落原图。
fn ensure_thumbnail(dir: &Path, filename: &str) -> Result<PathBuf, String> {
    let src_path = dir.join(filename);
    let src_mtime = std::fs::metadata(&src_path)
        .and_then(|m| m.modified())
        .map_err(|e| e.to_string())?;
    let td = thumbs_dir(dir)?;
    let thumb_path = td.join(format!("{filename}.jpg"));
    // 缓存命中：缩略图存在且比源新
    if let Ok(tm) = std::fs::metadata(&thumb_path).and_then(|m| m.modified()) {
        if tm > src_mtime {
            return Ok(thumb_path);
        }
    }
    // 解码 + CatmullRom 缩放（对齐上游 golang.org/x/image/draw）+ JPEG q85
    let img = image::open(&src_path).map_err(|e| format!("解码图片失败: {e}"))?;
    let (w, h) = (img.width(), img.height());
    if w == 0 || h == 0 {
        return Err("空图片".into());
    }
    let tw = w.min(THUMB_MAX_WIDTH); // 不放大
    let th = (h as u64 * tw as u64 / w as u64).max(1) as u32;
    let scaled = image::imageops::resize(&img, tw, th, image::imageops::FilterType::CatmullRom);
    let mut buf = Vec::new();
    let mut enc = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut buf, THUMB_JPEG_QUALITY);
    enc.encode_image(&scaled).map_err(|e| format!("编码缩略图失败: {e}"))?;
    std::fs::write(&thumb_path, buf).map_err(|e| e.to_string())?;
    Ok(thumb_path)
}

/// WallpaperThumbURL：缩略图 base64 JPEG data URL；解码不了（avif/bmp）回落原图。
#[tauri::command]
pub async fn wallpaper_thumb_url(filename: String) -> Result<String, String> {
    run_blocking(move || {
        validate_filename(&filename)?;
        let dir = wallpaper_dir()?;
        match ensure_thumbnail(&dir, &filename) {
            Ok(thumb) => {
                let data = std::fs::read(&thumb).map_err(|e| e.to_string())?;
                Ok(to_data_url("image/jpeg", &data))
            }
            Err(_) => wallpaper_data_url(filename), // 回落原图
        }
    })?
}

/// 后台线程执行阻塞任务（解码/缩放避免卡 UI 线程）。
fn run_blocking<T, F>(f: F) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce() -> T + Send + 'static,
{
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(f());
    });
    rx.recv().map_err(|_| "后台任务线程异常退出".to_string())
}

/// WallpaperPickFile：原生单文件选择器（图片过滤）；取消返回空串。
#[tauri::command]
pub async fn wallpaper_pick_file(app: tauri::AppHandle) -> Result<String, String> {
    use tauri_plugin_dialog::DialogExt;
    let (tx, rx) = std::sync::mpsc::channel::<Option<String>>();
    app.dialog()
        .file()
        .add_filter("图片", WALLPAPER_EXTS)
        .pick_file(move |file| {
            let path = file
                .and_then(|f| f.as_path().map(|p| p.display().to_string()))
                .unwrap_or_default();
            let _ = tx.send(Some(path));
        });
    let picked = std::thread::spawn(move || rx.recv().ok().flatten())
        .join()
        .map_err(|_| "对话框线程异常".to_string())?
        .unwrap_or_default();
    Ok(picked)
}

/// Abs：壁纸文件名在数据目录内的绝对路径。
#[tauri::command]
pub fn abs(filename: String) -> Result<String, String> {
    let dir = wallpaper_dir()?;
    Ok(dir.join(filename).display().to_string())
}

// ---------------------------------------------------------------------------
// 测试
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_root() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("bg-wallpaper-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::env::set_var("BYTE_GIT_HOME", &dir);
        dir
    }

    #[test]
    fn upload_list_delete_roundtrip() {
        let root = temp_root();
        // 穿越防护先验（无 fs 副作用）
        assert!(wallpaper_delete("../evil.png".into()).is_err());
        assert!(wallpaper_delete("a/b.png".into()).is_err());
        assert!(wallpaper_data_url("notes.txt".into()).is_err()); // 非白名单

        // 造一张小 PNG
        let src = root.join("src.png");
        let img = image::DynamicImage::new_rgb8(8, 6);
        img.save(&src).unwrap();

        let name = wallpaper_upload(src.display().to_string()).unwrap();
        assert!(name.starts_with(|c: char| c.is_ascii_digit()) && name.ends_with("src.png"));
        assert_eq!(wallpaper_list().unwrap(), vec![name.clone()]);

        // data URL mime
        let url = wallpaper_data_url(name.clone()).unwrap();
        assert!(url.starts_with("data:image/png;base64,"));

        // 缩略图生成（直测内部函数，绕开 async 命令壳）
        let dir = wallpaper_dir().unwrap();
        let thumb = ensure_thumbnail(&dir, &name).unwrap();
        let data = std::fs::read(&thumb).unwrap();
        assert!(to_data_url("image/jpeg", &data).starts_with("data:image/jpeg;base64,"));
        // 缓存文件存在
        assert!(dir.join(".thumbs").join(format!("{name}.jpg")).is_file());

        wallpaper_delete(name.clone()).unwrap();
        assert!(wallpaper_list().unwrap().is_empty());
        assert!(!wallpaper_dir().unwrap().join(".thumbs").join(format!("{name}.jpg")).exists());

        let _ = std::fs::remove_dir_all(&root);
    }
}
