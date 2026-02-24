mod packet;

use serde::{Deserialize, Serialize};
use std::fs;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tauri::{AppHandle, Emitter, Manager, WindowEvent};

// --- Emblem Data ---

struct EmblemInfo {
    buff_key: u32,
    duration: f64,
    name: &'static str,
}

const EMBLEMS: &[EmblemInfo] = &[
    EmblemInfo { buff_key: 122806656, duration: 20.0, name: "대마법사" },
    EmblemInfo { buff_key: 122806657, duration: 20.0, name: "무자비한 포식자" },
    EmblemInfo { buff_key: 122806658, duration: 20.0, name: "녹아내린 대지" },
    EmblemInfo { buff_key: 355098955, duration: 35.0, name: "흩날리는 검" },
    EmblemInfo { buff_key: 1184371696, duration: 35.0, name: "갈라진 땅" },
    EmblemInfo { buff_key: 1590198662, duration: 20.0, name: "아득한 빛" },
    EmblemInfo { buff_key: 1703435864, duration: 20.0, name: "부서진 하늘" },
    EmblemInfo { buff_key: 2024838942, duration: 20.0, name: "산맥 군주" },
];

fn find_emblem_by_buff_key(buff_key: u32) -> Option<&'static EmblemInfo> {
    EMBLEMS.iter().find(|e| e.buff_key == buff_key)
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
    #[serde(default = "default_blind_seer")]
    blind_seer: String,
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
    #[serde(default = "default_overlay_width")]
    overlay_width: u32,
    #[serde(default = "default_auto_repeat")]
    auto_repeat: bool,
    #[serde(default)]
    packet_capture_enabled: bool,
    #[serde(default)]
    network_interface: String,
}

fn default_blind_seer() -> String { "none".to_string() }
fn default_hotkey_vk() -> u32 { 0xC0 }
fn default_hotkey_name() -> String { "`".to_string() }
fn default_overlay_x() -> f64 { 760.0 }
fn default_overlay_y() -> f64 { 20.0 }
fn default_overlay_opacity() -> f64 { 0.85 }
fn default_overlay_width() -> u32 { 200 }
fn default_auto_repeat() -> bool { true }

