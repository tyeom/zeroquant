//! EODData CSV 기반 해외 심볼 동기화 모듈.
//!
//! Python 스크래퍼로 생성된 `data/eod_{exchange}.csv` 파일을 읽어
//! symbol_info 테이블을 업데이트합니다.
//!
//! ## CSV 파일 형식
//!
//! ```csv
//! ticker,name,exchange,yahoo_symbol
//! AAPL,Apple Inc.,NYSE,AAPL
//! MSFT,Microsoft Corporation,NASDAQ,MSFT
//! TSLA,Tesla Inc.,NASDAQ,TSLA
//! ```
//!
//! ## 지원 거래소
//!
//! | 코드 | 거래소 | Market | Yahoo 접미사 |
//! |------|--------|--------|--------------|
//! | NYSE | 뉴욕증권거래소 | US | (없음) |
//! | NASDAQ | 나스닥 | US | (없음) |
//! | AMEX | 아메리칸증권거래소 | US | (없음) |
//! | LSE | 런던증권거래소 | GB | .L |
//! | TSX | 토론토증권거래소 | CA | .TO |
//! | ASX | 호주증권거래소 | AU | .AX |
//! | HKEX | 홍콩증권거래소 | HK | .HK |
//! | SGX | 싱가포르증권거래소 | SG | .SI |
//!
//! ## 사용법
//!
//! ```rust,ignore
//! use trader_api::tasks::eod_csv_sync::{sync_eod_exchange, sync_eod_all};
//!
//! // 특정 거래소 동기화
//! let result = sync_eod_exchange(pool, "NYSE", "data/eod_nyse.csv").await?;
//!
//! // 모든 거래소 동기화
//! let results = sync_eod_all(pool, "data/").await?;
//! ```

use std::collections::HashMap;
use std::path::Path;

use sqlx::PgPool;
use tracing::{info, warn};

use crate::repository::{NewSymbolInfo, SymbolInfoRepository};

/// EODData CSV 레코드.
#[derive(Debug, Clone)]
struct EodSymbolRecord {
    /// 티커 심볼
    ticker: String,
    /// 회사/종목명
    name: String,
    /// 거래소 코드
    exchange: String,
    /// Yahoo Finance 심볼
    yahoo_symbol: String,
}

/// CSV 동기화 결과.
#[derive(Debug, Clone, Default)]
pub struct EodSyncResult {
    /// 거래소 코드
    pub exchange: String,
    /// 처리된 총 레코드 수
    pub total_processed: usize,
    /// 성공적으로 upsert된 수
    pub upserted: usize,
    /// 실패한 수
    pub failed: usize,
    /// 스킵된 수 (유효하지 않은 레코드)
    pub skipped: usize,
}

/// 거래소 코드를 Market 코드로 변환.
///
/// ## Returns
/// 2자리 국가/시장 코드
fn exchange_to_market(exchange: &str) -> &'static str {
    match exchange.to_uppercase().as_str() {
        "NYSE" | "NASDAQ" | "AMEX" | "OTC" | "ARCA" => "US",
        "LSE" | "LSE-INTL" => "GB",
        "TSX" | "TSX-V" => "CA",
        "ASX" => "AU",
        "HKEX" | "HKSE" => "HK",
        "SGX" => "SG",
        "XETRA" | "FWB" => "DE",
        "EURONEXT" | "EPA" => "EU",
        "SIX" | "SWX" => "CH",
        "JSE" => "ZA",
        "NSE" | "BSE" => "IN",
        "TSE" | "TYO" => "JP",
        "SSE" | "SZSE" => "CN",
        "TWSE" | "TPE" => "TW",
        "FOREX" | "FX" => "FX",
        "INDEX" => "INDEX",
        _ => "OTHER",
    }
}

