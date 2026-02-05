//! 구조적 피처 (Structural Features).
//!
//! "살아있는 횡보"와 "죽은 횡보"를 구분하여 돌파 가능성을 예측합니다.
//!
//! # 6개 구조적 피처
//!
//! 1. **low_trend**: Higher Low 강도 (-1.0 ~ 1.0)
//! 2. **vol_quality**: 거래량 품질 / 매집 판별 (-1.0 ~ 1.0)
//! 3. **range_pos**: 박스권 내 위치 (0.0 ~ 1.0)
//! 4. **dist_ma20**: MA20 이격도 (%)
//! 5. **bb_width**: 볼린저 밴드 폭 (%)
//! 6. **rsi**: RSI 14일 (0 ~ 100)
//!
//! # 사용 예시
//!
//! ```ignore
//! use trader_analytics::indicators::{IndicatorEngine, StructuralFeatures};
//!
//! let engine = IndicatorEngine::new();
//! let candles = /* 40개 이상의 OHLCV 데이터 */;
//!
//! let features = StructuralFeatures::from_candles(&candles, &engine)?;
//!
//! if features.is_alive_consolidation() {
//!     println!("살아있는 횡보 감지! 돌파 가능성: {}%", features.breakout_score());
//! }
//! ```

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use trader_core::Kline;

use super::{
    BollingerBandsParams, IndicatorEngine, IndicatorError, IndicatorResult, RsiParams, SmaParams,
};

/// 최소 필요 캔들 개수.
const MIN_CANDLES: usize = 40;

/// 구조적 피처 - 횡보 돌파 가능성 분석.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructuralFeatures {
    /// Higher Low 강도 (-1.0 ~ 1.0)
    ///
    /// 양수: 저가 상승 (매집 패턴)
    /// 음수: 저가 하락
    pub low_trend: f64,

    /// 거래량 품질 (-1.0 ~ 1.0)
    ///
    /// 양수: 상승 시 거래량 많음 (매집)
    /// 음수: 하락 시 거래량 많음 (이탈)
    pub vol_quality: f64,

    /// 박스권 내 위치 (0.0 ~ 1.0)
    ///
    /// 0.0: 박스 하단
    /// 1.0: 박스 상단
    pub range_pos: f64,

    /// MA20 이격도 (%)
    ///
    /// 양수: MA20 위
    /// 음수: MA20 아래
    pub dist_ma20: f64,

    /// 볼린저 밴드 폭 (%)
    ///
    /// 낮을수록 변동성 수축 (돌파 준비)
    pub bb_width: f64,

    /// RSI 14일 (0 ~ 100)
    pub rsi: f64,
}

impl StructuralFeatures {
    /// 캔들 데이터로부터 구조적 피처 계산.
    ///
    /// # 인자
    ///
    /// * `candles` - OHLCV 캔들 데이터 (최소 40개)
    /// * `engine` - 지표 계산 엔진
    ///
    /// # 반환
    ///
    /// 계산된 구조적 피처
    ///
    /// # 에러
    ///
    /// - 캔들 개수가 40개 미만인 경우
    /// - 지표 계산 실패
    pub fn from_candles(candles: &[Kline], engine: &IndicatorEngine) -> IndicatorResult<Self> {
        // 데이터 충분성 검증
        if candles.len() < MIN_CANDLES {
            return Err(IndicatorError::InsufficientData {
                required: MIN_CANDLES,
                provided: candles.len(),
            });
        }

        // 가격 데이터 추출
        let closes: Vec<Decimal> = candles.iter().map(|k| k.close).collect();
        let highs: Vec<Decimal> = candles.iter().map(|k| k.high).collect();
        let lows: Vec<Decimal> = candles.iter().map(|k| k.low).collect();

        // 1. MA20 이격도 계산
        let ma20 = engine.sma(&closes, SmaParams { period: 20 })?;
        let current_price = *closes.last().unwrap();
        let current_ma20 = ma20
            .last()
            .unwrap()
            .ok_or_else(|| IndicatorError::CalculationError("MA20 계산 실패".to_string()))?;

        let dist_ma20 = if current_ma20 > Decimal::ZERO {
            ((current_price - current_ma20) / current_ma20 * Decimal::from(100))
                .to_string()
                .parse::<f64>()
                .unwrap_or(0.0)
        } else {
            0.0 // MA20이 0이면 이격도 0으로 처리
        };

        // 2. 볼린저 밴드 폭 계산
        let bb = engine.bollinger_bands(&closes, BollingerBandsParams::default())?;
        let last_bb = bb
            .last()
            .ok_or_else(|| IndicatorError::CalculationError("볼린저 밴드 계산 실패".to_string()))?;

        let bb_width = match (last_bb.upper, last_bb.lower, last_bb.middle) {
            (Some(upper), Some(lower), Some(middle)) if middle > Decimal::ZERO => {
                ((upper - lower) / middle * Decimal::from(100))
                    .to_string()
                    .parse::<f64>()
                    .unwrap_or(0.0)
            }
            _ => 0.0,
        };

        // 3. RSI 계산
        let rsi_values = engine.rsi(&closes, RsiParams { period: 14 })?;
        let rsi = rsi_values
            .last()
            .unwrap()
            .ok_or_else(|| IndicatorError::CalculationError("RSI 계산 실패".to_string()))?
            .to_string()
            .parse::<f64>()
            .unwrap_or(50.0);

        // 4. 커스텀 피처 계산
        let low_trend = Self::calculate_low_trend(&lows, &closes)?;
        let vol_quality = Self::calculate_vol_quality(candles)?;
        let range_pos = Self::calculate_range_position(&highs, &lows, current_price)?;

        Ok(Self {
            low_trend,
            vol_quality,
            range_pos,
            dist_ma20,
            bb_width,
            rsi,
        })
    }

