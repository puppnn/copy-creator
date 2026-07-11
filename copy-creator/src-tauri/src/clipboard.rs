use std::io::Write;
use std::sync::atomic::Ordering;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_clipboard_manager::ClipboardExt;

fn is_url(text: &str) -> bool {
    let trimmed = text.trim();
    if trimmed.chars().any(char::is_control) {
        return false;
    }

    let lower = trimmed.to_lowercase();
    for prefix in ["http://", "https://", "ftp://", "ftps://"] {
        if let Some(rest) = lower.strip_prefix(prefix) {
            return !rest.trim().is_empty();
        }
    }
    false
}

#[cfg(test)]
mod url_tests {
    use super::is_url;

    #[test]
    fn accepts_links_with_default_handlers() {
        for url in [
            "https://example.com/path",
            "HTTP://EXAMPLE.COM",
            "ftp://files.example.com/archive.zip",
            " ftps://files.example.com ",
        ] {
            assert!(is_url(url), "expected a supported URL: {url}");
        }
    }

    #[test]
    fn rejects_unsupported_or_malformed_links() {
        for value in [
            "javascript:alert(1)",
            "file:///C:/Windows/System32/calc.exe",
            "https://",
            "https://example.com\nfile:///C:/secret.txt",
        ] {
            assert!(!is_url(value), "expected URL rejection: {value}");
        }
    }
}

#[tauri::command]
pub fn open_external_link(url: String) -> Result<(), String> {
    let url = url.trim();
    if !is_url(url) {
        return Err("Unsupported link protocol".to_string());
    }

    #[cfg(target_os = "windows")]
    {
        use windows::core::{w, PCWSTR};
        use windows::Win32::Foundation::HWND;
        use windows::Win32::UI::Shell::ShellExecuteW;
        use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

        let target: Vec<u16> = url.encode_utf16().chain(std::iter::once(0)).collect();
        let result = unsafe {
            ShellExecuteW(
                HWND::default(),
                w!("open"),
                PCWSTR(target.as_ptr()),
                PCWSTR::null(),
                PCWSTR::null(),
                SW_SHOWNORMAL,
            )
        };

        if result.0 as isize <= 32 {
            return Err(format!(
                "Failed to open link (ShellExecuteW code {})",
                result.0 as isize
            ));
        }
        Ok(())
    }

    #[cfg(not(target_os = "windows"))]
    {
        Err("Opening links is currently supported on Windows only".to_string())
    }
}

fn is_previewable_image_file(path: &str) -> bool {
    let lower = path.to_lowercase();
    lower.ends_with(".jpg") || lower.ends_with(".jpeg") || lower.ends_with(".png")
}

const LARGE_IMAGE_THRESHOLD_BYTES: u64 = 10 * 1024 * 1024;
const THUMBNAIL_MAX_SIZE: u32 = 360;
const TEXT_EVENT_PREVIEW_CHARS: usize = 600;

fn is_image_file(path: &str) -> bool {
    let lower = path.to_lowercase();
    lower.ends_with(".png")
        || lower.ends_with(".jpg")
        || lower.ends_with(".jpeg")
        || lower.ends_with(".gif")
        || lower.ends_with(".bmp")
        || lower.ends_with(".webp")
        || lower.ends_with(".ico")
}

fn make_text_event_content(record_type: &str, content: &str) -> (String, i64, bool) {
    let total_chars = content.chars().count();
    if record_type != "text" || total_chars <= TEXT_EVENT_PREVIEW_CHARS {
        return (content.to_string(), total_chars as i64, false);
    }

    (
        content.chars().take(TEXT_EVENT_PREVIEW_CHARS).collect(),
        total_chars as i64,
        true,
    )
}

struct StoredImage {
    relative_path: String,
    rgba: Vec<u8>,
    width: u32,
    height: u32,
    clipboard_png: Vec<u8>,
}

