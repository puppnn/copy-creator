use std::sync::atomic::{AtomicBool, AtomicI32, AtomicPtr, AtomicU64, Ordering};
#[cfg(target_os = "windows")]
use std::sync::mpsc::{sync_channel, SyncSender, TrySendError};
use std::sync::OnceLock;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_global_shortcut::GlobalShortcutExt;

static RADIAL_MENU_ENABLED: AtomicBool = AtomicBool::new(true);

#[cfg(target_os = "windows")]
use windows::Win32::Foundation::*;
#[cfg(target_os = "windows")]
use windows::Win32::UI::Input::KeyboardAndMouse::*;
#[cfg(target_os = "windows")]
use windows::Win32::UI::WindowsAndMessaging::*;

#[cfg(target_os = "windows")]
static APP_HANDLE: OnceLock<AppHandle> = OnceLock::new();
#[cfg(target_os = "windows")]
static HOOK_HANDLE: AtomicPtr<core::ffi::c_void> = AtomicPtr::new(core::ptr::null_mut());
#[cfg(target_os = "windows")]
static KEYBOARD_HOOK_HANDLE: AtomicPtr<core::ffi::c_void> = AtomicPtr::new(core::ptr::null_mut());
#[cfg(target_os = "windows")]
static WIN_V_DOWN: AtomicBool = AtomicBool::new(false);
#[cfg(target_os = "windows")]
static SHOW_CLIPBOARD_SENDER: OnceLock<SyncSender<()>> = OnceLock::new();
#[cfg(target_os = "windows")]
static SHOW_CLIPBOARD_PENDING: AtomicBool = AtomicBool::new(false);

static TOGGLING: AtomicBool = AtomicBool::new(false);

#[cfg(any(target_os = "windows", test))]
fn should_intercept_win_v(vk_code: u32, win: bool, ctrl: bool, shift: bool, alt: bool) -> bool {
    vk_code == b'V' as u32 && win && !ctrl && !shift && !alt
}

#[cfg(test)]
mod tests {
    use super::should_intercept_win_v;

    #[test]
    fn intercepts_only_plain_win_v() {
        assert!(should_intercept_win_v(
            b'V' as u32,
            true,
            false,
            false,
            false
        ));

        for modifiers in [
            (b'C' as u32, true, false, false, false),
            (b'V' as u32, false, false, false, false),
            (b'V' as u32, true, true, false, false),
            (b'V' as u32, true, false, true, false),
            (b'V' as u32, true, false, false, true),
        ] {
            assert!(!should_intercept_win_v(
                modifiers.0,
                modifiers.1,
                modifiers.2,
                modifiers.3,
                modifiers.4,
            ));
        }
    }
}

/// RAII guard that ensures TOGGLING is always reset, even on panic.
struct ToggleGuard;

impl Drop for ToggleGuard {
    fn drop(&mut self) {
        TOGGLING.store(false, Ordering::SeqCst);
    }
}

#[cfg(target_os = "windows")]
static RADIAL_RIGHT_DOWN: AtomicBool = AtomicBool::new(false);
#[cfg(target_os = "windows")]
static RADIAL_START_X: AtomicI32 = AtomicI32::new(0);
#[cfg(target_os = "windows")]
static RADIAL_START_Y: AtomicI32 = AtomicI32::new(0);
#[cfg(target_os = "windows")]
static LAST_MOVE_EMIT_MS: AtomicU64 = AtomicU64::new(0);

const MOVE_THROTTLE_MS: u64 = 16;

#[derive(serde::Serialize, Clone)]
struct RadialMenuPoint {
    x: i32,
    y: i32,
}

#[derive(serde::Serialize, Clone)]
struct RadialMenuDownPayload {
    x: i32,
    y: i32,
    theme: String,
}

