//! 기술적 지표 핸들러.
//!
//! SMA, EMA, RSI, MACD, 볼린저 밴드, 스토캐스틱, ATR 등의 지표 API를 제공합니다.

use axum::{extract::Query, response::IntoResponse, Json};
use chrono::{Duration, Utc};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use trader_analytics::{
    AtrParams, BollingerBandsParams, EmaParams, IndicatorEngine, MacdParams, RsiParams, SmaParams,
    StochasticParams,
};

use super::types::{
    AtrQuery, AvailableIndicatorsResponse, BollingerQuery, CalculateIndicatorsRequest,
    CalculateIndicatorsResponse, EmaQuery, IndicatorDataResponse, IndicatorInfo, IndicatorPoint,
    IndicatorSeries, MacdQuery, RsiQuery, SmaQuery, StochasticQuery,
};

/// 사용 가능한 지표 목록 조회.
///
/// GET /api/v1/analytics/indicators
pub async fn get_available_indicators() -> impl IntoResponse {
    let indicators = vec![
        IndicatorInfo {
            id: "sma".to_string(),
            name: "단순 이동평균 (SMA)".to_string(),
            description: "지정된 기간 동안의 종가 평균을 계산합니다.".to_string(),
            category: "추세".to_string(),
            default_params: serde_json::json!({ "period": 20 }),
            overlay: true,
        },
        IndicatorInfo {
            id: "ema".to_string(),
            name: "지수 이동평균 (EMA)".to_string(),
            description: "최근 가격에 더 큰 가중치를 부여하는 이동평균입니다.".to_string(),
            category: "추세".to_string(),
            default_params: serde_json::json!({ "period": 12 }),
            overlay: true,
        },
        IndicatorInfo {
            id: "rsi".to_string(),
            name: "상대강도지수 (RSI)".to_string(),
            description: "과매수/과매도 상태를 측정합니다. 70 이상: 과매수, 30 이하: 과매도."
                .to_string(),
            category: "모멘텀".to_string(),
            default_params: serde_json::json!({ "period": 14 }),
            overlay: false,
        },
        IndicatorInfo {
            id: "macd".to_string(),
            name: "MACD".to_string(),
            description: "두 EMA의 차이로 추세의 강도와 방향을 분석합니다.".to_string(),
            category: "추세".to_string(),
            default_params: serde_json::json!({
                "fast_period": 12,
                "slow_period": 26,
                "signal_period": 9
            }),
            overlay: false,
        },
        IndicatorInfo {
            id: "bollinger".to_string(),
            name: "볼린저 밴드".to_string(),
            description: "이동평균을 중심으로 표준편차 밴드를 그려 변동성을 시각화합니다."
                .to_string(),
            category: "변동성".to_string(),
            default_params: serde_json::json!({ "period": 20, "std_dev": 2.0 }),
            overlay: true,
        },
        IndicatorInfo {
            id: "stochastic".to_string(),
            name: "스토캐스틱".to_string(),
            description: "현재 가격이 일정 기간 가격 범위 내에서 어디에 위치하는지 측정합니다."
                .to_string(),
            category: "모멘텀".to_string(),
            default_params: serde_json::json!({ "k_period": 14, "d_period": 3 }),
            overlay: false,
        },
        IndicatorInfo {
            id: "atr".to_string(),
            name: "평균 실제 범위 (ATR)".to_string(),
            description: "가격 변동성을 측정합니다. 값이 클수록 변동성이 높습니다.".to_string(),
            category: "변동성".to_string(),
            default_params: serde_json::json!({ "period": 14 }),
            overlay: false,
        },
    ];

    Json(AvailableIndicatorsResponse { indicators })
}

