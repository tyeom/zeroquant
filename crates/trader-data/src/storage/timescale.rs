//! TimescaleDB 스토리지 구현.
//!
//! TimescaleDB(PostgreSQL + TimescaleDB 확장)를 사용하여 시계열 데이터를 저장하고
//! 조회하기 위한 repository 패턴 구현을 제공합니다.

use crate::error::{DataError, Result};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::Deserialize;
use sqlx::postgres::{PgPool, PgPoolOptions};
use sqlx::FromRow;
use std::time::Duration;
use trader_core::{Kline, Order, OrderStatusType, Side, Symbol, Timeframe, TradeTick};
use tracing::{debug, info, instrument};
use uuid::Uuid;

/// 데이터베이스 설정.
#[derive(Debug, Clone, Deserialize)]
pub struct DatabaseConfig {
    /// 데이터베이스 URL (postgresql://user:pass@host:port/db)
    pub url: String,
    /// 풀의 최대 연결 수
    #[serde(default = "default_max_connections")]
    pub max_connections: u32,
    /// 풀의 최소 연결 수
    #[serde(default = "default_min_connections")]
    pub min_connections: u32,
    /// 연결 타임아웃 (초)
    #[serde(default = "default_connect_timeout")]
    pub connect_timeout_secs: u64,
    /// 유휴 연결 타임아웃 (초)
    #[serde(default = "default_idle_timeout")]
    pub idle_timeout_secs: u64,
}

fn default_max_connections() -> u32 {
    10
}
fn default_min_connections() -> u32 {
    2
}
fn default_connect_timeout() -> u64 {
    30
}
fn default_idle_timeout() -> u64 {
    600
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            url: "postgresql://trader:trader@localhost:5432/trader".to_string(),
            max_connections: default_max_connections(),
            min_connections: default_min_connections(),
            connect_timeout_secs: default_connect_timeout(),
            idle_timeout_secs: default_idle_timeout(),
        }
    }
}

/// 데이터베이스 연결 풀 래퍼.
#[derive(Clone)]
pub struct Database {
    pool: PgPool,
}

impl Database {
    /// 새로운 데이터베이스 연결 풀을 생성합니다.
    pub async fn connect(config: &DatabaseConfig) -> Result<Self> {
        info!("Connecting to database...");

        let pool = PgPoolOptions::new()
            .max_connections(config.max_connections)
            .min_connections(config.min_connections)
            .acquire_timeout(Duration::from_secs(config.connect_timeout_secs))
            .idle_timeout(Duration::from_secs(config.idle_timeout_secs))
            .connect(&config.url)
            .await
            .map_err(|e| DataError::ConnectionError(e.to_string()))?;

        info!("Database connection established");

        Ok(Self { pool })
    }

    /// 기존 연결 풀에서 Database 인스턴스를 생성합니다.
    ///
    /// AppState 등에서 이미 생성된 풀을 재사용할 때 사용합니다.
    pub fn from_pool(pool: PgPool) -> Self {
        Self { pool }
    }

    /// 내부 연결 풀을 반환합니다.
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// 데이터베이스 마이그레이션을 실행합니다.
    pub async fn migrate(&self) -> Result<()> {
        info!("Running database migrations...");

        sqlx::migrate!("../../migrations")
            .run(&self.pool)
            .await
            .map_err(|e| DataError::MigrationError(e.to_string()))?;

        info!("Migrations completed successfully");
        Ok(())
    }

    /// 데이터베이스 상태를 확인합니다.
    pub async fn health_check(&self) -> Result<bool> {
        sqlx::query("SELECT 1")
            .execute(&self.pool)
            .await
            .map_err(|e| DataError::QueryError(e.to_string()))?;
        Ok(true)
    }
}

// =============================================================================
// Symbol Repository
// =============================================================================

/// 심볼 데이터베이스 레코드.
#[derive(Debug, Clone, FromRow)]
pub struct SymbolRecord {
    pub id: Uuid,
    pub exchange: String,
    pub base: String,
    pub quote: String,
    pub market_type: String,
    pub exchange_symbol: Option<String>,
    pub is_active: Option<bool>,
    pub price_precision: Option<i32>,
    pub quantity_precision: Option<i32>,
    pub min_quantity: Option<Decimal>,
    pub max_quantity: Option<Decimal>,
    pub quantity_step: Option<Decimal>,
    pub min_notional: Option<Decimal>,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
}

