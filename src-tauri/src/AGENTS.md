# Codex Notes for Rust Source

## Packet Detection

`packet.rs` is game-update-sensitive. The main update points are:

- `START_MARKER`: currently `83 4E 00 00 00 00 00 00 00`.
- `END_MARKER`: currently `1A 4F 00 00 00 00 00 00 00`.
- `SELF_DAMAGE_DATA_TYPE`: currently `20937`.
- `BUFF_START_DATA_TYPE`: currently `110055`.
- `EMBLEM_FIELD_KEYS`: maps `BUFF_START` `content[16:20]` bytes to `buff_key`.

Known emblem field values:

| Emblem | `content[16:20]` |
| --- | --- |
| 대마법사 | `9E 5A 0D 72` |
| 무자비한 포식자 | `A8 0F 5F 44` |
| 녹아내린 대지 | `CA 1A 9C 5F` |
| 아득한 빛 | `86 81 C8 5E` |
| 흩날리는 검 | `4B 61 2A 15` |
| 갈라진 땅 | `F0 13 98 46` |
| 부서진 하늘 | `58 5E 88 65` |
| 산맥 군주 | `1E 97 B0 78` |

## Packet Structures

`BUFF_START` content layout:

- `[0:8]`: user id, `u64 LE`.
- `[8:16]`: buff instance id, `u64 LE`, changes by session.
- `[16:20]`: emblem discriminator bytes.
- `[20:24]`: duration, `f32 LE`, in seconds.
- `[24:28]`: counter, `u32 LE`.
- `[28:36]`: repeated user id, `u64 LE`.
- `[36]`: flag, `u8`.

`SELF_DAMAGE` content layout:

- `[0:8]`: user id, `u64 LE`.
- `[8:16]`: target id, `u64 LE`.

## Detection Model

- `BUFF_START` packets can include nearby players, so an emblem packet is only emitted after matching its user id against recent `SELF_DAMAGE` candidates.
- Emblem detections are buffered up to `MAX_BUFF_AGE_SECS` to allow the subsequent self-damage packet to identify the player.
- `DetectedBuff` includes `buff_key`, `duration`, and `detected_at`; `lib.rs` consumes it from the timer tick loop.
- `EMBLEM_FIELD_KEYS` values must match the `EMBLEMS` table in `lib.rs`.

## Debugging

- Debug logging is gated with `#[cfg(debug_assertions)]`.
- Development logs go to `%APPDATA%\mobinogi-timer\debug.log`.
- The capture start clears the log and records the first few raw payloads for marker-change diagnosis.
