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

fn find_emblem_by_id(id: &str) -> Option<&'static EmblemInfo> {
    match id {
        "grand_mage" => EMBLEMS.iter().find(|e| e.buff_key == 122806656),
        "merciless_predator" => EMBLEMS.iter().find(|e| e.buff_key == 122806657),
        "melted_earth" => EMBLEMS.iter().find(|e| e.buff_key == 122806658),
        "scattering_sword" => EMBLEMS.iter().find(|e| e.buff_key == 355098955),
        "cracked_earth" => EMBLEMS.iter().find(|e| e.buff_key == 1184371696),
        "distant_light" => EMBLEMS.iter().find(|e| e.buff_key == 1590198662),
        "broken_sky" => EMBLEMS.iter().find(|e| e.buff_key == 1703435864),
        "mountain_lord" => EMBLEMS.iter().find(|e| e.buff_key == 2024838942),
        _ => None,
    }
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
    #[serde(default = "default_overlay_x")]
    overlay_x: f64,
    #[serde(default = "default_overlay_y")]
    overlay_y: f64,
    #[serde(default = "default_overlay_opacity")]
    overlay_opacity: f64,
    #[serde(default = "default_overlay_width")]
    overlay_width: u32,
    #[serde(default = "default_hotkey_vk")]
    hotkey_vk: u32,
    #[serde(default = "default_hotkey_name")]
    hotkey_name: String,
    #[serde(default = "default_auto_repeat")]
    auto_repeat: bool,
    #[serde(default = "default_selected_emblem")]
    selected_emblem: String,
}

fn default_blind_seer() -> String { "none".to_string() }
fn default_overlay_x() -> f64 { 760.0 }
fn default_overlay_y() -> f64 { 20.0 }
fn default_overlay_opacity() -> f64 { 0.80 }
fn default_overlay_width() -> u32 { 100 }
fn default_hotkey_vk() -> u32 { 0xC0 }
fn default_hotkey_name() -> String { "`".to_string() }
fn default_auto_repeat() -> bool { true }
fn default_selected_emblem() -> String { "grand_mage".to_string() }


impl Default for Settings {
    fn default() -> Self {
        Self {
            blind_seer: default_blind_seer(),
            overlay_x: default_overlay_x(),
            overlay_y: default_overlay_y(),
            overlay_opacity: default_overlay_opacity(),
            overlay_width: default_overlay_width(),
            hotkey_vk: default_hotkey_vk(),
            hotkey_name: default_hotkey_name(),
            auto_repeat: default_auto_repeat(),
            selected_emblem: default_selected_emblem(),
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
        }
    }

    fn start(&mut self) {
        if let Some(info) = find_emblem_by_id(&self.settings.selected_emblem) {
            self.start_with_emblem(info.duration, info.duration, 0.0, info.name);
        } else {
            self.start_with_emblem(20.0, 20.0, 0.0, "각성");
        }
    }

    fn start_with_emblem(&mut self, remaining: f64, total: f64, elapsed: f64, name: &str) {
        self.active_emblem = name.to_string();
        self.start_timer(remaining, total, elapsed);
    }