fn image_setting_u32(app: &AppHandle, key: &str, default: u32, min: u32, max: u32) -> u32 {
    crate::db::get_setting_sync(app, key)
        .and_then(|value| value.parse::<u32>().ok())
        .unwrap_or(default)
        .clamp(min, max)
}

fn encode_png(rgba: &[u8], width: u32, height: u32) -> Option<Vec<u8>> {
    let mut output = Vec::new();
    let encoder = image::codecs::png::PngEncoder::new(&mut output);
    use image::ImageEncoder;
    encoder
        .write_image(rgba, width, height, image::ExtendedColorType::Rgba8)
        .ok()?;
    Some(output)
}

fn process_and_store_image(
    app: &AppHandle,
    mut image: image::DynamicImage,
    source_bytes: u64,
) -> Option<StoredImage> {
    let handling = crate::db::get_setting_sync(app, "large_image_handling")
        .unwrap_or_else(|| "compress".to_string());
    let is_large = source_bytes >= LARGE_IMAGE_THRESHOLD_BYTES;
    if is_large && handling == "skip" {
        log::info!("clipboard: skipped image larger than 10 MB");
        return None;
    }

    let keep_large_original = is_large && handling == "keep";
    let max_dimension = image_setting_u32(app, "image_max_dimension", 4096, 512, 16_384);
    let quality = image_setting_u32(app, "image_compression_quality", 90, 40, 100) as u8;
    if !keep_large_original && image.width().max(image.height()) > max_dimension {
        image = image.resize(
            max_dimension,
            max_dimension,
            image::imageops::FilterType::Lanczos3,
        );
    }

    let rgba_image = image.to_rgba8();
    let (width, height) = rgba_image.dimensions();
    let has_transparency = rgba_image.pixels().any(|pixel| pixel.0[3] != 255);
    let clipboard_png = encode_png(rgba_image.as_raw(), width, height)?;
    let use_jpeg = !keep_large_original && quality < 100 && !has_transparency;
    let (stored_bytes, extension) = if use_jpeg {
        let rgb = image::DynamicImage::ImageRgba8(rgba_image.clone()).to_rgb8();
        let mut bytes = Vec::new();
        let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut bytes, quality);
        use image::ImageEncoder;
        encoder
            .write_image(rgb.as_raw(), width, height, image::ExtendedColorType::Rgb8)
            .ok()?;
        (bytes, "jpg")
    } else {
        (clipboard_png.clone(), "png")
    };

    let content_hash = stored_bytes.iter().fold(0u64, |acc, &byte| {
        acc.wrapping_mul(31).wrapping_add(byte as u64)
    });
    let filename = format!("{content_hash:016x}.{extension}");
    let relative_path = format!("images/{filename}");
    let mut images_dir = crate::db::get_storage_dir(app);
    images_dir.push("images");
    std::fs::create_dir_all(&images_dir).ok()?;
    let image_path = images_dir.join(&filename);
    if !image_path.exists() {
        let mut file = std::fs::File::create(&image_path).ok()?;
        file.write_all(&stored_bytes).ok()?;
    }

    let thumb_dir = images_dir.join("thumbs");
    std::fs::create_dir_all(&thumb_dir).ok()?;
    let thumb_path = thumb_dir.join(&filename);
    if !thumb_path.exists() {
        let thumbnail = image::DynamicImage::ImageRgba8(rgba_image.clone())
            .thumbnail(THUMBNAIL_MAX_SIZE, THUMBNAIL_MAX_SIZE);
        let mut thumb_output = std::io::Cursor::new(Vec::new());
        if thumbnail
            .write_to(&mut thumb_output, image::ImageFormat::Png)
            .is_ok()
        {
            let _ = std::fs::write(&thumb_path, thumb_output.into_inner());
        }
    }

    Some(StoredImage {
        relative_path,
        rgba: rgba_image.into_raw(),
        width,
        height,
        clipboard_png,
    })
}

