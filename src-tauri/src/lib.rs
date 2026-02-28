mod packet;

use serde::{Deserialize, Serialize};
use std::fs;
use std::sync::atomic::{AtomicBool, Ordering};
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
    #[serde(default = "default_overlay_x")]
    overlay_x: f64,
    #[serde(default = "default_overlay_y")]
    overlay_y: f64,
    #[serde(default = "default_overlay_opacity")]
    overlay_opacity: f64,
    #[serde(default = "default_overlay_width")]
    overlay_width: u32,
    #[serde(default)]
    network_interface: String,
    #[serde(default = "default_duration_warning_threshold")]
    duration_warning_threshold: u32,
    #[serde(default = "default_cooldown_warning_threshold")]
    cooldown_warning_threshold: u32,
}

fn default_blind_seer() -> String { "none".to_string() }
fn default_overlay_x() -> f64 { 760.0 }
fn default_overlay_y() -> f64 { 20.0 }
fn default_overlay_opacity() -> f64 { 0.80 }
fn default_overlay_width() -> u32 { 100 }
fn default_duration_warning_threshold() -> u32 { 10 }
fn default_cooldown_warning_threshold() -> u32 { 10 }


impl Default for Settings {
    fn default() -> Self {
        Self {
            blind_seer: default_blind_seer(),
            overlay_x: default_overlay_x(),
            overlay_y: default_overlay_y(),
            overlay_opacity: default_overlay_opacity(),
            overlay_width: default_overlay_width(),
            network_interface: String::new(),
            duration_warning_threshold: default_duration_warning_threshold(),
            cooldown_warning_threshold: default_cooldown_warning_threshold(),
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
                    self.phase = TimerPhase::Idle;
                    ("idle".into(), 0.0, 0.0, emblem)
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
    network_interface: String,
    duration_warning_threshold: u32,
    cooldown_warning_threshold: u32,
) {
    let current_settings = state.lock().unwrap().settings.clone();
    let new_settings = Settings {
        blind_seer,
        overlay_x: current_settings.overlay_x,
        overlay_y: current_settings.overlay_y,
        overlay_opacity,
        overlay_width,
        network_interface,
        duration_warning_threshold,
        cooldown_warning_threshold,
    };

    save_settings_to_file(&new_settings);
    {
        let mut timer = state.lock().unwrap();
        if new_settings.network_interface != current_settings.network_interface && is_npcap_installed() {
            timer.capture_stop.store(true, Ordering::Relaxed);
            let stop = Arc::new(AtomicBool::new(false));
            timer.capture_stop = stop.clone();
            let iface = new_settings.network_interface.clone();
            std::thread::spawn(move || {
                packet::start_capture(&iface, stop);
            });
        }
        timer.settings = new_settings;
    }
    app.emit("settings-updated", ()).ok();
}

#[tauri::command]
fn list_interfaces() -> serde_json::Value {
    if !is_npcap_installed() {
        return serde_json::json!({ "devices": [], "default": "" });
    }
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

fn is_npcap_installed() -> bool {
    let sys = std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".to_string());
    let base = std::path::Path::new(&sys).join("System32");
    base.join("Npcap").join("wpcap.dll").exists() || base.join("wpcap.dll").exists()
}

#[tauri::command]
fn check_npcap() -> bool {
    is_npcap_installed()
}

#[tauri::command]
fn get_version(app: AppHandle) -> String {
    app.config().version.clone().unwrap_or_default()
}

#[tauri::command]
fn check_update(app: AppHandle) -> serde_json::Value {
    let result = (|| -> Result<serde_json::Value, String> {
        let current_version = app.config().version.clone().unwrap_or_default();
        let current_major = current_version.split('.').next().unwrap_or("2026");

        // Extract suffix from current version (e.g., "2026.2.25-auto" -> Some("auto"))
        let current_suffix = if let Some(dash_pos) = current_version.find('-') {
            Some(&current_version[dash_pos + 1..])
        } else {
            None
        };

        // Get all releases
        let body: String = ureq::get("https://api.github.com/repos/zer0ken/mobinogi-timer/releases")
            .header("User-Agent", "mobinogi-timer")
            .header("Accept", "application/vnd.github+json")
            .call()
            .map_err(|e| e.to_string())?
            .body_mut()
            .read_to_string()
            .map_err(|e| e.to_string())?;

        let releases: Vec<serde_json::Value> = serde_json::from_str(&body).map_err(|e| e.to_string())?;

        // Filter releases by major version and suffix
        for release in releases {
            if let Some(tag) = release["tag_name"].as_str() {
                let version = tag.trim_start_matches('v');
                if let Some(major) = version.split('.').next() {
                    if major != current_major {
                        continue;
                    }

                    // Extract suffix from release version
                    let release_suffix = if let Some(dash_pos) = version.find('-') {
                        Some(&version[dash_pos + 1..])
                    } else {
                        None
                    };

                    // Match suffix: both have same suffix or both have no suffix
                    let suffix_matches = match (current_suffix, release_suffix) {
                        (Some(c), Some(r)) => c == r,
                        (None, None) => true,
                        // Legacy: if current has no suffix, accept -auto versions (migration path)
                        (None, Some("auto")) => true,
                        _ => false,
                    };

                    if suffix_matches && !release["draft"].as_bool().unwrap_or(false) && !release["prerelease"].as_bool().unwrap_or(false) {
                        let url = release["html_url"].as_str().unwrap_or("").to_string();
                        return Ok(serde_json::json!({ "tag": version, "url": url }));
                    }
                }
            }
        }

        Ok(serde_json::json!({ "tag": "", "url": "" }))
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

// --- Global detected buff (packet → tick loop) ---
pub struct DetectedBuff {
    pub buff_key: u32,
    pub detected_at: Instant,
}
pub static DETECTED_BUFF: Mutex<Option<DetectedBuff>> = Mutex::new(None);

// --- App Setup ---

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let settings = load_settings();
    let timer_state = Arc::new(Mutex::new(TimerState::new(settings.clone())));

    // Start packet capture thread (only if Npcap is installed)
    if is_npcap_installed() {
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
        .invoke_handler(tauri::generate_handler![get_settings, save_settings, list_interfaces, check_npcap, open_url, get_version, check_update])
        .setup(move |app| {
            let handle = app.handle().clone();

            // --- Timer tick loop (16ms) ---
            let tick_state = timer_state.clone();
            let tick_handle = handle.clone();
            std::thread::spawn(move || loop {
                std::thread::sleep(std::time::Duration::from_millis(16));

                // Check for auto-detected buff from packet capture
                let detected = DETECTED_BUFF.lock().unwrap().take();
                if let Some(det) = detected {
                    let elapsed_secs = det.detected_at.elapsed().as_secs_f64();
                    if let Some(info) = find_emblem_by_buff_key(det.buff_key) {
                        let adjusted_dur = (info.duration - elapsed_secs).max(0.0);
                        if adjusted_dur > 0.0 {
                            let mut timer = tick_state.lock().unwrap();
                            timer.start_with_emblem(adjusted_dur, info.duration, elapsed_secs, info.name);
                        }
                    }
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