/// CSV 파일에서 EODData 심볼 레코드 파싱.
///
/// 첫 번째 줄은 헤더로 가정합니다.
fn parse_eod_csv(content: &str) -> Vec<EodSymbolRecord> {
    let mut records = Vec::new();

    for (line_num, line) in content.lines().enumerate() {
        // 헤더 스킵
        if line_num == 0 {
            continue;
        }

        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        // CSV 파싱 (콤마 분리, 따옴표 처리)
        let parts: Vec<&str> = parse_csv_line(line);

        // ticker,name,exchange,yahoo_symbol
        if parts.len() >= 4 {
            let ticker = parts[0].trim().to_string();
            let name = parts[1].trim().to_string();
            let exchange = parts[2].trim().to_string();
            let yahoo_symbol = parts[3].trim().to_string();

            if !ticker.is_empty() && !name.is_empty() && is_valid_ticker(&ticker) {
                records.push(EodSymbolRecord {
                    ticker,
                    name,
                    exchange,
                    yahoo_symbol,
                });
            }
        } else if parts.len() >= 2 {
            // 최소 ticker, name만 있는 경우
            let ticker = parts[0].trim().to_string();
            let name = parts[1].trim().to_string();

            if !ticker.is_empty() && !name.is_empty() && is_valid_ticker(&ticker) {
                records.push(EodSymbolRecord {
                    ticker: ticker.clone(),
                    name,
                    exchange: String::new(),
                    yahoo_symbol: ticker,
                });
            }
        }
    }

    records
}

/// CSV 라인 파싱 (따옴표 처리).
fn parse_csv_line(line: &str) -> Vec<&str> {
    let mut result = Vec::new();
    let mut in_quotes = false;
    let mut field_start = 0;

    let chars: Vec<char> = line.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        let c = chars[i];

        if c == '"' {
            in_quotes = !in_quotes;
        } else if c == ',' && !in_quotes {
            let field = &line[field_start..byte_index(line, i)];
            result.push(field.trim_matches('"'));
            field_start = byte_index(line, i + 1);
        }

        i += 1;
    }

    // 마지막 필드
    if field_start < line.len() {
        let field = &line[field_start..];
        result.push(field.trim_matches('"'));
    }

    result
}

/// 문자 인덱스를 바이트 인덱스로 변환.
fn byte_index(s: &str, char_index: usize) -> usize {
    s.char_indices()
        .nth(char_index)
        .map(|(i, _)| i)
        .unwrap_or(s.len())
}

/// 유효한 티커인지 확인.
///
/// - 영숫자 및 일부 특수문자 (., -, ^) 허용
/// - 최소 1자, 최대 20자
fn is_valid_ticker(ticker: &str) -> bool {
    if ticker.is_empty() || ticker.len() > 20 {
        return false;
    }

    ticker.chars().all(|c| c.is_alphanumeric() || c == '.' || c == '-' || c == '^' || c == '_')
}

/// 특정 거래소 CSV 파일에서 심볼 정보를 읽어 DB에 동기화.
///
/// # Arguments
/// * `pool` - PostgreSQL 연결 풀
/// * `exchange_code` - 거래소 코드 (예: NYSE, NASDAQ)
/// * `csv_path` - CSV 파일 경로
///
/// # Returns
/// 동기화 결과
pub async fn sync_eod_exchange<P: AsRef<Path>>(
    pool: &PgPool,
    exchange_code: &str,
    csv_path: P,
) -> Result<EodSyncResult, Box<dyn std::error::Error + Send + Sync>> {
    let csv_path = csv_path.as_ref();
    info!(
        exchange = %exchange_code,
        path = %csv_path.display(),
        "EODData CSV 파일 로드 시작"
    );

    // 파일 읽기
    let content = tokio::fs::read_to_string(csv_path).await?;
    let records = parse_eod_csv(&content);

    if records.is_empty() {
        warn!(exchange = %exchange_code, "CSV 파일에서 유효한 레코드를 찾지 못함");
        return Ok(EodSyncResult {
            exchange: exchange_code.to_string(),
            ..Default::default()
        });
    }

    info!(
        exchange = %exchange_code,
        count = records.len(),
        "CSV 레코드 파싱 완료"
    );

    // Market 코드 결정
    let market = exchange_to_market(exchange_code);

    // NewSymbolInfo로 변환
    let new_symbols: Vec<NewSymbolInfo> = records
        .iter()
        .map(|r| {
            let exchange = if r.exchange.is_empty() {
                exchange_code.to_string()
            } else {
                r.exchange.clone()
            };

            NewSymbolInfo {
                ticker: r.ticker.clone(),
                name: r.name.clone(),
                name_en: Some(r.name.clone()), // 해외 종목은 영문명 사용
                market: market.to_string(),
                exchange: Some(exchange),
                sector: None,
                yahoo_symbol: if r.yahoo_symbol.is_empty() {
                    None
                } else {
                    Some(r.yahoo_symbol.clone())
                },
            }
        })
        .collect();

    // 배치 upsert
    let upserted = SymbolInfoRepository::upsert_batch(pool, &new_symbols).await?;

    let result = EodSyncResult {
        exchange: exchange_code.to_string(),
        total_processed: records.len(),
        upserted,
        failed: 0,
        skipped: records.len().saturating_sub(upserted),
    };

    info!(
        exchange = %exchange_code,
        total = result.total_processed,
        upserted = result.upserted,
        skipped = result.skipped,
        "EODData CSV 동기화 완료"
    );

    Ok(result)
}

