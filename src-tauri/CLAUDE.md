<claude-mem-context>
# Auto Version - 자동 버전

## Branch: auto

### 특징
- 패킷 캡처로 엠블럼 각성 자동 감지
- 네트워크 인터페이스 선택
- 자동으로 정확한 타이밍에 타이머 시작
- Npcap SDK 필요 (`C:\npcap-sdk`)

## 최근 업데이트 (2026-04-09)

### 패킷 감지 방식 전면 재작성 (게임 업데이트 대응)
- START_MARKER: `82 4E 00...` (이전: `80 4E 00...`)
- END_MARKER: `18 4F 00...` (이전: `12 4F 00...`)
- SELF_DAMAGE 타입: 20897 → 20919, user/target ID: u32(4B) → u64(8B)
- BUFF_START 타입: 100054 → 100055
- 엠블럼 감지: u32 키 스캔 → BUFF_START `field[16:20]` 고유값 매칭으로 변경
  - 오탐 제거: 비엠블럼 20초 버프와 구분 가능
  - 각 엠블럼별 고유 4바이트 값으로 정확히 식별
- 타이머 duration: 하드코딩 → 패킷에서 직접 읽음
- 디버그 로그: `debug_assertions` 플래그로 dev/release 자동 분리
  - dev 빌드: `%APPDATA%\mobinogi-timer\debug.log`
  - raw payload 첫 5개 항상 로깅 (마커 변경 진단용)

### 이전 업데이트 (2026-02-25)
- 타이머 시간이 밀리는 현상 수정
- 쿨다운 중에도 재감지 시 타이머 재시작 가능
- 설정 창에 현재 버전 표시 / 업데이트 안내
- Npcap 미설치 시 안전 실행

### 브랜치 구조
- **main**: README + GitHub Actions workflows
- **manual**: 수동 버전 (키보드 훅)
- **auto**: 자동 버전 (패킷 캡처) ← 현재 브랜치

### 릴리즈
- GitHub Actions로 자동 빌드 및 릴리즈
- Workflow: `release-auto.yml` (main 브랜치)
- 업데이트 체크: 접미사 `-auto` 필터링으로 수동 버전과 독립
</claude-mem-context>
