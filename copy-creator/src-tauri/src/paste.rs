use std::ptr;
use std::sync::atomic::{AtomicBool, AtomicPtr, Ordering};

use base64::Engine as _;

pub static PASTING: AtomicBool = AtomicBool::new(false);

#[cfg(target_os = "windows")]
static LAST_FOREGROUND_HWND: AtomicPtr<core::ffi::c_void> = AtomicPtr::new(ptr::null_mut());

#[cfg(target_os = "windows")]
static LAST_FOCUS_HWND: AtomicPtr<core::ffi::c_void> = AtomicPtr::new(ptr::null_mut());

#[cfg(target_os = "windows")]
static OUR_HWND: AtomicPtr<core::ffi::c_void> = AtomicPtr::new(ptr::null_mut());

#[cfg(target_os = "windows")]
static RADIAL_HWND: AtomicPtr<core::ffi::c_void> = AtomicPtr::new(ptr::null_mut());

#[cfg(target_os = "windows")]
pub fn register_radial_hwnd(window: &tauri::WebviewWindow) {
    let hwnd = window.hwnd().unwrap_or_default();
    RADIAL_HWND.store(hwnd.0, Ordering::SeqCst);
}

#[cfg(target_os = "windows")]
fn remember_foreground_target(hwnd: windows::Win32::Foundation::HWND, thread_id: u32) {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{
        GetGUIThreadInfo, GetWindowThreadProcessId, GUITHREADINFO,
    };

    if hwnd.is_invalid() {
        return;
    }

    unsafe {
        LAST_FOREGROUND_HWND.store(hwnd.0, Ordering::SeqCst);

        let target_thread = if thread_id == 0 {
            GetWindowThreadProcessId(hwnd, None)
        } else {
            thread_id
        };
        let mut info = GUITHREADINFO {
            cbSize: std::mem::size_of::<GUITHREADINFO>() as u32,
            ..Default::default()
        };
        let focus = if target_thread != 0 && GetGUIThreadInfo(target_thread, &mut info).is_ok() {
            if !info.hwndFocus.is_invalid() {
                info.hwndFocus
            } else if !info.hwndCaret.is_invalid() {
                info.hwndCaret
            } else {
                HWND::default()
            }
        } else {
            HWND::default()
        };
        LAST_FOCUS_HWND.store(focus.0, Ordering::SeqCst);

        log::debug!(
            "[paste] saved foreground=0x{:x}, focus=0x{:x}, thread={}",
            hwnd.0 as usize,
            focus.0 as usize,
            target_thread
        );
    }
}

#[cfg(target_os = "windows")]
pub fn save_foreground_window() {
    use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId};

    unsafe {
        let hwnd = GetForegroundWindow();
        let our = OUR_HWND.load(Ordering::SeqCst);
        let radial = RADIAL_HWND.load(Ordering::SeqCst);
        if !hwnd.is_invalid() && hwnd.0 != our && hwnd.0 != radial {
            remember_foreground_target(hwnd, GetWindowThreadProcessId(hwnd, None));
        }
    }
}

#[cfg(target_os = "windows")]
fn saved_target_has_focus() -> bool {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{
        GetForegroundWindow, GetGUIThreadInfo, GetWindowThreadProcessId, IsChild, IsWindow,
        GUITHREADINFO,
    };

    unsafe {
        let target = HWND(LAST_FOREGROUND_HWND.load(Ordering::SeqCst));
        if target.is_invalid() || !IsWindow(target).as_bool() || GetForegroundWindow() != target {
            return false;
        }

        let saved_focus = HWND(LAST_FOCUS_HWND.load(Ordering::SeqCst));
        if saved_focus.is_invalid() || !IsWindow(saved_focus).as_bool() {
            return true;
        }

        let focus_thread = GetWindowThreadProcessId(saved_focus, None);
        if focus_thread == 0 {
            return false;
        }
        let mut info = GUITHREADINFO {
            cbSize: std::mem::size_of::<GUITHREADINFO>() as u32,
            ..Default::default()
        };
        if GetGUIThreadInfo(focus_thread, &mut info).is_err() {
            return false;
        }

        let current_focus = if !info.hwndFocus.is_invalid() {
            info.hwndFocus
        } else {
            info.hwndCaret
        };
        if current_focus.is_invalid() {
            return false;
        }
        current_focus == saved_focus
            || IsChild(saved_focus, current_focus).as_bool()
            || IsChild(current_focus, saved_focus).as_bool()
    }
}