/// 샘플 OHLCV 데이터 생성 (테스트용).
fn generate_sample_ohlcv(
    days: i64,
) -> (
    Vec<i64>,
    Vec<Decimal>,
    Vec<Decimal>,
    Vec<Decimal>,
    Vec<Decimal>,
) {
    let base_time = Utc::now() - Duration::days(days);
    let mut timestamps = Vec::with_capacity(days as usize);
    let mut opens = Vec::with_capacity(days as usize);
    let mut highs = Vec::with_capacity(days as usize);
    let mut lows = Vec::with_capacity(days as usize);
    let mut closes = Vec::with_capacity(days as usize);

    let mut price = dec!(50000); // 시작 가격

    for i in 0..days {
        let ts = (base_time + Duration::days(i)).timestamp_millis();
        timestamps.push(ts);

        // 변동성 있는 가격 생성
        let change_pct = if i % 5 == 0 {
            dec!(-0.02)
        } else if i % 3 == 0 {
            dec!(0.015)
        } else {
            dec!(0.005)
        };

        let open = price;
        let close = price * (dec!(1.0) + change_pct);
        let high = if close > open {
            close * dec!(1.005)
        } else {
            open * dec!(1.005)
        };
        let low = if close < open {
            close * dec!(0.995)
        } else {
            open * dec!(0.995)
        };

        opens.push(open);
        highs.push(high);
        lows.push(low);
        closes.push(close);

        price = close;
    }

    (timestamps, opens, highs, lows, closes)
}

/// 기간 문자열을 일수로 변환.
fn parse_period_to_days(period: &str) -> i64 {
    match period.to_lowercase().as_str() {
        "1d" => 1,
        "1w" => 7,
        "1m" => 30,
        "3m" => 90,
        "6m" => 180,
        "1y" | "12m" => 365,
        "all" => 1000,
        _ => 90, // 기본값: 3개월
    }
}

/// SMA 지표 데이터 조회.
///
/// GET /api/v1/analytics/indicators/sma
pub async fn get_sma_indicator(Query(query): Query<SmaQuery>) -> impl IntoResponse {
    let days = parse_period_to_days(&query.period);
    let (timestamps, _, _, _, closes) = generate_sample_ohlcv(days);

    let engine = IndicatorEngine::new();
    let params = SmaParams {
        period: query.sma_period,
    };

    match engine.sma(&closes, params) {
        Ok(sma_values) => {
            let data: Vec<IndicatorPoint> = timestamps
                .iter()
                .zip(sma_values.iter())
                .map(|(&ts, value)| IndicatorPoint {
                    x: ts,
                    y: value.map(|v| v.to_string()),
                })
                .collect();

            Json(IndicatorDataResponse {
                indicator: "sma".to_string(),
                name: format!("SMA({})", query.sma_period),
                symbol: query.symbol,
                params: serde_json::json!({ "period": query.sma_period }),
                series: vec![IndicatorSeries {
                    name: "sma".to_string(),
                    data,
                    color: Some("#2196F3".to_string()),
                    series_type: "line".to_string(),
                }],
            })
        }
        Err(e) => Json(IndicatorDataResponse {
            indicator: "sma".to_string(),
            name: format!("SMA({}) - 오류", query.sma_period),
            symbol: query.symbol,
            params: serde_json::json!({ "error": e.to_string() }),
            series: vec![],
        }),
    }
}

/// EMA 지표 데이터 조회.
///
/// GET /api/v1/analytics/indicators/ema
pub async fn get_ema_indicator(Query(query): Query<EmaQuery>) -> impl IntoResponse {
    let days = parse_period_to_days(&query.period);
    let (timestamps, _, _, _, closes) = generate_sample_ohlcv(days);

    let engine = IndicatorEngine::new();
    let params = EmaParams {
        period: query.ema_period,
    };

    match engine.ema(&closes, params) {
        Ok(ema_values) => {
            let data: Vec<IndicatorPoint> = timestamps
                .iter()
                .zip(ema_values.iter())
                .map(|(&ts, value)| IndicatorPoint {
                    x: ts,
                    y: value.map(|v| v.to_string()),
                })
                .collect();

            Json(IndicatorDataResponse {
                indicator: "ema".to_string(),
                name: format!("EMA({})", query.ema_period),
                symbol: query.symbol,
                params: serde_json::json!({ "period": query.ema_period }),
                series: vec![IndicatorSeries {
                    name: "ema".to_string(),
                    data,
                    color: Some("#FF9800".to_string()),
                    series_type: "line".to_string(),
                }],
            })
        }
        Err(e) => Json(IndicatorDataResponse {
            indicator: "ema".to_string(),
            name: format!("EMA({}) - 오류", query.ema_period),
            symbol: query.symbol,
            params: serde_json::json!({ "error": e.to_string() }),
            series: vec![],
        }),
    }
}

