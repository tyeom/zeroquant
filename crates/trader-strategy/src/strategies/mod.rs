//! 내장 트레이딩 전략.
//!
//! 이 모듈은 검증된 여러 트레이딩 전략을 제공합니다:
//!
//! ## 단일 자산 전략
//! - **Grid Trading**: 일정 간격으로 매수/매도 주문 배치. 횡보장에 적합.
//! - **RSI Mean Reversion**: RSI 지표를 사용한 과매수/과매도 조건 트레이딩.
//! - **Bollinger Bands**: 동적 변동성 밴드를 사용한 평균 회귀.
//! - **Volatility Breakout**: 추세장을 위한 Larry Williams 모멘텀 전략.
//! - **Magic Split**: 레벨 기반 수익 실현과 함께하는 체계적 물타기.
//! - **SMA Crossover**: 이동평균 교차 전략.
//! - **Candle Pattern**: 35가지 캔들스틱 패턴 인식.
//! - **Infinity Bot**: 50라운드 피라미드 물타기.
//! - **Market Interest Day**: 거래량 급증 종목 단기 트레이딩.
//! - **Stock Gugan**: 구간분할 장기 투자 전략.
//! - **Sector VB**: 섹터 ETF 변동성 돌파 전략.
//!
//! ## 자산배분 전략
//! - **Simple Power**: MA130 필터를 적용한 TQQQ/SCHD/PFIX/TMF 모멘텀 자산 배분.
//! - **HAA**: 위험 감지를 위한 카나리아 자산을 포함한 계층적 자산 배분.
//! - **XAA**: 확장 자산배분 (Expanded Asset Allocation).
//! - **BAA**: Bold Asset Allocation - 카나리아 기반 듀얼 모멘텀 전략.
//! - **All Weather**: 레이 달리오 올웨더 포트폴리오 (US/KR).
//! - **Snow**: TIP 기반 이동평균 모멘텀 전략 (US/KR).
//! - **Stock Rotation**: 종목 갈아타기 시스템.
//! - **Market Cap TOP**: 미국 시총 상위 종목 투자.
//! - **US 3X Leverage**: 미국 3배 레버리지/인버스 ETF 조합 전략.
//! - **Sector Momentum**: 섹터 ETF 모멘텀 순위 기반 투자 (US/KR).
//! - **Dual Momentum**: 한국 주식 + 미국 채권 듀얼 모멘텀.
//! - **Small Cap Quant**: 코스닥 소형주 퀀트 전략.
//! - **Pension Bot**: 연금 자동화 정적+동적 자산배분.
//!
//! ## 한국 지수 전략
//! - **KOSPI BothSide**: 코스피 레버리지/인버스 양방향 매매 전략.
//! - **KOSDAQ Fire Rain**: 코스피/코스닥 복합 양방향 전략.
//!
//! ## 공통 유틸리티
//!
//! `common` 서브모듈은 재사용 가능한 컴포넌트를 제공합니다:
//! - **Momentum Calculator**: 자산 배분을 위한 다기간 모멘텀 스코어링

pub mod all_weather;
pub mod baa;
pub mod bollinger;
pub mod candle_pattern;
pub mod common;
pub mod dual_momentum;
pub mod grid;
pub mod haa;
pub mod infinity_bot;
pub mod kosdaq_fire_rain;
pub mod kospi_bothside;
pub mod magic_split;
pub mod market_cap_top;
pub mod market_interest_day;
pub mod pension_bot;
pub mod rsi;
pub mod sector_momentum;
pub mod sector_vb;
pub mod simple_power;
pub mod sma;
pub mod small_cap_quant;
pub mod snow;
pub mod stock_gugan;
pub mod stock_rotation;
pub mod us_3x_leverage;
pub mod volatility_breakout;
pub mod xaa;

pub use all_weather::{
    AllWeatherConfig, AllWeatherStrategy, AssetClass, AssetInfo as AllWeatherAssetInfo,
};
pub use baa::*;
pub use bollinger::*;
pub use candle_pattern::*;
pub use common::*;
pub use dual_momentum::*;
pub use grid::*;
pub use haa::{AssetInfo as HaaAssetInfo, AssetType, HaaConfig, HaaStrategy};
pub use infinity_bot::*;
pub use kosdaq_fire_rain::*;
pub use kospi_bothside::*;
pub use magic_split::*;
pub use market_cap_top::{
    MarketCapTopConfig, MarketCapTopState, MarketCapTopStrategy, PositionInfo,
    WeightingMethod as MarketCapWeightingMethod,
};
pub use market_interest_day::*;
pub use pension_bot::*;
pub use rsi::*;
pub use sector_momentum::{
    SectorInfo, SectorMomentumConfig, SectorMomentumMarket, SectorMomentumStrategy,
    WeightingMethod as SectorWeightingMethod,
};
pub use sector_vb::*;
pub use simple_power::*;
pub use sma::*;
pub use small_cap_quant::*;
pub use snow::*;
pub use stock_gugan::*;
pub use stock_rotation::*;
pub use us_3x_leverage::*;
pub use volatility_breakout::*;
pub use xaa::*;
