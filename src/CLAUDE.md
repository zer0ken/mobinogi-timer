<claude-mem-context>
# Recent Activity

## 2026-02-25: v2026.2.2501 릴리즈

### 주요 변경사항
- **Npcap 안전 처리**: wpcap.dll delay-load 추가하여 Npcap 미설치 시에도 앱 실행 가능
- **네트워크 목록 갱신**: Npcap 설치 재확인 버튼 클릭 시 네트워크 인터페이스 목록 자동 갱신
- **버전 표시**: 설정 창 타이틀에 현재 버전 표시
- **업데이트 확인**: GitHub 최신 릴리즈 확인 및 업데이트 안내

### 기술 세부사항
- `build.rs`: `/DELAYLOAD:wpcap.dll` 링커 플래그 추가
- `lib.rs`: `is_npcap_installed()` 함수로 DLL 파일 존재 여부 체크, pcap 호출 전 가드 처리
- `settings.html`: `recheckNpcap()` 함수에서 설치 확인 후 `loadNetworkInterfaces()` 호출
- 버전 형식 변경: `YYYY.M.DDNN` (예: 2026.2.2501 = 2월 25일 첫 번째 릴리즈)

### 프로젝트 규칙
- **Npcap 링크**: https://npcap.com/dist/npcap-1.87.exe (모든 릴리즈 노트 하단에 필수 요구사항 명시)
- **릴리즈 순서**: 커밋 → 빌드 → 검수 → `gh release create --latest`
- **버전 형식**: 같은 날 추가 릴리즈는 patch 증가 (2501, 2502, ...)
</claude-mem-context>
