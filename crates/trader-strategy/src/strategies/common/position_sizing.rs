//! 포지션 사이징 전략.
//!
//! 이 모듈은 자금 관리를 위한 다양한 포지션 사이징 방법을 제공합니다.

use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};

/// 포지션 사이징 결과.
#[derive(Debug, Clone)]
pub struct PositionSize {
    /// 포지션 크기 (자본 대비 비율 또는 절대값)
    pub size: Decimal,
    /// 포지션 크기 계산 방법
    pub method: String,
}

/// 포지션 사이저 trait.
///
/// 다양한 포지션 사이징 전략을 구현하기 위한 공통 인터페이스입니다.
pub trait PositionSizer: Send + Sync {
    /// 포지션 크기를 계산합니다.
    ///
    /// # Arguments
    /// * `capital` - 사용 가능한 자본
    /// * `entry_price` - 진입 가격
    /// * `stop_loss` - 손절가 (옵션)
    ///
    /// # Returns
    /// 계산된 포지션 크기
    fn calculate_size(
        &self,
        capital: Decimal,
        entry_price: Decimal,
        stop_loss: Option<Decimal>,
    ) -> PositionSize;
}

/// 고정 비율 포지션 사이저.
///
/// 항상 자본의 일정 비율을 사용합니다.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FixedRatioSizer {
    /// 자본 대비 포지션 비율 (0.0 ~ 1.0)
    pub ratio: Decimal,
}

impl FixedRatioSizer {
    pub fn new(ratio: Decimal) -> Self {
        Self {
            ratio: ratio.min(dec!(1)).max(dec!(0)),
        }
    }
}

impl PositionSizer for FixedRatioSizer {
    fn calculate_size(
        &self,
        capital: Decimal,
        _entry_price: Decimal,
        _stop_loss: Option<Decimal>,
    ) -> PositionSize {
        PositionSize {
            size: capital * self.ratio,
            method: "FixedRatio".to_string(),
        }
    }
}

/// 켈리 기준 포지션 사이저.
///
/// 켈리 공식을 사용하여 최적 포지션 크기를 계산합니다.
/// Kelly% = W - (1 - W) / R
/// 여기서 W = 승률, R = 평균 수익/평균 손실 비율
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KellyPositionSizer {
    /// 승률 (0.0 ~ 1.0)
    pub win_rate: Decimal,
    /// 손익비 (평균 수익 / 평균 손실)
    pub profit_loss_ratio: Decimal,
    /// 켈리 비율 조정 계수 (보수적으로 0.25 ~ 0.5 사용)
    pub kelly_fraction: Decimal,
}

impl KellyPositionSizer {
    pub fn new(win_rate: Decimal, profit_loss_ratio: Decimal, kelly_fraction: Decimal) -> Self {
        Self {
            win_rate: win_rate.min(dec!(1)).max(dec!(0)),
            profit_loss_ratio: profit_loss_ratio.max(dec!(0)),
            kelly_fraction: kelly_fraction.min(dec!(1)).max(dec!(0)),
        }
    }
}

impl PositionSizer for KellyPositionSizer {
    fn calculate_size(
        &self,
        capital: Decimal,
        _entry_price: Decimal,
        _stop_loss: Option<Decimal>,
    ) -> PositionSize {
        if self.profit_loss_ratio == dec!(0) {
            return PositionSize {
                size: dec!(0),
                method: "Kelly".to_string(),
            };
        }

        // Kelly% = W - (1 - W) / R
        let kelly_pct = self.win_rate - ((dec!(1) - self.win_rate) / self.profit_loss_ratio);

        // 음수면 진입하지 않음
        let kelly_pct = kelly_pct.max(dec!(0));

        // 켈리 분수를 적용하여 보수적으로 조정
        let adjusted_kelly = kelly_pct * self.kelly_fraction;

        // 최대 50%로 제한
        let final_kelly = adjusted_kelly.min(dec!(0.5));

        PositionSize {
            size: capital * final_kelly,
            method: "Kelly".to_string(),
        }
    }
}

/// ATR 기반 포지션 사이저.
///
/// ATR을 사용하여 변동성에 따라 포지션 크기를 조정합니다.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AtrPositionSizer {
    /// 리스크 자본 (전체 자본의 몇 %를 리스크할지)
    pub risk_ratio: Decimal,
    /// ATR 배수 (손절가 설정용)
    pub atr_multiplier: Decimal,
}