#[cfg(target_os = "windows")]
fn restore_foreground_target() -> bool {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::System::Threading::{AttachThreadInput, GetCurrentThreadId};
    use windows::Win32::UI::Input::KeyboardAndMouse::SetFocus;
    use windows::Win32::UI::WindowsAndMessaging::{
        GetWindowThreadProcessId, IsWindow, PeekMessageW, SetForegroundWindow, MSG, PM_NOREMOVE,
    };

    unsafe {
        let target = HWND(LAST_FOREGROUND_HWND.load(Ordering::SeqCst));
        if target.is_invalid() || !IsWindow(target).as_bool() {
            return false;
        }

        let saved_focus = HWND(LAST_FOCUS_HWND.load(Ordering::SeqCst));
        let focus_valid = !saved_focus.is_invalid() && IsWindow(saved_focus).as_bool();
        let current_thread = GetCurrentThreadId();
        let target_thread = GetWindowThreadProcessId(target, None);
        let focus_thread = if focus_valid {
            GetWindowThreadProcessId(saved_focus, None)
        } else {
            0
        };

        // AttachThreadInput requires the caller to own a message queue.
        let mut message = MSG::default();
        let _ = PeekMessageW(&mut message, HWND::default(), 0, 0, PM_NOREMOVE);

        let attached_target = target_thread != 0
            && target_thread != current_thread
            && AttachThreadInput(current_thread, target_thread, true).as_bool();
        let attached_focus = focus_thread != 0
            && focus_thread != current_thread
            && focus_thread != target_thread
            && AttachThreadInput(current_thread, focus_thread, true).as_bool();

        let foreground_set = SetForegroundWindow(target).as_bool();
        if focus_valid {
            let _ = SetFocus(saved_focus);
        }
        let ready = saved_target_has_focus();

        if attached_focus {
            let _ = AttachThreadInput(current_thread, focus_thread, false);
        }
        if attached_target {
            let _ = AttachThreadInput(current_thread, target_thread, false);
        }

        log::debug!(
            "[paste] restore foreground=0x{:x}, focus=0x{:x}, set={}, ready={}",
            target.0 as usize,
            saved_focus.0 as usize,
            foreground_set,
            ready
        );
        ready || (foreground_set && !focus_valid)
    }
}

#[cfg(target_os = "windows")]
fn restore_foreground_target_and_wait() -> bool {
    let _ = restore_foreground_target();
    let start = std::time::Instant::now();
    let timeout = Duration::from_millis(180);
    let mut retried = false;

    loop {
        if saved_target_has_focus() {
            return true;
        }
        if !retried && start.elapsed() >= Duration::from_millis(60) {
            let _ = restore_foreground_target();
            retried = true;
        }
        if start.elapsed() >= timeout {
            return false;
        }
        thread::sleep(Duration::from_millis(10));
    }
}

#[cfg(target_os = "windows")]
pub fn init_foreground_tracker(window: &tauri::WebviewWindow) {
    use windows::Win32::UI::Accessibility::SetWinEventHook;
    use windows::Win32::UI::WindowsAndMessaging::WINEVENT_OUTOFCONTEXT;

    const EVENT_SYSTEM_FOREGROUND: u32 = 0x0003;

    let our_hwnd = window.hwnd().unwrap_or_default();
    OUR_HWND.store(our_hwnd.0, Ordering::SeqCst);

    unsafe {
        SetWinEventHook(
            EVENT_SYSTEM_FOREGROUND,
            EVENT_SYSTEM_FOREGROUND,
            None,
            Some(foreground_change_hook),
            0,
            0,
            WINEVENT_OUTOFCONTEXT,
        );
    }
}

