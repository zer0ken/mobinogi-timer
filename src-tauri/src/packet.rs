use crate::DETECTED_BUFF_KEY;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Instant;

const START_MARKER: [u8; 9] = [0x80, 0x4E, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];
const END_MARKER: [u8; 9] = [0x12, 0x4F, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];
const BUFF_START_DATA_TYPE: u32 = 100054;
const SELF_DAMAGE_DATA_TYPE: u32 = 20897;

const EMBLEM_BUFF_KEYS: &[u32] = &[
    122806656,  // grand_mage (20s)
    355098955,  // scattering_sword (35s)
    1184371696, // cracked_earth (35s)
    1590198662, // distant_light (20s)
    1703435864, // broken_sky (20s)
    2024838942, // mountain_lord (20s)
];

// Virtual buff keys for runes that share buffKey 122806656
pub const GRAND_MAGE_KEY: u32 = 122806656;
pub const MERCILESS_PREDATOR_KEY: u32 = 122806657; // virtual
pub const MELTED_EARTH_KEY: u32 = 122806658;       // virtual

/// Microseconds elapsed since a buffered buff was first detected (0 for immediate detection).
pub static DETECTED_BUFF_ELAPSED_US: AtomicU32 = AtomicU32::new(0);

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

// --- Capture state ---

struct CaptureState {
    id_frequency: HashMap<u32, u32>,
    my_character_id: Option<u32>,
    pending_buffs: Vec<PendingBuff>,
}

struct PendingBuff {
    user_id: u32,
    buff_key: u32,
    detected_at: Instant,
}

// --- Capture entry points ---

pub fn start_capture(interface_name: &str, stop: Arc<AtomicBool>) {
    let device = if interface_name.is_empty() {
        match pcap::Device::lookup() {
            Ok(Some(d)) => d,
            Ok(None) => return,
            Err(_) => return,
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
        id_frequency: HashMap::new(),
        my_character_id: None,
        pending_buffs: Vec::new(),
    };

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

/// Returns IDs with the highest frequency (candidates for our character).
fn get_candidates(id_frequency: &HashMap<u32, u32>) -> Vec<u32> {
    if id_frequency.is_empty() {
        return vec![];
    }
    let max_freq = *id_frequency.values().max().unwrap();
    id_frequency
        .iter()
        .filter(|(_, &count)| count == max_freq)
        .map(|(&id, _)| id)
        .collect()
}

fn confirm_character_id(state: &mut CaptureState, id: u32) {
    state.my_character_id = Some(id);
}

fn emit_buff(buff_key: u32, elapsed_us: u32) {
    DETECTED_BUFF_KEY.store(buff_key, Ordering::Relaxed);
    DETECTED_BUFF_ELAPSED_US.store(elapsed_us, Ordering::Relaxed);
}

fn process_block(block: &[u8], state: &mut CaptureState) {
    let mut detected_user_id = 0u32;
    let mut detected_raw_key: Option<u32> = None;
    let mut has_marker_100175 = false;
    let mut has_marker_100055 = false;
    let mut freq_updated = false;

    for entry in iter_data_entries(block) {
        match entry.data_type {
            SELF_DAMAGE_DATA_TYPE if entry.content.len() >= 12 => {
                if state.my_character_id.is_some() {
                    continue;
                }

                let user_id =
                    u32::from_le_bytes(entry.content[0..4].try_into().unwrap_or([0; 4]));
                let target_id =
                    u32::from_le_bytes(entry.content[8..12].try_into().unwrap_or([0; 4]));

                if user_id > 0 {
                    *state.id_frequency.entry(user_id).or_insert(0) += 1;
                }
                if target_id > 0 && target_id != user_id {
                    *state.id_frequency.entry(target_id).or_insert(0) += 1;
                }
                freq_updated = true;
            }
            BUFF_START_DATA_TYPE
                if detected_raw_key.is_none() && entry.content.len() >= 4 =>
            {
                let buff_user_id =
                    u32::from_le_bytes(entry.content[0..4].try_into().unwrap_or([0; 4]));
                for &buff_key in EMBLEM_BUFF_KEYS {
                    if scan_for_u32(entry.content, buff_key) {
                        detected_user_id = buff_user_id;
                        detected_raw_key = Some(buff_key);
                        break;
                    }
                }
            }
            100175 => has_marker_100175 = true,
            100055 => has_marker_100055 = true,
            _ => {}
        }
    }

    // Resolve shared buff key and handle emblem detection
    if let Some(raw_key) = detected_raw_key {
        let resolved = if raw_key == GRAND_MAGE_KEY {
            if has_marker_100175 {
                MERCILESS_PREDATOR_KEY
            } else if has_marker_100055 {
                MELTED_EARTH_KEY
            } else {
                GRAND_MAGE_KEY
            }
        } else {
            raw_key
        };

        match state.my_character_id {
            Some(my_id) if detected_user_id == my_id => {
                emit_buff(resolved, 0);
            }
            Some(_) => {}
            None => {
                let candidates = get_candidates(&state.id_frequency);
                if candidates.is_empty() {
                    state.pending_buffs.push(PendingBuff {
                        user_id: detected_user_id,
                        buff_key: resolved,
                        detected_at: Instant::now(),
                    });
                } else if candidates.contains(&detected_user_id) {
                    confirm_character_id(state, detected_user_id);
                    emit_buff(resolved, 0);
                }
            }
        }
    }

    // After frequency update, try to identify our character
    if freq_updated && state.my_character_id.is_none() {
        let candidates = get_candidates(&state.id_frequency);
        if candidates.len() == 1 {
            let my_id = candidates[0];
            confirm_character_id(state, my_id);
            let pending = std::mem::take(&mut state.pending_buffs);
            for pb in pending {
                if pb.user_id == my_id {
                    let elapsed_us = pb.detected_at.elapsed().as_micros() as u32;
                    emit_buff(pb.buff_key, elapsed_us);
                    break;
                }
            }
        } else if candidates.len() > 1 && !state.pending_buffs.is_empty() {
            let pending = std::mem::take(&mut state.pending_buffs);
            for pb in pending {
                if candidates.contains(&pb.user_id) {
                    let elapsed_us = pb.detected_at.elapsed().as_micros() as u32;
                    confirm_character_id(state, pb.user_id);
                    emit_buff(pb.buff_key, elapsed_us);
                    break;
                }
            }
        }
    }
}

fn find_marker(data: &[u8], marker: &[u8]) -> Option<usize> {
    data.windows(marker.len()).position(|w| w == marker)
}

fn scan_for_u32(data: &[u8], target: u32) -> bool {
    let target_bytes = target.to_le_bytes();
    data.windows(4).any(|w| w == target_bytes)
}