/// 심볼 데이터 repository.
pub struct SymbolRepository {
    db: Database,
}

impl SymbolRepository {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    /// 심볼을 조회하거나 생성하고 데이터베이스 ID를 반환합니다.
    #[instrument(skip(self))]
    pub async fn get_or_create(&self, symbol: &Symbol, exchange: &str) -> Result<Uuid> {
        // 기존 레코드 찾기 시도
        let existing: Option<(Uuid,)> = sqlx::query_as(
            r#"
            SELECT id FROM symbols
            WHERE exchange = $1 AND base = $2 AND quote = $3
            "#,
        )
        .bind(exchange)
        .bind(&symbol.base)
        .bind(&symbol.quote)
        .fetch_optional(self.db.pool())
        .await?;

        if let Some((id,)) = existing {
            return Ok(id);
        }

        // 새로 생성
        let (id,): (Uuid,) = sqlx::query_as(
            r#"
            INSERT INTO symbols (exchange, base, quote, market_type)
            VALUES ($1, $2, $3, $4::market_type)
            RETURNING id
            "#,
        )
        .bind(exchange)
        .bind(&symbol.base)
        .bind(&symbol.quote)
        .bind(symbol.market_type.to_string().to_lowercase())
        .fetch_one(self.db.pool())
        .await?;

        debug!(symbol = %symbol, id = %id, "Created new symbol record");
        Ok(id)
    }

    /// ID로 심볼을 조회합니다.
    pub async fn get_by_id(&self, id: Uuid) -> Result<SymbolRecord> {
        sqlx::query_as("SELECT * FROM symbols WHERE id = $1")
            .bind(id)
            .fetch_one(self.db.pool())
            .await
            .map_err(Into::into)
    }

    /// 거래소의 모든 활성 심볼을 조회합니다.
    pub async fn list_active(&self, exchange: &str) -> Result<Vec<SymbolRecord>> {
        sqlx::query_as(
            "SELECT * FROM symbols WHERE exchange = $1 AND is_active = true ORDER BY base, quote",
        )
        .bind(exchange)
        .fetch_all(self.db.pool())
        .await
        .map_err(Into::into)
    }

    /// 심볼 거래 규칙을 업데이트합니다.
    pub async fn update_rules(
        &self,
        id: Uuid,
        price_precision: i32,
        quantity_precision: i32,
        min_quantity: Option<Decimal>,
        max_quantity: Option<Decimal>,
        min_notional: Option<Decimal>,
    ) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE symbols SET
                price_precision = $2,
                quantity_precision = $3,
                min_quantity = $4,
                max_quantity = $5,
                min_notional = $6,
                updated_at = NOW()
            WHERE id = $1
            "#,
        )
        .bind(id)
        .bind(price_precision)
        .bind(quantity_precision)
        .bind(min_quantity)
        .bind(max_quantity)
        .bind(min_notional)
        .execute(self.db.pool())
        .await?;

        Ok(())
    }
}

// =============================================================================
// Kline Repository
// =============================================================================

/// OHLCV 캔들스틱 데이터 repository.
pub struct KlineRepository {
    db: Database,
}