    /// Higher Low 강도 계산 (선형 회귀 기반).
    ///
    /// 최근 20일 저가의 추세를 분석하여 매집 여부를 판단합니다.
    fn calculate_low_trend(lows: &[Decimal], closes: &[Decimal]) -> IndicatorResult<f64> {
        let window = 20;
        let start_idx = lows.len().saturating_sub(window);
        let recent_lows = &lows[start_idx..];

        if recent_lows.len() < window {
            return Ok(0.0);
        }

        // 선형 회귀: y = mx + b
        let n = recent_lows.len() as f64;
        let x_values: Vec<f64> = (0..recent_lows.len()).map(|i| i as f64).collect();
        let y_values: Vec<f64> = recent_lows
            .iter()
            .map(|v| v.to_string().parse::<f64>().unwrap_or(0.0))
            .collect();

        let sum_x: f64 = x_values.iter().sum();
        let sum_y: f64 = y_values.iter().sum();
        let sum_xy: f64 = x_values.iter().zip(&y_values).map(|(x, y)| x * y).sum();
        let sum_x2: f64 = x_values.iter().map(|x| x * x).sum();

        let slope = (n * sum_xy - sum_x * sum_y) / (n * sum_x2 - sum_x * sum_x);

        // 일평균 가격 대비 정규화
        let avg_price = closes[start_idx..]
            .iter()
            .map(|v| v.to_string().parse::<f64>().unwrap_or(0.0))
            .sum::<f64>()
            / (closes.len() - start_idx) as f64;

        if avg_price > 0.0 {
            Ok((slope / avg_price * 100.0).clamp(-1.0, 1.0))
        } else {
            Ok(0.0)
        }
    }

    /// 거래량 품질 계산 (상승일 vs 하락일 거래량 비교).
    fn calculate_vol_quality(candles: &[Kline]) -> IndicatorResult<f64> {
        let window = 20;
        let start_idx = candles.len().saturating_sub(window);
        let recent_candles = &candles[start_idx..];

        if recent_candles.len() < window {
            return Ok(0.0);
        }

        let mut up_volume = 0.0;
        let mut down_volume = 0.0;
        let mut up_count = 0;
        let mut down_count = 0;

        for candle in recent_candles {
            let volume = candle.volume.to_string().parse::<f64>().unwrap_or(0.0);

            if candle.close > candle.open {
                up_volume += volume;
                up_count += 1;
            } else if candle.close < candle.open {
                down_volume += volume;
                down_count += 1;
            }
        }

        let avg_up = if up_count > 0 {
            up_volume / up_count as f64
        } else {
            0.0
        };
        let avg_down = if down_count > 0 {
            down_volume / down_count as f64
        } else {
            0.0
        };

        let total_avg = (up_volume + down_volume) / recent_candles.len() as f64;

        if total_avg > 0.0 {
            Ok(((avg_up - avg_down) / total_avg).clamp(-1.0, 1.0))
        } else {
            Ok(0.0)
        }
    }

    /// 박스권 내 위치 계산.
    fn calculate_range_position(
        highs: &[Decimal],
        lows: &[Decimal],
        current_price: Decimal,
    ) -> IndicatorResult<f64> {
        let window = 20;
        let start_idx = highs.len().saturating_sub(window);

        let max_high = highs[start_idx..]
            .iter()
            .max()
            .copied()
            .unwrap_or(Decimal::ZERO);
        let min_low = lows[start_idx..]
            .iter()
            .min()
            .copied()
            .unwrap_or(Decimal::ZERO);

        let range = max_high - min_low;

        if range > Decimal::ZERO {
            let pos = ((current_price - min_low) / range)
                .to_string()
                .parse::<f64>()
                .unwrap_or(0.0);
            Ok(pos.clamp(0.0, 1.0))
        } else {
            Ok(0.5) // 범위가 없으면 중간값
        }
    }

