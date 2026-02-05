//! 구조적 피처 계산기.
//!
//! "살아있는 횡보"와 "죽은 횡보"를 구분하여 돌파 가능성을 예측합니다.
//!
//! # 설계 원칙
//!
//! - 모든 공개 API는 ticker 문자열을 받습니다 (Symbol 객체가 아님)
//! - Symbol 정보가 필요한 경우 SymbolResolver를 통해 조회합니다

use chrono::Utc;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use trader_core::{domain::StructuralFeatures, Kline};

/// 구조적 피처 계산기
pub struct StructuralFeaturesCalculator;

impl StructuralFeaturesCalculator {
    /// OHLCV 데이터로부터 구조적 피처 계산.
    ///
    /// # 인자
    ///
    /// * `ticker` - 종목 티커 (예: "005930", "AAPL")
    /// * `candles` - OHLCV 데이터 (최소 60개 권장)
    ///
    /// # 반환
    ///
    /// StructuralFeatures
    pub fn from_candles(ticker: &str, candles: &[Kline]) -> Result<StructuralFeatures, String> {
        if candles.len() < 20 {
            return Err(format!("데이터 부족: {}개 (최소 20개 필요)", candles.len()));
        }

        Ok(StructuralFeatures {
            ticker: ticker.to_string(),
            low_trend: Self::calculate_low_trend(candles),
            vol_quality: Self::calculate_vol_quality(candles),
            range_pos: Self::calculate_range_pos(candles),
            dist_ma20: Self::calculate_dist_ma20(candles),
            bb_width: Self::calculate_bb_width(candles),
            rsi: Self::calculate_rsi(candles),
            timestamp: Utc::now(),
        })
    }

    /// Higher Low 강도 계산.
    ///
    /// 최근 20일간의 저점이 상승하는지 측정합니다.
    ///
    /// # 반환
    ///
    /// -1.0 ~ 1.0 (양수=저점 상승, 음수=저점 하락)
    fn calculate_low_trend(candles: &[Kline]) -> Decimal {
        let len = candles.len().min(20);
        if len < 10 {
            return Decimal::ZERO;
        }

        let recent = &candles[candles.len() - len..];

        // 최근 10개와 이전 10개의 평균 저가 비교
        let first_half: Decimal = recent[..len / 2].iter().map(|k| k.low).sum();
        let first_count = Decimal::from(len / 2);

        let second_half: Decimal = recent[len / 2..].iter().map(|k| k.low).sum();
        let second_count = Decimal::from(len - len / 2);

        if first_count.is_zero() || second_count.is_zero() || first_half.is_zero() {
            return Decimal::ZERO;
        }

        let avg_first = first_half / first_count;
        let avg_second = second_half / second_count;

        // 변화율을 -1.0 ~ 1.0 범위로 정규화
        let change_pct = (avg_second - avg_first) / avg_first * dec!(100);
        let clamped = change_pct.max(dec!(-10)).min(dec!(10));
        clamped / dec!(10)
    }

    /// 매집/이탈 판별.
    ///
    /// 거래량 패턴으로 기관 매집 또는 이탈을 감지합니다.
    ///
    /// # 반환
    ///
    /// 0 ~ 5 (2.0 이상=매집, -2.0 이하=이탈)
    fn calculate_vol_quality(candles: &[Kline]) -> Decimal {
        let len = candles.len().min(20);
        if len < 10 {
            return Decimal::ZERO;
        }

        let recent = &candles[candles.len() - len..];

        // 상승일 거래량 vs 하락일 거래량 비교
        let mut up_vol = Decimal::ZERO;
        let mut down_vol = Decimal::ZERO;

        for k in recent.iter() {
            if k.close > k.open {
                up_vol += k.volume;
            } else {
                down_vol += k.volume;
            }
        }

        // 거래량 비율을 0 ~ 5 범위로 정규화
        if down_vol.is_zero() {
            return dec!(5);
        }
        let ratio = up_vol / down_vol;
        (ratio - Decimal::ONE).max(dec!(-2)).min(dec!(4))
    }

    /// 박스권 위치 계산.
    ///
    /// 현재 가격이 최근 범위의 어디에 위치하는지 측정합니다.
    ///
    /// # 반환
    ///
    /// 0.0 ~ 1.0 (0=하단, 1=상단)
    fn calculate_range_pos(candles: &[Kline]) -> Decimal {
        let len = candles.len().min(60);
        if len < 20 {
            return dec!(0.5);
        }

        let recent = &candles[candles.len() - len..];
        let current_price = match candles.last() {
            Some(k) => k.close,
            None => return dec!(0.5),
        };

        let high_60d = recent.iter().map(|k| k.high).max().unwrap_or(Decimal::ZERO);
        let low_60d = recent.iter().map(|k| k.low).min().unwrap_or(Decimal::ZERO);

        if high_60d == low_60d {
            return dec!(0.5);
        }

        ((current_price - low_60d) / (high_60d - low_60d))
            .max(Decimal::ZERO)
            .min(Decimal::ONE)
    }