/// Import an image file from disk into the storage directory.
/// Returns true if the file was imported as an image record.
fn import_image_file(app: &AppHandle, file_path: &str) -> bool {
    let file_size = std::fs::metadata(file_path).map(|m| m.len()).unwrap_or(0);

    let img_bytes = match std::fs::read(file_path) {
        Ok(b) => b,
        Err(_) => return false,
    };

    let decoded = match image::load_from_memory(&img_bytes) {
        Ok(d) => d,
        Err(_) => return false,
    };

    let Some(stored) = process_and_store_image(app, decoded, file_size) else {
        return false;
    };
    crate::paste::cache_image(
        stored.relative_path.clone(),
        stored.rgba,
        stored.width,
        stored.height,
        stored.clipboard_png,
    );
    insert_and_emit(app, "image", &stored.relative_path);
    true
}

/// Lightweight: hash the raw clipboard image bytes (PNG or DIB) for stable dedup.
/// Raw clipboard bytes are deterministic across reads, unlike re-decoded RGBA.
#[cfg(target_os = "windows")]
fn get_clipboard_image_hash() -> u64 {
    use windows::Win32::Foundation::{HGLOBAL, HWND};
    use windows::Win32::System::DataExchange::*;
    use windows::Win32::System::Memory::*;

    unsafe {
        if OpenClipboard(HWND(std::ptr::null_mut())).is_err() {
            return 0;
        }

        let mut result = 0u64;

        // Hash PNG format if available (most stable)
        let png_format_name: Vec<u16> = "PNG\0".encode_utf16().collect();
        let cf_png = RegisterClipboardFormatW(windows::core::PCWSTR(png_format_name.as_ptr()));
        if cf_png != 0 && IsClipboardFormatAvailable(cf_png).is_ok() {
            if let Ok(handle) = GetClipboardData(cf_png) {
                let hglobal = HGLOBAL(handle.0);
                let size = GlobalSize(hglobal);
                if size > 0 {
                    let ptr = GlobalLock(hglobal);
                    if !ptr.is_null() {
                        let bytes = std::slice::from_raw_parts(ptr as *const u8, size);
                        result = bytes
                            .iter()
                            .fold(0u64, |acc, &b| acc.wrapping_mul(31).wrapping_add(b as u64));
                        let _ = GlobalUnlock(hglobal);
                        let _ = CloseClipboard();
                        return result;
                    }
                    let _ = GlobalUnlock(hglobal);
                }
            }
        }

        // Fallback: hash DIB header + first 256 pixels for stable fingerprint
        const CF_DIB_VAL: u32 = 8;
        if IsClipboardFormatAvailable(CF_DIB_VAL).is_ok() {
            if let Ok(handle) = GetClipboardData(CF_DIB_VAL) {
                let hglobal = HGLOBAL(handle.0);
                let size = GlobalSize(hglobal);
                if size >= 40 {
                    let ptr = GlobalLock(hglobal);
                    if !ptr.is_null() {
                        let src = ptr as *const u8;
                        // Hash DIB header (40 bytes) + first 1024 bytes of pixel data
                        let hash_len = (40 + 1024).min(size);
                        let bytes = std::slice::from_raw_parts(src, hash_len);
                        result = bytes
                            .iter()
                            .fold(0u64, |acc, &b| acc.wrapping_mul(31).wrapping_add(b as u64));
                    }
                    let _ = GlobalUnlock(hglobal);
                }
            }
        }

        let _ = CloseClipboard();
        result
    }
}