/// 디렉토리 내 모든 EODData CSV 파일 동기화.
///
/// `eod_*.csv` 패턴의 파일을 찾아 각각 동기화합니다.
///
/// # Arguments
/// * `pool` - PostgreSQL 연결 풀
/// * `data_dir` - CSV 파일들이 있는 디렉토리
///
/// # Returns
/// 각 거래소별 동기화 결과 맵
pub async fn sync_eod_all<P: AsRef<Path>>(
    pool: &PgPool,
    data_dir: P,
) -> Result<HashMap<String, EodSyncResult>, Box<dyn std::error::Error + Send + Sync>> {
    let data_dir = data_dir.as_ref();
    info!(path = %data_dir.display(), "EODData 전체 동기화 시작");

    let mut results = HashMap::new();

    // 디렉토리 내 eod_*.csv 파일 검색
    let mut dir = tokio::fs::read_dir(data_dir).await?;

    while let Some(entry) = dir.next_entry().await? {
        let path = entry.path();

        if let Some(filename) = path.file_name().and_then(|n| n.to_str()) {
            // eod_nyse.csv -> nyse
            if filename.starts_with("eod_") && filename.ends_with(".csv") {
                // eod_all_symbols.csv 제외
                if filename == "eod_all_symbols.csv" {
                    continue;
                }

                let exchange_code = filename
                    .strip_prefix("eod_")
                    .and_then(|s| s.strip_suffix(".csv"))
                    .map(|s| s.to_uppercase())
                    .unwrap_or_default();

                if !exchange_code.is_empty() {
                    match sync_eod_exchange(pool, &exchange_code, &path).await {
                        Ok(result) => {
                            results.insert(exchange_code, result);
                        }
                        Err(e) => {
                            warn!(
                                exchange = %exchange_code,
                                error = %e,
                                "거래소 동기화 실패"
                            );
                        }
                    }
                }
            }
        }
    }

    // 총계 로그
    let total_upserted: usize = results.values().map(|r| r.upserted).sum();
    let total_processed: usize = results.values().map(|r| r.total_processed).sum();

    info!(
        exchanges = results.len(),
        total_processed = total_processed,
        total_upserted = total_upserted,
        "EODData 전체 동기화 완료"
    );

    Ok(results)
}

