<claude-mem-context>
# packet.rs - 패킷 감지 구조 (2026-04-23 기준)

## 상수 (업데이트 시 수정 대상)
- `START_MARKER`, `END_MARKER`: 9바이트 프레임 마커
- `SELF_DAMAGE_DATA_TYPE` (20937): 전투 타격 패킷
- `BUFF_START_DATA_TYPE` (110055): 버프 적용 패킷
- `EMBLEM_FIELD_KEYS`: field[16:20] → buff_key 매핑 테이블

## 엠블럼 식별 방식
BUFF_START content[16:20] 4바이트가 엠블럼별 고유 식별자.
각성 시 고유값 1개 + 공유값(`80 E1 51 07`, `CC 79 DC 1B`) 2개가 세트로 등장.

| 엠블럼 | field[16:20] |
|--------|-------------|
| 대마법사 | `9E 5A 0D 72` |
| 무자비한 포식자 | `A8 0F 5F 44` |
| 녹아내린 대지 | `CA 1A 9C 5F` |
| 아득한 빛 | `86 81 C8 5E` |
| 흩날리는 검 | `4B 61 2A 15` |
| 갈라진 땅 | `F0 13 98 46` |
| 부서진 하늘 | `58 5E 88 65` |
| 산맥 군주 | `1E 97 B0 78` |

## BUFF_START 패킷 구조 (36바이트)
- [0:8]   user_id (u64 LE)
- [8:16]  buff_instance_id (u64 LE) — 세션마다 변동
- [16:20] buff_type_field (4B) — 엠블럼 식별자
- [20:24] duration (f32 LE) — 버프 지속시간(초)
- [24:28] counter (u32 LE)
- [28:36] user_id 반복 (u64 LE)
- [36]    flag (u8)

## SELF_DAMAGE 패킷 구조
- [0:8]  user_id (u64 LE) — 공격한 쪽
- [8:16] target_id (u64 LE) — 맞은 쪽

## 디버그 로깅
`#[cfg(debug_assertions)]`로 dev 빌드에서만 활성화.
`%APPDATA%\mobinogi-timer\debug.log`에 기록.
캡처 시작 시 로그 초기화, 경과 시간 형식(`[+ 0.000s]`).

# lib.rs - 주요 구조

## EMBLEMS 테이블
buff_key → name 매핑. duration은 패킷에서 직접 읽으므로 제거됨.

## DetectedBuff
`buff_key`, `duration`, `detected_at` 포함.
packet.rs에서 설정, lib.rs tick loop에서 소비.

## TimerState
- `start_with_emblem(remaining, total, elapsed, name)`: 각성 타이머 시작
- cooldown은 `compute_cooldown(&blind_seer)`로 계산
</claude-mem-context>
