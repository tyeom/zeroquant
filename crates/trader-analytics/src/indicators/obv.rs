//! OBV (On-Balance Volume) 지표.
//!
//! OBV는 거래량을 이용하여 스마트 머니(기관투자자)의 자금 흐름을 추적하는 지표입니다.
//!
//! ## 계산 방식
//! - 종가 상승: OBV += 거래량
//! - 종가 하락: OBV -= 거래량
//! - 종가 동일: OBV 변화 없음
//!
//! ## 활용
//! - 가격 상승 + OBV 상승: 강한 상승 추세
//! - 가격 상승 + OBV 하락: 약한 상승 (다이버전스)
//! - OBV 돌파: 추세 전환 신호

use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;
use serde::{Deserialize, Serialize};

use super::{IndicatorError, IndicatorResult};

/// OBV 파라미터.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ObvParams {
    /// 초기값 (기본: 0).
    pub initial_value: i64,
}

impl Default for ObvParams {
    fn default() -> Self {
        Self { initial_value: 0 }
    }
}

/// OBV 결과.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ObvResult {
    /// OBV 값.
    pub obv: i64,
    /// OBV 변화량 (전일 대비).
    pub change: i64,
}

/// OBV 계산기.
#[derive(Debug, Default)]
pub struct ObvIndicator;

impl ObvIndicator {
    /// 새로운 OBV 계산기 생성.
    pub fn new() -> Self {
        Self
    }

    /// OBV (On-Balance Volume) 계산.
    ///
    /// # 인자
    /// * `close` - 종가 데이터
    /// * `volume` - 거래량 데이터
    /// * `params` - OBV 파라미터
    ///
    /// # 반환
    /// 각 시점의 OBV 값과 변화량
    pub fn calculate(
        &self,
        close: &[Decimal],
        volume: &[Decimal],
        params: ObvParams,
    ) -> IndicatorResult<Vec<ObvResult>> {
        if close.len() != volume.len() {
            return Err(IndicatorError::InvalidParameter(
                "종가와 거래량 데이터의 길이가 일치하지 않습니다".to_string(),
            ));
        }

        if close.is_empty() {
            return Err(IndicatorError::InsufficientData {
                required: 2,
                provided: close.len(),
            });
        }

        let mut result = Vec::with_capacity(close.len());
        let mut current_obv = params.initial_value;

        for i in 0..close.len() {
            let change = if i == 0 {
                // 첫 번째 데이터는 변화 없음
                0
            } else {
                let price_change = close[i] - close[i - 1];

                if price_change > Decimal::ZERO {
                    // 가격 상승: 거래량 추가
                    self.volume_to_i64(volume[i])?
                } else if price_change < Decimal::ZERO {
                    // 가격 하락: 거래량 차감
                    -self.volume_to_i64(volume[i])?
                } else {
                    // 가격 동일: 변화 없음
                    0
                }
            };

            current_obv = current_obv.saturating_add(change);

            result.push(ObvResult {
                obv: current_obv,
                change,
            });
        }

        Ok(result)
    }

    /// OBV 다이버전스 감지.
    ///
    /// 가격과 OBV의 방향이 반대인 경우를 감지합니다.
    ///
    /// # 인자
    /// * `close` - 종가 데이터
    /// * `obv_results` - OBV 계산 결과
    /// * `lookback` - 비교 기간 (기본: 5)
    ///
    /// # 반환
    /// 각 시점에서 다이버전스 발생 여부
    /// - true: 약세 다이버전스 (가격 상승, OBV 하락)
    /// - false: 정상 또는 강세 다이버전스
    pub fn detect_divergence(
        &self,
        close: &[Decimal],
        obv_results: &[ObvResult],
        lookback: usize,
    ) -> IndicatorResult<Vec<bool>> {
        if close.len() != obv_results.len() {
            return Err(IndicatorError::InvalidParameter(
                "종가와 OBV 데이터의 길이가 일치하지 않습니다".to_string(),
            ));
        }

        let mut divergences = Vec::with_capacity(close.len());

        for i in 0..close.len() {
            if i < lookback {
                divergences.push(false);
                continue;
            }

            let price_change = close[i] - close[i - lookback];
            let obv_change = obv_results[i].obv - obv_results[i - lookback].obv;

            // 약세 다이버전스: 가격은 상승했지만 OBV는 하락
            let is_bearish_divergence =
                price_change > Decimal::ZERO && obv_change < 0;

            divergences.push(is_bearish_divergence);
        }

        Ok(divergences)
    }