impl KlineRepository {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    /// 단일 kline을 삽입합니다.
    #[instrument(skip(self, kline))]
    pub async fn insert(&self, symbol_id: Uuid, kline: &Kline) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO klines (symbol_id, timeframe, time, open, high, low, close, volume, quote_volume, num_trades)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            ON CONFLICT (symbol_id, timeframe, time) DO UPDATE SET
                high = GREATEST(klines.high, EXCLUDED.high),
                low = LEAST(klines.low, EXCLUDED.low),
                close = EXCLUDED.close,
                volume = EXCLUDED.volume,
                quote_volume = EXCLUDED.quote_volume,
                num_trades = EXCLUDED.num_trades
            "#,
        )
        .bind(symbol_id)
        .bind(kline.timeframe.to_string())
        .bind(kline.open_time)  // DB의 time 컬럼은 open_time을 의미함
        .bind(kline.open)
        .bind(kline.high)
        .bind(kline.low)
        .bind(kline.close)
        .bind(kline.volume)
        .bind(kline.quote_volume)
        .bind(kline.num_trades.map(|n| n as i32))
        .execute(self.db.pool())
        .await?;

        Ok(())
    }

    /// 여러 kline을 일괄 삽입합니다.
    #[instrument(skip(self, klines), fields(count = klines.len()))]
    pub async fn insert_batch(&self, symbol_id: Uuid, klines: &[Kline]) -> Result<usize> {
        if klines.is_empty() {
            return Ok(0);
        }

        let mut inserted = 0;

        // 성능 향상을 위해 청크 단위 삽입 사용
        for chunk in klines.chunks(1000) {
            let mut query_builder = String::from(
                r#"
                INSERT INTO klines (symbol_id, timeframe, time, open, high, low, close, volume, quote_volume, num_trades)
                VALUES
                "#,
            );

            for (i, _kline) in chunk.iter().enumerate() {
                if i > 0 {
                    query_builder.push_str(", ");
                }
                let base = i * 10;
                query_builder.push_str(&format!(
                    "(${}, ${}, ${}, ${}, ${}, ${}, ${}, ${}, ${}, ${})",
                    base + 1,
                    base + 2,
                    base + 3,
                    base + 4,
                    base + 5,
                    base + 6,
                    base + 7,
                    base + 8,
                    base + 9,
                    base + 10
                ));
            }

            query_builder.push_str(
                r#"
                ON CONFLICT (symbol_id, timeframe, time) DO UPDATE SET
                    high = GREATEST(klines.high, EXCLUDED.high),
                    low = LEAST(klines.low, EXCLUDED.low),
                    close = EXCLUDED.close,
                    volume = EXCLUDED.volume,
                    quote_volume = EXCLUDED.quote_volume,
                    num_trades = EXCLUDED.num_trades
                "#,
            );

            let mut query = sqlx::query(&query_builder);

            for kline in chunk {
                query = query
                    .bind(symbol_id)
                    .bind(kline.timeframe.to_string())
                    .bind(kline.open_time)  // DB의 time 컬럼
                    .bind(kline.open)
                    .bind(kline.high)
                    .bind(kline.low)
                    .bind(kline.close)
                    .bind(kline.volume)
                    .bind(kline.quote_volume)
                    .bind(kline.num_trades.map(|n| n as i32));
            }

            let result = query.execute(self.db.pool()).await?;
            inserted += result.rows_affected() as usize;
        }

        debug!(inserted = inserted, "Inserted klines");
        Ok(inserted)
    }

    /// 심볼과 타임프레임에 대해 시간 범위 내의 kline을 조회합니다.
    #[instrument(skip(self))]
    pub async fn get_range(
        &self,
        symbol_id: Uuid,
        timeframe: Timeframe,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        limit: Option<i32>,
    ) -> Result<Vec<KlineRecord>> {
        let limit = limit.unwrap_or(1000);

        let klines: Vec<KlineRecord> = sqlx::query_as(
            r#"
            SELECT symbol_id, timeframe, time, open, high, low, close, volume, quote_volume, num_trades FROM klines
            WHERE symbol_id = $1 AND timeframe = $2 AND time >= $3 AND time < $4
            ORDER BY time ASC
            LIMIT $5
            "#,
        )
        .bind(symbol_id)
        .bind(timeframe.to_string())
        .bind(start)
        .bind(end)
        .bind(limit)
        .fetch_all(self.db.pool())
        .await?;

        Ok(klines)
    }

    /// 가장 최근의 kline을 조회합니다.
    pub async fn get_latest(
        &self,
        symbol_id: Uuid,
        timeframe: Timeframe,
        count: i32,
    ) -> Result<Vec<KlineRecord>> {
        let klines: Vec<KlineRecord> = sqlx::query_as(
            r#"
            SELECT symbol_id, timeframe, time, open, high, low, close, volume, quote_volume, num_trades FROM klines
            WHERE symbol_id = $1 AND timeframe = $2
            ORDER BY time DESC
            LIMIT $3
            "#,
        )
        .bind(symbol_id)
        .bind(timeframe.to_string())
        .bind(count)
        .fetch_all(self.db.pool())
        .await?;

        // 오름차순으로 정렬하기 위해 역순 처리
        let mut klines = klines;
        klines.reverse();
        Ok(klines)
    }

    /// 심볼과 타임프레임에 대한 마지막 kline을 조회합니다.
    pub async fn get_last(
        &self,
        symbol_id: Uuid,
        timeframe: Timeframe,
    ) -> Result<Option<KlineRecord>> {
        sqlx::query_as(
            r#"
            SELECT symbol_id, timeframe, time, open, high, low, close, volume, quote_volume, num_trades FROM klines
            WHERE symbol_id = $1 AND timeframe = $2
            ORDER BY time DESC
            LIMIT 1
            "#,
        )
        .bind(symbol_id)
        .bind(timeframe.to_string())
        .fetch_optional(self.db.pool())
        .await
        .map_err(Into::into)
    }

    /// 오래된 kline을 삭제합니다 (데이터 보존 정책용).
    pub async fn delete_older_than(
        &self,
        symbol_id: Uuid,
        timeframe: Timeframe,
        before: DateTime<Utc>,
    ) -> Result<u64> {
        let result = sqlx::query(
            "DELETE FROM klines WHERE symbol_id = $1 AND timeframe = $2 AND time < $3",
        )
        .bind(symbol_id)
        .bind(timeframe.to_string())
        .bind(before)
        .execute(self.db.pool())
        .await?;

        Ok(result.rows_affected())
    }
}

