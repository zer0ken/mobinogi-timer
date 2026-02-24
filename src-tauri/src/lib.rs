mod packet;

use serde::{Deserialize, Serialize};
use std::fs;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tauri::{AppHandle, Emitter, Manager, WindowEvent};

// --- Emblem Data ---

struct EmblemInfo {
    key: &'static str,
    buff_key: u32,
    duration: f64,
}

const EMBLEMS: &[EmblemInfo] = &[
    EmblemInfo { key: "grand_mage", buff_key: 122806656, duration: 20.0 },
    EmblemInfo { key: "scattering_sword", buff_key: 355098955, duration: 35.0 },
    EmblemInfo { key: "cracked_earth", buff_key: 1184371696, duration: 35.0 },
    EmblemInfo { key: "distant_light", buff_key: 1590198662, duration: 20.0 },
    EmblemInfo { key: "broken_sky", buff_key: 1703435864, duration: 20.0 },
    EmblemInfo { key: "mountain_lord", buff_key: 2024838942, duration: 20.0 },
];

fn get_emblem(name: &str) -> Option<&'static EmblemInfo> {
    EMBLEMS.iter().find(|e| e.key == name)
}

fn compute_duration(emblem_name: &str) -> f64 {
    get_emblem(emblem_name).map(|e| e.duration).unwrap_or(20.0)
}

fn compute_cooldown(blind_seer: &str) -> f64 {
    match blind_seer {
        "base" => 52.0,
        "plus" => 51.0,
        "plusplus" => 50.0,
        _ => 90.0,
    }
}

// --- Settings ---

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Settings {
    #[serde(default = "default_emblem_name")]
    emblem_name: String,
    #[serde(default = "default_blind_seer")]
    blind_seer: String,
    #[serde(default = "default_duration_secs")]
    duration_secs: f64,
    #[serde(default = "default_cooldown_secs")]
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
    #[serde(default)]
    packet_capture_enabled: bool,
    #[serde(default)]
    network_interface: String,
}

fn default_emblem_name() -> String { "grand_mage".to_string() }
fn default_blind_seer() -> String { "none".to_string() }
fn default_duration_secs() -> f64 { 20.0 }
fn default_cooldown_secs() -> f64 { 90.0 }
fn default_hotkey_vk() -> u32 { 0xC0 }
fn default_hotkey_name() -> String { "`".to_string() }
fn default_overlay_x() -> f64 { 760.0 }
fn default_overlay_y() -> f64 { 20.0 }
fn default_overlay_opacity() -> f64 { 0.85 }
fn default_auto_repeat() -> bool { true }

impl Default for Settings {
    fn default() -> Self {
        Self {
            emblem_name: default_emblem_name(),
            blind_seer: default_blind_seer(),
            duration_secs: default_duration_secs(),
            cooldown_secs: default_cooldown_secs(),
            hotkey_vk: default_hotkey_vk(),
            hotkey_name: default_hotkey_name(),
            overlay_x: default_overlay_x(),
            overlay_y: default_overlay_y(),
            overlay_opacity: default_overlay_opacity(),
            auto_repeat: default_auto_repeat(),
            packet_capture_enabled: false,
            network_interface: String::new(),
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
    let mut settings: Settings = fs::read_to_string(settings_path())
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();
    // Recompute duration/cooldown from emblem/blind_seer to ensure consistency
    settings.duration_secs = compute_duration(&settings.emblem_name);
    settings.cooldown_secs = compute_cooldown(&settings.blind_seer);
    settings
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
                    if self.settings.auto_repeat && !self.settings.packet_capture_enabled {
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
    emblem_name: String,
    blind_seer: String,
    hotkey_vk: u32,
    hotkey_name: String,
    overlay_opacity: f64,
    auto_repeat: bool,
    packet_capture_enabled: bool,
    network_interface: String,
) {
    let duration_secs = compute_duration(&emblem_name);
    let cooldown_secs = compute_cooldown(&blind_seer);

    let current_settings = state.lock().unwrap().settings.clone();
    let new_settings = Settings {
        emblem_name,
        blind_seer,
        duration_secs,
        cooldown_secs,
        hotkey_vk,
        hotkey_name,
        overlay_x: current_settings.overlay_x,
        overlay_y: current_settings.overlay_y,
        overlay_opacity,
        auto_repeat,
        packet_capture_enabled,
        network_interface,
    };
    save_settings_to_file(&new_settings);
    HOTKEY_VK.store(new_settings.hotkey_vk, Ordering::Relaxed);
    {
        let mut timer = state.lock().unwrap();
        timer.settings = new_settings;
    }
    app.emit("settings-updated", ()).ok();
}

#[tauri::command]
fn list_interfaces() -> Vec<serde_json::Value> {
    packet::list_devices()
        .into_iter()
        .map(|(name, desc)| serde_json::json!({ "name": name, "desc": desc }))
        .collect()
}

// --- Global flag for hotkey press (set by keyboard hook or packet capture) ---
pub static HOTKEY_PRESSED: AtomicBool = AtomicBool::new(false);
static HOTKEY_VK: AtomicU32 = AtomicU32::new(0xC0);

// --- App Setup ---

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let settings = load_settings();
    HOTKEY_VK.store(settings.hotkey_vk, Ordering::Relaxed);

    // Start packet capture thread if enabled
    if settings.packet_capture_enabled {
        let buff_key = get_emblem(&settings.emblem_name)
            .map(|e| e.buff_key)
            .unwrap_or(0);
        let iface = settings.network_interface.clone();
        if buff_key != 0 {
            std::thread::spawn(move || {
                packet::start_capture(buff_key, &iface);
            });
        }
    }

    let timer_state = Arc::new(Mutex::new(TimerState::new(settings)));

    tauri::Builder::default()
        .manage(timer_state.clone())
        .invoke_handler(tauri::generate_handler![get_settings, save_settings, list_interfaces])
        .setup(move |app| {
            let handle = app.handle().clone();

            // --- Global keyboard hook (works without focus, even with IME) ---
            install_keyboard_hook();

            // --- Timer tick loop (16ms) ---
            let tick_state = timer_state.clone();
            let tick_handle = handle.clone();
            std::thread::spawn(move || loop {
                std::thread::sleep(std::time::Duration::from_millis(16));

                // Check if hotkey was pressed via keyboard hook or packet capture
                if HOTKEY_PRESSED.swap(false, Ordering::Relaxed) {
                    let mut timer = tick_state.lock().unwrap();
                    timer.start();
                    eprintln!("[mobinogi] Timer started");
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