#[cfg(target_os = "windows")]
unsafe extern "system" fn foreground_change_hook(
    _hook: windows::Win32::UI::Accessibility::HWINEVENTHOOK,
    _event: u32,
    hwnd: windows::Win32::Foundation::HWND,
    _id_object: i32,
    _id_child: i32,
    event_thread: u32,
    _event_time: u32,
) {
    let our = OUR_HWND.load(Ordering::SeqCst);
    let radial = RADIAL_HWND.load(Ordering::SeqCst);
    if !PASTING.load(Ordering::SeqCst) && hwnd.0 != our && hwnd.0 != radial && !hwnd.is_invalid() {
        remember_foreground_target(hwnd, event_thread);
    }
}

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

struct CachedImage {
    rgba: Arc<Vec<u8>>,
    width: u32,
    height: u32,
    png_bytes: Arc<Vec<u8>>,
}

struct ImageCache {
    map: HashMap<String, CachedImage>,
    order: Vec<String>,
}

static IMAGE_CACHE: OnceLock<Mutex<ImageCache>> = OnceLock::new();

fn get_image_cache() -> &'static Mutex<ImageCache> {
    IMAGE_CACHE.get_or_init(|| {
        Mutex::new(ImageCache {
            map: HashMap::new(),
            order: Vec::new(),
        })
    })
}

struct PasteGuard;

impl Drop for PasteGuard {
    fn drop(&mut self) {
        PASTING.store(false, Ordering::SeqCst);
    }
}

pub fn cache_image(path: String, rgba: Vec<u8>, width: u32, height: u32, png_bytes: Vec<u8>) {
    let mut cache = get_image_cache().lock().unwrap();
    // Evict oldest entries (deterministic insertion order)
    if cache.map.len() >= 30 {
        let evict_count = 15.min(cache.order.len());
        let evicted: Vec<String> = cache.order.drain(..evict_count).collect();
        for k in &evicted {
            cache.map.remove(k);
        }
    }
    cache.order.push(path.clone());
    cache.map.insert(
        path,
        CachedImage {
            rgba: Arc::new(rgba),
            width,
            height,
            png_bytes: Arc::new(png_bytes),
        },
    );
}

use enigo::{Direction, Enigo, Key, Keyboard, Settings};
use std::thread;
use std::time::Duration;
use tauri::{AppHandle, Manager};
use tauri_plugin_clipboard_manager::ClipboardExt;

type ClipboardImageData = (Arc<Vec<u8>>, u32, u32, Arc<Vec<u8>>);

fn load_image_for_clipboard(app: &AppHandle, path: &str) -> Result<ClipboardImageData, String> {
    {
        let cache = get_image_cache().lock().map_err(|e| e.to_string())?;
        if let Some(cached) = cache.map.get(path) {
            return Ok((
                cached.rgba.clone(),
                cached.width,
                cached.height,
                cached.png_bytes.clone(),
            ));
        }
    }

    let image_path = crate::db::get_storage_dir(app).join(path);
    let bytes = std::fs::read(&image_path)
        .map_err(|e| format!("read image {}: {e}", image_path.display()))?;
    let image = image::load_from_memory(&bytes).map_err(|e| format!("decode image: {e}"))?;
    let rgba = image.to_rgba8().into_raw();
    let (width, height) = (image.width(), image.height());
    let png_bytes = if image::guess_format(&bytes).ok() == Some(image::ImageFormat::Png) {
        bytes
    } else {
        let mut output = std::io::Cursor::new(Vec::new());
        image
            .write_to(&mut output, image::ImageFormat::Png)
            .map_err(|e| format!("encode clipboard PNG: {e}"))?;
        output.into_inner()
    };

    cache_image(
        path.to_string(),
        rgba.clone(),
        width,
        height,
        png_bytes.clone(),
    );
    Ok((Arc::new(rgba), width, height, Arc::new(png_bytes)))
}

