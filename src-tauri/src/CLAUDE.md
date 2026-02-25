<claude-mem-context>
# Recent Activity

## 2026-02-25: lib.rs 주요 변경

### check_update 함수
- 접미사 필터링 추가: `-manual`, `-auto` 별도 추적
- 레거시 마이그레이션: 접미사 없는 버전 → `-auto` 자동 매칭
- 모든 릴리즈 조회 후 메이저 버전 + 접미사로 필터링

### TimerState (main 브랜치)
- `start()` 메서드: `selected_emblem`로 타이머 시작
- 자동 반복 지원: cooldown 종료 시 `auto_repeat` 체크

### Global State (main 브랜치)
- `HOTKEY_PRESSED`: 키보드 훅에서 설정
- `HOTKEY_VK`: 현재 단축키 가상 키 코드
- `install_keyboard_hook()`: Windows 키보드 훅 설치

### Settings (main 브랜치)
- `hotkey_vk`, `hotkey_name`: 단축키 설정
- `auto_repeat`: 자동 반복 옵션
- `selected_emblem`: 선택된 엠블럼
</claude-mem-context>