/// Direct Windows clipboard image read as a supplement to the clipboard plugin.
/// Returns decoded RGBA data + dimensions (only called when image hash changed).
#[cfg(target_os = "windows")]
fn read_clipboard_image_raw() -> Option<(Vec<u8>, u32, u32)> {
    use windows::Win32::Foundation::{HGLOBAL, HWND};
    use windows::Win32::System::DataExchange::*;
    use windows::Win32::System::Memory::*;

    const CF_DIB_VAL: u32 = 8;
    const CF_DIBV5_VAL: u32 = 17;

    unsafe {
        if OpenClipboard(HWND(std::ptr::null_mut())).is_err() {
            return None;
        }

        // Try PNG format first (lossless, preserves alpha)
        let png_format_name: Vec<u16> = "PNG\0".encode_utf16().collect();
        let cf_png = RegisterClipboardFormatW(windows::core::PCWSTR(png_format_name.as_ptr()));
        if cf_png != 0 && IsClipboardFormatAvailable(cf_png).is_ok() {
            if let Ok(handle) = GetClipboardData(cf_png) {
                let hglobal = HGLOBAL(handle.0);
                let size = GlobalSize(hglobal);
                if size > 0 {
                    let ptr = GlobalLock(hglobal);
                    if !ptr.is_null() {
                        let bytes = std::slice::from_raw_parts(ptr as *const u8, size).to_vec();
                        let _ = GlobalUnlock(hglobal);
                        let _ = CloseClipboard();
                        if let Ok(img) = image::load_from_memory(&bytes) {
                            let rgba = img.to_rgba8();
                            let (w, h) = (img.width(), img.height());
                            return Some((rgba.to_vec(), w, h));
                        }
                    }
                    let _ = GlobalUnlock(hglobal);
                }
            }
        }

        // Try CF_DIBV5 first, then CF_DIB
        for format in [CF_DIBV5_VAL, CF_DIB_VAL] {
            if IsClipboardFormatAvailable(format).is_err() {
                continue;
            }
            if let Ok(handle) = GetClipboardData(format) {
                let hglobal = HGLOBAL(handle.0);
                let size = GlobalSize(hglobal);
                if size >= 40 {
                    let ptr = GlobalLock(hglobal);
                    if !ptr.is_null() {
                        let header = ptr as *const u32;
                        let bi_size = *header;
                        let bpp = *((ptr as *const u8).add(14)) as u16;
                        let compression = *header.add(4);
                        let w = *header.add(1) as i32;
                        let h = (*header.add(2) as i32).abs();
                        if w > 0 && h > 0 && w < 20000 && h < 20000 {
                            let rgba = if bpp == 32 && compression == 0 {
                                let pixel_count = (w * h) as usize;
                                let src = (ptr as *const u8).add(bi_size as usize);
                                let mut rgba = vec![0u8; pixel_count * 4];
                                for i in 0..pixel_count {
                                    rgba[i * 4] = *src.add(i * 4 + 2);
                                    rgba[i * 4 + 1] = *src.add(i * 4 + 1);
                                    rgba[i * 4 + 2] = *src.add(i * 4);
                                    rgba[i * 4 + 3] = *src.add(i * 4 + 3);
                                }
                                rgba
                            } else {
                                let full =
                                    std::slice::from_raw_parts(ptr as *const u8, size).to_vec();
                                let _ = GlobalUnlock(hglobal);
                                let _ = CloseClipboard();
                                let pixel_offset = bi_size;
                                let file_size = 14 + size as u32;
                                let mut bmp: Vec<u8> = Vec::with_capacity(file_size as usize);
                                bmp.extend_from_slice(b"BM");
                                bmp.extend_from_slice(&file_size.to_le_bytes());
                                bmp.extend_from_slice(&0u32.to_le_bytes());
                                bmp.extend_from_slice(&(14u32 + pixel_offset).to_le_bytes());
                                bmp.extend_from_slice(&full);
                                if let Ok(img) = image::load_from_memory(&bmp) {
                                    let rgba = img.to_rgba8();
                                    return Some((rgba.to_vec(), img.width(), img.height()));
                                }
                                return None;
                            };
                            let _ = GlobalUnlock(hglobal);
                            let _ = CloseClipboard();
                            return Some((rgba, w as u32, h as u32));
                        }
                        let _ = GlobalUnlock(hglobal);
                    }
                }
            }
        }

        let _ = CloseClipboard();
    }
    None
}