fn paste_with_defocus(app: &AppHandle) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    unsafe {
        use windows::Win32::UI::WindowsAndMessaging::AllowSetForegroundWindow;
        let _ = AllowSetForegroundWindow(0xFFFFFFFF);
    }

    // Hide radial popup if visible
    if let Some(radial) = app.get_webview_window("radial-menu") {
        let _ = radial.hide();
    }

    let window = app.get_webview_window("main").ok_or("no window")?;

    if !window.is_always_on_top().unwrap_or(false) {
        window.hide().map_err(|e| e.to_string())?;
    }

    // Wait for modifiers from the popup gestures to be released before sending Ctrl+V.
    #[cfg(target_os = "windows")]
    {
        use windows::Win32::UI::Input::KeyboardAndMouse::{
            GetAsyncKeyState, VK_CONTROL, VK_LWIN, VK_MENU, VK_RWIN,
        };
        let start = std::time::Instant::now();
        let timeout = Duration::from_millis(500);
        loop {
            let ctrl_up = unsafe { (GetAsyncKeyState(VK_CONTROL.0 as i32) as u16) & 0x8000 } == 0;
            let alt_up = unsafe { (GetAsyncKeyState(VK_MENU.0 as i32) as u16) & 0x8000 } == 0;
            let win_up = unsafe {
                (GetAsyncKeyState(VK_LWIN.0 as i32) as u16) & 0x8000 == 0
                    && (GetAsyncKeyState(VK_RWIN.0 as i32) as u16) & 0x8000 == 0
            };
            if ctrl_up && alt_up && win_up {
                break;
            }
            if start.elapsed() > timeout {
                break;
            }
            thread::sleep(Duration::from_millis(10));
        }
        let restored = restore_foreground_target_and_wait();
        if !restored {
            log::warn!("[paste] target focus was not confirmed before Ctrl+V");
        }
        thread::sleep(Duration::from_millis(10));
    }

    #[cfg(not(target_os = "windows"))]
    {
        thread::sleep(Duration::from_millis(200));
    }

    let mut enigo = Enigo::new(&Settings::default()).map_err(|e| format!("enigo init: {}", e))?;

    #[cfg(target_os = "windows")]
    {
        enigo
            .key(Key::Control, Direction::Press)
            .map_err(|e| e.to_string())?;
        thread::sleep(Duration::from_millis(30));
        enigo
            .key(Key::V, Direction::Click)
            .map_err(|e| e.to_string())?;
        thread::sleep(Duration::from_millis(10));
        enigo
            .key(Key::Control, Direction::Release)
            .map_err(|e| e.to_string())?;
    }

    #[cfg(target_os = "macos")]
    {
        enigo
            .key(Key::Meta, Direction::Press)
            .map_err(|e| e.to_string())?;
        thread::sleep(Duration::from_millis(30));
        enigo
            .key(Key::V, Direction::Press)
            .map_err(|e| e.to_string())?;
        thread::sleep(Duration::from_millis(10));
        enigo
            .key(Key::V, Direction::Release)
            .map_err(|e| e.to_string())?;
        thread::sleep(Duration::from_millis(10));
        enigo
            .key(Key::Meta, Direction::Release)
            .map_err(|e| e.to_string())?;
    }

    Ok(())
}