/// 통합 CSV 파일 (eod_all_symbols.csv) 동기화.
///
/// 모든 거래소의 심볼이 하나의 파일에 있는 경우 사용합니다.
///
/// # Arguments
/// * `pool` - PostgreSQL 연결 풀
/// * `csv_path` - 통합 CSV 파일 경로
///
/// # Returns
/// 거래소별 동기화 결과
pub async fn sync_eod_unified<P: AsRef<Path>>(
    pool: &PgPool,
    csv_path: P,
) -> Result<HashMap<String, EodSyncResult>, Box<dyn std::error::Error + Send + Sync>> {
    let csv_path = csv_path.as_ref();
    info!(path = %csv_path.display(), "EODData 통합 CSV 동기화 시작");

    // 파일 읽기
    let content = tokio::fs::read_to_string(csv_path).await?;
    let records = parse_eod_csv(&content);

    if records.is_empty() {
        warn!("통합 CSV 파일에서 유효한 레코드를 찾지 못함");
        return Ok(HashMap::new());
    }

    // 거래소별로 그룹화
    let mut by_exchange: HashMap<String, Vec<EodSymbolRecord>> = HashMap::new();
    for record in records {
        let exchange = if record.exchange.is_empty() {
            "UNKNOWN".to_string()
        } else {
            record.exchange.to_uppercase()
        };
        by_exchange.entry(exchange).or_default().push(record);
    }

    let mut results = HashMap::new();

    for (exchange_code, exchange_records) in by_exchange {
        let market = exchange_to_market(&exchange_code);

        let new_symbols: Vec<NewSymbolInfo> = exchange_records
            .iter()
            .map(|r| NewSymbolInfo {
                ticker: r.ticker.clone(),
                name: r.name.clone(),
                name_en: Some(r.name.clone()),
                market: market.to_string(),
                exchange: Some(exchange_code.clone()),
                sector: None,
                yahoo_symbol: if r.yahoo_symbol.is_empty() {
                    None
                } else {
                    Some(r.yahoo_symbol.clone())
                },
            })
            .collect();

        let count = new_symbols.len();
        let upserted = SymbolInfoRepository::upsert_batch(pool, &new_symbols).await?;

        results.insert(
            exchange_code.clone(),
            EodSyncResult {
                exchange: exchange_code,
                total_processed: count,
                upserted,
                failed: 0,
                skipped: count.saturating_sub(upserted),
            },
        );
    }

    let total_upserted: usize = results.values().map(|r| r.upserted).sum();
    info!(
        exchanges = results.len(),
        total_upserted = total_upserted,
        "EODData 통합 CSV 동기화 완료"
    );

    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_eod_csv() {
        let csv = r#"ticker,name,exchange,yahoo_symbol
AAPL,Apple Inc.,NASDAQ,AAPL
MSFT,Microsoft Corporation,NASDAQ,MSFT
GOOGL,Alphabet Inc.,NASDAQ,GOOGL"#;

        let records = parse_eod_csv(csv);

        assert_eq!(records.len(), 3);
        assert_eq!(records[0].ticker, "AAPL");
        assert_eq!(records[0].name, "Apple Inc.");
        assert_eq!(records[0].exchange, "NASDAQ");
        assert_eq!(records[1].ticker, "MSFT");
        assert_eq!(records[2].yahoo_symbol, "GOOGL");
    }

    #[test]
    fn test_parse_csv_with_quotes() {
        let csv = r#"ticker,name,exchange,yahoo_symbol
BRK.A,"Berkshire Hathaway, Inc.",NYSE,BRK-A
"VOO","Vanguard S&P 500 ETF",ARCA,VOO"#;

        let records = parse_eod_csv(csv);

        assert_eq!(records.len(), 2);
        assert_eq!(records[0].ticker, "BRK.A");
        assert_eq!(records[0].name, "Berkshire Hathaway, Inc.");
        assert_eq!(records[1].ticker, "VOO");
    }

    #[test]
    fn test_is_valid_ticker() {
        assert!(is_valid_ticker("AAPL"));
        assert!(is_valid_ticker("BRK.A"));
        assert!(is_valid_ticker("BRK-B"));
        assert!(is_valid_ticker("^GSPC")); // S&P 500 인덱스
        assert!(is_valid_ticker("005930")); // KRX 종목
        assert!(!is_valid_ticker("")); // 빈 문자열
        assert!(!is_valid_ticker("AAAAAAAAAAAAAAAAAAAAA")); // 너무 긴 티커
    }

    #[test]
    fn test_exchange_to_market() {
        assert_eq!(exchange_to_market("NYSE"), "US");
        assert_eq!(exchange_to_market("NASDAQ"), "US");
        assert_eq!(exchange_to_market("nasdaq"), "US"); // 소문자
        assert_eq!(exchange_to_market("LSE"), "GB");
        assert_eq!(exchange_to_market("HKEX"), "HK");
        assert_eq!(exchange_to_market("TSX"), "CA");
        assert_eq!(exchange_to_market("UNKNOWN"), "OTHER");
    }

    #[test]
    fn test_parse_minimal_csv() {
        // ticker, name만 있는 경우
        let csv = r#"ticker,name
AAPL,Apple Inc.
MSFT,Microsoft"#;

        let records = parse_eod_csv(csv);

        assert_eq!(records.len(), 2);
        assert_eq!(records[0].ticker, "AAPL");
        assert_eq!(records[0].yahoo_symbol, "AAPL"); // ticker를 yahoo_symbol로 사용
    }
}