pub fn toggle_window(app: &AppHandle) {
    if TOGGLING.swap(true, Ordering::SeqCst) {
        log::info!("[toggle_window] skipped (re-entrant)");
        return;
    }
    let _guard = ToggleGuard;

    if let Some(window) = app.get_webview_window("main") {
        let visible = window.is_visible().unwrap_or(false);
        log::info!("[toggle_window] visible={}", visible);

        if visible {
            log::info!("[toggle_window] hiding window");
            let _ = window.hide();
        } else {
            #[cfg(target_os = "windows")]
            {
                crate::paste::save_foreground_window();
                // Allow our own process (or any process) to call SetForegroundWindow.
                // The thread has temporary foreground permission from the hotkey / hook
                // input, so this ASFW call makes SetForegroundWindow bulletproof.
                unsafe {
                    use windows::Win32::UI::WindowsAndMessaging::AllowSetForegroundWindow;
                    let _ = AllowSetForegroundWindow(0xFFFFFFFF);
                }
            }

            log::info!("[toggle_window] showing window");
            let _ = window.show();
            let _ = window.set_focus();
        }
    } else {
        log::warn!("[toggle_window] main window not found");
    }
}

pub fn show_clipboard_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = app.emit("show-clipboard", ());
        let _ = window.show();
        let _ = window.set_focus();
    } else {
        log::warn!("[show_clipboard_window] main window not found");
    }
}

#[cfg(target_os = "windows")]
fn install_show_clipboard_worker(app: &AppHandle) -> Result<(), String> {
    if SHOW_CLIPBOARD_SENDER.get().is_some() {
        return Ok(());
    }

    let (sender, receiver) = sync_channel(1);
    let app = app.clone();
    std::thread::Builder::new()
        .name("clipboard-shortcut-worker".to_string())
        .spawn(move || {
            while receiver.recv().is_ok() {
                show_clipboard_window(&app);
                SHOW_CLIPBOARD_PENDING.store(false, Ordering::SeqCst);
            }
        })
        .map_err(|error| format!("Failed to start clipboard shortcut worker: {error}"))?;

    SHOW_CLIPBOARD_SENDER
        .set(sender)
        .map_err(|_| "Clipboard shortcut worker already initialized".to_string())
}

#[cfg(target_os = "windows")]
fn queue_show_clipboard_window() {
    if SHOW_CLIPBOARD_PENDING.swap(true, Ordering::SeqCst) {
        return;
    }

    let Some(sender) = SHOW_CLIPBOARD_SENDER.get() else {
        SHOW_CLIPBOARD_PENDING.store(false, Ordering::SeqCst);
        return;
    };

    match sender.try_send(()) {
        Ok(()) | Err(TrySendError::Full(())) => {}
        Err(TrySendError::Disconnected(())) => {
            SHOW_CLIPBOARD_PENDING.store(false, Ordering::SeqCst);
            log::error!("Clipboard shortcut worker disconnected");
        }
    }
}

#[cfg(target_os = "windows")]
fn screen_to_css(window: &tauri::WebviewWindow, screen_x: i32, screen_y: i32) -> Option<(i32, i32)> {
    let win_pos = window.outer_position().ok()?;
    let scale = window.scale_factor().ok().unwrap_or(1.0);
    let rel_x = ((screen_x - win_pos.x) as f64 / scale).round() as i32;
    let rel_y = ((screen_y - win_pos.y) as f64 / scale).round() as i32;
    Some((rel_x, rel_y))
}