#[cfg(target_os = "windows")]
fn build_image_html(png_bytes: &[u8]) -> Vec<u8> {
    let b64 = base64::engine::general_purpose::STANDARD.encode(png_bytes);
    let img_tag = format!("<img src=\"data:image/png;base64,{}\"/>", b64);
    let fragment = format!("<!--StartFragment-->{}<!--EndFragment-->", img_tag);
    let html_body = format!("<html><body>{}</body></html>", fragment);

    // Build a template header with placeholder zeros to measure its exact length
    let placeholder_header = "Version:0.9\r\nStartHTML:00000000\r\nEndHTML:00000000\r\nStartFragment:00000000\r\nEndFragment:00000000\r\n";
    let header_len = placeholder_header.len();

    // Offsets are byte positions in the combined data (header + body)
    let start_html = header_len;
    let end_html = header_len + html_body.len();
    let start_frag = header_len + html_body.find(&fragment).unwrap_or(0);
    let end_frag =
        header_len + html_body.find("<!--EndFragment-->").unwrap_or(0) + "<!--EndFragment-->".len();

    let header = format!(
        "Version:0.9\r\nStartHTML:{:08}\r\nEndHTML:{:08}\r\nStartFragment:{:08}\r\nEndFragment:{:08}\r\n",
        start_html, end_html, start_frag, end_frag,
    );

    let mut result = header.into_bytes();
    result.extend_from_slice(html_body.as_bytes());
    result
}

