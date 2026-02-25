<claude-mem-context>
# Recent Activity

## 2026-02-25: 독립 버전 체계 도입

### 프로젝트 구조
- **main 브랜치**: 수동 버전 (단축키 기반)
- **npcap 브랜치**: 자동 버전 (패킷 캡처)
- 각 브랜치는 독립적으로 릴리즈

### 버전 체계
- **형식**: `YYYY.M.DDNN-{manual|auto}`
  - 예: `2026.2.2501-manual`, `2026.2.2501-auto`
  - 날짜 기반 유지 + 접미사로 버전 구분
- **Semver 호환**: prerelease 형식 사용
- **업데이트 체크**: 접미사별 필터링 (manual은 manual만, auto는 auto만)

### GitHub Actions 자동화
- **Workflow**: `.github/workflows/release-manual.yml`, `release-auto.yml`
- **트리거**:
  - 태그 푸시: `v*-manual`, `v*-auto`
  - 수동 실행: workflow_dispatch (릴리즈 노트 입력 가능)
- **빌드**: Windows 환경에서 자동 빌드 및 릴리즈 생성

### 릴리즈 프로세스
1. **수동 실행 (추천)**:
   - GitHub Actions → 해당 workflow 선택
   - Run workflow → 버전과 릴리즈 노트 입력
   - 자동 빌드 & 릴리즈 생성

2. **태그 푸시**:
   ```bash
   git tag v2026.2.25-manual
   git push origin v2026.2.25-manual
   ```

### 주요 기능
- **수동 버전**: 단축키 설정, 엠블럼 선택, 자동 반복, 타이머 힌트
- **자동 버전**: 패킷 캡처, 네트워크 인터페이스 설정, 자동 감지

### 기술 세부사항
- **check_update**: 접미사 필터링으로 독립 업데이트 체크
  - 레거시 마이그레이션: `2026.2.2501` → `2026.2.2501-auto` 자동 감지
- **릴리즈 노트**: Warning (비인가 프로그램), Npcap 설치 안내 포함 (자동 버전)
</claude-mem-context>