#[cfg(target_os = "windows")]
fn read_clipboard_files() -> Option<Vec<String>> {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::System::DataExchange::*;
    use windows::Win32::UI::Shell::{DragQueryFileW, HDROP};

    const CF_HDROP: u32 = 15;

    unsafe {
        if OpenClipboard(HWND(std::ptr::null_mut())).is_err() {
            return None;
        }

        if IsClipboardFormatAvailable(CF_HDROP).is_err() {
            let _ = CloseClipboard();
            return None;
        }

        let handle = match GetClipboardData(CF_HDROP) {
            Ok(h) => h,
            Err(_) => {
                let _ = CloseClipboard();
                return None;
            }
        };

        let hdrop = HDROP(handle.0);

        let count = DragQueryFileW(hdrop, 0xFFFFFFFF, None);
        if count == 0 {
            let _ = CloseClipboard();
            return None;
        }

        let mut paths = Vec::new();
        for i in 0..count {
            let len = DragQueryFileW(hdrop, i, None);
            if len == 0 {
                continue;
            }
            let mut buf = vec![0u16; (len as usize) + 1];
            DragQueryFileW(hdrop, i, Some(&mut buf));
            let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
            let path = String::from_utf16_lossy(&buf[..end]);
            if !path.is_empty() {
                paths.push(path);
            }
        }

        let _ = CloseClipboard();

        if paths.is_empty() {
            None
        } else {
            Some(paths)
        }
    }
}

/// Cached clipboard state, updated by the monitor and by paste functions.
/// When paste writes to the clipboard, it syncs these to prevent duplicate records.
pub static LAST_CLIPBOARD_TEXT: std::sync::Mutex<String> = std::sync::Mutex::new(String::new());
pub static LAST_CLIPBOARD_IMAGE_HASH: std::sync::Mutex<u64> = std::sync::Mutex::new(0);
#[cfg(target_os = "windows")]
pub static LAST_CLIPBOARD_FILES_KEY: std::sync::Mutex<String> =
    std::sync::Mutex::new(String::new());

/// Windows clipboard sequence number — increments on every clipboard change,
/// even if content is identical. Used to detect re-copies of the same content.
#[cfg(target_os = "windows")]
static LAST_CLIPBOARD_SEQ: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

#[cfg(target_os = "windows")]
fn get_clipboard_sequence() -> u32 {
    use windows::Win32::System::DataExchange::GetClipboardSequenceNumber;
    unsafe { GetClipboardSequenceNumber() }
}

fn is_main_window_active(app: &AppHandle) -> bool {
    app.get_webview_window("main").is_some_and(|window| {
        window.is_visible().unwrap_or(false) && window.is_focused().unwrap_or(false)
    })
}

fn notification_preview(app: &AppHandle, record_type: &str, content: &str) -> (String, String) {
    let lang = crate::db::get_setting_sync(app, "language").unwrap_or_else(|| "zh-CN".to_string());
    let is_english = lang == "en";
    let title = if is_english {
        "New clipboard item"
    } else {
        "新的剪贴板内容"
    };
    let body = if (record_type == "text" || record_type == "link") && crate::db::is_api_key(content)
    {
        if is_english {
            "API Key copied"
        } else {
            "已复制新的 API Key"
        }
        .to_string()
    } else {
        match record_type {
            "image" => if is_english {
                "Image copied"
            } else {
                "已复制一张图片"
            }
            .to_string(),
            "file" => {
                let name = std::path::Path::new(content)
                    .file_name()
                    .and_then(|value| value.to_str())
                    .unwrap_or(content);
                format!("{}: {}", if is_english { "File" } else { "文件" }, name)
            }
            _ => {
                let normalized = content.split_whitespace().collect::<Vec<_>>().join(" ");
                let mut chars = normalized.chars();
                let preview: String = chars.by_ref().take(80).collect();
                if chars.next().is_some() {
                    format!("{preview}...")
                } else {
                    preview
                }
            }
        }
    };
    (title.to_string(), body)
}

fn maybe_show_clipboard_notification(app: &AppHandle, record_type: &str, content: &str) {
    if is_main_window_active(app)
        || crate::db::get_setting_sync(app, "clipboard_notifications").as_deref() != Some("1")
    {
        return;
    }
    use tauri_plugin_notification::NotificationExt;
    let (title, body) = notification_preview(app, record_type, content);
    if let Err(error) = app.notification().builder().title(title).body(body).show() {
        log::warn!("clipboard notification failed: {error}");
    }
}

