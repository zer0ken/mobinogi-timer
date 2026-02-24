use crate::DETECTED_BUFF_KEY;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;

const START_MARKER: [u8; 9] = [0x80, 0x4E, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];
const END_MARKER: [u8; 9] = [0x12, 0x4F, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];
const BUFF_START_DATA_TYPE: u32 = 100054;
const SELF_DAMAGE_DATA_TYPE: u32 = 20897;
const SELF_DAMAGE_CAP: u64 = 2_095_071_572; // M-INBODY's damage cap

/// Learned character ID via accumulated self-damage totals (same as M-INBODY).
/// The userId with the highest total self-damage is identified as our own character.
static SELF_CHARACTER_ID: AtomicU32 = AtomicU32::new(0);

const EMBLEM_BUFF_KEYS: &[u32] = &[
    122806656,  // grand_mage (20s)
    355098955,  // scattering_sword (35s)
    1184371696, // cracked_earth (35s)
    1590198662, // distant_light (20s)
    1703435864, // broken_sky (20s)
    2024838942, // mountain_lord (20s)
];

pub fn start_capture(interface_name: &str, stop: Arc<AtomicBool>) {
    let device = if interface_name.is_empty() {
        match pcap::Device::lookup() {
            Ok(Some(d)) => d,
            Ok(None) => {
                eprintln!("[mobinogi] No network device found");
                return;
            }
            Err(e) => {
                eprintln!("[mobinogi] Failed to find network device: {}", e);
                return;
            }
        }
    } else {
        match pcap::Device::list() {
            Ok(devices) => match devices.into_iter().find(|d| d.name == interface_name) {
                Some(d) => d,
                None => {
                    eprintln!("[mobinogi] Interface not found: {}", interface_name);
                    return;
                }
            },
            Err(e) => {
                eprintln!("[mobinogi] Failed to list devices: {}", e);
                return;
            }
        }
    };

    eprintln!("[mobinogi] Opening capture on: {}", device.name);

    let mut cap = match pcap::Capture::from_device(device)
        .unwrap()
        .promisc(false)
        .snaplen(65535)
        .timeout(1000)
        .open()
    {
        Ok(c) => c,
        Err(e) => {
            eprintln!("[mobinogi] Failed to open capture: {}", e);
            return;
        }
    };

    if let Err(e) = cap.filter("tcp and src port 16000", true) {
        eprintln!("[mobinogi] Failed to set filter: {}", e);
        return;
    }

    eprintln!("[mobinogi] Packet capture started (learning character ID from SELF_DAMAGE)");

    let mut buffer: Vec<u8> = Vec::new();
    let mut self_damage_totals: HashMap<u32, u64> = HashMap::new();
    let mut leader_total: u64 = 0;

    loop {
        if stop.load(Ordering::Relaxed) {
            eprintln!("[mobinogi] Capture thread stopping");
            break;
        }
        match cap.next_packet() {
            Ok(packet) => {
                let payload = extract_tcp_payload(packet.data);
                if payload.is_empty() {
                    continue;
                }
                buffer.extend_from_slice(payload);
                process_buffer(&mut buffer, &mut self_damage_totals, &mut leader_total);
            }
            Err(pcap::Error::TimeoutExpired) => continue,
            Err(e) => {
                eprintln!("[mobinogi] Capture error: {}", e);
                break;
            }
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
                // Has netmask = real interface. Check if it has a routable IPv4.
                if let std::net::IpAddr::V4(ip) = addr.addr {
                    let octets = ip.octets();
                    // Skip loopback (127.x), link-local (169.254.x), and zeroes
                    if octets[0] == 127 || (octets[0] == 169 && octets[1] == 254) || octets[0] == 0 {
                        continue;
                    }
                    // Check netmask is reasonable (not /32 or /0)
                    if let std::net::IpAddr::V4(mask) = gw {
                        let mask_bits = u32::from_be_bytes(mask.octets());
                        if mask_bits != 0 && mask_bits != 0xFFFFFFFF {
                            eprintln!("[mobinogi] Default interface candidate: {} ({})", d.name, ip);
                            return d.name.clone();
                        }
                    }
                }
            }
        }
    }
    // Fallback to pcap default
    pcap::Device::lookup()
        .ok()
        .flatten()
        .map(|d| d.name)
        .unwrap_or_default()
}

fn extract_tcp_payload(data: &[u8]) -> &[u8] {
    // Ethernet(14) + IP(min 20) + TCP(min 20)
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

fn process_buffer(
    buffer: &mut Vec<u8>,
    self_damage_totals: &mut HashMap<u32, u64>,
    leader_total: &mut u64,
) {
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

        // Learn our character ID from SELF_DAMAGE packets
        learn_self_id(&block, self_damage_totals, leader_total);

        if let Some(buff_key) = check_block_for_any_buff(&block) {
            eprintln!("[mobinogi] Emblem awakening detected! buffKey={}", buff_key);
            DETECTED_BUFF_KEY.store(buff_key, Ordering::Relaxed);
        }
    }
}