#[cfg(target_os = "windows")]
fn write_image_to_clipboard(rgba: &[u8], w: u32, h: u32, png_bytes: &[u8]) -> Result<(), String> {
    use windows::Win32::Foundation::{HANDLE, HWND};
    use windows::Win32::System::DataExchange::*;
    use windows::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE};
    use windows::Win32::UI::Shell::DROPFILES;

    const CF_DIB: u32 = 8;
    const CF_HDROP: u32 = 15;

    // Write PNG to a temp file for CF_HDROP
    let temp_png_path = {
        let mut dir = std::env::temp_dir();
        dir.push("copy_creator_paste");
        std::fs::create_dir_all(&dir).ok();
        dir.push(format!("paste_{}.png", uuid::Uuid::new_v4()));
        std::fs::write(&dir, png_bytes).map_err(|e| format!("Temp file write: {}", e))?;
        dir
    };
    let wide_path: Vec<u16> = temp_png_path
        .to_string_lossy()
        .encode_utf16()
        .chain(std::iter::once(0u16))
        .collect();

    unsafe {
        if OpenClipboard(HWND(std::ptr::null_mut())).is_err() {
            return Err("OpenClipboard failed".to_string());
        }
        let _ = EmptyClipboard();

        let dib_size = 40 + (w * h * 4) as usize;
        let hmem_dib = GlobalAlloc(GMEM_MOVEABLE, dib_size).map_err(|e| {
            let _ = CloseClipboard();
            format!("GlobalAlloc DIB failed: {}", e)
        })?;

        let ptr_dib = GlobalLock(hmem_dib);
        if ptr_dib.is_null() {
            let _ = CloseClipboard();
            return Err("GlobalLock DIB failed".to_string());
        }

        let bmi = ptr_dib as *mut u8;
        // Zero the DIB header to avoid garbage biCompression / biClrUsed etc.
        std::ptr::write_bytes(bmi, 0u8, 40);
        let bmi_header = std::slice::from_raw_parts_mut(bmi as *mut u32, 10);
        bmi_header[0] = 40;
        bmi_header[1] = w;
        bmi_header[2] = (-(h as i32)) as u32;
        *(bmi.add(12) as *mut u16) = 1;
        *(bmi.add(14) as *mut u16) = 32;
        *(bmi.add(20) as *mut u32) = w * h * 4;

        // Convert RGBA → BGRA (DIB expects BGRA pixel order)
        let pixel_offset = 40;
        let dst = bmi.add(pixel_offset);
        let src = rgba.as_ptr();
        for i in 0..(w * h) as usize {
            *dst.add(i * 4) = *src.add(i * 4 + 2); // B = R
            *dst.add(i * 4 + 1) = *src.add(i * 4 + 1); // G = G
            *dst.add(i * 4 + 2) = *src.add(i * 4); // R = B
            *dst.add(i * 4 + 3) = *src.add(i * 4 + 3); // A = A
        }
        let _ = GlobalUnlock(hmem_dib);

        if SetClipboardData(CF_DIB, HANDLE(hmem_dib.0)).is_err() {
            let _ = CloseClipboard();
            return Err("SetClipboardData DIB failed".to_string());
        }

        let png_format_name: Vec<u16> = "PNG\0".encode_utf16().collect();
        let cf_png = RegisterClipboardFormatW(windows::core::PCWSTR(png_format_name.as_ptr()));
        if cf_png != 0 {
            let hmem_png = GlobalAlloc(GMEM_MOVEABLE, png_bytes.len()).map_err(|e| {
                let _ = CloseClipboard();
                format!("GlobalAlloc PNG failed: {}", e)
            })?;

            let ptr_png = GlobalLock(hmem_png);
            if ptr_png.is_null() {
                let _ = CloseClipboard();
                return Err("GlobalLock PNG failed".to_string());
            }

            std::ptr::copy_nonoverlapping(png_bytes.as_ptr(), ptr_png as *mut u8, png_bytes.len());
            let _ = GlobalUnlock(hmem_png);

            if SetClipboardData(cf_png, HANDLE(hmem_png.0)).is_err() {
                let _ = CloseClipboard();
                return Err("SetClipboardData PNG failed".to_string());
            }
        }

        // Write HTML format for Electron/Chromium-based apps (Feishu, DingTalk, etc.)
        let html_data = build_image_html(png_bytes);
        let html_format_name: Vec<u16> = "HTML Format\0".encode_utf16().collect();
        let cf_html = RegisterClipboardFormatW(windows::core::PCWSTR(html_format_name.as_ptr()));
        if cf_html != 0 {
            let hmem_html = GlobalAlloc(GMEM_MOVEABLE, html_data.len()).map_err(|e| {
                let _ = CloseClipboard();
                format!("GlobalAlloc HTML failed: {}", e)
            })?;
            let ptr_html = GlobalLock(hmem_html);
            if ptr_html.is_null() {
                let _ = CloseClipboard();
                return Err("GlobalLock HTML failed".to_string());
            }
            std::ptr::copy_nonoverlapping(html_data.as_ptr(), ptr_html as *mut u8, html_data.len());
            let _ = GlobalUnlock(hmem_html);
            if SetClipboardData(cf_html, HANDLE(hmem_html.0)).is_err() {
                let _ = CloseClipboard();
                return Err("SetClipboardData HTML failed".to_string());
            }
        }

        // Write CF_HDROP (temp file path) — required by Electron/Chromium apps
        {
            let dropfiles_size = std::mem::size_of::<DROPFILES>();
            let path_bytes = wide_path.len() * std::mem::size_of::<u16>();
            let data_size = dropfiles_size + path_bytes;
            let mut data: Vec<u8> = vec![0u8; data_size];
            let df = data.as_mut_ptr() as *mut DROPFILES;
            (*df).pFiles = dropfiles_size as u32;
            (*df).pt = windows::Win32::Foundation::POINT { x: 0, y: 0 };
            (*df).fNC = windows::Win32::Foundation::BOOL(0);
            (*df).fWide = windows::Win32::Foundation::BOOL(1);
            let dest = data.as_mut_ptr().add(dropfiles_size) as *mut u16;
            std::ptr::copy_nonoverlapping(wide_path.as_ptr(), dest, wide_path.len());
            let hmem_drop = GlobalAlloc(GMEM_MOVEABLE, data_size).map_err(|e| {
                let _ = CloseClipboard();
                format!("GlobalAlloc HDROP failed: {}", e)
            })?;
            let ptr_drop = GlobalLock(hmem_drop);
            if ptr_drop.is_null() {
                let _ = CloseClipboard();
                return Err("GlobalLock HDROP failed".to_string());
            }
            std::ptr::copy_nonoverlapping(data.as_ptr(), ptr_drop as *mut u8, data_size);
            let _ = GlobalUnlock(hmem_drop);
            if SetClipboardData(CF_HDROP, HANDLE(hmem_drop.0)).is_err() {
                let _ = CloseClipboard();
                return Err("SetClipboardData HDROP failed".to_string());
            }
        }

        let _ = CloseClipboard();
    }

    Ok(())
}

