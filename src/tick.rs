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
use binance::model::*;
#[derive(Debug)]
pub enum TickError {
    NoKline,
}

// 10800 s = 3h
//#[cached(time=10800, size=300, key = "String", convert = r#"{ format!("{}", pair.name) }"#)]
pub async fn get_avg(pool: &Pool<Postgres>, pair: db::Pair) -> (f64,f64, i64){
    let result = match db::get_pair_avg_data(&pool, pair.id).await {
        Ok(res) => {
            let (v,p,t) = res;
            if v < 0. {
                error!("pair {} avg volume < 0 !", pair.name);
            }
            res
        }
        Err(e) => {
            error!("error getting avg ticks :  {:?} ", e);
            (-1.,-1.,-1)
        }
    };
  //  warn!("getting AVG for {} - {:?}", pair.name, result);

    result
}

pub async fn main(pair: db::Pair, avg_volume: f64, avg_price:f64, avg_trades:i64, candle:KlineEvent) ->() {
    //let avg_volume = get_avg(&pool, pair.clone()).await;
    // warn!("{} avg volume: {}",pair.name, avg_volume);
    let candle = candle.kline;
    let diff = env::var("DIFF").unwrap_or_else(|_| String::from("30")).parse::<f64>().unwrap_or_else(|_| 30.);

    
     
            info!("{} volume {}", pair.name, candle.volume);

            if avg_volume > 0. {
                helpers::check_volume(
                    "MINUTE",
                    avg_volume,
                    avg_price, avg_trades,
                    diff,
                    pair.name.as_str(),
                    candle.volume.parse::<f64>().unwrap(),
                    pair.base.as_str(),
                    candle.open.parse::<f64>().unwrap() < candle.close.parse::<f64>().unwrap(),
                    candle.number_of_trades as i64
                )
                .await;
               // buy in real account (green candle)
                if avg_volume * diff < candle.volume.parse::<f64>().unwrap() && candle.open.parse::<f64>().unwrap() < candle.close.parse::<f64>().unwrap() {
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
                // big volume move
                if avg_volume * diff * 3. < candle.volume.parse::<f64>().unwrap() && candle.open.parse::<f64>().unwrap() < candle.close.parse::<f64>().unwrap() {
                    exchange::buy(&pair, String::from("5344546")).await;
                }
                // SUPER big volume move
                if avg_volume * diff * 6. < candle.volume.parse::<f64>().unwrap() && candle.open.parse::<f64>().unwrap() < candle.close.parse::<f64>().unwrap() {
                    exchange::buy(&pair, String::from("5345149")).await;
                }
                // under avg price  
                // if avg_volume * 30. < candle.volume.parse::<f64>().unwrap() && candle.open.parse::<f64>().unwrap() < candle.close.parse::<f64>().unwrap() && avg_price   <= candle.close.parse::<f64>().unwrap() {
                //     exchange::buy(&pair, String::from("5345093")).await;
                // }
                 // big volume move + under avg price  
                 if avg_volume * diff * 3. < candle.volume.parse::<f64>().unwrap() && candle.open.parse::<f64>().unwrap() < candle.close.parse::<f64>().unwrap()  && avg_price   <= candle.close.parse::<f64>().unwrap() {
                    exchange::buy(&pair, String::from("5345393")).await;
                }
                // buy for paper account (red candle)
                if avg_volume * diff < candle.volume.parse::<f64>().unwrap() && candle.open.parse::<f64>().unwrap() > candle.close.parse::<f64>().unwrap() {
                    exchange::buy(&pair, format!("5320720")).await;
                }
                // buy for paper account (green candle)
                if avg_volume * diff < candle.volume.parse::<f64>().unwrap() && candle.open.parse::<f64>().unwrap() < candle.close.parse::<f64>().unwrap() {
                    exchange::buy(&pair, String::from("5320761")).await;
                }
            }
            
       
    
    ()

}