    fn start_timer(&mut self, remaining: f64, total: f64, elapsed: f64) {
        let total_cd = compute_cooldown(&self.settings.blind_seer);
        self.duration_remaining = remaining;
        self.cooldown_remaining = (total_cd - elapsed).max(0.0);
        self.duration_total = total;
        self.cooldown_total = total_cd;
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
                self.cooldown_remaining -= dt;
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
                    if self.settings.auto_repeat {
                        self.start();
                        let pct = (self.duration_remaining / self.duration_total) * 100.0;
                        ("duration".into(), pct, self.duration_remaining, self.active_emblem.clone())
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
    overlay_opacity: f64,
    overlay_width: u32,
    hotkey_vk: u32,
    hotkey_name: String,
    auto_repeat: bool,
    selected_emblem: String,
) {
    let current_settings = state.lock().unwrap().settings.clone();
    let new_settings = Settings {
        blind_seer,
        overlay_x: current_settings.overlay_x,
        overlay_y: current_settings.overlay_y,
        overlay_opacity,
        overlay_width,
        hotkey_vk,
        hotkey_name,
        auto_repeat,
        selected_emblem,
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
fn get_version(app: AppHandle) -> String {
    app.config().version.clone().unwrap_or_default()
}

#[tauri::command]
fn check_update() -> serde_json::Value {
    let result = (|| -> Result<serde_json::Value, String> {
        let body: String = ureq::get("https://api.github.com/repos/zer0ken/mobinogi-timer/releases/latest")
            .header("User-Agent", "mobinogi-timer")
            .header("Accept", "application/vnd.github+json")
            .call()
            .map_err(|e| e.to_string())?
            .body_mut()
            .read_to_string()
            .map_err(|e| e.to_string())?;
        let json: serde_json::Value = serde_json::from_str(&body).map_err(|e| e.to_string())?;
        let tag = json["tag_name"].as_str().unwrap_or("").to_string();
        let url = json["html_url"].as_str().unwrap_or("").to_string();
        Ok(serde_json::json!({ "tag": tag, "url": url }))
    })();
    match result {
        Ok(v) => v,
        Err(_) => serde_json::json!({ "tag": "", "url": "" }),
    }
}

#[tauri::command]
fn open_url(url: String) {
    #[cfg(target_os = "windows")]
    { let _ = std::process::Command::new("cmd").args(["/C", "start", "", &url]).spawn(); }
    #[cfg(target_os = "macos")]
    { let _ = std::process::Command::new("open").arg(&url).spawn(); }
    #[cfg(target_os = "linux")]
    { let _ = std::process::Command::new("xdg-open").arg(&url).spawn(); }
}

// --- Global hotkey state ---
static HOTKEY_PRESSED: AtomicBool = AtomicBool::new(false);
static HOTKEY_VK: AtomicU32 = AtomicU32::new(0xC0);

// --- App Setup ---

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let settings = load_settings();
    HOTKEY_VK.store(settings.hotkey_vk, Ordering::Relaxed);
    let timer_state = Arc::new(Mutex::new(TimerState::new(settings.clone())));

    tauri::Builder::default()
        .manage(timer_state.clone())
        .invoke_handler(tauri::generate_handler![get_settings, save_settings, open_url, get_version, check_update])
        .setup(move |app| {
            install_keyboard_hook();
            let handle = app.handle().clone();

            // --- Timer tick loop (16ms) ---
            let tick_state = timer_state.clone();
            let tick_handle = handle.clone();
            std::thread::spawn(move || loop {
                std::thread::sleep(std::time::Duration::from_millis(16));

                // Check for hotkey press
                if HOTKEY_PRESSED.swap(false, Ordering::Relaxed) {
                    let mut timer = tick_state.lock().unwrap();
                    timer.start();
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

// --- Keyboard Hook ---

#[cfg(target_os = "windows")]
fn install_keyboard_hook() {
    use std::ptr::null_mut;
    use std::sync::atomic::Ordering;

    type HHOOK = *mut std::ffi::c_void;
    type LRESULT = isize;
    type WPARAM = usize;
    type LPARAM = isize;

    const WH_KEYBOARD_LL: i32 = 13;
    const WM_KEYDOWN: u32 = 0x0100;
    const HC_ACTION: i32 = 0;

    #[repr(C)]
    struct KBDLLHOOKSTRUCT {
        vk_code: u32,
        scan_code: u32,
        flags: u32,
        time: u32,
        dw_extra_info: usize,
    }

    #[link(name = "user32")]
    extern "system" {
        fn SetWindowsHookExW(id_hook: i32, lpfn: unsafe extern "system" fn(i32, WPARAM, LPARAM) -> LRESULT, hmod: *mut std::ffi::c_void, dw_thread_id: u32) -> HHOOK;
        fn CallNextHookEx(hhk: HHOOK, n_code: i32, w_param: WPARAM, l_param: LPARAM) -> LRESULT;
        fn GetMessageW(lpmsg: *mut MSG, hwnd: *mut std::ffi::c_void, wmsg_filter_min: u32, wmsg_filter_max: u32) -> i32;
        fn TranslateMessage(lpmsg: *const MSG) -> i32;
        fn DispatchMessageW(lpmsg: *const MSG) -> LRESULT;
    }

    #[repr(C)]
    struct MSG {
        hwnd: *mut std::ffi::c_void,
        message: u32,
        w_param: WPARAM,
        l_param: LPARAM,
        time: u32,
        pt: POINT,
    }

    #[repr(C)]
    struct POINT {
        x: i32,
        y: i32,
    }

    unsafe extern "system" fn hook_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        if code == HC_ACTION && wparam as u32 == WM_KEYDOWN {
            let kb = &*(lparam as *const KBDLLHOOKSTRUCT);
            let target_vk = HOTKEY_VK.load(Ordering::Relaxed);
            if kb.vk_code == target_vk {
                HOTKEY_PRESSED.store(true, Ordering::Relaxed);
            }
        }
        CallNextHookEx(null_mut(), code, wparam, lparam)
    }

    std::thread::spawn(|| unsafe {
        let hook = SetWindowsHookExW(WH_KEYBOARD_LL, hook_proc, null_mut(), 0);
        if hook.is_null() {
            return;
        }
        let mut msg: MSG = std::mem::zeroed();
        while GetMessageW(&mut msg, null_mut(), 0, 0) > 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    });
}

#[cfg(not(target_os = "windows"))]
fn install_keyboard_hook() {
    // No-op on non-Windows platforms
}