    /// Decimal 거래량을 i64로 변환.
    ///
    /// # 인자
    /// * `volume` - 거래량 (Decimal)
    ///
    /// # 반환
    /// i64 거래량
    fn volume_to_i64(&self, volume: Decimal) -> IndicatorResult<i64> {
        // Decimal을 정수로 변환 (소수점 이하 버림)
        volume
            .to_i64()
            .ok_or_else(|| {
                IndicatorError::CalculationError(
                    format!("거래량을 i64로 변환할 수 없습니다: {}", volume)
                )
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn sample_data() -> (Vec<Decimal>, Vec<Decimal>) {
        let close = vec![
            dec!(100.0),
            dec!(102.0), // 상승
            dec!(101.0), // 하락
            dec!(103.0), // 상승
            dec!(103.0), // 동일
            dec!(102.0), // 하락
            dec!(104.0), // 상승
        ];

        let volume = vec![
            dec!(1000.0),
            dec!(1500.0),
            dec!(1200.0),
            dec!(1800.0),
            dec!(1000.0),
            dec!(1300.0),
            dec!(2000.0),
        ];

        (close, volume)
    }

    #[test]
    fn test_obv_calculation() {
        let indicator = ObvIndicator::new();
        let (close, volume) = sample_data();

        let obv = indicator
            .calculate(&close, &volume, ObvParams::default())
            .unwrap();

        assert_eq!(obv.len(), close.len());

        // 첫 번째는 변화 없음
        assert_eq!(obv[0].change, 0);

        // 두 번째는 가격 상승 -> 거래량 추가
        assert_eq!(obv[1].change, 1500);
        assert_eq!(obv[1].obv, 1500);

        // 세 번째는 가격 하락 -> 거래량 차감
        assert_eq!(obv[2].change, -1200);
        assert_eq!(obv[2].obv, 300);

        // 네 번째는 가격 상승 -> 거래량 추가
        assert_eq!(obv[3].change, 1800);
        assert_eq!(obv[3].obv, 2100);

        // 다섯 번째는 가격 동일 -> 변화 없음
        assert_eq!(obv[4].change, 0);
        assert_eq!(obv[4].obv, 2100);
    }

    #[test]
    fn test_obv_with_custom_initial() {
        let indicator = ObvIndicator::new();
        let (close, volume) = sample_data();

        let obv = indicator
            .calculate(
                &close,
                &volume,
                ObvParams {
                    initial_value: 10000,
                },
            )
            .unwrap();

        // 초기값이 반영되어야 함
        assert_eq!(obv[0].obv, 10000);
        assert_eq!(obv[1].obv, 11500);
    }

    #[test]
    fn test_divergence_detection() {
        let indicator = ObvIndicator::new();
        let close = vec![
            dec!(100.0),
            dec!(102.0),
            dec!(104.0),
            dec!(106.0),
            dec!(108.0), // 가격은 계속 상승
        ];
        let volume = vec![
            dec!(2000.0),
            dec!(1500.0), // 거래량은 감소
            dec!(1000.0),
            dec!(800.0),
            dec!(500.0),
        ];

        let obv_results = indicator
            .calculate(&close, &volume, ObvParams::default())
            .unwrap();

        let divergences = indicator
            .detect_divergence(&close, &obv_results, 2)
            .unwrap();

        // 가격 상승 + OBV 하락 = 약세 다이버전스
        assert!(divergences[divergences.len() - 1]);
    }

    #[test]
    fn test_mismatched_length_error() {
        let indicator = ObvIndicator::new();
        let close = vec![dec!(100.0), dec!(101.0)];
        let volume = vec![dec!(1000.0)];

        let result = indicator.calculate(&close, &volume, ObvParams::default());
        assert!(result.is_err());
    }

    #[test]
    fn test_empty_data_error() {
        let indicator = ObvIndicator::new();
        let close: Vec<Decimal> = vec![];
        let volume: Vec<Decimal> = vec![];

        let result = indicator.calculate(&close, &volume, ObvParams::default());
        assert!(result.is_err());
    }
}