/// Learn our character ID from SELF_DAMAGE data blocks by accumulating damage totals.
/// Same approach as M-INBODY: the userId with the highest total self-damage is our character.
/// Content layout: [userId: u32LE @ 0] [targetId: u32LE @ 4] [damage: u32LE @ 8] ...
fn learn_self_id(
    block: &[u8],
    totals: &mut HashMap<u32, u64>,
    leader_total: &mut u64,
) {
    let mut offset = 0;
    while offset + 9 <= block.len() {
        let data_type =
            u32::from_le_bytes(block[offset..offset + 4].try_into().unwrap_or([0; 4]));
        let length =
            u32::from_le_bytes(block[offset + 4..offset + 8].try_into().unwrap_or([0; 4]))
                as usize;
        let content_start = offset + 9;

        if content_start + length > block.len() {
            break;
        }

        if data_type == SELF_DAMAGE_DATA_TYPE && length >= 12 {
            let content = &block[content_start..content_start + length];
            let user_id = u32::from_le_bytes(content[0..4].try_into().unwrap_or([0; 4]));
            let damage = u32::from_le_bytes(content[8..12].try_into().unwrap_or([0; 4])) as u64;

            if user_id > 0 && damage > 0 && damage <= SELF_DAMAGE_CAP {
                let total = totals.entry(user_id).or_insert(0);
                *total = total.saturating_add(damage);

                if *total > *leader_total {
                    *leader_total = *total;
                    let prev = SELF_CHARACTER_ID.swap(user_id, Ordering::Relaxed);
                    if prev != user_id {
                        eprintln!(
                            "[mobinogi] Character ID identified: {} (total damage: {})",
                            user_id, *total
                        );
                    }
                }
            }
        }

        offset = content_start + length;
    }
}

fn find_marker(data: &[u8], marker: &[u8]) -> Option<usize> {
    data.windows(marker.len()).position(|w| w == marker)
}

// Virtual buff keys for runes that share buffKey 122806656
pub const GRAND_MAGE_KEY: u32 = 122806656;
pub const MERCILESS_PREDATOR_KEY: u32 = 122806657; // virtual
pub const MELTED_EARTH_KEY: u32 = 122806658;       // virtual

fn check_block_for_any_buff(block: &[u8]) -> Option<u32> {
    // Parse data blocks: [dataType: u32LE][length: u32LE][encodeType: u8][content...]
    let mut offset = 0;
    while offset + 9 <= block.len() {
        let data_type =
            u32::from_le_bytes(block[offset..offset + 4].try_into().unwrap_or([0; 4]));
        let length =
            u32::from_le_bytes(block[offset + 4..offset + 8].try_into().unwrap_or([0; 4]))
                as usize;
        let content_start = offset + 9;

        if content_start + length > block.len() {
            break;
        }

        if data_type == BUFF_START_DATA_TYPE {
            let content = &block[content_start..content_start + length];

            // BUFF_START content: [userId: u32LE @ 0] ... [buffKey: u32LE @ 16] ...
            // Only accept buffs from our own character
            let my_id = SELF_CHARACTER_ID.load(Ordering::Relaxed);
            if my_id != 0 && content.len() >= 4 {
                let buff_user_id =
                    u32::from_le_bytes(content[0..4].try_into().unwrap_or([0; 4]));
                if buff_user_id != my_id {
                    offset = content_start + length;
                    continue;
                }
            }

            for &buff_key in EMBLEM_BUFF_KEYS {
                if scan_for_u32(content, buff_key) {
                    if buff_key == GRAND_MAGE_KEY {
                        return Some(resolve_shared_buff(block));
                    }
                    return Some(buff_key);
                }
            }
        }

        offset = content_start + length;
    }
    None
}

/// For buffKey 122806656, distinguish runes by block-level marker data types
fn resolve_shared_buff(block: &[u8]) -> u32 {
    if block_has_data_type(block, 100175) {
        return MERCILESS_PREDATOR_KEY;
    }
    if block_has_data_type(block, 100055) {
        return MELTED_EARTH_KEY;
    }
    GRAND_MAGE_KEY
}

fn block_has_data_type(block: &[u8], target: u32) -> bool {
    let mut offset = 0;
    while offset + 9 <= block.len() {
        let data_type =
            u32::from_le_bytes(block[offset..offset + 4].try_into().unwrap_or([0; 4]));
        let length =
            u32::from_le_bytes(block[offset + 4..offset + 8].try_into().unwrap_or([0; 4]))
                as usize;
        let content_start = offset + 9;
        if content_start + length > block.len() {
            break;
        }
        if data_type == target {
            return true;
        }
        offset = content_start + length;
    }
    false
}

fn scan_for_u32(data: &[u8], target: u32) -> bool {
    let target_bytes = target.to_le_bytes();
    data.windows(4).any(|w| w == target_bytes)
}
