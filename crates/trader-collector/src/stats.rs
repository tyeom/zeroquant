//! 수집 통계 구조체.

use serde::{Deserialize, Serialize};
use std::time::Duration;

/// 수집 작업 통계
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CollectionStats {
    /// 총 시도 횟수
    pub total: usize,
    /// 성공 횟수
    pub success: usize,
    /// 에러 횟수
    pub errors: usize,
    /// 건너뛴 횟수 (이미 최신 데이터)
    pub skipped: usize,
    /// 빈 데이터 (조회 성공, 데이터 없음)
    pub empty: usize,
    /// 저장된 총 캔들 수
    pub total_klines: usize,
    /// 소요 시간
    #[serde(skip)]
    pub elapsed: Duration,
}

impl CollectionStats {
    /// 새 통계 객체 생성
    pub fn new() -> Self {
        Self::default()
    }

    /// 성공률 계산 (%)
    pub fn success_rate(&self) -> f64 {
        if self.total == 0 {
            0.0
        } else {
            (self.success as f64 / self.total as f64) * 100.0
        }
    }

    /// 통계 요약 로그 출력
    pub fn log_summary(&self, operation: &str) {
        tracing::info!(
            operation = operation,
            total = self.total,
            success = self.success,
            errors = self.errors,
            skipped = self.skipped,
            empty = self.empty,
            total_klines = self.total_klines,
            success_rate = format!("{:.1}%", self.success_rate()),
            elapsed = format!("{:.1}s", self.elapsed.as_secs_f64()),
            "수집 완료"
        );
    }
}
