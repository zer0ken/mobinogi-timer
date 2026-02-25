<claude-mem-context>
# Auto Version - 자동 버전

## Branch: auto

### 특징
- 패킷 캡처로 엠블럼 각성 자동 감지
- 네트워크 인터페이스 선택
- 자동으로 정확한 타이밍에 타이머 시작
- Npcap SDK 필요 (`C:\npcap-sdk`)

## 최근 업데이트 (2026-02-25)

### 버전: v2026.2.2501-auto
- 타이머 시간이 밀리는 현상 수정
- 쿨다운 중에도 재감지 시 타이머 재시작 가능
- 설정 창에 현재 버전 표시
- 새 버전이 나오면 설정 창 하단에 업데이트 안내 표시
- Npcap 미설치 시에도 앱이 안전하게 실행되도록 개선
- Npcap 설치 재확인 버튼 클릭 시 네트워크 목록 자동 갱신

### 브랜치 구조
- **main**: README + GitHub Actions workflows
- **manual**: 수동 버전 (키보드 훅)
- **auto**: 자동 버전 (패킷 캡처) ← 현재 브랜치

### 릴리즈
- GitHub Actions로 자동 빌드 및 릴리즈
- Workflow: `release-auto.yml` (main 브랜치)
- 업데이트 체크: 접미사 `-auto` 필터링으로 수동 버전과 독립
</claude-mem-context>
