# IME Caret 2.0

Windows에서 현재 입력 가능한 텍스트 캐럿 옆에 IME 상태(한/영/일)를 작게 표시하는 Rust 프로그램입니다.

## 상태 표시

상태 표시 문자는 다음과 같습니다.

| 상태 | 표시 |
|---|---|
| 한글 | `가` |
| 영문, Caps Lock 꺼짐 | `a` |
| 영문, Caps Lock 켜짐 | `A` |
| 일본어 히라가나 | `ひ` |
| 일본어 가타카나 | `カ` |

## 설정

트레이 아이콘을 우클릭하면 설정 메뉴가 있습니다. 설정에서는 다음 항목을 변경할 수 있습니다.

- 상태 변경 소리 전체 사용 여부
- 영문 전환 소리 재생 여부
- 일본어 전환 소리 재생 여부
- 한글 전환 소리 재생 여부
- 상태 표시 위치 설정 (캐럿 우측 / 캐럿 우상단 / 캐럿 우하단)
- 상태 표시 글자색 및 배경색 설정

기본 설정 파일:

```ini
[Settings]
PlayEnglishSound=1
PlayJapaneseSound=1
PlayKoreanSound=1
PlaySounds=0
IndicatorPosition=Below
IndicatorTextColor=FFFFFFA5
EnglishBackgroundColor=FF6262A5
JapaneseBackgroundColor=62FF62A5
KoreanBackgroundColor=6262FFA5
```

`IndicatorPosition`에는 `Right`, `Above`, `Below` 중 하나를 사용할 수 있습니다.
Color값은 RRGGBBAA 형식입니다 (AA=00이면 완전 투명, AA=FF이면 완전 불투명).

## 2.0 변경 사항

- Excel 일부 버전에서 상태 표시 안되던 문제 수정.

## 1.9 변경 사항

- 상태(한/영/일)에 따라 상태 표시 글자색 및 배경색을 설정할 수 있는 기능 추가.

## 1.8 변경 사항

- 아웃룩에서 약속 본문 작성 시, 작업 본문 작성 시, 연락처 메모 작성 시 상태 표시 안되던 문제 수정.

## 1.7 변경 사항

- 서로 다른 배율의 모니터 사이에서 캐럿 좌표가 어긋나고 입력할수록 오차가 커지는 문제 수정.

## 1.6 변경 사항

- Word 와 PowerPoint 에서 상태 표시 안되던 문제 수정.

## 1.5 변경 사항

- 최초 공개.

## 소스 구조

```text
IME-Caret
├─ Cargo.toml
├─ Cargo.lock
├─ IMECaret.ini
├─ build.cmd
├─ build.ps1
├─ assets
│  ├─ IMEE.wav
│  ├─ IMEJ.wav
│  └─ IMEK.wav
├─ src
│  ├─ main.rs          애플리케이션, 캐럿 표시, 트레이 및 설정 UI
│  ├─ editability.rs   편집 가능 여부 및 캐럿 위치 탐색
│  ├─ ime.rs           포커스 입력 컨트롤의 IME 상태 조회
│  ├─ outlook.rs       Outlook 읽기·작성 상태 조회
│  ├─ config.rs        INI 설정
│  ├─ assets.rs        고정 앱/트레이 아이콘 데이터
│  └─ win.rs           Win32 및 UI Automation FFI
└─ tools
   └─ static_check.py
```

## 빌드

필요 환경:

- Windows 10 또는 Windows 11
- Rust stable MSVC toolchain
- Visual Studio C++ Build Tools

프로젝트 폴더에서 실행합니다.

```bat
build.cmd
```

또는 PowerShell에서 실행합니다.

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\build.ps1
```

빌드 결과는 `dist` 폴더에 생성됩니다.

```text
dist\IMECaret.exe
dist\IMECaret.ini
dist\IMEE.wav
dist\IMEJ.wav
dist\IMEK.wav
```
