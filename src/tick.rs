use chrono::{NaiveDate, NaiveDateTime};
use sqlx::{pool::Pool, postgres::PgPoolOptions, Postgres};

use std::error::Error;

use crate::db;
use crate::exchange;
use crate::helpers;
use binance::api::*;
use binance::market::*;
use binance::model::{KlineSummaries, KlineSummary, Symbol};

pub async fn main(pool: Pool<Postgres>, pair: db::Pair, candle: KlineSummary) {
    let avg_volume: f64 = match db::get_pair_avg_volume(&pool, pair.id).await {
        Ok(v) => {
            if v < 0. {
                error!("pair {} avg volume < 0 !", pair.name);
            }
            v
        }
        Err(e) => {
            error!("error getting avg ticks :  {:?} ", e);
            -1.
        }
    };
    let market: Market = Binance::new(None, None);

    info!("{} volume {}", pair.name, candle.volume);

    match db::add_tick(
        &pool,
        pair.id,
        candle.open_time,
        candle.open,
        candle.close,
        candle.high,
        candle.low,
        candle.volume,
        candle.number_of_trades as i32,
    )
    .await
    {
        Ok(_) => (),
        Err(e) => {
            error!("error inserting tick :  {:?} ", e);
            //     continue;
        }
    };
    if avg_volume > 0. {
        helpers::check_volume(
            "MINUTE",
            avg_volume,
            30.,
            pair.name.as_str(),
            candle.volume,
            pair.base.as_str(),
        )
        .await;
        if avg_volume * 30. < candle.volume && candle.open > candle.close {
            match exchange::buy(&pair).await {
                Ok(r) => warn!("response from buy {} order: \n{}", pair.name, r),
                Err(e) => error!("error buying {} :\n{:?}", pair.name, e),
            };
        }
    }
}
