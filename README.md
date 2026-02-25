# 모비노기 각성 타이머 — 자동 버전

> [!WARNING]
> 이 프로그램은 비인가 프로그램으로 판단될 여지가 있습니다. 프로그램 설치 전에 [이슈 #2](https://github.com/zer0ken/mobinogi-timer/issues/2)를 확인해주세요.

마비노기 모바일 각성 패시브의 지속시간과 쿨다운을 추적하는 오버레이 타이머입니다.

패킷 캡처를 통해 엠블럼 각성 발동을 자동으로 감지합니다.

## 기능

- **자동 각성 감지** — 패킷 캡처를 통해 엠블럼 각성 발동을 자동으로 감지합니다.
- **엠블럼 자동 인식** — 대마법사, 흩날리는 검, 갈라진 땅 등 엠블럼 종류를 자동으로 인식하여 지속시간을 적용합니다.
- **눈먼예언자 룬 지원** — 착용/초월 단계에 따라 쿨다운이 자동 계산됩니다.
- **오버레이 타이머 바** — 게임 화면 위에 항상 표시되어, 게임 중에도 타이머를 확인할 수 있습니다.
- **투명도/너비 조절** — 타이머 바의 투명도와 너비를 자유롭게 조절할 수 있습니다.
- **위치 저장** — 타이머 바를 드래그하여 이동하면 위치가 자동 저장됩니다.

## 설치

1. [Npcap](https://npcap.com/dist/npcap-1.87.exe)을 설치합니다. **(필수)**
2. [최신 릴리즈](https://github.com/zer0ken/mobinogi-timer/releases)에서 `mobinogi-timer-auto.exe`를 다운로드하여 실행하세요.

## 사용법

### 설정

1. **눈먼예언자** — 룬 착용 여부와 초월 단계를 선택합니다.
2. **투명도** — 타이머 바의 투명도를 조절합니다.
3. **바 너비** — 타이머 바의 너비를 조절합니다.
4. **네트워크** — 게임이 사용하는 네트워크 인터페이스를 선택합니다.

설정 창이 선택된 상태에서 타이머 바를 드래그하여 원하는 위치로 이동할 수 있습니다.

### 타이머

게임 플레이 중 각성이 발동되면 타이머가 자동으로 시작됩니다.

| 색상 | 상태 |
|------|------|
| 파랑 | 각성 준비됨 |
| 초록 | 각성 효과 지속 중 |
| 빨강 | 각성 쿨다운 진행 중 |

## 직접 빌드하기

### 필요한 프로그램

- [Node.js](https://nodejs.org/) (v18 이상)
- [Rust](https://www.rust-lang.org/tools/install) (1.70 이상)
- [Npcap SDK](https://npcap.com/#download) — `C:\npcap-sdk`에 설치
- Windows 10/11

### 빌드 방법

```bash
git clone https://github.com/zer0ken/mobinogi-timer.git
cd mobinogi-timer
git checkout auto
npm install
npx tauri build
```

완성된 파일 위치: `src-tauri/target/release/mobinogi-timer.exe`

### 개발 모드

```bash
npx tauri dev
```

## 라이선스

MIT