/// RSI 지표 데이터 조회.
///
/// GET /api/v1/analytics/indicators/rsi
pub async fn get_rsi_indicator(Query(query): Query<RsiQuery>) -> impl IntoResponse {
    let days = parse_period_to_days(&query.period);
    let (timestamps, _, _, _, closes) = generate_sample_ohlcv(days);

    let engine = IndicatorEngine::new();
    let params = RsiParams {
        period: query.rsi_period,
    };

    match engine.rsi(&closes, params) {
        Ok(rsi_values) => {
            let data: Vec<IndicatorPoint> = timestamps
                .iter()
                .zip(rsi_values.iter())
                .map(|(&ts, value)| IndicatorPoint {
                    x: ts,
                    y: value.map(|v| v.to_string()),
                })
                .collect();

            Json(IndicatorDataResponse {
                indicator: "rsi".to_string(),
                name: format!("RSI({})", query.rsi_period),
                symbol: query.symbol,
                params: serde_json::json!({ "period": query.rsi_period }),
                series: vec![IndicatorSeries {
                    name: "rsi".to_string(),
                    data,
                    color: Some("#9C27B0".to_string()),
                    series_type: "line".to_string(),
                }],
            })
        }
        Err(e) => Json(IndicatorDataResponse {
            indicator: "rsi".to_string(),
            name: format!("RSI({}) - 오류", query.rsi_period),
            symbol: query.symbol,
            params: serde_json::json!({ "error": e.to_string() }),
            series: vec![],
        }),
    }
}

/// MACD 지표 데이터 조회.
///
/// GET /api/v1/analytics/indicators/macd
pub async fn get_macd_indicator(Query(query): Query<MacdQuery>) -> impl IntoResponse {
    let days = parse_period_to_days(&query.period);
    let (timestamps, _, _, _, closes) = generate_sample_ohlcv(days);

    let engine = IndicatorEngine::new();
    let params = MacdParams {
        fast_period: query.fast_period,
        slow_period: query.slow_period,
        signal_period: query.signal_period,
    };

    match engine.macd(&closes, params) {
        Ok(macd_results) => {
            let macd_data: Vec<IndicatorPoint> = timestamps
                .iter()
                .zip(macd_results.iter())
                .map(|(&ts, result)| IndicatorPoint {
                    x: ts,
                    y: result.macd.map(|v| v.to_string()),
                })
                .collect();

            let signal_data: Vec<IndicatorPoint> = timestamps
                .iter()
                .zip(macd_results.iter())
                .map(|(&ts, result)| IndicatorPoint {
                    x: ts,
                    y: result.signal.map(|v| v.to_string()),
                })
                .collect();

            let histogram_data: Vec<IndicatorPoint> = timestamps
                .iter()
                .zip(macd_results.iter())
                .map(|(&ts, result)| IndicatorPoint {
                    x: ts,
                    y: result.histogram.map(|v| v.to_string()),
                })
                .collect();

            Json(IndicatorDataResponse {
                indicator: "macd".to_string(),
                name: format!(
                    "MACD({},{},{})",
                    query.fast_period, query.slow_period, query.signal_period
                ),
                symbol: query.symbol,
                params: serde_json::json!({
                    "fast_period": query.fast_period,
                    "slow_period": query.slow_period,
                    "signal_period": query.signal_period
                }),
                series: vec![
                    IndicatorSeries {
                        name: "macd".to_string(),
                        data: macd_data,
                        color: Some("#2196F3".to_string()),
                        series_type: "line".to_string(),
                    },
                    IndicatorSeries {
                        name: "signal".to_string(),
                        data: signal_data,
                        color: Some("#FF5722".to_string()),
                        series_type: "line".to_string(),
                    },
                    IndicatorSeries {
                        name: "histogram".to_string(),
                        data: histogram_data,
                        color: Some("#4CAF50".to_string()),
                        series_type: "bar".to_string(),
                    },
                ],
            })
        }
        Err(e) => Json(IndicatorDataResponse {
            indicator: "macd".to_string(),
            name: "MACD - 오류".to_string(),
            symbol: query.symbol,
            params: serde_json::json!({ "error": e.to_string() }),
            series: vec![],
        }),
    }
}

