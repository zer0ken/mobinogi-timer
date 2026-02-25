<claude-mem-context>
# Recent Activity

## 2026-02-25: 독립 버전 체계 전환

### 변경사항
- 패킷 캡처 제거, 수동 단축키 방식으로 복귀 (main 브랜치)
- 날짜 기반 버전 + 접미사 체계 도입 (2026.2.2501-manual, 2026.2.2501-auto)
- 독립적인 릴리즈 관리 (main: 수동, npcap: 자동)
- GitHub Actions 자동화 (릴리즈 노트 입력 기능)
- check_update 함수에 접미사 필터링 추가

### 아키텍처
- **main 브랜치**: 키보드 훅 기반 수동 타이머
  - 엠블럼 선택, 단축키 설정, 자동 반복
- **npcap 브랜치**: 패킷 캡처 기반 자동 타이머
  - 네트워크 인터페이스 선택, 자동 감지
</claude-mem-context>
