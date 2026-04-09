use crate::{DetectedBuff, DETECTED_BUFF};
use std::fs::OpenOptions;
use std::io::Write as IoWrite;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;

const START_MARKER: [u8; 9] = [0x82, 0x4E, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];
const END_MARKER: [u8; 9] = [0x18, 0x4F, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];
const BUFF_START_DATA_TYPE: u32 = 100055;
const SELF_DAMAGE_DATA_TYPE: u32 = 20919;

/// Maps field[16:20] bytes in BUFF_START packets to (buff_key, emblem name).
/// buff_key must match an entry in lib.rs EMBLEMS.
const EMBLEM_FIELD_KEYS: &[([u8; 4], u32)] = &[
    ([0x9E, 0x5A, 0x0D, 0x72], 122806656),  // 대마법사
    ([0xA8, 0x0F, 0x5F, 0x44], 122806657),  // 무자비한 포식자
    ([0xCA, 0x1A, 0x9C, 0x5F], 122806658),  // 녹아내린 대지
    ([0x86, 0x81, 0xC8, 0x5E], 1590198662), // 아득한 빛
    ([0x4B, 0x61, 0x2A, 0x15], 355098955),  // 흩날리는 검
    ([0xF0, 0x13, 0x98, 0x46], 1184371696), // 갈라진 땅
    ([0x58, 0x5E, 0x88, 0x65], 1703435864), // 부서진 하늘
    ([0x1E, 0x97, 0xB0, 0x78], 2024838942), // 산맥 군주
];

// --- Debug logging ---

fn log_path() -> std::path::PathBuf {
    let base = std::env::var("APPDATA").unwrap_or_else(|_| ".".to_string());
    let dir = std::path::PathBuf::from(base).join("mobinogi-timer");
    std::fs::create_dir_all(&dir).ok();
    dir.join("debug.log")
}

static LOG_START: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();

fn dlog(#[allow(unused_variables)] msg: &str) {
    #[cfg(debug_assertions)]
    {
        let elapsed = LOG_START.get_or_init(Instant::now).elapsed();
        if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(log_path()) {
            let _ = writeln!(f, "[+{:6.3}s] {}", elapsed.as_secs_f64(), msg);
        }
    }
}

fn hex_str(data: &[u8]) -> String {
    let limit = data.len().min(48);
    let hex: String = data[..limit]
        .iter()
        .map(|b| format!("{:02X}", b))
        .collect::<Vec<_>>()
        .join(" ");
    if data.len() > 48 { format!("{} ...({} bytes)", hex, data.len()) } else { format!("{} ({} bytes)", hex, data.len()) }
}

/// Extract all printable ASCII strings of length >= 5 from data.
fn scan_ascii(data: &[u8]) -> Vec<String> {
    let mut results = Vec::new();
    let mut cur = String::new();
    for &b in data {
        if b.is_ascii_graphic() || b == b' ' {
            cur.push(b as char);
        } else {
            if cur.len() >= 5 {
                results.push(cur.clone());
            }
            cur.clear();
        }
    }
    if cur.len() >= 5 { results.push(cur); }
    results
}

// --- Block iterator ---

struct DataEntry<'a> {
    data_type: u32,
    content: &'a [u8],
}

/// Iterates over [dataType: u32LE][length: u32LE][encodeType: u8][content: length bytes] entries.
fn iter_data_entries(block: &[u8]) -> impl Iterator<Item = DataEntry<'_>> {
    let mut offset = 0usize;
    std::iter::from_fn(move || {
        if offset + 9 > block.len() {
            return None;
        }
        let data_type = u32::from_le_bytes(block[offset..offset + 4].try_into().ok()?);
        let length =
            u32::from_le_bytes(block[offset + 4..offset + 8].try_into().ok()?) as usize;
        let content_start = offset + 9;
        if content_start + length > block.len() {
            offset = block.len();
            return None;
        }
        let entry = DataEntry {
            data_type,
            content: &block[content_start..content_start + length],
        };
        offset = content_start + length;
        Some(entry)
    })
}

// --- Character identification & emblem detection ---
//
// SELF_DAMAGE 패킷은 내 캐릭터가 관련된 전투에서만 수신된다.
// 내가 공격하면 userId가 나, 내가 맞으면 targetId가 나.
// 따라서 가장 최근 SELF_DAMAGE의 userId·targetId 둘이 곧 내 캐릭터 후보다.
//
// 엠블럼 각성(BUFF_START) 패킷은 주변 모든 플레이어의 것이 수신되므로,
// 그 userId가 후보에 속할 때만 내 각성으로 판단하고 타이머를 발동한다.
//
// 버퍼링 전략:
// - 엠블럼 패킷은 후보 존재 여부와 무관하게 무조건 큐에 저장한다.
// - SELF_DAMAGE가 올 때마다 후보를 갱신한 뒤 큐를 재검사한다.
// - 지속시간(최대 35초)이 지난 항목은 만료 제거한다.

