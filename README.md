# KagiLink (카기링크)

KagiLink는 사용자가 승인한 비밀 문자열을 가까운 컴퓨터에 완전 오프라인 BLE로
일회성 암호화 전달하는 AreaTM의 Apache-2.0 오픈소스 프로젝트입니다.
공식 사이트: https://key.minori.in. [공개 저장소](https://github.com/areatmadm/kagilink).

**현재: Phase 0 연구용. 실제 비밀번호 전송, 저장소, PAKE, 제품용 BLE 전송 및 RDP
자동 입력은 구현/활성화되어 있지 않습니다.**

OS별 네이티브 BLE 시험판 소스가 추가되었습니다. Windows 수신기와 Android·
iOS/iPadOS 송신기, macOS 송수신기는 공개된 고정 문자열만 교환하도록 제한됩니다.
제품 앱이 아니며 플랫폼별 실제 검증 상태는 [구현 현황](docs/IMPLEMENTATION_STATUS.md),
빌드·기기 시험 순서는 [BLE 시험판 안내](docs/PHASE0_BLE_PROBE.md)를 따르세요.
Windows 진단 창은 빌드 후 `target/phase0/kagilink.exe`를 더블클릭해 엽니다.
`어댑터 확인`과 `BLE 연결 시험`을 직접 선택하고 결과를 창에서 확인할 수 있습니다.
기존 `kagilink-phase0-probe.exe`는 CLI 시험 도구로 유지합니다.
모바일 요구사항은 Android 14 이상과 iOS/iPadOS 18 이상입니다. Android는 최신
안정 SDK 기준, iOS는 iPhone Duo 대응을 위해 SDK 27.1로 빌드합니다. 두 모바일
화면에 Liquid Glass 스타일을 적용하며, 기기별 렌더링·접근성 검증은 남아 있습니다.

Windows 보안 업데이트의 자격 증명 입력 제한과 SPAKE2 구현 선정 문제가
출시 전제조건입니다. 수신 PC가 감염되면 복호화된 비밀번호가 노출될 수 있습니다.

## 로컬 합성 검증

Rust 1.85 이상과 Windows의 경우 Visual Studio 2022 C++ 도구/Windows SDK가 필요합니다.

```powershell
cargo test --workspace --locked
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
.\apps\windows\build-probe.ps1
.\target\phase0\kagilink.exe
.\target\phase0\kagilink-phase0-probe.exe --inventory
```

`--inventory`는 BLE 어댑터 API와 TPM 제공자 접근만 확인합니다. 실제 BLE 상호운용,
TPM 키 생성/사용 또는 RDP 입력 성공을 의미하지 않습니다. 자체 창에 고정 합성
문자열을 넣는 수동 실험은 [Windows 안내](apps/windows/README.md)를 따르세요.

정책: [AGENTS.md](AGENTS.md). 진행 상태: [IMPLEMENTATION_STATUS](docs/IMPLEMENTATION_STATUS.md).
AI 작업 기록은 별도 비공개 저장소로 관리합니다. [접근 및 설치 안내](docs/AI_MEMORY.md).
설계/검증 근거: [ARCHITECTURE](docs/ARCHITECTURE.md), [PHASE0_RESEARCH](docs/PHASE0_RESEARCH.md),
[TEST_PLAN](docs/TEST_PLAN.md). 공개 요구사항은 [REQUIREMENTS](docs/REQUIREMENTS.md)에 정리되어 있습니다.
명칭 이관 및 호환성 기록: [KagiLink migration](docs/MIGRATION_KAGILINK.md).