#[cfg(target_os = "windows")]
unsafe extern "system" fn mouse_hook_callback(
    n_code: i32,
    w_param: WPARAM,
    l_param: LPARAM,
) -> LRESULT {
    if n_code >= 0 {
        let msg = w_param.0 as u32;

        if msg == WM_RBUTTONDOWN {
            let ctrl = (GetAsyncKeyState(VK_CONTROL.0 as i32) as u16) & 0x8000 != 0;
            let shift = (GetAsyncKeyState(VK_SHIFT.0 as i32) as u16) & 0x8000 != 0;
            let alt = (GetAsyncKeyState(VK_MENU.0 as i32) as u16) & 0x8000 != 0;

            if ctrl && shift {
                if let Some(app) = APP_HANDLE.get() {
                    toggle_window(app);
                }
                return LRESULT(1);
            }

            if ctrl && alt && !shift {
                if !RADIAL_MENU_ENABLED.load(Ordering::SeqCst) {
                    let hook = HHOOK(HOOK_HANDLE.load(Ordering::SeqCst));
                    return unsafe { CallNextHookEx(hook, n_code, w_param, l_param) };
                }
                if let Some(app) = APP_HANDLE.get() {
                    if let Some(window) = app.get_webview_window("radial-menu") {
                        crate::paste::save_foreground_window();

                        let hook_struct = &*(l_param.0 as *const MSLLHOOKSTRUCT);
                        let sx = hook_struct.pt.x;
                        let sy = hook_struct.pt.y;

                        let scale = window.scale_factor().unwrap_or(1.0);
                        let half_w = (150.0 * scale) as i32;
                        let top_off = (30.0 * scale) as i32;

                        // Pre-calc CSS coords before positioning (avoids stale outer_position)
                        let css_x = ((half_w as f64) / scale).round() as i32;
                        let css_y = ((top_off as f64) / scale).round() as i32;

                        let _ = window.set_position(tauri::Position::Physical(
                            tauri::PhysicalPosition::new(sx - half_w, sy - top_off),
                        ));

                        RADIAL_RIGHT_DOWN.store(true, Ordering::SeqCst);
                        RADIAL_START_X.store(sx, Ordering::SeqCst);
                        RADIAL_START_Y.store(sy, Ordering::SeqCst);

                        let theme = crate::db::get_setting(app.clone(), "theme".to_string())
                            .unwrap_or_else(|_| "light".to_string());

                        log::info!("radial-menu-down: screen=({}, {}), css=({}, {}), theme={}", sx, sy, css_x, css_y, theme);
                        let _ = app.emit(
                            "radial-menu-down",
                            RadialMenuDownPayload { x: css_x, y: css_y, theme },
                        );

                        let _ = window.show();
                        let _ = window.set_focus();
                    }
                }
                return LRESULT(1);
            }
        }

        if msg == WM_MOUSEMOVE && RADIAL_RIGHT_DOWN.load(Ordering::SeqCst) {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64;
            let last = LAST_MOVE_EMIT_MS.load(Ordering::SeqCst);
            if now.saturating_sub(last) >= MOVE_THROTTLE_MS {
                LAST_MOVE_EMIT_MS.store(now, Ordering::SeqCst);

                if let Some(app) = APP_HANDLE.get() {
                    if let Some(window) = app.get_webview_window("radial-menu") {
                        let hook_struct = &*(l_param.0 as *const MSLLHOOKSTRUCT);
                        let sx = hook_struct.pt.x;
                        let sy = hook_struct.pt.y;

                        if let Some((cx, cy)) = screen_to_css(&window, sx, sy) {
                            let _ = app.emit(
                                "radial-menu-move",
                                RadialMenuPoint { x: cx, y: cy },
                            );
                        }
                    }
                }
            }
        }

        if msg == WM_RBUTTONUP && RADIAL_RIGHT_DOWN.load(Ordering::SeqCst) {
            RADIAL_RIGHT_DOWN.store(false, Ordering::SeqCst);
            log::info!("radial-menu-up");

            if let Some(app) = APP_HANDLE.get() {
                let _ = app.emit("radial-menu-up", ());
            }
            return LRESULT(1);
        }
    }

    let hook = HHOOK(HOOK_HANDLE.load(Ordering::SeqCst));
    unsafe { CallNextHookEx(hook, n_code, w_param, l_param) }
}

