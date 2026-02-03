//! 스크리닝 결과 및 GlobalScore 전략 연동.
//!
//! 전략이 StrategyContext에서 스크리닝 결과와 GlobalScore를 활용할 수 있도록 지원합니다.

use trader_core::domain::{RouteState, ScreeningResult, StrategyContext};
use trader_core::types::Symbol;

/// 스크리닝 결과 활용 trait.
///
/// 전략이 StrategyContext의 스크리닝 결과를 활용할 수 있도록 인터페이스를 제공합니다.
pub trait ScreeningAware {
    /// 스크리닝 결과를 전략에 설정.
    ///
    /// # 인자
    ///
    /// * `results` - 스크리닝 결과 목록
    fn set_screening_results(&mut self, results: Vec<ScreeningResult>);

    /// RouteState 기반 필터링.
    ///
    /// # 인자
    ///
    /// * `state` - 필터링할 RouteState
    ///
    /// # 반환
    ///
    /// 해당 상태의 종목 목록
    fn filter_by_route_state(&self, state: RouteState) -> Vec<&ScreeningResult>;

    /// GlobalScore 기반 필터링.
    ///
    /// # 인자
    ///
    /// * `min_score` - 최소 점수
    /// * `limit` - 최대 반환 개수
    ///
    /// # 반환
    ///
    /// 점수 높은 순으로 정렬된 종목 목록
    fn filter_by_global_score(&self, min_score: f32, limit: Option<usize>) -> Vec<&ScreeningResult>;
}

/// StrategyContext에서 특정 RouteState 종목 추출.
///
/// # 인자
///
/// * `context` - 전략 실행 컨텍스트
/// * `preset` - 스크리닝 프리셋 이름
/// * `state` - 필터링할 RouteState
///
/// # 반환
///
/// (Symbol, ScreeningResult) 쌍의 벡터
///
/// # 예시
///
/// ```rust,ignore
/// // ATTACK 상태 종목만 선택
/// let attack_symbols = get_symbols_by_route_state(&context, "kosdaq_momentum", RouteState::Attack);
/// ```
pub fn get_symbols_by_route_state<'a>(
    context: &'a StrategyContext,
    preset: &str,
    state: RouteState,
) -> Vec<(Symbol, &'a ScreeningResult)> {
    context
        .screening_results
        .get(preset)
        .map(|results| {
            results
                .iter()
                .filter(|r| r.route_state == state)
                .map(|r| (r.symbol.clone(), r))
                .collect()
        })
        .unwrap_or_default()
}

/// StrategyContext에서 GlobalScore 기반 상위 종목 추출.
///
/// # 인자
///
/// * `context` - 전략 실행 컨텍스트
/// * `preset` - 스크리닝 프리셋 이름
/// * `min_score` - 최소 GlobalScore
/// * `limit` - 최대 반환 개수
///
/// # 반환
///
/// (Symbol, ScreeningResult) 쌍의 벡터 (점수 높은 순)
///
/// # 예시
///
/// ```rust,ignore
/// // 80점 이상 상위 5개
/// let top_symbols = get_symbols_by_global_score(&context, "growth", 80.0, Some(5));
/// ```
pub fn get_symbols_by_global_score<'a>(
    context: &'a StrategyContext,
    preset: &str,
    min_score: f32,
    limit: Option<usize>,
) -> Vec<(Symbol, &'a ScreeningResult)> {
    context
        .screening_results
        .get(preset)
        .map(|results| {
            let mut filtered: Vec<_> = results
                .iter()
                .filter(|r| r.overall_score >= min_score)
                .map(|r| (r.symbol.clone(), r))
                .collect();

            // overall_score 내림차순 정렬
            filtered.sort_by(|a, b| {
                b.1.overall_score
                    .partial_cmp(&a.1.overall_score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });

            // limit 적용
            if let Some(limit) = limit {
                filtered.truncate(limit);
            }

            filtered
        })
        .unwrap_or_default()
}

/// 섹터별 상위 N개 종목 추출.
///
/// # 인자
///
/// * `context` - 전략 실행 컨텍스트
/// * `preset` - 스크리닝 프리셋 이름
/// * `top_n_per_sector` - 섹터당 상위 개수
///
/// # 반환
///
/// (Symbol, ScreeningResult) 쌍의 벡터
///
/// # 예시
///
/// ```rust,ignore
/// // 섹터별 상위 5개
/// let sector_leaders = get_top_symbols_per_sector(&context, "sector_rotation", 5);
/// ```
pub fn get_top_symbols_per_sector<'a>(
    context: &'a StrategyContext,
    preset: &str,
    top_n_per_sector: usize,
) -> Vec<(Symbol, &'a ScreeningResult)> {
    use std::collections::HashMap;

    context
        .screening_results
        .get(preset)
        .map(|results| {
            // 섹터별로 그룹화 (sector_rs 필드 사용)
            let mut by_sector: HashMap<String, Vec<&ScreeningResult>> = HashMap::new();

            for result in results.iter() {
                // 섹터 정보가 있으면 그룹화
                if let Some(sector_rs) = result.sector_rs {
                    let sector_key = format!("sector_{}", sector_rs as i32); // 간단한 섹터 키
                    by_sector.entry(sector_key).or_default().push(result);
                }
            }

            // 각 섹터에서 상위 N개 선택 (overall_score 기준)
            let mut top_symbols = Vec::new();

            for (_sector, mut sector_results) in by_sector {
                // 점수 기준 정렬
                sector_results.sort_by(|a, b| {
                    b.overall_score
                        .partial_cmp(&a.overall_score)
                        .unwrap_or(std::cmp::Ordering::Equal)
                });

                // 상위 N개 선택
                for result in sector_results.iter().take(top_n_per_sector) {
                    top_symbols.push((result.symbol.clone(), *result));
                }
            }

            top_symbols
        })
        .unwrap_or_default()
}