    /// MA20 이격도 계산.
    ///
    /// # 반환
    ///
    /// % (-20 ~ +20)
    fn calculate_dist_ma20(candles: &[Kline]) -> Decimal {
        if candles.len() < 20 {
            return Decimal::ZERO;
        }

        let recent = &candles[candles.len() - 20..];
        let sum: Decimal = recent.iter().map(|k| k.close).sum();
        let ma20 = sum / dec!(20);

        let current_price = match candles.last() {
            Some(k) => k.close,
            None => return Decimal::ZERO,
        };

        if ma20.is_zero() {
            return Decimal::ZERO;
        }

        ((current_price - ma20) / ma20 * dec!(100))
            .max(dec!(-20))
            .min(dec!(20))
    }

    /// 볼린저 밴드 폭 계산.
    ///
    /// # 반환
    ///
    /// % (0 ~ 50)
    fn calculate_bb_width(candles: &[Kline]) -> Decimal {
        if candles.len() < 20 {
            return Decimal::ZERO;
        }

        let recent = &candles[candles.len() - 20..];
        let closes: Vec<Decimal> = recent.iter().map(|k| k.close).collect();

        // SMA
        let sum: Decimal = closes.iter().sum();
        let sma = sum / dec!(20);

        if sma.is_zero() {
            return Decimal::ZERO;
        }

        // 표준편차 (f64로 계산 후 Decimal 변환 - 제곱근 계산 위해)
        let sma_f64 = sma.to_f64().unwrap_or(0.0);
        let variance: f64 = closes
            .iter()
            .map(|c| {
                let c_f64 = c.to_f64().unwrap_or(0.0);
                (c_f64 - sma_f64).powi(2)
            })
            .sum::<f64>()
            / 20.0;
        let std_dev = variance.sqrt();

        // 밴드 폭 (%)
        let width = (std_dev * 2.0 * 2.0) / sma_f64 * 100.0;
        Decimal::from_f64_retain(width.clamp(0.0, 50.0)).unwrap_or(Decimal::ZERO)
    }

    /// RSI 14일 계산.
    ///
    /// # 반환
    ///
    /// 0 ~ 100
    fn calculate_rsi(candles: &[Kline]) -> Decimal {
        if candles.len() < 15 {
            return dec!(50);
        }

        let period = 14;
        let recent = &candles[candles.len() - period - 1..];

        let mut gains = Decimal::ZERO;
        let mut losses = Decimal::ZERO;

        for i in 1..recent.len() {
            let change = recent[i].close - recent[i - 1].close;
            if change > Decimal::ZERO {
                gains += change;
            } else {
                losses += -change;
            }
        }

        let period_dec = Decimal::from(period);
        let avg_gain = gains / period_dec;
        let avg_loss = losses / period_dec;

        if avg_loss.is_zero() {
            return dec!(100);
        }

        let rs = avg_gain / avg_loss;
        let rsi = dec!(100) - (dec!(100) / (Decimal::ONE + rs));

        rsi.max(Decimal::ZERO).min(dec!(100))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_candles(count: usize) -> Vec<Kline> {
        let ticker = "TEST/USD".to_string();
        (0..count)
            .map(|i| {
                let now = chrono::Utc::now();
                Kline {
                    ticker: ticker.clone(),
                    timeframe: trader_core::types::Timeframe::D1,
                    open_time: now,
                    close_time: now,
                    open: dec!(100) + Decimal::from(i as i64),
                    high: dec!(105) + Decimal::from(i as i64),
                    low: dec!(95) + Decimal::from(i as i64),
                    close: dec!(102) + Decimal::from(i as i64),
                    volume: dec!(1000),
                    quote_volume: Some(dec!(0)),
                    num_trades: Some(0),
                }
            })
            .collect()
    }

    #[test]
    fn test_from_candles() {
        let ticker = "TEST";
        let candles = create_test_candles(60);

        let result = StructuralFeaturesCalculator::from_candles(ticker, &candles);
        assert!(result.is_ok());

        let features = result.unwrap();
        assert_eq!(features.ticker, ticker);
        assert!(features.low_trend >= dec!(-1) && features.low_trend <= dec!(1));
        assert!(features.range_pos >= Decimal::ZERO && features.range_pos <= Decimal::ONE);
        assert!(features.rsi >= Decimal::ZERO && features.rsi <= dec!(100));
    }

    #[test]
    fn test_insufficient_data() {
        let ticker = "TEST";
        let candles = create_test_candles(10);

        let result = StructuralFeaturesCalculator::from_candles(ticker, &candles);
        assert!(result.is_err());
    }
}