/// 볼린저 밴드 지표 데이터 조회.
///
/// GET /api/v1/analytics/indicators/bollinger
pub async fn get_bollinger_indicator(Query(query): Query<BollingerQuery>) -> impl IntoResponse {
    let days = parse_period_to_days(&query.period);
    let (timestamps, _, _, _, closes) = generate_sample_ohlcv(days);

    let engine = IndicatorEngine::new();
    let params = BollingerBandsParams {
        period: query.bb_period,
        std_dev_multiplier: Decimal::from_f64_retain(query.std_dev).unwrap_or(dec!(2.0)),
    };

    match engine.bollinger_bands(&closes, params) {
        Ok(bb_results) => {
            let upper_data: Vec<IndicatorPoint> = timestamps
                .iter()
                .zip(bb_results.iter())
                .map(|(&ts, result)| IndicatorPoint {
                    x: ts,
                    y: result.upper.map(|v| v.to_string()),
                })
                .collect();

            let middle_data: Vec<IndicatorPoint> = timestamps
                .iter()
                .zip(bb_results.iter())
                .map(|(&ts, result)| IndicatorPoint {
                    x: ts,
                    y: result.middle.map(|v| v.to_string()),
                })
                .collect();

            let lower_data: Vec<IndicatorPoint> = timestamps
                .iter()
                .zip(bb_results.iter())
                .map(|(&ts, result)| IndicatorPoint {
                    x: ts,
                    y: result.lower.map(|v| v.to_string()),
                })
                .collect();

            Json(IndicatorDataResponse {
                indicator: "bollinger".to_string(),
                name: format!("BB({}, {})", query.bb_period, query.std_dev),
                symbol: query.symbol,
                params: serde_json::json!({
                    "period": query.bb_period,
                    "std_dev": query.std_dev
                }),
                series: vec![
                    IndicatorSeries {
                        name: "upper".to_string(),
                        data: upper_data,
                        color: Some("#E91E63".to_string()),
                        series_type: "line".to_string(),
                    },
                    IndicatorSeries {
                        name: "middle".to_string(),
                        data: middle_data,
                        color: Some("#9C27B0".to_string()),
                        series_type: "line".to_string(),
                    },
                    IndicatorSeries {
                        name: "lower".to_string(),
                        data: lower_data,
                        color: Some("#2196F3".to_string()),
                        series_type: "line".to_string(),
                    },
                ],
            })
        }
        Err(e) => Json(IndicatorDataResponse {
            indicator: "bollinger".to_string(),
            name: "Bollinger Bands - 오류".to_string(),
            symbol: query.symbol,
            params: serde_json::json!({ "error": e.to_string() }),
            series: vec![],
        }),
    }
}

/// 스토캐스틱 지표 데이터 조회.
///
/// GET /api/v1/analytics/indicators/stochastic
pub async fn get_stochastic_indicator(Query(query): Query<StochasticQuery>) -> impl IntoResponse {
    let days = parse_period_to_days(&query.period);
    let (timestamps, _, highs, lows, closes) = generate_sample_ohlcv(days);

    let engine = IndicatorEngine::new();
    let params = StochasticParams {
        k_period: query.k_period,
        d_period: query.d_period,
    };

    match engine.stochastic(&highs, &lows, &closes, params) {
        Ok(stoch_results) => {
            let k_data: Vec<IndicatorPoint> = timestamps
                .iter()
                .zip(stoch_results.iter())
                .map(|(&ts, result)| IndicatorPoint {
                    x: ts,
                    y: result.k.map(|v| v.to_string()),
                })
                .collect();

            let d_data: Vec<IndicatorPoint> = timestamps
                .iter()
                .zip(stoch_results.iter())
                .map(|(&ts, result)| IndicatorPoint {
                    x: ts,
                    y: result.d.map(|v| v.to_string()),
                })
                .collect();

            Json(IndicatorDataResponse {
                indicator: "stochastic".to_string(),
                name: format!("Stochastic({}, {})", query.k_period, query.d_period),
                symbol: query.symbol,
                params: serde_json::json!({
                    "k_period": query.k_period,
                    "d_period": query.d_period
                }),
                series: vec![
                    IndicatorSeries {
                        name: "%K".to_string(),
                        data: k_data,
                        color: Some("#2196F3".to_string()),
                        series_type: "line".to_string(),
                    },
                    IndicatorSeries {
                        name: "%D".to_string(),
                        data: d_data,
                        color: Some("#FF9800".to_string()),
                        series_type: "line".to_string(),
                    },
                ],
            })
        }
        Err(e) => Json(IndicatorDataResponse {
            indicator: "stochastic".to_string(),
            name: "Stochastic - 오류".to_string(),
            symbol: query.symbol,
            params: serde_json::json!({ "error": e.to_string() }),
            series: vec![],
        }),
    }
}

