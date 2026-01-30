//! Strategy persistence repository.
//!
//! Handles database operations for storing and retrieving trading strategies.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::PgPool;

/// Database representation of a strategy.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct StrategyRecord {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub strategy_type: Option<String>,
    pub symbols: Option<Value>,
    pub market: Option<String>,
    pub timeframe: Option<String>,
    pub version: Option<String>,
    pub is_active: bool,
    pub config: Value,
    pub risk_limits: Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub last_started_at: Option<DateTime<Utc>>,
    pub last_stopped_at: Option<DateTime<Utc>>,
}

/// Input for creating a new strategy.
#[derive(Debug)]
pub struct CreateStrategyInput {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub strategy_type: String,
    pub symbols: Vec<String>,
    pub market: String,
    pub timeframe: String,
    pub config: Value,
}

/// Strategy repository for database operations.
pub struct StrategyRepository;

impl StrategyRepository {
    /// Save a new strategy to the database.
    pub async fn create(pool: &PgPool, input: CreateStrategyInput) -> Result<StrategyRecord, sqlx::Error> {
        let symbols_json = serde_json::to_value(&input.symbols).unwrap_or(Value::Array(vec![]));

        let record = sqlx::query_as::<_, StrategyRecord>(
            r#"
            INSERT INTO strategies (id, name, description, strategy_type, symbols, market, timeframe, config, is_active)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, false)
            RETURNING *
            "#
        )
        .bind(&input.id)
        .bind(&input.name)
        .bind(&input.description)
        .bind(&input.strategy_type)
        .bind(&symbols_json)
        .bind(&input.market)
        .bind(&input.timeframe)
        .bind(&input.config)
        .fetch_one(pool)
        .await?;

        Ok(record)
    }

    /// Get a strategy by ID.
    pub async fn get_by_id(pool: &PgPool, id: &str) -> Result<Option<StrategyRecord>, sqlx::Error> {
        let record = sqlx::query_as::<_, StrategyRecord>(
            "SELECT * FROM strategies WHERE id = $1"
        )
        .bind(id)
        .fetch_optional(pool)
        .await?;

        Ok(record)
    }

    /// Get all strategies.
    pub async fn get_all(pool: &PgPool) -> Result<Vec<StrategyRecord>, sqlx::Error> {
        let records = sqlx::query_as::<_, StrategyRecord>(
            "SELECT * FROM strategies ORDER BY created_at DESC"
        )
        .fetch_all(pool)
        .await?;

        Ok(records)
    }

    /// Update strategy configuration.
    pub async fn update_config(
        pool: &PgPool,
        id: &str,
        config: Value,
    ) -> Result<StrategyRecord, sqlx::Error> {
        let record = sqlx::query_as::<_, StrategyRecord>(
            r#"
            UPDATE strategies
            SET config = $2, updated_at = NOW()
            WHERE id = $1
            RETURNING *
            "#
        )
        .bind(id)
        .bind(config)
        .fetch_one(pool)
        .await?;

        Ok(record)
    }

    /// Update strategy active status.
    pub async fn set_active(
        pool: &PgPool,
        id: &str,
        is_active: bool,
    ) -> Result<(), sqlx::Error> {
        let timestamp_field = if is_active {
            "last_started_at"
        } else {
            "last_stopped_at"
        };

        sqlx::query(&format!(
            r#"
            UPDATE strategies
            SET is_active = $2, {} = NOW(), updated_at = NOW()
            WHERE id = $1
            "#,
            timestamp_field
        ))
        .bind(id)
        .bind(is_active)
        .execute(pool)
        .await?;

        Ok(())
    }

    /// Delete a strategy by ID.
    pub async fn delete(pool: &PgPool, id: &str) -> Result<bool, sqlx::Error> {
        let result = sqlx::query("DELETE FROM strategies WHERE id = $1")
            .bind(id)
            .execute(pool)
            .await?;

        Ok(result.rows_affected() > 0)
    }

    /// Check if a strategy exists by ID.
    pub async fn exists(pool: &PgPool, id: &str) -> Result<bool, sqlx::Error> {
        let result: (bool,) = sqlx::query_as(
            "SELECT EXISTS(SELECT 1 FROM strategies WHERE id = $1)"
        )
        .bind(id)
        .fetch_one(pool)
        .await?;

        Ok(result.0)
    }

    /// Load all strategies from the database and register them with the engine.
    ///
    /// This is called during server startup to restore previously saved strategies.
    pub async fn load_strategies_into_engine(
        pool: &PgPool,
        engine: &trader_strategy::StrategyEngine,
    ) -> Result<usize, Box<dyn std::error::Error + Send + Sync>> {
        use trader_strategy::strategies::{
            BollingerStrategy, GridStrategy, HaaStrategy, MagicSplitStrategy,
            RsiStrategy, SimplePowerStrategy, SmaStrategy, StockRotationStrategy,
            VolatilityBreakoutStrategy, XaaStrategy,
        };
        use trader_strategy::Strategy;

        let records = Self::get_all(pool).await?;
        let mut loaded_count = 0;

        for record in records {
            let strategy_type = record.strategy_type.as_deref().unwrap_or("unknown");

            // Create strategy instance based on type
            let strategy: Option<Box<dyn Strategy>> = match strategy_type {
                "rsi" | "rsi_mean_reversion" => Some(Box::new(RsiStrategy::new())),
                "grid" | "grid_trading" => Some(Box::new(GridStrategy::new())),
                "bollinger" | "bollinger_bands" => Some(Box::new(BollingerStrategy::new())),
                "volatility_breakout" | "volatility" => Some(Box::new(VolatilityBreakoutStrategy::new())),
                "magic_split" | "split" => Some(Box::new(MagicSplitStrategy::new())),
                "simple_power" => Some(Box::new(SimplePowerStrategy::new())),
                "haa" => Some(Box::new(HaaStrategy::new())),
                "xaa" => Some(Box::new(XaaStrategy::new())),
                "sma" | "sma_crossover" | "ma_crossover" => Some(Box::new(SmaStrategy::new())),
                "stock_rotation" => Some(Box::new(StockRotationStrategy::new())),
                _ => {
                    tracing::warn!("Unknown strategy type: {} for strategy {}", strategy_type, record.id);
                    None
                }
            };

            if let Some(strategy) = strategy {
                match engine
                    .register_strategy(
                        &record.id,
                        strategy,
                        record.config.clone(),
                        Some(record.name.clone()),
                    )
                    .await
                {
                    Ok(_) => {
                        tracing::info!("Loaded strategy from DB: {} ({})", record.name, record.id);
                        loaded_count += 1;
                    }
                    Err(e) => {
                        tracing::error!("Failed to load strategy {}: {:?}", record.id, e);
                    }
                }
            }
        }

        Ok(loaded_count)
    }
}