/// 복합 필터: RouteState + GlobalScore.
///
/// RouteState로 먼저 필터링하고, GlobalScore로 재정렬.
///
/// # 인자
///
/// * `context` - 전략 실행 컨텍스트
/// * `preset` - 스크리닝 프리셋 이름
/// * `state` - RouteState
/// * `min_score` - 최소 GlobalScore
/// * `limit` - 최대 반환 개수
///
/// # 반환
///
/// (Symbol, ScreeningResult) 쌍의 벡터
pub fn get_symbols_by_state_and_score<'a>(
    context: &'a StrategyContext,
    preset: &str,
    state: RouteState,
    min_score: f32,
    limit: Option<usize>,
) -> Vec<(Symbol, &'a ScreeningResult)> {
    context
        .screening_results
        .get(preset)
        .map(|results| {
            let mut filtered: Vec<_> = results
                .iter()
                .filter(|r| r.route_state == state && r.overall_score >= min_score)
                .map(|r| (r.symbol.clone(), r))
                .collect();

            // GlobalScore 내림차순 정렬
            filtered.sort_by(|a, b| {
                b.1.overall_score
                    .partial_cmp(&a.1.overall_score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });

            if let Some(limit) = limit {
                filtered.truncate(limit);
            }

            filtered
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use std::collections::HashMap;
    use trader_core::types::MarketType;

    fn create_test_screening_result(
        ticker: &str,
        route_state: RouteState,
        overall_score: f32,
        sector_rs: Option<f32>,
    ) -> ScreeningResult {
        ScreeningResult {
            symbol: Symbol::new(ticker, "", MarketType::KrStock),
            preset_name: "test".to_string(),
            passed: true,
            overall_score,
            route_state,
            criteria_results: HashMap::new(),
            timestamp: Utc::now(),
            sector_rs,
            sector_rank: None,
        }
    }

    #[test]
    fn test_get_symbols_by_route_state() {
        let mut screening_results = HashMap::new();
        screening_results.insert(
            "test_preset".to_string(),
            vec![
                create_test_screening_result("005930", RouteState::Attack, 85.0, None),
                create_test_screening_result("000660", RouteState::Wait, 65.0, None),
                create_test_screening_result("035420", RouteState::Attack, 90.0, None),
            ],
        );

        let context = StrategyContext::default();
        let context = StrategyContext {
            screening_results,
            ..context
        };

        let attack_symbols = get_symbols_by_route_state(&context, "test_preset", RouteState::Attack);
        assert_eq!(attack_symbols.len(), 2);
    }

    #[test]
    fn test_get_symbols_by_global_score() {
        let mut screening_results = HashMap::new();
        screening_results.insert(
            "test_preset".to_string(),
            vec![
                create_test_screening_result("005930", RouteState::Neutral, 85.5, None),
                create_test_screening_result("000660", RouteState::Neutral, 65.0, None),
                create_test_screening_result("035420", RouteState::Neutral, 90.2, None),
            ],
        );

        let context = StrategyContext::default();
        let context = StrategyContext {
            screening_results,
            ..context
        };

        let top_symbols = get_symbols_by_global_score(&context, "test_preset", 80.0, Some(2));
        assert_eq!(top_symbols.len(), 2);
        // 점수 순 정렬 확인
        assert_eq!(top_symbols[0].0.base, "035420"); // 90.2점
        assert_eq!(top_symbols[1].0.base, "005930"); // 85.5점
    }

    #[test]
    fn test_get_symbols_by_state_and_score() {
        let mut screening_results = HashMap::new();
        screening_results.insert(
            "test_preset".to_string(),
            vec![
                create_test_screening_result("005930", RouteState::Attack, 85.5, None),
                create_test_screening_result("000660", RouteState::Attack, 65.0, None),
                create_test_screening_result("035420", RouteState::Overheat, 90.2, None),
            ],
        );

        let context = StrategyContext::default();
        let context = StrategyContext {
            screening_results,
            ..context
        };

        // ATTACK 상태 + 80점 이상
        let filtered = get_symbols_by_state_and_score(
            &context,
            "test_preset",
            RouteState::Attack,
            80.0,
            None,
        );
        assert_eq!(filtered.len(), 1); // 005930만 해당
        assert_eq!(filtered[0].0.base, "005930");
    }
}
