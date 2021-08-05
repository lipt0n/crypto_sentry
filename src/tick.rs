use async_std::task;
use chrono::prelude::*;
use chrono::{NaiveDate, NaiveDateTime};
use colored::*;
use sqlx::{pool::Pool, postgres::PgPoolOptions, Postgres};
use std::env;

use rand::Rng;
use std::error::Error;

use crate::db;
use crate::exchange;
use crate::helpers;
use binance::api::*;
use binance::market::*;
use binance::model::{KlineSummaries, KlineSummary, Symbol};
use cached::proc_macro::cached;

#[derive(Debug)]
pub enum TickError {
    NoKline,
}

// 10800 s = 3h
//#[cached(time=10800, size=300, key = "String", convert = r#"{ format!("{}", pair.name) }"#)]
pub async fn get_avg(pool: &Pool<Postgres>, pair: db::Pair) -> f64 {
    let result = match db::get_pair_avg_volume(&pool, pair.id).await {
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
    warn!("getting AVG for {} - {}", pair.name, result);

    result
}

pub async fn main(pair: db::Pair, avg_volume: f64) -> Result<KlineSummary, TickError> {
    //let avg_volume = get_avg(&pool, pair.clone()).await;
    // warn!("{} avg volume: {}",pair.name, avg_volume);
    let market: Market = Binance::new(None, None);

    let candles = match market.get_klines(pair.name.as_str(), "1m", 1, None, None) {
        Ok(klines) => match klines {
            KlineSummaries::AllKlineSummaries(klines) => klines,
        },
        Err(e) => {
            error!("NO KLINE FOR {} \n {:?}", pair.name, e);
            return Err(TickError::NoKline);
        }
    };

    let candle = candles.clone().into_iter().nth(0);

    let result = match candle {
        Some(candle) => {
            info!("{} volume {}", pair.name, candle.volume);

            let candle_before = candles.clone().into_iter().nth(0).unwrap();
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
                // buy in real account (green candle)
                if avg_volume * 30. < candle.volume && candle.open < candle.close {
                    let bot_id = env::var("_3COMMAS_BOT_ID").expect("_3COMMAS_BOT_ID not set");
                    match exchange::buy(&pair, bot_id).await {
                        Ok(r) => {
                            warn!("{} {}  \n", "----------- BOUGHT ".green().bold(), pair.name)
                        }
                        Err(ureq::Error::Status(code, response)) => {
                            /* the server returned an unexpected status
                            code (such as 400, 500 etc) */
                            error!(
                                "{} error buying {} :\n{}\n{}",
                                "---------------------".red(),
                                pair.name,
                                format!("{}", code).red(),
                                response.into_string().unwrap_or_default()
                            )
                        }
                        Err(e) => error!("error buying {} :\n{}\n{:?}", pair.name, e, e),
                    };
                    exchange::buy(&pair, String::from("5343834")).await; // small profit bot
                }
                // buy for paper account (red candle)
                if avg_volume * 30. < candle.volume && candle.open > candle.close {
                    exchange::buy(&pair, format!("5320720")).await;
                }
                // buy for paper account (green candle)
                if avg_volume * 30. < candle.volume && candle.open < candle.close {
                    exchange::buy(&pair, String::from("5320761")).await;
                }
            }
            candle
        }
        None => return Err(TickError::NoKline),
    };
    Ok(result)
}
