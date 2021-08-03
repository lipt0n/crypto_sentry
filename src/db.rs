use chrono::{NaiveDate, NaiveDateTime};
use sqlx::postgres::PgQueryResult;
use sqlx::{pool::Pool, postgres::PgPoolOptions, Postgres};
use std::time::Duration;

use bigdecimal::BigDecimal;
use bigdecimal::FromPrimitive;
use bigdecimal::ToPrimitive;
use chrono::prelude::*;
use dotenv::dotenv;
use std::env;
use std::error::Error;

pub struct V {
    pub v: Option<BigDecimal>,
}
#[derive(Clone)]
pub struct Pair {
    pub id: i32,
    pub base: String,
    pub quote: String,
    pub name: String,
    pub enabled: bool,
}

pub async fn establish_connection() -> Result<Pool<Postgres>, sqlx::Error> {
    dotenv().ok();

    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect_timeout(Duration::from_secs(60))
        .connect(database_url.as_str())
        .await?;
    Ok(pool)
}

pub async fn get_pairs(pool: &Pool<Postgres>) -> Result<Vec<Pair>, sqlx::Error> {
    let result = sqlx::query_as!(
        Pair,
        "
        SELECT *
        FROM pairs
        WHERE enabled = $1
    ",
        true
    )
    .fetch_all(pool)
    .await?;
    Ok(result)
}

pub async fn get_pair(
    pool: &Pool<Postgres>,
    base_asset: &str,
    quote_asset: &str,
) -> Result<Pair, sqlx::Error> {
    let result = sqlx::query_as!(
        Pair,
        "
        SELECT *
        FROM pairs
        WHERE   base = $1
        AND     quote = $2
        LIMIT 1
    ",
        base_asset,
        quote_asset
    )
    .fetch_one(pool)
    .await?;
    Ok(result)
}
pub async fn get_pair_by_name(pool: &Pool<Postgres>, pair_name: &str) -> Result<Pair, sqlx::Error> {
    let result = sqlx::query_as!(
        Pair,
        "
        SELECT *
        FROM pairs
        WHERE   name = $1
        LIMIT 1
    ",
        pair_name
    )
    .fetch_one(pool)
    .await?;
    Ok(result)
}

pub async fn add_pair(
    pool: &Pool<Postgres>,
    base_asset: &str,
    quote_asset: &str,
    pair_name: &str,
) -> Result<PgQueryResult, sqlx::Error> {
    let result: PgQueryResult = sqlx::query!(
        "
    INSERT INTO pairs
    (base, quote, name)
    VALUES
    ($1,$2,$3)
    ON CONFLICT DO NOTHING
    ",
        base_asset,
        quote_asset,
        pair_name
    )
    .execute(pool)
    .await?;
    Ok(result)
}

pub async fn add_tick(
    pool: &Pool<Postgres>,
    pair_id: i32,
    time: i64,
    open: f64,
    close: f64,
    high: f64,
    low: f64,
    volume: f64,
    orders: i32,
) -> Result<PgQueryResult, sqlx::Error> {
    let duration = Duration::from_millis(time as u64);

    let result = sqlx::query!(
        "
        INSERT INTO ticks 
        (pair_id, timestamp, open, close, high, low, volume, orders)
        VALUES
        ($1,$2,$3,$4,$5,$6,$7,$8)
        ON CONFLICT DO NOTHING
        ",
        pair_id,
        NaiveDateTime::from_timestamp(duration.as_secs() as i64, duration.subsec_nanos()),
        BigDecimal::from_f64(open),
        BigDecimal::from_f64(close),
        BigDecimal::from_f64(high),
        BigDecimal::from_f64(low),
        BigDecimal::from_f64(volume),
        orders
    )
    .execute(pool)
    .await?;
    Ok(result)
}
pub async fn get_pair_avg_volume(
    pool: &Pool<Postgres>,
    pair_id: i32,
) -> Result<f64, Box<dyn Error>> {
    let local: DateTime<Local> = Local::now();
    let _50_days_in_s = 4_320_000;
    let timestamp = local.timestamp() - _50_days_in_s;
    let result = sqlx::query_as!(
        V,
        "
        SELECT AVG(volume) as v
        FROM ticks
        WHERE   pair_id = $1
        AND     timestamp > $2
    ",
        pair_id,
        NaiveDateTime::from_timestamp(timestamp, 0)
    )
    .fetch_one(pool)
    .await?;
    let r = match result.v {
        Some(v) => v.to_f64().unwrap(),
        None => -1.,
    };
  //  info!("checking avg for {} from {:?} till now. avg: {} ", pair_id, NaiveDateTime::from_timestamp(timestamp, 0), r);

    Ok(r)
}