/// Insert a new record into the DB and emit clipboard-update.
/// Skips insertion only if the most recent record has identical type and content
/// AND was created within the last 2 seconds (debounce window).
/// Re-copies after 2 seconds are treated as intentional and recorded normally.
fn insert_and_emit(app: &AppHandle, record_type: &str, content: &str) {
    let one_second_ago = chrono::Utc::now() - chrono::Duration::seconds(1);
    let cutoff = one_second_ago.to_rfc3339();

    let is_duplicate: bool = {
        let state = app.state::<crate::db::DbState>();
        let x = match state.conn.lock() {
            Ok(conn) => conn.query_row(
                "SELECT type, content, created_at FROM clipboard_records ORDER BY created_at DESC LIMIT 1",
                [],
                |row| {
                    let last_type: String = row.get(0)?;
                    let last_content: String = row.get(1)?;
                    let last_created: String = row.get(2)?;
                    Ok(last_type == record_type && last_content == content && last_created >= cutoff)
                },
            )
            .unwrap_or(false),
            Err(_) => false,
        };
        x
    };

    if is_duplicate {
        return;
    }

    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    let inserted = {
        let state = app.state::<crate::db::DbState>();
        let result = match state.conn.lock() {
            Ok(conn) => conn.execute(
                "INSERT INTO clipboard_records (id, type, content, source_app, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
                rusqlite::params![&id, record_type, content, "", &now],
            ).is_ok(),
            Err(_) => false,
        };
        result
    };
    if !inserted {
        return;
    }
    // API Key detection
    let (is_key, key_preview, guessed_service) =
        if (record_type == "text" || record_type == "link") && crate::db::is_api_key(content) {
            let preview = crate::db::make_key_preview(content);
            let guess = crate::db::guess_service(content).map(|s| s.to_string());
            if !crate::db::is_toast_shown_internal(app, &preview) {
                crate::db::mark_toast_shown_internal(app, &preview);
                app.emit(
                    "api-key-detected",
                    serde_json::json!({
                        "record_id": id,
                        "key_preview": &preview,
                        "guess": &guess,
                    }),
                )
                .ok();
            }
            (true, preview, guess)
        } else {
            (false, String::new(), None::<String>)
        };

    let (event_content, content_length, content_truncated) =
        make_text_event_content(record_type, content);

    app.emit(
        "clipboard-update",
        serde_json::json!({
            "id": id,
            "type": record_type,
            "content": event_content,
            "content_length": content_length,
            "content_truncated": content_truncated,
            "source_app": "",
            "created_at": now,
            "is_api_key": is_key,
            "key_preview": key_preview,
            "guessed_service": guessed_service,
            "label": null,
            "is_favorite": false,
        }),
    )
    .ok();

    crate::db::increment_unread_if_hidden(app);
    maybe_show_clipboard_notification(app, record_type, content);
    if let Err(error) = crate::db::enforce_clipboard_limits(app) {
        log::warn!("clipboard limit enforcement failed: {error}");
    }
    crate::tray::refresh_tray_menu(app).ok();
}

pub fn sync_monitor_cache(handle: &AppHandle) {
    if let Ok(text) = handle.clipboard().read_text() {
        *LAST_CLIPBOARD_TEXT.lock().unwrap() = text.trim().to_string();
    }
    #[cfg(target_os = "windows")]
    {
        let h = get_clipboard_image_hash();
        if h != 0 {
            *LAST_CLIPBOARD_IMAGE_HASH.lock().unwrap() = h;
        }
        if let Some(files) = read_clipboard_files() {
            *LAST_CLIPBOARD_FILES_KEY.lock().unwrap() = files.join("|");
        }
        LAST_CLIPBOARD_SEQ.store(get_clipboard_sequence(), Ordering::SeqCst);
    }
}