impl Default for Settings {
    fn default() -> Self {
        Self {
            blind_seer: default_blind_seer(),
            hotkey_vk: default_hotkey_vk(),
            hotkey_name: default_hotkey_name(),
            overlay_x: default_overlay_x(),
            overlay_y: default_overlay_y(),
            overlay_opacity: default_overlay_opacity(),
            overlay_width: default_overlay_width(),
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

struct TimerState {
    phase: TimerPhase,
    duration_remaining: f64,
    cooldown_remaining: f64,
    duration_total: f64,
    cooldown_total: f64,
    last_tick: Instant,
    active_emblem: String,
    settings: Settings,
    capture_stop: Arc<AtomicBool>,
}

impl TimerState {
    fn new(settings: Settings) -> Self {
        Self {
            phase: TimerPhase::Idle,
            duration_remaining: 0.0,
            cooldown_remaining: 0.0,
            duration_total: 0.0,
            cooldown_total: 0.0,
            last_tick: Instant::now(),
            active_emblem: String::new(),
            settings,
            capture_stop: Arc::new(AtomicBool::new(false)),
        }
    }

    fn start(&mut self) {
        self.active_emblem = "각성".to_string();
        self.start_timer(20.0);
    }

    fn start_with_emblem(&mut self, dur: f64, name: &str) {
        self.active_emblem = name.to_string();
        self.start_timer(dur);
    }

    fn start_timer(&mut self, dur: f64) {
        let total_cd = compute_cooldown(&self.settings.blind_seer);
        let wait = total_cd - dur;
        self.duration_remaining = dur;
        self.cooldown_remaining = wait;
        self.duration_total = dur;
        self.cooldown_total = wait;
        self.phase = TimerPhase::Duration;
        self.last_tick = Instant::now();
    }

    fn tick(&mut self) -> (String, f64, f64, String) {
        if self.phase == TimerPhase::Idle {
            return ("idle".into(), 0.0, 0.0, String::new());
        }

        let now = Instant::now();
        let dt = now.duration_since(self.last_tick).as_secs_f64();
        self.last_tick = now;

        let emblem = self.active_emblem.clone();
        match self.phase {
            TimerPhase::Idle => unreachable!(),
            TimerPhase::Duration => {
                self.duration_remaining -= dt;
                if self.duration_remaining <= 0.0 {
                    self.cooldown_remaining += self.duration_remaining;
                    self.duration_remaining = 0.0;
                    self.phase = TimerPhase::Cooldown;
                    let pct = (1.0 - self.cooldown_remaining / self.cooldown_total) * 100.0;
                    ("cooldown".into(), pct, self.cooldown_remaining.max(0.0), emblem)
                } else {
                    let pct = (self.duration_remaining / self.duration_total) * 100.0;
                    ("duration".into(), pct, self.duration_remaining, emblem)
                }
            }
            TimerPhase::Cooldown => {
                self.cooldown_remaining -= dt;
                if self.cooldown_remaining <= 0.0 {
                    if self.settings.auto_repeat && !self.settings.packet_capture_enabled {
                        self.start();
                        ("duration".into(), 100.0, self.duration_remaining, emblem)
                    } else {
                        self.phase = TimerPhase::Idle;
                        ("idle".into(), 0.0, 0.0, emblem)
                    }
                } else {
                    let pct = (1.0 - self.cooldown_remaining / self.cooldown_total) * 100.0;
                    ("cooldown".into(), pct, self.cooldown_remaining, emblem)
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
    blind_seer: String,
    hotkey_vk: u32,
    hotkey_name: String,
    overlay_opacity: f64,
    overlay_width: u32,
    auto_repeat: bool,
    packet_capture_enabled: bool,
    network_interface: String,
) {
    let current_settings = state.lock().unwrap().settings.clone();
    let new_settings = Settings {
        blind_seer,
        hotkey_vk,
        hotkey_name,
        overlay_x: current_settings.overlay_x,
        overlay_y: current_settings.overlay_y,
        overlay_opacity,
        overlay_width,
        auto_repeat,
        packet_capture_enabled,
        network_interface,
    };

    save_settings_to_file(&new_settings);
    HOTKEY_VK.store(new_settings.hotkey_vk, Ordering::Relaxed);
    {
        let mut timer = state.lock().unwrap();
        let capture_changed = new_settings.packet_capture_enabled != current_settings.packet_capture_enabled
            || new_settings.network_interface != current_settings.network_interface;

        if capture_changed {
            timer.capture_stop.store(true, Ordering::Relaxed);

            if new_settings.packet_capture_enabled {
                let stop = Arc::new(AtomicBool::new(false));
                timer.capture_stop = stop.clone();
                let iface = new_settings.network_interface.clone();
                std::thread::spawn(move || {
                    packet::start_capture(&iface, stop);
                });
            }
        }

        timer.settings = new_settings;
    }
    app.emit("settings-updated", ()).ok();
}

#[tauri::command]
fn list_interfaces() -> serde_json::Value {
    let default_name = packet::find_default_device_name();

    let devices: Vec<serde_json::Value> = packet::list_devices()
        .into_iter()
        .map(|(name, desc)| serde_json::json!({ "name": name, "desc": desc }))
        .collect();

    serde_json::json!({
        "devices": devices,
        "default": default_name,
    })
}

// --- Global flags ---
pub static HOTKEY_PRESSED: AtomicBool = AtomicBool::new(false);
pub static DETECTED_BUFF_KEY: AtomicU32 = AtomicU32::new(0);
static HOTKEY_VK: AtomicU32 = AtomicU32::new(0xC0);

// --- App Setup ---

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let settings = load_settings();
    HOTKEY_VK.store(settings.hotkey_vk, Ordering::Relaxed);

    let timer_state = Arc::new(Mutex::new(TimerState::new(settings.clone())));

    // Start packet capture thread if enabled
    if settings.packet_capture_enabled {
        let iface = settings.network_interface.clone();
        let stop = {
            let ts = timer_state.lock().unwrap();
            ts.capture_stop.clone()
        };
        std::thread::spawn(move || {
            packet::start_capture(&iface, stop);
        });
    }

    tauri::Builder::default()
        .manage(timer_state.clone())
        .invoke_handler(tauri::generate_handler![get_settings, save_settings, list_interfaces])
        .setup(move |app| {
            let handle = app.handle().clone();

            // --- Global keyboard hook ---
            install_keyboard_hook();

            // --- Timer tick loop (16ms) ---
            let tick_state = timer_state.clone();
            let tick_handle = handle.clone();
            std::thread::spawn(move || loop {
                std::thread::sleep(std::time::Duration::from_millis(16));

                // Check for auto-detected buff from packet capture
                let detected = DETECTED_BUFF_KEY.swap(0, Ordering::Relaxed);
                if detected != 0 {
                    if let Some(info) = find_emblem_by_buff_key(detected) {
                        let mut timer = tick_state.lock().unwrap();
                        timer.start_with_emblem(info.duration, info.name);
                        eprintln!("[mobinogi] Timer auto-started ({}, dur={}s)", info.name, info.duration);
                    }
                }

                // Check for manual hotkey
                if HOTKEY_PRESSED.swap(false, Ordering::Relaxed) {
                    let mut timer = tick_state.lock().unwrap();
                    timer.start();
                    eprintln!("[mobinogi] Timer started (manual)");
                }

                let (phase_str, percent, remaining, emblem) = {
                    let mut timer = tick_state.lock().unwrap();
                    timer.tick()
                };

                tick_handle
                    .emit(
                        "timer-update",
                        serde_json::json!({ "state": phase_str, "percent": percent, "remaining": remaining, "emblem": emblem }),
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
                use tauri::PhysicalPosition;
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
