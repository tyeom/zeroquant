//! GlobalScore 유틸리티 함수.
//!
//! StrategyContext에서 GlobalScore를 활용하여 종목을 선택하고 가중치를 계산합니다.

use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use std::collections::HashMap;

use trader_core::domain::{GlobalScoreResult, StrategyContext};
use trader_core::types::Symbol;

/// GlobalScore 기반 종목 필터링 옵션.
#[derive(Debug, Clone)]
pub struct ScoreFilterOptions {
    /// 최소 점수 (0~100)
    pub min_score: Option<f32>,
    /// 필수 등급 (BUY, WATCH, HOLD, AVOID)
    pub required_grade: Option<String>,
    /// 최소 신뢰도 (HIGH, MEDIUM, LOW)
    pub min_confidence: Option<String>,
    /// 최대 반환 개수
    pub limit: Option<usize>,
}

impl Default for ScoreFilterOptions {
    fn default() -> Self {
        Self {
            min_score: Some(70.0), // 기본 70점 이상
            required_grade: None,
            min_confidence: None,
            limit: Some(10), // 기본 상위 10개
        }
    }
}

/// StrategyContext에서 GlobalScore 기반 상위 종목 선택.
///
/// # 인자
///
/// * `context` - 전략 실행 컨텍스트
/// * `options` - 필터링 옵션
///
/// # 반환
///
/// (Symbol, GlobalScoreResult) 쌍의 벡터 (overall_score DESC 정렬)
///
/// # 예시
///
/// ```rust,ignore
/// let options = ScoreFilterOptions {
///     min_score: Some(75.0),
///     required_grade: Some("BUY".to_string()),
///     limit: Some(5),
///     ..Default::default()
/// };
///
/// let top_symbols = select_top_symbols(&context, options);
/// for (symbol, score) in top_symbols {
///     println!("{}: {}", symbol, score.overall_score);
/// }
/// ```
pub fn select_top_symbols(
    context: &StrategyContext,
    options: ScoreFilterOptions,
) -> Vec<(Symbol, GlobalScoreResult)> {
    let mut results: Vec<_> = context
        .global_scores
        .iter()
        .filter(|(_, score)| {
            // 최소 점수 필터
            if let Some(min) = options.min_score {
                if score.overall_score < min {
                    return false;
                }
            }

            // 등급 필터
            if let Some(ref required_grade) = options.required_grade {
                if &score.recommendation != required_grade {
                    return false;
                }
            }

            // 신뢰도 필터
            if let Some(ref min_conf) = options.min_confidence {
                let conf_level = if score.confidence >= 0.8 {
                    "HIGH"
                } else if score.confidence >= 0.6 {
                    "MEDIUM"
                } else {
                    "LOW"
                };

                // HIGH > MEDIUM > LOW 순서
                let required_level = match min_conf.as_str() {
                    "HIGH" => 2,
                    "MEDIUM" => 1,
                    _ => 0,
                };

                let current_level = match conf_level {
                    "HIGH" => 2,
                    "MEDIUM" => 1,
                    _ => 0,
                };

                if current_level < required_level {
                    return false;
                }
            }

            true
        })
        .map(|(sym, score)| (sym.clone(), score.clone()))
        .collect();

    // overall_score 내림차순 정렬
    results.sort_by(|a, b| {
        b.1.overall_score
            .partial_cmp(&a.1.overall_score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    // limit 적용
    if let Some(limit) = options.limit {
        results.truncate(limit);
    }

    results
}

/// 특정 종목의 GlobalScore 조회.
///
/// # 인자
///
/// * `context` - 전략 실행 컨텍스트
/// * `symbol` - 조회할 종목
///
/// # 반환
///
/// GlobalScoreResult (없으면 None)
pub fn get_score<'a>(context: &'a StrategyContext, symbol: &Symbol) -> Option<&'a GlobalScoreResult> {
    context.global_scores.get(symbol)
}

/// GlobalScore를 포지션 사이징 가중치로 변환.
///
/// # 점수별 가중치
///
/// - 90점 이상: 1.5x (최대)
/// - 80~90점: 1.2x
/// - 70~80점: 1.0x (기본)
/// - 60~70점: 0.8x
/// - 60점 미만: 0.5x (최소)
///
/// # 인자
///
/// * `score` - GlobalScore (0~100)
///
/// # 반환
///
/// 가중치 (0.5 ~ 1.5)
pub fn calculate_score_weight(score: f32) -> Decimal {
    if score >= 90.0 {
        dec!(1.5)
    } else if score >= 80.0 {
        dec!(1.2)
    } else if score >= 70.0 {
        dec!(1.0)
    } else if score >= 60.0 {
        dec!(0.8)
    } else {
        dec!(0.5)
    }
}

/// GlobalScore를 리스크 조정 계수로 변환.
///
/// 높은 점수일수록 리스크를 더 감수 (더 큰 포지션)
///
/// # 점수별 계수
///
/// - 90점 이상: 1.0 (리스크 그대로)
/// - 80~90점: 0.8
/// - 70~80점: 0.6
/// - 60~70점: 0.4
/// - 60점 미만: 0.2 (최소 리스크)
///
/// # 인자
///
/// * `score` - GlobalScore (0~100)
///
/// # 반환
///
/// 리스크 조정 계수 (0.2 ~ 1.0)
pub fn calculate_risk_adjustment(score: f32) -> Decimal {
    if score >= 90.0 {
        dec!(1.0)
    } else if score >= 80.0 {
        dec!(0.8)
    } else if score >= 70.0 {
        dec!(0.6)
    } else if score >= 60.0 {
        dec!(0.4)
    } else {
        dec!(0.2)
    }
}

/// 종목 그룹을 GlobalScore로 가중 평균.
///
/// 포트폴리오 리밸런싱 시 사용.
///
/// # 인자
///
/// * `scores` - 종목별 GlobalScore
///
/// # 반환
///
/// 가중 평균 점수
pub fn calculate_weighted_average(scores: &HashMap<Symbol, GlobalScoreResult>) -> f32 {
    if scores.is_empty() {
        return 0.0;
    }

    let total_score: f32 = scores.values().map(|s| s.overall_score).sum();
    total_score / scores.len() as f32
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use trader_core::types::MarketType;

    fn create_test_score(symbol: &str, score: f32, grade: &str, confidence: f32) -> (Symbol, GlobalScoreResult) {
        let sym = Symbol::new(symbol, "", MarketType::KrStock);
        let result = GlobalScoreResult {
            symbol: Some(sym.clone()),
            market_type: Some(MarketType::KrStock),
            overall_score: score,
            component_scores: Default::default(),
            recommendation: grade.to_string(),
            confidence,
            timestamp: Utc::now(),
        };
        (sym, result)
    }

    #[test]
    fn test_calculate_score_weight() {
        assert_eq!(calculate_score_weight(95.0), dec!(1.5));
        assert_eq!(calculate_score_weight(85.0), dec!(1.2));
        assert_eq!(calculate_score_weight(75.0), dec!(1.0));
        assert_eq!(calculate_score_weight(65.0), dec!(0.8));
        assert_eq!(calculate_score_weight(50.0), dec!(0.5));
    }

    #[test]
    fn test_calculate_risk_adjustment() {
        assert_eq!(calculate_risk_adjustment(95.0), dec!(1.0));
        assert_eq!(calculate_risk_adjustment(85.0), dec!(0.8));
        assert_eq!(calculate_risk_adjustment(75.0), dec!(0.6));
        assert_eq!(calculate_risk_adjustment(65.0), dec!(0.4));
        assert_eq!(calculate_risk_adjustment(50.0), dec!(0.2));
    }

    #[test]
    fn test_select_top_symbols_with_min_score() {
        use trader_core::domain::{StrategyAccountInfo, ExchangeConstraints};
        use std::collections::HashMap;

        let mut scores = HashMap::new();
        scores.insert(create_test_score("A", 90.0, "BUY", 0.9).0, create_test_score("A", 90.0, "BUY", 0.9).1);
        scores.insert(create_test_score("B", 75.0, "BUY", 0.8).0, create_test_score("B", 75.0, "BUY", 0.8).1);
        scores.insert(create_test_score("C", 60.0, "WATCH", 0.7).0, create_test_score("C", 60.0, "WATCH", 0.7).1);

        let context = StrategyContext {
            account: StrategyAccountInfo::default(),
            positions: HashMap::new(),
            pending_orders: Vec::new(),
            exchange_constraints: ExchangeConstraints::default(),
            global_scores: scores,
            route_states: HashMap::new(),
            screening_results: HashMap::new(),
            structural_features: HashMap::new(),
            last_exchange_sync: Utc::now(),
            last_analytics_sync: Utc::now(),
            created_at: Utc::now(),
        };

        let options = ScoreFilterOptions {
            min_score: Some(70.0),
            required_grade: None,
            min_confidence: None,
            limit: None,
        };

        let results = select_top_symbols(&context, options);

        // 70점 이상만 선택됨
        assert_eq!(results.len(), 2);
        // 점수 순 정렬 확인
        assert_eq!(results[0].1.overall_score, 90.0);
        assert_eq!(results[1].1.overall_score, 75.0);
    }

    #[test]
    fn test_select_top_symbols_with_grade_filter() {
        use trader_core::domain::{StrategyAccountInfo, ExchangeConstraints};
        use std::collections::HashMap;

        let mut scores = HashMap::new();
        scores.insert(create_test_score("A", 90.0, "BUY", 0.9).0, create_test_score("A", 90.0, "BUY", 0.9).1);
        scores.insert(create_test_score("B", 75.0, "BUY", 0.8).0, create_test_score("B", 75.0, "BUY", 0.8).1);
        scores.insert(create_test_score("C", 85.0, "WATCH", 0.7).0, create_test_score("C", 85.0, "WATCH", 0.7).1);

        let context = StrategyContext {
            account: StrategyAccountInfo::default(),
            positions: HashMap::new(),
            pending_orders: Vec::new(),
            exchange_constraints: ExchangeConstraints::default(),
            global_scores: scores,
            route_states: HashMap::new(),
            screening_results: HashMap::new(),
            structural_features: HashMap::new(),
            last_exchange_sync: Utc::now(),
            last_analytics_sync: Utc::now(),
            created_at: Utc::now(),
        };

        let options = ScoreFilterOptions {
            min_score: None,
            required_grade: Some("BUY".to_string()),
            min_confidence: None,
            limit: None,
        };

        let results = select_top_symbols(&context, options);

        // BUY 등급만 선택됨
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].1.recommendation, "BUY");
        assert_eq!(results[1].1.recommendation, "BUY");
    }
}
