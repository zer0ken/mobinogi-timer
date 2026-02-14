use serde::{Deserialize, Serialize};
use std::fs;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tauri::{AppHandle, Emitter, Manager, WindowEvent};

// --- Settings ---

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Settings {
    duration_secs: f64,
    cooldown_secs: f64,
    #[serde(default = "default_hotkey_vk")]
    hotkey_vk: u32,
    #[serde(default = "default_hotkey_name")]
    hotkey_name: String,
    #[serde(default = "default_overlay_x")]
    overlay_x: f64,
    #[serde(default = "default_overlay_y")]
    overlay_y: f64,
    #[serde(default = "default_overlay_opacity")]
    overlay_opacity: f64,
    #[serde(default = "default_auto_repeat")]
    auto_repeat: bool,
}

fn default_hotkey_vk() -> u32 {
    0xC0
}

fn default_hotkey_name() -> String {
    "`".to_string()
}

fn default_overlay_x() -> f64 {
    760.0
}

fn default_overlay_y() -> f64 {
    20.0
}

fn default_overlay_opacity() -> f64 {
    0.85
}

fn default_auto_repeat() -> bool {
    true
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            duration_secs: 30.0,
            cooldown_secs: 60.0,
            hotkey_vk: default_hotkey_vk(),
            hotkey_name: default_hotkey_name(),
            overlay_x: default_overlay_x(),
            overlay_y: default_overlay_y(),
            overlay_opacity: default_overlay_opacity(),
            auto_repeat: default_auto_repeat(),
        }
    }
}

fn settings_path() -> std::path::PathBuf {
    let dir = dirs::config_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("mobinogi-timer");
    fs::create_dir_all(&dir).ok();
    dir.join("settings.json")
}