pub fn start_monitor(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    let handle = app.clone();

    {
        let initial_text = handle
            .clipboard()
            .read_text()
            .map(|s| s.trim().to_string())
            .unwrap_or_default();
        *LAST_CLIPBOARD_TEXT.lock().unwrap() = initial_text;
    }

    #[cfg(target_os = "windows")]
    {
        *LAST_CLIPBOARD_IMAGE_HASH.lock().unwrap() = get_clipboard_image_hash();
    }

    #[cfg(target_os = "windows")]
    {
        let key = read_clipboard_files()
            .map(|files| files.join("|"))
            .unwrap_or_default();
        *LAST_CLIPBOARD_FILES_KEY.lock().unwrap() = key;
        LAST_CLIPBOARD_SEQ.store(get_clipboard_sequence(), Ordering::SeqCst);
    }

    std::thread::spawn(move || {
        let mut poll_count: u32 = 0;
        loop {
            std::thread::sleep(std::time::Duration::from_millis(800));
            poll_count += 1;

            // Skip first 2 polls (1.6s) to avoid recording startup clipboard state
            if poll_count <= 2 {
                sync_monitor_cache(&handle);
                continue;
            }

            if crate::paste::PASTING.load(std::sync::atomic::Ordering::SeqCst) {
                sync_monitor_cache(&handle);
                continue;
            }

            // Detect clipboard changes via Windows sequence number.
            // This catches re-copies of identical content (e.g. same image twice).
            let seq_changed = {
                #[cfg(target_os = "windows")]
                {
                    let current_seq = get_clipboard_sequence();
                    let last_seq = LAST_CLIPBOARD_SEQ.load(Ordering::SeqCst);
                    if current_seq != last_seq {
                        LAST_CLIPBOARD_SEQ.store(current_seq, Ordering::SeqCst);
                        true
                    } else {
                        false
                    }
                }
                #[cfg(not(target_os = "windows"))]
                {
                    false
                }
            };

            if !seq_changed {
                continue;
            }

            let mut image_recorded = false;

            let mut image_data: Option<(Vec<u8>, u32, u32)> = None;
            // Track whether the image hash stayed the same (re-copy of same image)
            let mut image_is_same = false;

            // Stable dedup: hash raw clipboard bytes (deterministic) rather than RGBA
            #[cfg(target_os = "windows")]
            {
                let raw_hash = get_clipboard_image_hash();
                let mut cached_hash = LAST_CLIPBOARD_IMAGE_HASH.lock().unwrap();
                if raw_hash != 0 && raw_hash != *cached_hash {
                    *cached_hash = raw_hash;
                    drop(cached_hash);
                    if let Some((rgba, w, h)) = read_clipboard_image_raw() {
                        image_data = Some((rgba, w, h));
                    }
                } else if raw_hash != 0 {
                    // Sequence changed but image hash didn't — same image re-copied
                    image_is_same = true;
                    drop(cached_hash);
                } else {
                    drop(cached_hash);
                }
            }

            #[cfg(not(target_os = "windows"))]
            {
                // Non-Windows: use plugin-based image read with RGBA hash
                if let Ok(image) = handle.clipboard().read_image() {
                    let rgba = image.rgba();
                    if !rgba.is_empty() && image.width() > 0 && image.height() > 0 {
                        let hash = rgba
                            .iter()
                            .take(400)
                            .fold(0u64, |acc, &b| acc.wrapping_mul(31).wrapping_add(b as u64));
                        let mut cached_hash = LAST_CLIPBOARD_IMAGE_HASH.lock().unwrap();
                        if hash != *cached_hash {
                            *cached_hash = hash;
                            image_data = Some((rgba.to_vec(), image.width(), image.height()));
                        }
                    }
                }
            }

            if let Some((rgba_vec, img_w, img_h)) = image_data.take() {
                let source_bytes = rgba_vec.len() as u64;
                if let Some(buffer) = image::RgbaImage::from_raw(img_w, img_h, rgba_vec) {
                    if let Some(stored) = process_and_store_image(
                        &handle,
                        image::DynamicImage::ImageRgba8(buffer),
                        source_bytes,
                    ) {
                        log::info!(
                            "clipboard: recorded image {}x{} as {}",
                            stored.width,
                            stored.height,
                            stored.relative_path
                        );
                        crate::paste::cache_image(
                            stored.relative_path.clone(),
                            stored.rgba,
                            stored.width,
                            stored.height,
                            stored.clipboard_png,
                        );
                        insert_and_emit(&handle, "image", &stored.relative_path);
                        image_recorded = true;
                    }
                }
            }

            // Handle re-copy of same image: sequence changed but raw hash didn't.
            // Still insert a new chronological record pointing to the same file on disk.
            if image_is_same {
                #[cfg(target_os = "windows")]
                {
                    if let Some((rgba_vec, img_w, img_h)) = read_clipboard_image_raw() {
                        let source_bytes = rgba_vec.len() as u64;
                        if let Some(buffer) = image::RgbaImage::from_raw(img_w, img_h, rgba_vec) {
                            if let Some(stored) = process_and_store_image(
                                &handle,
                                image::DynamicImage::ImageRgba8(buffer),
                                source_bytes,
                            ) {
                                crate::paste::cache_image(
                                    stored.relative_path.clone(),
                                    stored.rgba,
                                    stored.width,
                                    stored.height,
                                    stored.clipboard_png,
                                );
                                insert_and_emit(&handle, "image", &stored.relative_path);
                            }
                        }
                    } else {
                        log::warn!("clipboard: image_is_same but read_clipboard_image_raw failed, record lost");
                    }
                }
                sync_monitor_cache(&handle);
            } else if image_recorded {
                if let Ok(text) = handle.clipboard().read_text() {
                    *LAST_CLIPBOARD_TEXT.lock().unwrap() = text.trim().to_string();
                }
                #[cfg(target_os = "windows")]
                {
                    if let Some(files) = read_clipboard_files() {
                        *LAST_CLIPBOARD_FILES_KEY.lock().unwrap() = files.join("|");
                    }
                }
            } else {
                if let Ok(text) = handle.clipboard().read_text() {
                    let text = text.trim().to_string();
                    if !text.is_empty() && text != *LAST_CLIPBOARD_TEXT.lock().unwrap() {
                        *LAST_CLIPBOARD_TEXT.lock().unwrap() = text.clone();
                        let record_type = if is_url(&text) { "link" } else { "text" };
                        insert_and_emit(&handle, record_type, &text);
                    } else if !text.is_empty() {
                        // Same text re-copied (sequence changed, text matches cache)
                        let record_type = if is_url(&text) { "link" } else { "text" };
                        insert_and_emit(&handle, record_type, &text);
                    }
                }

                #[cfg(target_os = "windows")]
                {
                    if let Some(files) = read_clipboard_files() {
                        let key = files.join("|");
                        {
                            let mut cached = LAST_CLIPBOARD_FILES_KEY.lock().unwrap();
                            if key == *cached {
                                // Same file paths re-copied — insert new records
                                for file_path in &files {
                                    if file_path.trim().is_empty() {
                                        continue;
                                    }
                                    if is_previewable_image_file(file_path)
                                        || is_image_file(file_path)
                                    {
                                        import_image_file(&handle, file_path);
                                        continue;
                                    }
                                    insert_and_emit(&handle, "file", file_path);
                                }
                                continue;
                            }
                            *cached = key.clone();
                        }

                        for file_path in files {
                            if file_path.trim().is_empty() {
                                continue;
                            }
                            if is_previewable_image_file(&file_path) || is_image_file(&file_path) {
                                if import_image_file(&handle, &file_path) {
                                    continue;
                                }
                                // If import failed and it's an image file, skip (don't record raw path)
                                continue;
                            }
                            insert_and_emit(&handle, "file", &file_path);
                        }
                    }
                }
            }
        }
    });

    Ok(())
}