#[cfg(target_os = "windows")]
fn write_files_to_clipboard(paths: &[String]) -> Result<(), String> {
    use windows::Win32::Foundation::{HANDLE, HWND};
    use windows::Win32::System::DataExchange::*;
    use windows::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE};
    use windows::Win32::UI::Shell::DROPFILES;

    const CF_HDROP: u32 = 15;

    let wide_paths: Vec<Vec<u16>> = paths
        .iter()
        .map(|p| p.encode_utf16().chain(std::iter::once(0u16)).collect())
        .collect();
    let total_wide_len: usize = wide_paths.iter().map(|p| p.len()).sum();

    let dropfiles_size = std::mem::size_of::<DROPFILES>();
    let data_size = dropfiles_size + (total_wide_len + 1) * std::mem::size_of::<u16>();

    let mut data: Vec<u8> = vec![0u8; data_size];

    let df = data.as_mut_ptr() as *mut DROPFILES;
    unsafe {
        (*df).pFiles = dropfiles_size as u32;
        (*df).pt = windows::Win32::Foundation::POINT { x: 0, y: 0 };
        (*df).fNC = windows::Win32::Foundation::BOOL(0);
        (*df).fWide = windows::Win32::Foundation::BOOL(1);
    }

    let offset = dropfiles_size;
    let mut pos = offset;
    for wp in &wide_paths {
        let byte_len = wp.len() * std::mem::size_of::<u16>();
        data[pos..pos + byte_len].copy_from_slice(unsafe {
            std::slice::from_raw_parts(wp.as_ptr() as *const u8, byte_len)
        });
        pos += byte_len;
    }

    unsafe {
        if OpenClipboard(HWND(std::ptr::null_mut())).is_err() {
            return Err("OpenClipboard failed".to_string());
        }
        let _ = EmptyClipboard();

        let hmem = GlobalAlloc(GMEM_MOVEABLE, data_size).map_err(|e| {
            let _ = CloseClipboard();
            format!("GlobalAlloc failed: {}", e)
        })?;

        let ptr = GlobalLock(hmem);
        if ptr.is_null() {
            let _ = CloseClipboard();
            return Err("GlobalLock failed".to_string());
        }

        std::ptr::copy_nonoverlapping(data.as_ptr(), ptr as *mut u8, data_size);
        let _ = GlobalUnlock(hmem);

        if SetClipboardData(CF_HDROP, HANDLE(hmem.0)).is_err() {
            let _ = CloseClipboard();
            return Err("SetClipboardData failed".to_string());
        }

        let _ = CloseClipboard();
    }

    Ok(())
}

fn write_text(app: AppHandle, text: String, paste_after: bool) -> Result<(), String> {
    if PASTING.swap(true, Ordering::SeqCst) {
        return Ok(());
    }

    let guard = PasteGuard;

    if let Err(e) = app.clipboard().write_text(text) {
        drop(guard);
        return Err(e.to_string());
    }

    crate::clipboard::sync_monitor_cache(&app);

    if paste_after {
        let handle = app.clone();
        std::thread::spawn(move || {
            let _guard = guard;
            paste_with_defocus(&handle).ok();
        });
    } else {
        drop(guard);
    }

    Ok(())
}

#[tauri::command]
pub fn copy_text(app: AppHandle, text: String) -> Result<(), String> {
    write_text(app, text, false)
}

