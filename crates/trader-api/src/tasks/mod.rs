//! 백그라운드 태스크 모듈.
//!
//! 서버 실행 중 주기적으로 실행되는 백그라운드 작업을 정의합니다.
//! - Fundamental 데이터 수집: Yahoo Finance에서 펀더멘털 데이터 배치 수집
//! - 심볼 동기화: KRX/Binance에서 종목 목록 자동 가져오기
//! - KRX CSV 동기화: 로컬 CSV 파일에서 KRX 종목/섹터 정보 가져오기
//! - EODData CSV 동기화: 해외 거래소 심볼 정보 가져오기

pub mod eod_csv_sync;
pub mod fundamental;
pub mod krx_csv_sync;
pub mod symbol_sync;

pub use eod_csv_sync::{sync_eod_all, sync_eod_exchange, sync_eod_unified, EodSyncResult};
pub use fundamental::{start_fundamental_collector, FundamentalCollectorConfig};
pub use krx_csv_sync::{sync_krx_from_csv, sync_krx_full, update_sectors_from_csv, CsvSyncResult};
pub use symbol_sync::{sync_symbols, SymbolSyncConfig};