fn load_settings() -> Settings {
    fs::read_to_string(settings_path())
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn save_settings_to_file(settings: &Settings) {
    if let Ok(json) = serde_json::to_string_pretty(settings) {
        fs::write(settings_path(), json).ok();
    }
}

// --- Timer State ---

#[derive(Debug, Clone, PartialEq)]
enum TimerPhase {
    Idle,
    Duration,
    Cooldown,
}

#[derive(Debug, Clone)]
struct TimerState {
    phase: TimerPhase,
    start_time: Option<Instant>,
    settings: Settings,
}

impl TimerState {
    fn new(settings: Settings) -> Self {
        Self {
            phase: TimerPhase::Idle,
            start_time: None,
            settings,
        }
    }

    fn start(&mut self) {
        self.phase = TimerPhase::Duration;
        self.start_time = Some(Instant::now());
    }

    fn tick(&mut self) -> (String, f64, f64) {
        let Some(start) = self.start_time else {
            return ("idle".into(), 0.0, 0.0);
        };

        let elapsed = start.elapsed().as_secs_f64();
        let duration = self.settings.duration_secs;
        let total = self.settings.cooldown_secs;

        match self.phase {
            TimerPhase::Idle => ("idle".into(), 0.0, 0.0),
            TimerPhase::Duration => {
                if elapsed >= duration {
                    self.phase = TimerPhase::Cooldown;
                    let remaining = total - elapsed;
                    let pct = (elapsed / total) * 100.0;
                    ("cooldown".into(), pct, remaining.max(0.0))
                } else {
                    let pct = (1.0 - elapsed / duration) * 100.0;
                    let remaining = duration - elapsed;
                    ("duration".into(), pct, remaining.max(0.0))
                }
            }
            TimerPhase::Cooldown => {
                if elapsed >= total {
                    if self.settings.auto_repeat {
                        self.phase = TimerPhase::Duration;
                        self.start_time = Some(Instant::now());
                        let pct = (1.0 - 0.0 / duration) * 100.0;
                        ("duration".into(), pct, duration)
                    } else {
                        self.phase = TimerPhase::Idle;
                        self.start_time = None;
                        ("idle".into(), 0.0, 0.0)
                    }
                } else {
                    let pct = (elapsed / total) * 100.0;
                    let remaining = total - elapsed;
                    ("cooldown".into(), pct, remaining.max(0.0))
                }
            }
        }
    }
}

// --- Tauri Commands ---

#[tauri::command]
fn get_settings(state: tauri::State<'_, Arc<Mutex<TimerState>>>) -> Settings {
    state.lock().unwrap().settings.clone()
}

#[tauri::command]
fn save_settings(
    app: AppHandle,
    state: tauri::State<'_, Arc<Mutex<TimerState>>>,
    duration_secs: f64,
    cooldown_secs: f64,
    hotkey_vk: u32,
    hotkey_name: String,
    overlay_opacity: f64,
    auto_repeat: bool,
) {
    let current_settings = state.lock().unwrap().settings.clone();
    let new_settings = Settings {
        duration_secs,
        cooldown_secs,
        hotkey_vk,
        hotkey_name,
        overlay_x: current_settings.overlay_x,
        overlay_y: current_settings.overlay_y,
        overlay_opacity,
        auto_repeat,
    };
    save_settings_to_file(&new_settings);
    HOTKEY_VK.store(hotkey_vk, Ordering::Relaxed);
    {
        let mut timer = state.lock().unwrap();
        timer.settings = new_settings;
    }
    app.emit("settings-updated", ()).ok();
}

// --- Global flag for hotkey press (set by keyboard hook) ---
static HOTKEY_PRESSED: AtomicBool = AtomicBool::new(false);
static HOTKEY_VK: AtomicU32 = AtomicU32::new(0xC0);

// --- App Setup ---

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let settings = load_settings();
    HOTKEY_VK.store(settings.hotkey_vk, Ordering::Relaxed);
    let timer_state = Arc::new(Mutex::new(TimerState::new(settings)));

    tauri::Builder::default()
        .manage(timer_state.clone())
        .invoke_handler(tauri::generate_handler![get_settings, save_settings])
        .setup(move |app| {
            let handle = app.handle().clone();

            // --- Global keyboard hook (works without focus, even with IME) ---
            install_keyboard_hook();

            // --- Timer tick loop (16ms) ---
            let tick_state = timer_state.clone();
            let tick_handle = handle.clone();
            std::thread::spawn(move || loop {
                std::thread::sleep(std::time::Duration::from_millis(16));

                // Check if hotkey was pressed via keyboard hook
                if HOTKEY_PRESSED.swap(false, Ordering::Relaxed) {
                    let mut timer = tick_state.lock().unwrap();
                    timer.start();
                    eprintln!("[mobinogi] Hotkey pressed - timer started");
                }

                let (phase_str, percent, remaining) = {
                    let mut timer = tick_state.lock().unwrap();
                    timer.tick()
                };

                tick_handle
                    .emit(
                        "timer-update",
                        serde_json::json!({ "state": phase_str, "percent": percent, "remaining": remaining }),
                    )
                    .ok();
            });

            // --- Focus polling: overlay draggable when our app is focused ---
            let focus_handle = handle.clone();
            std::thread::spawn(move || {
                let mut was_app_focused = false;
                loop {
                    std::thread::sleep(std::time::Duration::from_millis(100));
                    let settings_focused = focus_handle
                        .get_webview_window("settings")
                        .and_then(|w| w.is_focused().ok())
                        .unwrap_or(false);
                    let overlay_focused = focus_handle
                        .get_webview_window("overlay")
                        .and_then(|w| w.is_focused().ok())
                        .unwrap_or(false);
                    let app_focused = settings_focused || overlay_focused;
                    if app_focused != was_app_focused {
                        was_app_focused = app_focused;
                        if let Some(win) = focus_handle.get_webview_window("overlay") {
                            win.set_ignore_cursor_events(!app_focused).ok();
                        }
                    }
                }
            });

            // Restore overlay position from settings & set initial mouse pass-through
            if let Some(win) = handle.get_webview_window("overlay") {
                use tauri::{PhysicalPosition, PhysicalSize};
                win.set_min_size(Some(PhysicalSize::new(1u32, 1u32))).ok();
                win.set_size(PhysicalSize::new(200u32, 24u32)).ok();
                let pos = {
                    let timer = timer_state.lock().unwrap();
                    (timer.settings.overlay_x, timer.settings.overlay_y)
                };
                win.set_position(PhysicalPosition::new(pos.0 as i32, pos.1 as i32)).ok();
                win.set_ignore_cursor_events(true).ok();
            }

            Ok(())
        })
        .on_window_event(|window, event| {
            match event {
                WindowEvent::CloseRequested { .. } => {
                    if window.label() == "settings" {
                        window.app_handle().exit(0);
                    }
                }
                WindowEvent::Moved(position) => {
                    if window.label() == "overlay" {
                        let state: tauri::State<'_, Arc<Mutex<TimerState>>> = window.state();
                        let mut timer = state.lock().unwrap();
                        timer.settings.overlay_x = position.x as f64;
                        timer.settings.overlay_y = position.y as f64;
                        save_settings_to_file(&timer.settings);
                    }
                }
                _ => {}
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

// --- Platform-specific: Low-level keyboard hook ---

#[cfg(target_os = "windows")]
fn install_keyboard_hook() {
    use std::ffi::c_int;
    use std::ptr;

    type WPARAM = usize;
    type LPARAM = isize;
    type LRESULT = isize;
    type DWORD = u32;
    type HHOOK = *mut std::ffi::c_void;
    type HINSTANCE = *mut std::ffi::c_void;
    type HWND = *mut std::ffi::c_void;

    const WH_KEYBOARD_LL: c_int = 13;
    const WM_KEYDOWN: WPARAM = 0x0100;

    #[repr(C)]
    struct KBDLLHOOKSTRUCT {
        vk_code: DWORD,
        scan_code: DWORD,
        flags: DWORD,
        time: DWORD,
        dw_extra_info: usize,
    }

    #[repr(C)]
    struct MSG {
        hwnd: HWND,
        message: u32,
        w_param: WPARAM,
        l_param: LPARAM,
        time: DWORD,
        pt_x: i32,
        pt_y: i32,
    }

    extern "system" {
        fn SetWindowsHookExW(
            id_hook: c_int,
            lpfn: unsafe extern "system" fn(c_int, WPARAM, LPARAM) -> LRESULT,
            hmod: HINSTANCE,
            dw_thread_id: DWORD,
        ) -> HHOOK;
        fn CallNextHookEx(
            hhk: HHOOK,
            n_code: c_int,
            w_param: WPARAM,
            l_param: LPARAM,
        ) -> LRESULT;
        fn GetMessageW(
            msg: *mut MSG,
            hwnd: HWND,
            filter_min: u32,
            filter_max: u32,
        ) -> c_int;
        fn GetModuleHandleW(module_name: *const u16) -> HINSTANCE;
    }

    unsafe extern "system" fn hook_proc(
        n_code: c_int,
        w_param: WPARAM,
        l_param: LPARAM,
    ) -> LRESULT {
        if n_code >= 0 && w_param == WM_KEYDOWN {
            let kb = &*(l_param as *const KBDLLHOOKSTRUCT);
            if kb.vk_code == HOTKEY_VK.load(Ordering::Relaxed) {
                HOTKEY_PRESSED.store(true, Ordering::Relaxed);
            }
        }
        CallNextHookEx(ptr::null_mut(), n_code, w_param, l_param)
    }

    std::thread::spawn(|| unsafe {
        let hmod = GetModuleHandleW(ptr::null());
        let hook = SetWindowsHookExW(WH_KEYBOARD_LL, hook_proc, hmod, 0);
        if hook.is_null() {
            eprintln!("[mobinogi] Failed to install keyboard hook");
            return;
        }
        eprintln!("[mobinogi] Keyboard hook installed OK");
        // Message loop required to keep the low-level hook alive
        let mut msg: MSG = std::mem::zeroed();
        while GetMessageW(&mut msg, ptr::null_mut(), 0, 0) > 0 {}
    });
}

#[cfg(not(target_os = "windows"))]
fn install_keyboard_hook() {
    // TODO: macOS implementation
}