const MAX_BUFF_AGE_SECS: f64 = 35.0;

struct CaptureState {
    candidates: [u64; 2], // [userId, targetId] from latest SELF_DAMAGE
    buff_queue: Vec<BufferedBuff>,
}

struct BufferedBuff {
    user_id: u64,
    buff_key: u32,
    duration: f64,
    detected_at: Instant,
}

// --- Capture entry points ---

pub fn start_capture(interface_name: &str, stop: Arc<AtomicBool>) {
    let _ = std::fs::write(log_path(), "");
    dlog("=== Capture started ===");

    let device = if interface_name.is_empty() {
        match pcap::Device::lookup() {
            Ok(Some(d)) => d,
            _ => return,
        }
    } else {
        match pcap::Device::list() {
            Ok(devices) => match devices.into_iter().find(|d| d.name == interface_name) {
                Some(d) => d,
                None => return,
            },
            Err(_) => return,
        }
    };

    let mut cap = match pcap::Capture::from_device(device)
        .unwrap()
        .promisc(false)
        .snaplen(65535)
        .timeout(1000)
        .open()
    {
        Ok(c) => c,
        Err(_) => return,
    };

    if cap.filter("tcp and src port 16000", true).is_err() {
        return;
    }

    let mut buffer: Vec<u8> = Vec::new();
    let mut state = CaptureState {
        candidates: [0; 2],
        buff_queue: Vec::new(),
    };
    #[cfg(debug_assertions)]
    let mut raw_count = 0u32;

    loop {
        if stop.load(Ordering::Relaxed) {
            break;
        }
        match cap.next_packet() {
            Ok(packet) => {
                let payload = extract_tcp_payload(packet.data);
                if payload.is_empty() {
                    continue;
                }
                #[cfg(debug_assertions)]
                {
                    raw_count += 1;
                    if raw_count <= 5 {
                        dlog(&format!("RAW payload #{}: {}", raw_count, hex_str(payload)));
                    }
                }
                buffer.extend_from_slice(payload);
                process_buffer(&mut buffer, &mut state);
            }
            Err(pcap::Error::TimeoutExpired) => continue,
            Err(_) => break,
        }
    }
}

pub fn list_devices() -> Vec<(String, String)> {
    pcap::Device::list()
        .unwrap_or_default()
        .into_iter()
        .map(|d| {
            let desc = d.desc.clone().unwrap_or_default();
            (d.name, desc)
        })
        .collect()
}

/// Find the best default interface: one with a non-loopback IPv4 gateway address.
pub fn find_default_device_name() -> String {
    let devices = pcap::Device::list().unwrap_or_default();
    for d in &devices {
        for addr in &d.addresses {
            if let Some(gw) = &addr.netmask {
                if let std::net::IpAddr::V4(ip) = addr.addr {
                    let octets = ip.octets();
                    if octets[0] == 127
                        || (octets[0] == 169 && octets[1] == 254)
                        || octets[0] == 0
                    {
                        continue;
                    }
                    if let std::net::IpAddr::V4(mask) = gw {
                        let mask_bits = u32::from_be_bytes(mask.octets());
                        if mask_bits != 0 && mask_bits != 0xFFFFFFFF {
                            return d.name.clone();
                        }
                    }
                }
            }
        }
    }
    pcap::Device::lookup()
        .ok()
        .flatten()
        .map(|d| d.name)
        .unwrap_or_default()
}

// --- Internal ---

fn extract_tcp_payload(data: &[u8]) -> &[u8] {
    if data.len() < 54 {
        return &[];
    }
    let ip_header_len = ((data[14] & 0x0F) as usize) * 4;
    let tcp_offset = 14 + ip_header_len;
    if tcp_offset + 13 > data.len() {
        return &[];
    }
    let tcp_header_len = ((data[tcp_offset + 12] >> 4) as usize) * 4;
    let payload_offset = tcp_offset + tcp_header_len;
    if payload_offset >= data.len() {
        return &[];
    }
    &data[payload_offset..]
}