/// Kline 데이터베이스 레코드.
#[derive(Debug, Clone, FromRow)]
pub struct KlineRecord {
    pub symbol_id: Uuid,
    pub timeframe: String,
    pub time: DateTime<Utc>,
    pub open: Decimal,
    pub high: Decimal,
    pub low: Decimal,
    pub close: Decimal,
    pub volume: Decimal,
    pub quote_volume: Option<Decimal>,
    pub num_trades: Option<i32>,
}

impl KlineRecord {
    /// 도메인 Kline으로 변환합니다 (심볼 정보 필요).
    pub fn to_kline(&self, symbol: Symbol) -> Kline {
        let timeframe = self.timeframe.parse().unwrap_or(Timeframe::D1);
        // close_time은 open_time(DB의 time 컬럼) + timeframe 기간으로 계산
        let close_time = match timeframe {
            Timeframe::M1 => self.time + chrono::Duration::minutes(1) - chrono::Duration::seconds(1),
            Timeframe::M3 => self.time + chrono::Duration::minutes(3) - chrono::Duration::seconds(1),
            Timeframe::M5 => self.time + chrono::Duration::minutes(5) - chrono::Duration::seconds(1),
            Timeframe::M15 => self.time + chrono::Duration::minutes(15) - chrono::Duration::seconds(1),
            Timeframe::M30 => self.time + chrono::Duration::minutes(30) - chrono::Duration::seconds(1),
            Timeframe::H1 => self.time + chrono::Duration::hours(1) - chrono::Duration::seconds(1),
            Timeframe::H2 => self.time + chrono::Duration::hours(2) - chrono::Duration::seconds(1),
            Timeframe::H4 => self.time + chrono::Duration::hours(4) - chrono::Duration::seconds(1),
            Timeframe::H6 => self.time + chrono::Duration::hours(6) - chrono::Duration::seconds(1),
            Timeframe::H8 => self.time + chrono::Duration::hours(8) - chrono::Duration::seconds(1),
            Timeframe::H12 => self.time + chrono::Duration::hours(12) - chrono::Duration::seconds(1),
            Timeframe::D1 => self.time + chrono::Duration::days(1) - chrono::Duration::seconds(1),
            Timeframe::D3 => self.time + chrono::Duration::days(3) - chrono::Duration::seconds(1),
            Timeframe::W1 => self.time + chrono::Duration::weeks(1) - chrono::Duration::seconds(1),
            Timeframe::MN1 => self.time + chrono::Duration::days(30) - chrono::Duration::seconds(1),
        };

        Kline {
            symbol,
            timeframe,
            open_time: self.time,
            open: self.open,
            high: self.high,
            low: self.low,
            close: self.close,
            volume: self.volume,
            close_time,
            quote_volume: self.quote_volume,
            num_trades: self.num_trades.map(|n| n as u32),
        }
    }
}

// =============================================================================
// Trade Tick Repository
// =============================================================================

/// 체결 틱 데이터 repository.
pub struct TradeTickRepository {
    db: Database,
}

