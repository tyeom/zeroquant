//! KRX CSV 기반 심볼 동기화 모듈.
//!
//! `data/krx_codes.csv`와 `data/krx_sector_map.csv` 파일을 읽어
//! symbol_info 테이블을 업데이트합니다.
//!
//! ## CSV 파일 형식
//!
//! ### krx_codes.csv
//! ```csv
//! 종목코드,종목명
//! 005930,삼성전자
//! 000660,SK하이닉스
//! ```
//!
//! ### krx_sector_map.csv
//! ```csv
//! 종목코드,업종
//! 005930,반도체 제조업
//! 000660,반도체 제조업
//! ```
//!
//! ## 사용법
//!
//! ```rust,ignore
//! use trader_api::tasks::krx_csv_sync::{sync_krx_from_csv, update_sectors_from_csv};
//!
//! // 전체 심볼 동기화
//! let count = sync_krx_from_csv(pool, "data/krx_codes.csv").await?;
//!
//! // 섹터 정보만 업데이트
//! let count = update_sectors_from_csv(pool, "data/krx_sector_map.csv").await?;
//! ```

use std::collections::HashMap;
use std::path::Path;

use sqlx::PgPool;
use tracing::{info, warn};

use crate::repository::{NewSymbolInfo, SymbolInfoRepository};

/// KRX 심볼 CSV 레코드.
#[derive(Debug, Clone)]
struct KrxSymbolRecord {
    /// 종목코드 (6자리)
    ticker: String,
    /// 종목명
    name: String,
}

/// KRX 섹터 CSV 레코드.
#[derive(Debug, Clone)]
struct KrxSectorRecord {
    /// 종목코드
    ticker: String,
    /// 업종
    sector: String,
}

/// CSV 동기화 결과.
#[derive(Debug, Clone, Default)]
pub struct CsvSyncResult {
    /// 처리된 총 레코드 수
    pub total_processed: usize,
    /// 성공적으로 upsert된 수
    pub upserted: usize,
    /// 실패한 수
    pub failed: usize,
    /// 스킵된 수 (유효하지 않은 레코드)
    pub skipped: usize,
}

/// CSV 파일에서 KRX 종목코드 파싱.
///
/// 첫 번째 줄은 헤더로 가정합니다.
fn parse_krx_codes_csv(content: &str) -> Vec<KrxSymbolRecord> {
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

        if parts.len() >= 2 {
            let ticker = parts[0].trim().to_string();
            let name = parts[1].trim().to_string();

            // 유효한 티커 확인 (6자리 숫자 또는 영숫자)
            if !ticker.is_empty() && !name.is_empty() && is_valid_krx_ticker(&ticker) {
                records.push(KrxSymbolRecord { ticker, name });
            }
        }
    }

    records
}