#[tauri::command]
pub fn paste_text(app: AppHandle, text: String) -> Result<(), String> {
    write_text(app, text, true)
}

fn write_image(app: AppHandle, path: String, paste_after: bool) -> Result<(), String> {
    if PASTING.swap(true, Ordering::SeqCst) {
        return Ok(());
    }

    let handle = app.clone();
    std::thread::spawn(move || {
        let _guard = PasteGuard;
        let (rgba, w, h, png) = match load_image_for_clipboard(&handle, &path) {
            Ok(image) => image,
            Err(error) => {
                log::error!("write_image: {error}");
                return;
            }
        };

        #[cfg(target_os = "windows")]
        {
            if let Err(e) = write_image_to_clipboard(&rgba, w, h, &png) {
                log::error!("write_image: clipboard error: {e}");
                return;
            }
        }

        #[cfg(not(target_os = "windows"))]
        {
            let tauri_img = tauri::image::Image::new_owned(rgba.to_vec(), w, h);
            if let Err(e) = handle.clipboard().write_image(&tauri_img) {
                log::error!("paste_image: write clipboard error: {}", e);
                return;
            }
        }

        crate::clipboard::sync_monitor_cache(&handle);
        if paste_after {
            paste_with_defocus(&handle).ok();
        }
    });

    Ok(())
}

#[tauri::command]
pub fn copy_image(app: AppHandle, path: String) -> Result<(), String> {
    write_image(app, path, false)
}

#[tauri::command]
pub fn paste_image(app: AppHandle, path: String) -> Result<(), String> {
    write_image(app, path, true)
}

fn write_file(app: AppHandle, path: String, paste_after: bool) -> Result<(), String> {
    if PASTING.swap(true, Ordering::SeqCst) {
        return Ok(());
    }

    let file_meta = std::fs::metadata(&path);
    if file_meta.is_err() {
        log::error!("write_file: file not found: {}", path);
        PASTING.store(false, Ordering::SeqCst);
        return Err(format!("File not found: {}", path));
    }

    let handle = app.clone();
    std::thread::spawn(move || {
        let _guard = PasteGuard;

        #[cfg(target_os = "windows")]
        {
            if let Err(e) = write_files_to_clipboard(std::slice::from_ref(&path)) {
                log::error!("write_file: clipboard error: {}", e);
                return;
            }
        }

        #[cfg(not(target_os = "windows"))]
        {
            if let Err(e) = handle.clipboard().write_text(&path) {
                log::error!("paste_file: write clipboard error: {}", e);
                return;
            }
        }

        crate::clipboard::sync_monitor_cache(&handle);
        if paste_after {
            paste_with_defocus(&handle).ok();
        }
    });

    Ok(())
}

#[tauri::command]
pub fn copy_file(app: AppHandle, path: String) -> Result<(), String> {
    write_file(app, path, false)
}

#[tauri::command]
pub fn paste_file(app: AppHandle, path: String) -> Result<(), String> {
    write_file(app, path, true)
}

fn run_record_action(app: &AppHandle, id: &str, paste_after: bool) -> Result<(), String> {
    let record = crate::db::get_clipboard_action_record(app, id)?;
    match (record.record_type.as_str(), paste_after) {
        ("image", false) => copy_image(app.clone(), record.content),
        ("image", true) => paste_image(app.clone(), record.content),
        ("file", false) => copy_file(app.clone(), record.content),
        ("file", true) => paste_file(app.clone(), record.content),
        (_, false) => copy_text(app.clone(), record.content),
        (_, true) => paste_text(app.clone(), record.content),
    }
}

pub fn copy_record_by_id(app: &AppHandle, id: &str) -> Result<(), String> {
    run_record_action(app, id, false)
}

pub fn paste_record_by_id(app: &AppHandle, id: &str) -> Result<(), String> {
    run_record_action(app, id, true)
}