impl TradeTickRepository {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    /// 체결 틱을 삽입합니다.
    pub async fn insert(&self, symbol_id: Uuid, trade: &TradeTick) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO trade_ticks (symbol_id, trade_id, price, quantity, side, timestamp)
            VALUES ($1, $2, $3, $4, $5, $6)
            ON CONFLICT (symbol_id, trade_id) DO NOTHING
            "#,
        )
        .bind(symbol_id)
        .bind(&trade.id)
        .bind(trade.price)
        .bind(trade.quantity)
        .bind(trade.side.to_string())
        .bind(trade.timestamp)
        .execute(self.db.pool())
        .await?;

        Ok(())
    }

    /// 여러 체결 틱을 일괄 삽입합니다.
    pub async fn insert_batch(&self, symbol_id: Uuid, trades: &[TradeTick]) -> Result<usize> {
        if trades.is_empty() {
            return Ok(0);
        }

        let mut inserted = 0;

        for chunk in trades.chunks(1000) {
            let mut tx = self.db.pool().begin().await?;

            for trade in chunk {
                let result = sqlx::query(
                    r#"
                    INSERT INTO trade_ticks (symbol_id, trade_id, price, quantity, side, timestamp)
                    VALUES ($1, $2, $3, $4, $5, $6)
                    ON CONFLICT (symbol_id, trade_id) DO NOTHING
                    "#,
                )
                .bind(symbol_id)
                .bind(&trade.id)
                .bind(trade.price)
                .bind(trade.quantity)
                .bind(trade.side.to_string())
                .bind(trade.timestamp)
                .execute(&mut *tx)
                .await?;

                inserted += result.rows_affected() as usize;
            }

            tx.commit().await?;
        }

        Ok(inserted)
    }

    /// 시간 범위 내의 체결 틱을 조회합니다.
    pub async fn get_range(
        &self,
        symbol_id: Uuid,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        limit: Option<i32>,
    ) -> Result<Vec<TradeTickRecord>> {
        let limit = limit.unwrap_or(10000);

        sqlx::query_as(
            r#"
            SELECT * FROM trade_ticks
            WHERE symbol_id = $1 AND timestamp >= $2 AND timestamp < $3
            ORDER BY timestamp ASC
            LIMIT $4
            "#,
        )
        .bind(symbol_id)
        .bind(start)
        .bind(end)
        .bind(limit)
        .fetch_all(self.db.pool())
        .await
        .map_err(Into::into)
    }

    /// 최근 체결 틱을 조회합니다.
    pub async fn get_recent(&self, symbol_id: Uuid, count: i32) -> Result<Vec<TradeTickRecord>> {
        sqlx::query_as(
            r#"
            SELECT * FROM trade_ticks
            WHERE symbol_id = $1
            ORDER BY timestamp DESC
            LIMIT $2
            "#,
        )
        .bind(symbol_id)
        .bind(count)
        .fetch_all(self.db.pool())
        .await
        .map_err(Into::into)
    }
}

/// 체결 틱 데이터베이스 레코드.
#[derive(Debug, Clone, FromRow)]
pub struct TradeTickRecord {
    pub symbol_id: Uuid,
    pub trade_id: String,
    pub price: Decimal,
    pub quantity: Decimal,
    pub side: String,
    pub timestamp: DateTime<Utc>,
}

// =============================================================================
// Order Repository
// =============================================================================

/// 주문 데이터 repository.
pub struct OrderRepository {
    db: Database,
}