    /// 돌파 가능성 점수 계산 (0 ~ 100).
    ///
    /// 가중치 기반:
    /// - low_trend: 30%
    /// - vol_quality: 25%
    /// - range_pos: 20%
    /// - bb_width: 15% (좁을수록 가산)
    /// - dist_ma20: 10%
    pub fn breakout_score(&self) -> f64 {
        let score = (self.low_trend * 0.3 + 0.3) * 50.0 // -1~1 → 0~50
            + (self.vol_quality * 0.25 + 0.25) * 50.0
            + self.range_pos * 0.2 * 100.0
            + (1.0 - (self.bb_width / 20.0).min(1.0)) * 0.15 * 100.0
            + (self.dist_ma20.abs() / 10.0).min(1.0) * 0.1 * 100.0;

        score.clamp(0.0, 100.0)
    }

    /// "살아있는 횡보" 판정.
    ///
    /// 조건:
    /// - low_trend > 0.2 (저가 상승)
    /// - vol_quality > 0.1 (매집 패턴)
    /// - bb_width < 3.0 (변동성 수축)
    pub fn is_alive_consolidation(&self) -> bool {
        self.low_trend > 0.2 && self.vol_quality > 0.1 && self.bb_width < 3.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use rust_decimal_macros::dec;
    use trader_core::Timeframe;

    /// 테스트용 캔들 생성 헬퍼.
    fn create_test_candles(count: usize, trend: &str) -> Vec<Kline> {
        let ticker = "TEST/USD".to_string();
        let mut candles = Vec::with_capacity(count);

        for i in 0..count {
            let base_price = dec!(100.0) + Decimal::from(i as i64);
            let (open, close) = match trend {
                "up" => (base_price, base_price + dec!(1.0)),
                "down" => (base_price, base_price - dec!(1.0)),
                _ => (base_price, base_price),
            };

            candles.push(Kline {
                ticker: ticker.clone(),
                timeframe: Timeframe::D1,
                open_time: Utc::now(),
                open,
                high: close + dec!(0.5),
                low: open - dec!(0.5),
                close,
                volume: Decimal::from(1000),
                close_time: Utc::now(),
                quote_volume: Some(Decimal::from(100000)),
                num_trades: Some(100),
            });
        }

        candles
    }

    #[test]
    fn test_insufficient_data_error() {
        let engine = IndicatorEngine::new();
        let candles = create_test_candles(30, "up"); // 40개 미만

        let result = StructuralFeatures::from_candles(&candles, &engine);

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            IndicatorError::InsufficientData {
                required: 40,
                provided: 30
            }
        ));
    }

    #[test]
    fn test_from_candles_success() {
        let engine = IndicatorEngine::new();
        let candles = create_test_candles(50, "up");

        let result = StructuralFeatures::from_candles(&candles, &engine);

        assert!(result.is_ok());
        let features = result.unwrap();

        // 기본 범위 검증
        assert!(features.low_trend >= -1.0 && features.low_trend <= 1.0);
        assert!(features.vol_quality >= -1.0 && features.vol_quality <= 1.0);
        assert!(features.range_pos >= 0.0 && features.range_pos <= 1.0);
        assert!(features.rsi >= 0.0 && features.rsi <= 100.0);
    }

    #[test]
    fn test_low_trend_uptrend() {
        let engine = IndicatorEngine::new();
        let candles = create_test_candles(50, "up");

        let features = StructuralFeatures::from_candles(&candles, &engine).unwrap();

        // 상승 추세이므로 low_trend는 양수여야 함
        assert!(features.low_trend > 0.0);
    }

    #[test]
    fn test_breakout_score_range() {
        let engine = IndicatorEngine::new();
        let candles = create_test_candles(50, "up");

        let features = StructuralFeatures::from_candles(&candles, &engine).unwrap();
        let score = features.breakout_score();

        assert!(score >= 0.0 && score <= 100.0);
    }

    #[test]
    fn test_is_alive_consolidation() {
        // 수동으로 "살아있는 횡보" 피처 생성
        let features = StructuralFeatures {
            low_trend: 0.3,   // 조건 충족
            vol_quality: 0.2, // 조건 충족
            bb_width: 2.5,    // 조건 충족
            range_pos: 0.5,
            dist_ma20: 0.0,
            rsi: 50.0,
        };

        assert!(features.is_alive_consolidation());

        // 조건 미충족
        let features_dead = StructuralFeatures {
            low_trend: 0.1, // 조건 미충족 (< 0.2)
            vol_quality: 0.2,
            bb_width: 2.5,
            range_pos: 0.5,
            dist_ma20: 0.0,
            rsi: 50.0,
        };

        assert!(!features_dead.is_alive_consolidation());
    }
}
