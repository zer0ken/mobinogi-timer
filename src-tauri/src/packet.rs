use crate::HOTKEY_PRESSED;
use std::sync::atomic::Ordering;

const START_MARKER: [u8; 9] = [0x80, 0x4E, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];
const END_MARKER: [u8; 9] = [0x12, 0x4F, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];
const BUFF_START_DATA_TYPE: u32 = 100054;

pub fn start_capture(target_buff_key: u32, interface_name: &str) {
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

    eprintln!("[mobinogi] Packet capture started (buffKey={})", target_buff_key);

    let mut buffer: Vec<u8> = Vec::new();

    loop {
        match cap.next_packet() {
            Ok(packet) => {
                let payload = extract_tcp_payload(packet.data);
                if payload.is_empty() {
                    continue;
                }
                buffer.extend_from_slice(payload);
                process_buffer(&mut buffer, target_buff_key);
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

fn process_buffer(buffer: &mut Vec<u8>, target_buff_key: u32) {
    loop {
        let Some(start_pos) = find_marker(buffer, &START_MARKER) else {
            // Trim buffer if it's too large (keep tail for partial marker)
            if buffer.len() > 1024 * 1024 {
                let keep_from = buffer.len() - START_MARKER.len();
                buffer.drain(..keep_from);
            }
            break;
        };

        let data_start = start_pos + START_MARKER.len();
        let Some(end_offset) = find_marker(&buffer[data_start..], &END_MARKER) else {
            // Discard data before the start marker to prevent unbounded growth
            if start_pos > 0 {
                buffer.drain(..start_pos);
            }
            break;
        };

        let block = buffer[data_start..data_start + end_offset].to_vec();
        let remove_to = data_start + end_offset + END_MARKER.len();
        buffer.drain(..remove_to);

        if check_block_for_buff(&block, target_buff_key) {
            eprintln!("[mobinogi] Emblem awakening detected! buffKey={}", target_buff_key);
            HOTKEY_PRESSED.store(true, Ordering::Relaxed);
        }
    }
}

fn find_marker(data: &[u8], marker: &[u8]) -> Option<usize> {
    data.windows(marker.len()).position(|w| w == marker)
}

fn check_block_for_buff(block: &[u8], target_buff_key: u32) -> bool {
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
            // Scan content for target buff key as u32 LE
            if scan_for_u32(content, target_buff_key) {
                return true;
            }
        }

        offset = content_start + length;
    }
    false
}

fn scan_for_u32(data: &[u8], target: u32) -> bool {
    let target_bytes = target.to_le_bytes();
    data.windows(4).any(|w| w == target_bytes)
}