impl OrderRepository {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    /// 새 주문을 삽입합니다.
    #[instrument(skip(self, order))]
    pub async fn insert(&self, order: &Order) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO orders (
                id, exchange, exchange_order_id, symbol, side, order_type,
                quantity, price, stop_price, status, filled_quantity,
                average_fill_price, time_in_force, strategy_id, client_order_id,
                created_at, updated_at, metadata
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18)
            "#,
        )
        .bind(order.id)
        .bind(&order.exchange)
        .bind(&order.exchange_order_id)
        .bind(order.symbol.to_string())
        .bind(order.side.to_string())
        .bind(order.order_type.to_string())
        .bind(order.quantity)
        .bind(order.price)
        .bind(order.stop_price)
        .bind(format!("{:?}", order.status).to_lowercase())
        .bind(order.filled_quantity)
        .bind(order.average_fill_price)
        .bind(format!("{:?}", order.time_in_force))
        .bind(&order.strategy_id)
        .bind(&order.client_order_id)
        .bind(order.created_at)
        .bind(order.updated_at)
        .bind(&order.metadata)
        .execute(self.db.pool())
        .await?;

        debug!(order_id = %order.id, "Order inserted");
        Ok(())
    }

    /// 주문 상태를 업데이트합니다.
    #[instrument(skip(self))]
    pub async fn update_status(
        &self,
        order_id: Uuid,
        status: OrderStatusType,
        filled_quantity: Decimal,
        average_fill_price: Option<Decimal>,
    ) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE orders SET
                status = $2,
                filled_quantity = $3,
                average_fill_price = $4,
                updated_at = NOW()
            WHERE id = $1
            "#,
        )
        .bind(order_id)
        .bind(format!("{:?}", status).to_lowercase())
        .bind(filled_quantity)
        .bind(average_fill_price)
        .execute(self.db.pool())
        .await?;

        debug!(order_id = %order_id, status = ?status, "Order status updated");
        Ok(())
    }

    /// 주문 접수 후 거래소 주문 ID를 설정합니다.
    pub async fn set_exchange_order_id(
        &self,
        order_id: Uuid,
        exchange_order_id: &str,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE orders SET exchange_order_id = $2, updated_at = NOW() WHERE id = $1",
        )
        .bind(order_id)
        .bind(exchange_order_id)
        .execute(self.db.pool())
        .await?;

        Ok(())
    }

    /// ID로 주문을 조회합니다.
    pub async fn get_by_id(&self, order_id: Uuid) -> Result<OrderRecord> {
        sqlx::query_as("SELECT * FROM orders WHERE id = $1")
            .bind(order_id)
            .fetch_one(self.db.pool())
            .await
            .map_err(Into::into)
    }

    /// 거래소 주문 ID로 주문을 조회합니다.
    pub async fn get_by_exchange_id(
        &self,
        exchange: &str,
        exchange_order_id: &str,
    ) -> Result<Option<OrderRecord>> {
        sqlx::query_as("SELECT * FROM orders WHERE exchange = $1 AND exchange_order_id = $2")
            .bind(exchange)
            .bind(exchange_order_id)
            .fetch_optional(self.db.pool())
            .await
            .map_err(Into::into)
    }

    /// 전략별 주문을 조회합니다.
    pub async fn get_by_strategy(&self, strategy_id: &str) -> Result<Vec<OrderRecord>> {
        sqlx::query_as(
            "SELECT * FROM orders WHERE strategy_id = $1 ORDER BY created_at DESC",
        )
        .bind(strategy_id)
        .fetch_all(self.db.pool())
        .await
        .map_err(Into::into)
    }

    /// 미체결 주문을 조회합니다.
    pub async fn get_open_orders(&self, exchange: Option<&str>) -> Result<Vec<OrderRecord>> {
        if let Some(exchange) = exchange {
            sqlx::query_as(
                r#"
                SELECT * FROM orders
                WHERE exchange = $1 AND status IN ('pending', 'open', 'partially_filled')
                ORDER BY created_at DESC
                "#,
            )
            .bind(exchange)
            .fetch_all(self.db.pool())
            .await
            .map_err(Into::into)
        } else {
            sqlx::query_as(
                r#"
                SELECT * FROM orders
                WHERE status IN ('pending', 'open', 'partially_filled')
                ORDER BY created_at DESC
                "#,
            )
            .fetch_all(self.db.pool())
            .await
            .map_err(Into::into)
        }
    }

    /// 최근 주문을 조회합니다.
    pub async fn get_recent(
        &self,
        exchange: Option<&str>,
        limit: i32,
    ) -> Result<Vec<OrderRecord>> {
        if let Some(exchange) = exchange {
            sqlx::query_as(
                "SELECT * FROM orders WHERE exchange = $1 ORDER BY created_at DESC LIMIT $2",
            )
            .bind(exchange)
            .bind(limit)
            .fetch_all(self.db.pool())
            .await
            .map_err(Into::into)
        } else {
            sqlx::query_as("SELECT * FROM orders ORDER BY created_at DESC LIMIT $1")
                .bind(limit)
                .fetch_all(self.db.pool())
                .await
                .map_err(Into::into)
        }
    }
}