/// CSV 파일에서 KRX 섹터 매핑 파싱.
fn parse_krx_sector_csv(content: &str) -> Vec<KrxSectorRecord> {
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

        // CSV 파싱
        let parts: Vec<&str> = parse_csv_line(line);

        if parts.len() >= 2 {
            let ticker = parts[0].trim().to_string();
            let sector = parts[1].trim().to_string();

            if !ticker.is_empty() && !sector.is_empty() {
                records.push(KrxSectorRecord { ticker, sector });
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

/// 유효한 KRX 티커인지 확인.
///
/// - 일반 종목: 6자리 숫자 (예: 005930)
/// - ETF/ETN: 6자리 숫자 (예: 069500)
/// - 우선주 등 특수 형식도 허용
fn is_valid_krx_ticker(ticker: &str) -> bool {
    // 최소 6자리
    if ticker.len() < 6 {
        return false;
    }

    // 숫자 또는 영숫자 조합 허용
    ticker.chars().all(|c| c.is_alphanumeric())
}

/// KRX 종목코드로부터 거래소(KOSPI/KOSDAQ) 판별.
///
/// ## 규칙
/// - `0`으로 시작: KOSPI (예: 005930 삼성전자)
/// - `1~4`로 시작: KOSDAQ (예: 373220 LG에너지솔루션, 488900 현대우주항공)
/// - 그 외: 기본적으로 KOSDAQ 처리 (신규 상장 등)
///
/// ## Returns
/// ("KOSPI" | "KOSDAQ", ".KS" | ".KQ")
fn determine_exchange(ticker: &str) -> (&'static str, &'static str) {
    if ticker.is_empty() {
        return ("KOSPI", ".KS");
    }

    let first_char = ticker.chars().next().unwrap();

    match first_char {
        '0' => ("KOSPI", ".KS"),
        '1'..='4' => ("KOSDAQ", ".KQ"),
        _ => ("KOSDAQ", ".KQ"), // 5~9로 시작하는 경우도 KOSDAQ일 가능성 높음
    }
}

/// 6자리 숫자 티커를 Yahoo Finance 심볼로 변환.
///
/// - KOSPI (0으로 시작): .KS 접미사
/// - KOSDAQ (1~4로 시작): .KQ 접미사
fn to_yahoo_symbol(ticker: &str) -> String {
    let (_, suffix) = determine_exchange(ticker);
    format!("{}{}", ticker, suffix)
}

/// krx_codes.csv에서 심볼 정보를 읽어 DB에 동기화.
///
/// # Arguments
/// * `pool` - PostgreSQL 연결 풀
/// * `csv_path` - CSV 파일 경로
///
/// # Returns
/// 동기화 결과
pub async fn sync_krx_from_csv<P: AsRef<Path>>(
    pool: &PgPool,
    csv_path: P,
) -> Result<CsvSyncResult, Box<dyn std::error::Error + Send + Sync>> {
    let csv_path = csv_path.as_ref();
    info!(path = %csv_path.display(), "KRX CSV 파일 로드 시작");

    // 파일 읽기
    let content = tokio::fs::read_to_string(csv_path).await?;
    let records = parse_krx_codes_csv(&content);

    if records.is_empty() {
        warn!("CSV 파일에서 유효한 레코드를 찾지 못함");
        return Ok(CsvSyncResult::default());
    }

    info!(count = records.len(), "CSV 레코드 파싱 완료");

    // NewSymbolInfo로 변환 (거래소 정보 포함)
    let new_symbols: Vec<NewSymbolInfo> = records
        .iter()
        .map(|r| {
            let (exchange, _) = determine_exchange(&r.ticker);
            NewSymbolInfo {
                ticker: r.ticker.clone(),
                name: r.name.clone(),
                name_en: None, // CSV에 영문명 없음
                market: "KR".to_string(),
                exchange: Some(exchange.to_string()), // KOSPI 또는 KOSDAQ
                sector: None, // 별도 CSV에서 업데이트
                yahoo_symbol: Some(to_yahoo_symbol(&r.ticker)),
            }
        })
        .collect();

    // 배치 upsert
    let upserted = SymbolInfoRepository::upsert_batch(pool, &new_symbols).await?;

    let result = CsvSyncResult {
        total_processed: records.len(),
        upserted,
        failed: 0,
        skipped: records.len() - upserted,
    };

    info!(
        total = result.total_processed,
        upserted = result.upserted,
        skipped = result.skipped,
        "KRX CSV 동기화 완료"
    );

    Ok(result)
}

/// krx_sector_map.csv에서 섹터 정보를 읽어 DB 업데이트.
///
/// 기존 symbol_info 레코드의 sector 컬럼만 업데이트합니다.
///
/// # Arguments
/// * `pool` - PostgreSQL 연결 풀
/// * `csv_path` - 섹터 매핑 CSV 파일 경로
///
/// # Returns
/// 업데이트 결과
pub async fn update_sectors_from_csv<P: AsRef<Path>>(
    pool: &PgPool,
    csv_path: P,
) -> Result<CsvSyncResult, Box<dyn std::error::Error + Send + Sync>> {
    let csv_path = csv_path.as_ref();
    info!(path = %csv_path.display(), "섹터 CSV 파일 로드 시작");

    // 파일 읽기
    let content = tokio::fs::read_to_string(csv_path).await?;
    let records = parse_krx_sector_csv(&content);

    if records.is_empty() {
        warn!("섹터 CSV 파일에서 유효한 레코드를 찾지 못함");
        return Ok(CsvSyncResult::default());
    }

    info!(count = records.len(), "섹터 레코드 파싱 완료");

    // 섹터 맵 생성
    let sector_map: HashMap<String, String> = records
        .into_iter()
        .map(|r| (r.ticker, r.sector))
        .collect();

    // 배치 업데이트
    let updated = update_sectors_batch(pool, &sector_map).await?;

    let result = CsvSyncResult {
        total_processed: sector_map.len(),
        upserted: updated,
        failed: 0,
        skipped: sector_map.len() - updated,
    };

    info!(
        total = result.total_processed,
        updated = result.upserted,
        skipped = result.skipped,
        "섹터 업데이트 완료"
    );

    Ok(result)
}

/// 섹터 정보 일괄 업데이트.
async fn update_sectors_batch(
    pool: &PgPool,
    sector_map: &HashMap<String, String>,
) -> Result<usize, sqlx::Error> {
    let mut updated = 0;

    // 청크로 나누어 처리 (한 번에 100개씩)
    let tickers: Vec<_> = sector_map.keys().collect();

    for chunk in tickers.chunks(100) {
        for ticker in chunk {
            if let Some(sector) = sector_map.get(*ticker) {
                let result = sqlx::query(
                    r#"
                    UPDATE symbol_info
                    SET sector = $1, updated_at = NOW()
                    WHERE ticker = $2 AND market = 'KR'
                    "#,
                )
                .bind(sector)
                .bind(*ticker)
                .execute(pool)
                .await?;

                if result.rows_affected() > 0 {
                    updated += 1;
                }
            }
        }
    }

    Ok(updated)
}

/// CSV 파일들을 이용한 전체 KRX 동기화.
///
/// 1. krx_codes.csv에서 심볼 목록 동기화
/// 2. krx_sector_map.csv에서 섹터 정보 업데이트
///
/// # Arguments
/// * `pool` - PostgreSQL 연결 풀
/// * `codes_csv` - 종목 코드 CSV 경로
/// * `sector_csv` - 섹터 매핑 CSV 경로
///
/// # Returns
/// (심볼 동기화 결과, 섹터 업데이트 결과)
pub async fn sync_krx_full<P1: AsRef<Path>, P2: AsRef<Path>>(
    pool: &PgPool,
    codes_csv: P1,
    sector_csv: P2,
) -> Result<(CsvSyncResult, CsvSyncResult), Box<dyn std::error::Error + Send + Sync>> {
    info!("KRX 전체 동기화 시작");

    // 1. 심볼 동기화
    let symbol_result = sync_krx_from_csv(pool, codes_csv).await?;

    // 2. 섹터 업데이트
    let sector_result = update_sectors_from_csv(pool, sector_csv).await?;

    info!(
        symbols_synced = symbol_result.upserted,
        sectors_updated = sector_result.upserted,
        "KRX 전체 동기화 완료"
    );

    Ok((symbol_result, sector_result))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_krx_codes_csv() {
        let csv = r#"종목코드,종목명
005930,삼성전자
000660,SK하이닉스
005380,현대차"#;

        let records = parse_krx_codes_csv(csv);

        assert_eq!(records.len(), 3);
        assert_eq!(records[0].ticker, "005930");
        assert_eq!(records[0].name, "삼성전자");
        assert_eq!(records[1].ticker, "000660");
        assert_eq!(records[2].name, "현대차");
    }

    #[test]
    fn test_parse_krx_sector_csv() {
        let csv = r#"종목코드,업종
005930,반도체 제조업
000660,반도체 제조업
005380,자동차 신품 부품 제조업"#;

        let records = parse_krx_sector_csv(csv);

        assert_eq!(records.len(), 3);
        assert_eq!(records[0].ticker, "005930");
        assert_eq!(records[0].sector, "반도체 제조업");
    }

    #[test]
    fn test_parse_csv_line_with_quotes() {
        let line = r#"403850,"영화, 비디오물, 방송프로그램 제작 및 배급업""#;
        let parts = parse_csv_line(line);

        assert_eq!(parts.len(), 2);
        assert_eq!(parts[0], "403850");
        assert_eq!(parts[1], "영화, 비디오물, 방송프로그램 제작 및 배급업");
    }

    #[test]
    fn test_is_valid_krx_ticker() {
        assert!(is_valid_krx_ticker("005930"));
        assert!(is_valid_krx_ticker("000660"));
        assert!(is_valid_krx_ticker("373220"));
        assert!(!is_valid_krx_ticker("12345")); // 5자리
        assert!(!is_valid_krx_ticker("")); // 빈 문자열
    }

    #[test]
    fn test_to_yahoo_symbol() {
        // KOSPI (0으로 시작)
        assert_eq!(to_yahoo_symbol("005930"), "005930.KS"); // 삼성전자
        assert_eq!(to_yahoo_symbol("000660"), "000660.KS"); // SK하이닉스

        // KOSDAQ (1~4로 시작)
        assert_eq!(to_yahoo_symbol("373220"), "373220.KQ"); // LG에너지솔루션
        assert_eq!(to_yahoo_symbol("247540"), "247540.KQ"); // 에코프로비엠
    }

    #[test]
    fn test_determine_exchange() {
        // KOSPI - 0으로 시작
        assert_eq!(determine_exchange("005930"), ("KOSPI", ".KS"));
        assert_eq!(determine_exchange("000660"), ("KOSPI", ".KS"));
        assert_eq!(determine_exchange("035420"), ("KOSPI", ".KS"));

        // KOSDAQ - 1~4로 시작
        assert_eq!(determine_exchange("112040"), ("KOSDAQ", ".KQ"));
        assert_eq!(determine_exchange("247540"), ("KOSDAQ", ".KQ"));
        assert_eq!(determine_exchange("373220"), ("KOSDAQ", ".KQ"));

        // 5~9로 시작 → KOSDAQ 처리
        assert_eq!(determine_exchange("550750"), ("KOSDAQ", ".KQ"));

        // 빈 문자열 → 기본값 KOSPI
        assert_eq!(determine_exchange(""), ("KOSPI", ".KS"));
    }
}