/// ATR 지표 데이터 조회.
///
/// GET /api/v1/analytics/indicators/atr
pub async fn get_atr_indicator(Query(query): Query<AtrQuery>) -> impl IntoResponse {
    let days = parse_period_to_days(&query.period);
    let (timestamps, _, highs, lows, closes) = generate_sample_ohlcv(days);

    let engine = IndicatorEngine::new();
    let params = AtrParams {
        period: query.atr_period,
    };

    match engine.atr(&highs, &lows, &closes, params) {
        Ok(atr_values) => {
            let data: Vec<IndicatorPoint> = timestamps
                .iter()
                .zip(atr_values.iter())
                .map(|(&ts, value)| IndicatorPoint {
                    x: ts,
                    y: value.map(|v| v.to_string()),
                })
                .collect();

            Json(IndicatorDataResponse {
                indicator: "atr".to_string(),
                name: format!("ATR({})", query.atr_period),
                symbol: query.symbol,
                params: serde_json::json!({ "period": query.atr_period }),
                series: vec![IndicatorSeries {
                    name: "atr".to_string(),
                    data,
                    color: Some("#795548".to_string()),
                    series_type: "line".to_string(),
                }],
            })
        }
        Err(e) => Json(IndicatorDataResponse {
            indicator: "atr".to_string(),
            name: format!("ATR({}) - 오류", query.atr_period),
            symbol: query.symbol,
            params: serde_json::json!({ "error": e.to_string() }),
            series: vec![],
        }),
    }
}