fn process_buffer(buffer: &mut Vec<u8>, state: &mut CaptureState) {
    loop {
        let Some(start_pos) = find_marker(buffer, &START_MARKER) else {
            if buffer.len() > 1024 * 1024 {
                let keep_from = buffer.len() - START_MARKER.len();
                buffer.drain(..keep_from);
            }
            break;
        };

        let data_start = start_pos + START_MARKER.len();
        let Some(end_offset) = find_marker(&buffer[data_start..], &END_MARKER) else {
            if start_pos > 0 {
                buffer.drain(..start_pos);
            }
            break;
        };

        let block = buffer[data_start..data_start + end_offset].to_vec();
        let remove_to = data_start + end_offset + END_MARKER.len();
        buffer.drain(..remove_to);

        process_block(&block, state);
    }
}

fn emit_buff(buff_key: u32, duration: f64, detected_at: Instant) {
    *DETECTED_BUFF.lock().unwrap() = Some(DetectedBuff { buff_key, duration, detected_at });
}

fn process_block(block: &[u8], state: &mut CaptureState) {
    let mut detected_user_id = 0u64;
    let mut detected_buff_key: Option<(u32, f64)> = None;
    let mut candidates_updated = false;
    let mut found_emblem_20s = false;

    // First pass: detect emblem buff and ASCII strings in all entries
    #[cfg(debug_assertions)]
    {
        for entry in iter_data_entries(block) {
            let strings = scan_ascii(entry.content);
            if !strings.is_empty() {
                dlog(&format!("  ASCII type={} strings={:?}", entry.data_type, strings));
            }
            if entry.data_type == BUFF_START_DATA_TYPE && entry.content.len() >= 24 {
                let field: [u8; 4] = entry.content[16..20].try_into().unwrap_or([0; 4]);
                if EMBLEM_FIELD_KEYS.iter().any(|(k, _)| k == &field) { found_emblem_20s = true; }
            }
        }
        if found_emblem_20s {
            dlog("  ^ block contains emblem-duration buff");
        }
    }

    for entry in iter_data_entries(block) {
        match entry.data_type {
            SELF_DAMAGE_DATA_TYPE if entry.content.len() >= 16 => {
                let user_id =
                    u64::from_le_bytes(entry.content[0..8].try_into().unwrap_or([0; 8]));
                let target_id =
                    u64::from_le_bytes(entry.content[8..16].try_into().unwrap_or([0; 8]));
                dlog(&format!("HIT: user={:016X} target={:016X}", user_id, target_id));
                state.candidates = [user_id, target_id];
                candidates_updated = true;
            }
            BUFF_START_DATA_TYPE if entry.content.len() >= 24 => {
                let buff_user_id =
                    u64::from_le_bytes(entry.content[0..8].try_into().unwrap_or([0; 8]));
                let field: [u8; 4] = entry.content[16..20].try_into().unwrap_or([0; 4]);
                let duration =
                    f32::from_le_bytes(entry.content[20..24].try_into().unwrap_or([0; 4]));
                dlog(&format!(
                    "BUFF_START: user={:016X} field[16:20]={} duration={:.1} | full={}",
                    buff_user_id, hex_str(&field), duration, hex_str(entry.content)
                ));

                if detected_buff_key.is_none() {
                    let found = EMBLEM_FIELD_KEYS.iter()
                        .find(|(k, _)| k == &field)
                        .map(|(_, buff_key)| (*buff_key, duration as f64));

                    if let Some((buff_key, dur)) = found {
                        detected_user_id = buff_user_id;
                        detected_buff_key = Some((buff_key, dur));
                    }
                }
            }
            _ => {}
        }
    }

    // Always buffer emblem detections
    if let Some((buff_key, duration)) = detected_buff_key {
        dlog(&format!("  -> queued emblem buff_key={} duration={} user={:016X}", buff_key, duration, detected_user_id));
        state.buff_queue.push(BufferedBuff {
            user_id: detected_user_id,
            buff_key,
            duration,
            detected_at: Instant::now(),
        });
    }

    // After each SELF_DAMAGE, check buffer against candidates
    if candidates_updated {
        let now = Instant::now();
        state.buff_queue.retain(|b| now.duration_since(b.detected_at).as_secs_f64() < MAX_BUFF_AGE_SECS);

        if let Some(idx) = state.buff_queue.iter().position(|b| state.candidates.contains(&b.user_id)) {
            let b = &state.buff_queue[idx];
            dlog(&format!("  -> MATCH: emitting timer buff_key={} duration={}", b.buff_key, b.duration));
            emit_buff(b.buff_key, b.duration, b.detected_at);
            state.buff_queue.drain(..=idx);
        } else {
            dlog(&format!("  -> no match: candidates=[{:016X},{:016X}] queue={}",
                state.candidates[0], state.candidates[1], state.buff_queue.len()));
        }
    }
}

fn find_marker(data: &[u8], marker: &[u8]) -> Option<usize> {
    data.windows(marker.len()).position(|w| w == marker)
}