#[cfg(target_os = "windows")]
unsafe extern "system" fn keyboard_hook_callback(
    n_code: i32,
    w_param: WPARAM,
    l_param: LPARAM,
) -> LRESULT {
    if n_code >= 0 {
        let msg = w_param.0 as u32;
        let hook_struct = &*(l_param.0 as *const KBDLLHOOKSTRUCT);

        if hook_struct.vkCode == VK_V.0 as u32 {
            let is_key_down = msg == WM_KEYDOWN || msg == WM_SYSKEYDOWN;
            let is_key_up = msg == WM_KEYUP || msg == WM_SYSKEYUP;

            if is_key_up && WIN_V_DOWN.swap(false, Ordering::SeqCst) {
                return LRESULT(1);
            }

            if is_key_down {
                if WIN_V_DOWN.load(Ordering::SeqCst) {
                    return LRESULT(1);
                }

                let win = (GetAsyncKeyState(VK_LWIN.0 as i32) as u16) & 0x8000 != 0
                    || (GetAsyncKeyState(VK_RWIN.0 as i32) as u16) & 0x8000 != 0;
                let ctrl = (GetAsyncKeyState(VK_CONTROL.0 as i32) as u16) & 0x8000 != 0;
                let shift = (GetAsyncKeyState(VK_SHIFT.0 as i32) as u16) & 0x8000 != 0;
                let alt = (GetAsyncKeyState(VK_MENU.0 as i32) as u16) & 0x8000 != 0;

                if should_intercept_win_v(hook_struct.vkCode, win, ctrl, shift, alt) {
                    WIN_V_DOWN.store(true, Ordering::SeqCst);
                    let _ = AllowSetForegroundWindow(0xFFFFFFFF);
                    queue_show_clipboard_window();
                    return LRESULT(1);
                }
            }
        }
    }

    let hook = HHOOK(KEYBOARD_HOOK_HANDLE.load(Ordering::SeqCst));
    unsafe { CallNextHookEx(hook, n_code, w_param, l_param) }
}

pub fn install_mouse_hook(app: &AppHandle) {
    #[cfg(target_os = "windows")]
    {
        // Restore persisted radial menu enabled state
        if let Ok(val) = crate::db::get_setting(app.clone(), "radial_menu_enabled".to_string()) {
            RADIAL_MENU_ENABLED.store(val == "1", Ordering::SeqCst);
        }

        APP_HANDLE.set(app.clone()).ok();
        let hook = unsafe {
            SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_hook_callback), None, 0)
        };
        if let Ok(h) = hook {
            HOOK_HANDLE.store(h.0, Ordering::SeqCst);
            log::info!("Global mouse hook installed (Ctrl+Shift+RightClick / Ctrl+Alt+RightClick)");
        } else {
            log::warn!("Failed to install mouse hook");
        }
    }
}

pub fn install_keyboard_hook(app: &AppHandle) {
    #[cfg(target_os = "windows")]
    {
        APP_HANDLE.set(app.clone()).ok();
        if let Err(error) = install_show_clipboard_worker(app) {
            log::error!("{error}");
            return;
        }
        let hook =
            unsafe { SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_hook_callback), None, 0) };
        if let Ok(h) = hook {
            KEYBOARD_HOOK_HANDLE.store(h.0, Ordering::SeqCst);
            log::info!("Global keyboard hook installed (Win+V)");
        } else {
            log::warn!("Failed to install keyboard hook for Win+V");
        }
    }
}

pub fn register_keyboard_shortcut(
    app: &AppHandle,
    shortcut: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    if shortcut.is_empty() {
        return Ok(());
    }
    app.global_shortcut().register(shortcut)?;
    Ok(())
}

pub fn unregister_keyboard_shortcut(
    app: &AppHandle,
    shortcut: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    if shortcut.is_empty() {
        return Ok(());
    }
    let _ = app.global_shortcut().unregister(shortcut);
    Ok(())
}

#[tauri::command]
pub fn update_shortcut(
    app: AppHandle,
    old_shortcut: String,
    new_shortcut: String,
) -> Result<(), String> {
    if !old_shortcut.is_empty() {
        let _ = unregister_keyboard_shortcut(&app, &old_shortcut);
    }
    if !new_shortcut.is_empty() {
        register_keyboard_shortcut(&app, &new_shortcut)
            .map_err(|e| format!("Failed to register shortcut: {}", e))?;
    }
    Ok(())
}

#[tauri::command]
pub fn set_radial_menu_enabled(app: AppHandle, enabled: bool) -> Result<(), String> {
    RADIAL_MENU_ENABLED.store(enabled, Ordering::SeqCst);
    let state = app.state::<crate::db::DbState>();
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT INTO settings (key, value) VALUES ('radial_menu_enabled', ?1) ON CONFLICT(key) DO UPDATE SET value = ?1",
        rusqlite::params![if enabled { "1" } else { "0" }],
    ).map_err(|e| e.to_string())?;
    Ok(())
}