/// 주문 데이터베이스 레코드.
#[derive(Debug, Clone, FromRow)]
pub struct OrderRecord {
    pub id: Uuid,
    pub exchange: String,
    pub exchange_order_id: Option<String>,
    pub symbol: String,
    pub side: String,
    pub order_type: String,
    pub quantity: Decimal,
    pub price: Option<Decimal>,
    pub stop_price: Option<Decimal>,
    pub status: String,
    pub filled_quantity: Decimal,
    pub average_fill_price: Option<Decimal>,
    pub time_in_force: String,
    pub strategy_id: Option<String>,
    pub client_order_id: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub metadata: Option<serde_json::Value>,
}

// =============================================================================
// Trade Repository (Executed trades)
// =============================================================================

/// 체결된 거래 데이터 repository.
pub struct TradeRepository {
    db: Database,
}

impl TradeRepository {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    /// 새 거래를 삽입합니다.
    pub async fn insert(
        &self,
        order_id: Uuid,
        exchange_trade_id: &str,
        price: Decimal,
        quantity: Decimal,
        commission: Decimal,
        commission_asset: &str,
        executed_at: DateTime<Utc>,
    ) -> Result<Uuid> {
        let id = Uuid::new_v4();

        sqlx::query(
            r#"
            INSERT INTO trades (id, order_id, exchange_trade_id, price, quantity, commission, commission_asset, executed_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            "#,
        )
        .bind(id)
        .bind(order_id)
        .bind(exchange_trade_id)
        .bind(price)
        .bind(quantity)
        .bind(commission)
        .bind(commission_asset)
        .bind(executed_at)
        .execute(self.db.pool())
        .await?;

        Ok(id)
    }

    /// 주문에 대한 거래를 조회합니다.
    pub async fn get_by_order(&self, order_id: Uuid) -> Result<Vec<TradeRecord>> {
        sqlx::query_as("SELECT * FROM trades WHERE order_id = $1 ORDER BY executed_at ASC")
            .bind(order_id)
            .fetch_all(self.db.pool())
            .await
            .map_err(Into::into)
    }

    /// 시간 범위 내의 거래를 조회합니다.
    pub async fn get_range(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<TradeRecord>> {
        sqlx::query_as(
            "SELECT * FROM trades WHERE executed_at >= $1 AND executed_at < $2 ORDER BY executed_at ASC",
        )
        .bind(start)
        .bind(end)
        .fetch_all(self.db.pool())
        .await
        .map_err(Into::into)
    }

    /// 최근 거래를 조회합니다.
    pub async fn get_recent(&self, limit: i32) -> Result<Vec<TradeRecord>> {
        sqlx::query_as("SELECT * FROM trades ORDER BY executed_at DESC LIMIT $1")
            .bind(limit)
            .fetch_all(self.db.pool())
            .await
            .map_err(Into::into)
    }

    /// 기간별 총 수수료를 계산합니다.
    pub async fn total_commission(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Decimal> {
        let result: (Decimal,) = sqlx::query_as(
            "SELECT COALESCE(SUM(commission), 0) FROM trades WHERE executed_at >= $1 AND executed_at < $2",
        )
        .bind(start)
        .bind(end)
        .fetch_one(self.db.pool())
        .await?;

        Ok(result.0)
    }
}

/// 거래 데이터베이스 레코드.
#[derive(Debug, Clone, FromRow)]
pub struct TradeRecord {
    pub id: Uuid,
    pub order_id: Uuid,
    pub exchange_trade_id: String,
    pub price: Decimal,
    pub quantity: Decimal,
    pub commission: Decimal,
    pub commission_asset: String,
    pub executed_at: DateTime<Utc>,
}

// =============================================================================
// Position Repository
// =============================================================================

/// 포지션 데이터 repository.
pub struct PositionRepository {
    db: Database,
}

impl PositionRepository {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    /// 포지션을 삽입하거나 업데이트합니다.
    pub async fn upsert(
        &self,
        exchange: &str,
        symbol: &str,
        strategy_id: Option<&str>,
        side: Side,
        quantity: Decimal,
        entry_price: Decimal,
        unrealized_pnl: Decimal,
        realized_pnl: Decimal,
    ) -> Result<Uuid> {
        let id = Uuid::new_v4();

        sqlx::query(
            r#"
            INSERT INTO positions (id, exchange, symbol, strategy_id, side, quantity, entry_price, unrealized_pnl, realized_pnl, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, NOW())
            ON CONFLICT (exchange, symbol, strategy_id) DO UPDATE SET
                side = EXCLUDED.side,
                quantity = EXCLUDED.quantity,
                entry_price = EXCLUDED.entry_price,
                unrealized_pnl = EXCLUDED.unrealized_pnl,
                realized_pnl = EXCLUDED.realized_pnl,
                updated_at = NOW()
            RETURNING id
            "#,
        )
        .bind(id)
        .bind(exchange)
        .bind(symbol)
        .bind(strategy_id)
        .bind(side.to_string())
        .bind(quantity)
        .bind(entry_price)
        .bind(unrealized_pnl)
        .bind(realized_pnl)
        .fetch_one(self.db.pool())
        .await?;

        Ok(id)
    }

    /// 거래소와 심볼로 포지션을 조회합니다.
    pub async fn get(
        &self,
        exchange: &str,
        symbol: &str,
        strategy_id: Option<&str>,
    ) -> Result<Option<PositionRecord>> {
        if let Some(strategy_id) = strategy_id {
            sqlx::query_as(
                "SELECT * FROM positions WHERE exchange = $1 AND symbol = $2 AND strategy_id = $3",
            )
            .bind(exchange)
            .bind(symbol)
            .bind(strategy_id)
            .fetch_optional(self.db.pool())
            .await
            .map_err(Into::into)
        } else {
            sqlx::query_as(
                "SELECT * FROM positions WHERE exchange = $1 AND symbol = $2 AND strategy_id IS NULL",
            )
            .bind(exchange)
            .bind(symbol)
            .fetch_optional(self.db.pool())
            .await
            .map_err(Into::into)
        }
    }

    /// 모든 열린 포지션을 조회합니다.
    pub async fn get_all_open(&self) -> Result<Vec<PositionRecord>> {
        sqlx::query_as("SELECT * FROM positions WHERE quantity > 0 ORDER BY updated_at DESC")
            .fetch_all(self.db.pool())
            .await
            .map_err(Into::into)
    }

    /// 전략별 포지션을 조회합니다.
    pub async fn get_by_strategy(&self, strategy_id: &str) -> Result<Vec<PositionRecord>> {
        sqlx::query_as("SELECT * FROM positions WHERE strategy_id = $1 ORDER BY updated_at DESC")
            .bind(strategy_id)
            .fetch_all(self.db.pool())
            .await
            .map_err(Into::into)
    }

    /// 포지션을 청산합니다 (수량을 0으로 설정).
    pub async fn close(
        &self,
        exchange: &str,
        symbol: &str,
        strategy_id: Option<&str>,
        realized_pnl: Decimal,
    ) -> Result<()> {
        if let Some(strategy_id) = strategy_id {
            sqlx::query(
                r#"
                UPDATE positions SET quantity = 0, realized_pnl = $4, unrealized_pnl = 0, updated_at = NOW()
                WHERE exchange = $1 AND symbol = $2 AND strategy_id = $3
                "#,
            )
            .bind(exchange)
            .bind(symbol)
            .bind(strategy_id)
            .bind(realized_pnl)
            .execute(self.db.pool())
            .await?;
        } else {
            sqlx::query(
                r#"
                UPDATE positions SET quantity = 0, realized_pnl = $3, unrealized_pnl = 0, updated_at = NOW()
                WHERE exchange = $1 AND symbol = $2 AND strategy_id IS NULL
                "#,
            )
            .bind(exchange)
            .bind(symbol)
            .bind(realized_pnl)
            .execute(self.db.pool())
            .await?;
        }

        Ok(())
    }
}

/// 포지션 데이터베이스 레코드.
#[derive(Debug, Clone, FromRow)]
pub struct PositionRecord {
    pub id: Uuid,
    pub exchange: String,
    pub symbol: String,
    pub strategy_id: Option<String>,
    pub side: String,
    pub quantity: Decimal,
    pub entry_price: Decimal,
    pub unrealized_pnl: Decimal,
    pub realized_pnl: Decimal,
    pub updated_at: DateTime<Utc>,
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = DatabaseConfig::default();
        assert_eq!(config.max_connections, 10);
        assert_eq!(config.min_connections, 2);
    }
}
