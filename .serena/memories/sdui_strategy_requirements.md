# SDUI 전략 UI 요구사항

## 2026-02-05 사용자 요청사항

### 1. Symbol 컴포넌트 (완료 ✅)
- 티커를 사용하는 컴포넌트는 **SymbolSearch** 컴포넌트 사용
- SymbolDisplay 계열 컴포넌트로 심볼 검색/선택 UI 제공

### 2. 파생 전략의 variant 필드 (진행 중 🔄)
- 파생 전략(RSI, Bollinger, Grid, MagicSplit 등)에서는 **variant가 이미 고정**
- UI에서 variant 필드를 **표시하지 않음** (hidden 처리)
- 각 파생 전략은 이미 특정 variant를 가지고 있어야 함

### 3. Risk 옵션 (완료 ✅)
- 모든 전략에서 사용 가능한 **모든 risk 옵션 제공**
- `risk.exit_config` fragment에 손절/익절/트레일링 스탑 모두 포함

### 4. 기본값 (진행 중 🔄)
- 모든 파라미터에는 **해당 전략에서 권장되는 기본값** 필수
- **시그널이 동작하는 파라미터**를 기본값으로 설정
- 테스트를 통해 기본값으로 시그널이 발생하는지 확인

## 구현 상태

| 항목 | 상태 | 비고 |
|------|------|------|
| SymbolSearch 적용 | ✅ | SDUIField, MultiSymbolInput |
| hidden 필드 지원 | 🔄 | FieldSchema, 매크로 수정 중 |
| risk.exit_config 확장 | ✅ | 트레일링 스탑 추가 |
| 기본값 설정 | 🔄 | 각 전략별 기본값 검토 필요 |
| 파생 전략 분리 | 📋 | 계획 필요 |
