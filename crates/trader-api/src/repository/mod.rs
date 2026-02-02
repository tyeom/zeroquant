//! Repository pattern for database operations.
//!
//! 데이터베이스 접근 로직을 라우트 핸들러에서 분리하여 관리합니다.
//! 모든 Repository는 static methods 패턴을 사용합니다.

pub mod backtest_results;
pub mod cost_basis;
pub mod equity_history;
pub mod execution_cache;
pub mod journal;
pub mod klines;
pub mod orders;
pub mod portfolio;
pub mod positions;
pub mod screening;
pub mod strategies;
pub mod symbol_fundamental;
pub mod symbol_info;

pub use backtest_results::{
    BacktestResultDto, BacktestResultInput, BacktestResultRecord, BacktestResultsRepository,
    ListResultsFilter, ListResultsResponse as BacktestListResponse,
};
pub use equity_history::{
    EquityHistoryRepository, EquityPoint, ExecutionForSync, MonthlyReturn, PortfolioSnapshot,
    SyncResult,
};
pub use execution_cache::{
    CacheMeta, CachedExecution, ExecutionCacheRepository, ExecutionProvider, NewExecution,
};
pub use klines::{CacheMetadata, KlineRecord, KlinesRepository, NewKline};
pub use orders::{Order, OrderInput, OrderRepository, OrderStatus};
pub use portfolio::{PortfolioRepository, Position, PositionUpdate};
pub use positions::{
    HoldingPosition, PositionInput, PositionRecord, PositionRepository,
    SyncResult as PositionSyncResult,
};
pub use screening::{
    MomentumScreenResult, ScreeningFilter, ScreeningPreset, ScreeningRepository, ScreeningResult,
};
pub use strategies::StrategyRepository;
pub use symbol_fundamental::{
    NewSymbolFundamental, SymbolFundamental, SymbolFundamentalRepository, SymbolWithFundamental,
};
pub use symbol_info::{
    DeactivatedStats, ExternalFetchError, FailedSymbolInfo, FetchFailureResult, NewSymbolInfo,
    SymbolInfo, SymbolInfoRepository, SymbolSearchResult, MAX_FETCH_FAILURES,
};

pub use journal::{
    CumulativePnL,
    CurrentPosition,
    DailySummary,
    ExecutionFilter,
    JournalRepository,
    MonthlyPnL,
    PnLSummary,
    PositionSnapshotInput,
    PositionSnapshotRecord,
    StrategyPerformance,
    SymbolPnL,
    SyncResult as JournalSyncResult,
    TradeExecutionInput,
    TradeExecutionRecord,
    // 인사이트 타입
    TradingInsights,
    // 기간별 손익 타입
    WeeklyPnL,
    YearlyPnL,
};

pub use cost_basis::{
    build_tracker_from_executions, CostBasisSummary, CostBasisTracker, FifoSaleResult, Lot,
    LotUsage, TradeExecution,
};