/// 다중 지표 계산.
///
/// POST /api/v1/analytics/indicators/calculate
pub async fn calculate_indicators(
    Json(request): Json<CalculateIndicatorsRequest>,
) -> impl IntoResponse {
    let days = parse_period_to_days(&request.period);
    let (timestamps, _, highs, lows, closes) = generate_sample_ohlcv(days);

    let engine = IndicatorEngine::new();
    let mut results = Vec::new();

    for config in &request.indicators {
        let indicator_result = match config.indicator_type.as_str() {
            "sma" => {
                let period = config
                    .params
                    .get("period")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(20) as usize;

                if let Ok(values) = engine.sma(&closes, SmaParams { period }) {
                    let data: Vec<IndicatorPoint> = timestamps
                        .iter()
                        .zip(values.iter())
                        .map(|(&ts, v)| IndicatorPoint {
                            x: ts,
                            y: v.map(|d| d.to_string()),
                        })
                        .collect();

                    Some(IndicatorDataResponse {
                        indicator: "sma".to_string(),
                        name: config
                            .name
                            .clone()
                            .unwrap_or_else(|| format!("SMA({})", period)),
                        symbol: request.symbol.clone(),
                        params: serde_json::json!({ "period": period }),
                        series: vec![IndicatorSeries {
                            name: "sma".to_string(),
                            data,
                            color: config.color.clone().or_else(|| Some("#2196F3".to_string())),
                            series_type: "line".to_string(),
                        }],
                    })
                } else {
                    None
                }
            }
            "ema" => {
                let period = config
                    .params
                    .get("period")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(12) as usize;

                if let Ok(values) = engine.ema(&closes, EmaParams { period }) {
                    let data: Vec<IndicatorPoint> = timestamps
                        .iter()
                        .zip(values.iter())
                        .map(|(&ts, v)| IndicatorPoint {
                            x: ts,
                            y: v.map(|d| d.to_string()),
                        })
                        .collect();

                    Some(IndicatorDataResponse {
                        indicator: "ema".to_string(),
                        name: config
                            .name
                            .clone()
                            .unwrap_or_else(|| format!("EMA({})", period)),
                        symbol: request.symbol.clone(),
                        params: serde_json::json!({ "period": period }),
                        series: vec![IndicatorSeries {
                            name: "ema".to_string(),
                            data,
                            color: config.color.clone().or_else(|| Some("#FF9800".to_string())),
                            series_type: "line".to_string(),
                        }],
                    })
                } else {
                    None
                }
            }
            "rsi" => {
                let period = config
                    .params
                    .get("period")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(14) as usize;

                if let Ok(values) = engine.rsi(&closes, RsiParams { period }) {
                    let data: Vec<IndicatorPoint> = timestamps
                        .iter()
                        .zip(values.iter())
                        .map(|(&ts, v)| IndicatorPoint {
                            x: ts,
                            y: v.map(|d| d.to_string()),
                        })
                        .collect();

                    Some(IndicatorDataResponse {
                        indicator: "rsi".to_string(),
                        name: config
                            .name
                            .clone()
                            .unwrap_or_else(|| format!("RSI({})", period)),
                        symbol: request.symbol.clone(),
                        params: serde_json::json!({ "period": period }),
                        series: vec![IndicatorSeries {
                            name: "rsi".to_string(),
                            data,
                            color: config.color.clone().or_else(|| Some("#9C27B0".to_string())),
                            series_type: "line".to_string(),
                        }],
                    })
                } else {
                    None
                }
            }
            "macd" => {
                let fast = config
                    .params
                    .get("fast_period")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(12) as usize;
                let slow = config
                    .params
                    .get("slow_period")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(26) as usize;
                let signal = config
                    .params
                    .get("signal_period")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(9) as usize;

                if let Ok(macd_results) = engine.macd(
                    &closes,
                    MacdParams {
                        fast_period: fast,
                        slow_period: slow,
                        signal_period: signal,
                    },
                ) {
                    let macd_data: Vec<IndicatorPoint> = timestamps
                        .iter()
                        .zip(macd_results.iter())
                        .map(|(&ts, r)| IndicatorPoint {
                            x: ts,
                            y: r.macd.map(|d| d.to_string()),
                        })
                        .collect();
                    let signal_data: Vec<IndicatorPoint> = timestamps
                        .iter()
                        .zip(macd_results.iter())
                        .map(|(&ts, r)| IndicatorPoint {
                            x: ts,
                            y: r.signal.map(|d| d.to_string()),
                        })
                        .collect();
                    let hist_data: Vec<IndicatorPoint> = timestamps
                        .iter()
                        .zip(macd_results.iter())
                        .map(|(&ts, r)| IndicatorPoint {
                            x: ts,
                            y: r.histogram.map(|d| d.to_string()),
                        })
                        .collect();

                    Some(IndicatorDataResponse {
                        indicator: "macd".to_string(),
                        name: config
                            .name
                            .clone()
                            .unwrap_or_else(|| format!("MACD({},{},{})", fast, slow, signal)),
                        symbol: request.symbol.clone(),
                        params: serde_json::json!({ "fast_period": fast, "slow_period": slow, "signal_period": signal }),
                        series: vec![
                            IndicatorSeries {
                                name: "macd".to_string(),
                                data: macd_data,
                                color: Some("#2196F3".to_string()),
                                series_type: "line".to_string(),
                            },
                            IndicatorSeries {
                                name: "signal".to_string(),
                                data: signal_data,
                                color: Some("#FF5722".to_string()),
                                series_type: "line".to_string(),
                            },
                            IndicatorSeries {
                                name: "histogram".to_string(),
                                data: hist_data,
                                color: Some("#4CAF50".to_string()),
                                series_type: "bar".to_string(),
                            },
                        ],
                    })
                } else {
                    None
                }
            }
            "bollinger" => {
                let period = config
                    .params
                    .get("period")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(20) as usize;
                let std_dev = config
                    .params
                    .get("std_dev")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(2.0);

                if let Ok(bb_results) = engine.bollinger_bands(
                    &closes,
                    BollingerBandsParams {
                        period,
                        std_dev_multiplier: Decimal::from_f64_retain(std_dev).unwrap_or(dec!(2.0)),
                    },
                ) {
                    let upper: Vec<IndicatorPoint> = timestamps
                        .iter()
                        .zip(bb_results.iter())
                        .map(|(&ts, r)| IndicatorPoint {
                            x: ts,
                            y: r.upper.map(|d| d.to_string()),
                        })
                        .collect();
                    let middle: Vec<IndicatorPoint> = timestamps
                        .iter()
                        .zip(bb_results.iter())
                        .map(|(&ts, r)| IndicatorPoint {
                            x: ts,
                            y: r.middle.map(|d| d.to_string()),
                        })
                        .collect();
                    let lower: Vec<IndicatorPoint> = timestamps
                        .iter()
                        .zip(bb_results.iter())
                        .map(|(&ts, r)| IndicatorPoint {
                            x: ts,
                            y: r.lower.map(|d| d.to_string()),
                        })
                        .collect();

                    Some(IndicatorDataResponse {
                        indicator: "bollinger".to_string(),
                        name: config
                            .name
                            .clone()
                            .unwrap_or_else(|| format!("BB({}, {})", period, std_dev)),
                        symbol: request.symbol.clone(),
                        params: serde_json::json!({ "period": period, "std_dev": std_dev }),
                        series: vec![
                            IndicatorSeries {
                                name: "upper".to_string(),
                                data: upper,
                                color: Some("#E91E63".to_string()),
                                series_type: "line".to_string(),
                            },
                            IndicatorSeries {
                                name: "middle".to_string(),
                                data: middle,
                                color: Some("#9C27B0".to_string()),
                                series_type: "line".to_string(),
                            },
                            IndicatorSeries {
                                name: "lower".to_string(),
                                data: lower,
                                color: Some("#2196F3".to_string()),
                                series_type: "line".to_string(),
                            },
                        ],
                    })
                } else {
                    None
                }
            }
            "stochastic" => {
                let k_period = config
                    .params
                    .get("k_period")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(14) as usize;
                let d_period = config
                    .params
                    .get("d_period")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(3) as usize;

                if let Ok(stoch_results) = engine.stochastic(
                    &highs,
                    &lows,
                    &closes,
                    StochasticParams { k_period, d_period },
                ) {
                    let k_data: Vec<IndicatorPoint> = timestamps
                        .iter()
                        .zip(stoch_results.iter())
                        .map(|(&ts, r)| IndicatorPoint {
                            x: ts,
                            y: r.k.map(|d| d.to_string()),
                        })
                        .collect();
                    let d_data: Vec<IndicatorPoint> = timestamps
                        .iter()
                        .zip(stoch_results.iter())
                        .map(|(&ts, r)| IndicatorPoint {
                            x: ts,
                            y: r.d.map(|d| d.to_string()),
                        })
                        .collect();

                    Some(IndicatorDataResponse {
                        indicator: "stochastic".to_string(),
                        name: config
                            .name
                            .clone()
                            .unwrap_or_else(|| format!("Stochastic({}, {})", k_period, d_period)),
                        symbol: request.symbol.clone(),
                        params: serde_json::json!({ "k_period": k_period, "d_period": d_period }),
                        series: vec![
                            IndicatorSeries {
                                name: "%K".to_string(),
                                data: k_data,
                                color: Some("#2196F3".to_string()),
                                series_type: "line".to_string(),
                            },
                            IndicatorSeries {
                                name: "%D".to_string(),
                                data: d_data,
                                color: Some("#FF9800".to_string()),
                                series_type: "line".to_string(),
                            },
                        ],
                    })
                } else {
                    None
                }
            }
            "atr" => {
                let period = config
                    .params
                    .get("period")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(14) as usize;

                if let Ok(values) = engine.atr(&highs, &lows, &closes, AtrParams { period }) {
                    let data: Vec<IndicatorPoint> = timestamps
                        .iter()
                        .zip(values.iter())
                        .map(|(&ts, v)| IndicatorPoint {
                            x: ts,
                            y: v.map(|d| d.to_string()),
                        })
                        .collect();

                    Some(IndicatorDataResponse {
                        indicator: "atr".to_string(),
                        name: config
                            .name
                            .clone()
                            .unwrap_or_else(|| format!("ATR({})", period)),
                        symbol: request.symbol.clone(),
                        params: serde_json::json!({ "period": period }),
                        series: vec![IndicatorSeries {
                            name: "atr".to_string(),
                            data,
                            color: config.color.clone().or_else(|| Some("#795548".to_string())),
                            series_type: "line".to_string(),
                        }],
                    })
                } else {
                    None
                }
            }
            _ => None,
        };

        if let Some(result) = indicator_result {
            results.push(result);
        }
    }

    Json(CalculateIndicatorsResponse {
        symbol: request.symbol,
        period: request.period,
        results,
    })
}