impl AtrPositionSizer {
    pub fn new(risk_ratio: Decimal, atr_multiplier: Decimal) -> Self {
        Self {
            risk_ratio: risk_ratio.min(dec!(0.05)).max(dec!(0.005)), // 0.5% ~ 5%
            atr_multiplier: atr_multiplier.max(dec!(1)),
        }
    }

    /// ATR 값을 사용하여 포지션 크기 계산.
    ///
    /// # Arguments
    /// * `capital` - 사용 가능한 자본
    /// * `entry_price` - 진입 가격
    /// * `atr` - ATR 값
    ///
    /// # Returns
    /// 포지션 크기 (수량)
    pub fn calculate_with_atr(
        &self,
        capital: Decimal,
        entry_price: Decimal,
        atr: Decimal,
    ) -> PositionSize {
        if entry_price == dec!(0) || atr == dec!(0) {
            return PositionSize {
                size: dec!(0),
                method: "ATR".to_string(),
            };
        }

        // 리스크할 금액
        let risk_amount = capital * self.risk_ratio;

        // ATR 기반 손실폭
        let stop_distance = atr * self.atr_multiplier;

        // 포지션 크기 (수량) = 리스크 금액 / 손실폭
        let quantity = risk_amount / stop_distance;

        // 포지션 가치
        let position_value = quantity * entry_price;

        PositionSize {
            size: position_value,
            method: "ATR".to_string(),
        }
    }
}

impl PositionSizer for AtrPositionSizer {
    fn calculate_size(
        &self,
        capital: Decimal,
        entry_price: Decimal,
        stop_loss: Option<Decimal>,
    ) -> PositionSize {
        if let Some(stop) = stop_loss {
            if stop == dec!(0) || entry_price == dec!(0) {
                return PositionSize {
                    size: dec!(0),
                    method: "ATR".to_string(),
                };
            }

            // 손절가가 주어진 경우, 손실폭 계산
            let stop_distance = (entry_price - stop).abs();

            // 리스크할 금액
            let risk_amount = capital * self.risk_ratio;

            // 포지션 크기 (수량)
            let quantity = risk_amount / stop_distance;

            // 포지션 가치
            let position_value = quantity * entry_price;

            PositionSize {
                size: position_value,
                method: "ATR".to_string(),
            }
        } else {
            // 손절가가 없으면 기본 비율 사용
            PositionSize {
                size: capital * dec!(0.1),
                method: "ATR".to_string(),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fixed_ratio_sizer() {
        let sizer = FixedRatioSizer::new(dec!(0.2)); // 20%
        let result = sizer.calculate_size(dec!(10000), dec!(100), None);
        assert_eq!(result.size, dec!(2000));
    }

    #[test]
    fn test_kelly_sizer() {
        let sizer = KellyPositionSizer::new(
            dec!(0.6),  // 60% 승률
            dec!(2),    // 2:1 손익비
            dec!(0.25), // 켈리 분수 25%
        );
        let result = sizer.calculate_size(dec!(10000), dec!(100), None);

        // Kelly% = 0.6 - (1 - 0.6) / 2 = 0.6 - 0.2 = 0.4
        // Adjusted = 0.4 * 0.25 = 0.1
        // Size = 10000 * 0.1 = 1000
        assert_eq!(result.size, dec!(1000));
    }

    #[test]
    fn test_atr_sizer_with_stop_loss() {
        let sizer = AtrPositionSizer::new(dec!(0.01), dec!(2)); // 1% 리스크, 2x ATR
        let capital = dec!(10000);
        let entry_price = dec!(100);
        let stop_loss = dec!(98); // 2% 손실

        let result = sizer.calculate_size(capital, entry_price, Some(stop_loss));

        // 리스크 금액 = 10000 * 0.01 = 100
        // 손실폭 = 100 - 98 = 2
        // 수량 = 100 / 2 = 50
        // 포지션 가치 = 50 * 100 = 5000
        assert_eq!(result.size, dec!(5000));
    }

    #[test]
    fn test_atr_sizer_with_atr() {
        let sizer = AtrPositionSizer::new(dec!(0.01), dec!(2)); // 1% 리스크, 2x ATR
        let capital = dec!(10000);
        let entry_price = dec!(100);
        let atr = dec!(1); // ATR = 1

        let result = sizer.calculate_with_atr(capital, entry_price, atr);

        // 리스크 금액 = 10000 * 0.01 = 100
        // 손실폭 = 1 * 2 = 2
        // 수량 = 100 / 2 = 50
        // 포지션 가치 = 50 * 100 = 5000
        assert_eq!(result.size, dec!(5000));
    }
}
