#[macro_use]
extern crate log;
#[macro_use]
extern crate rocket;

use dict::DictEntry;
use futures::join;
use std::collections::HashMap;
use std::sync::Arc;

use async_std::future;

extern crate simplelog;

use futures::future::join_all;
use sqlx::{pool::Pool, Postgres};
use ureq;

use async_std::task;
use std::time::Duration;

use chrono::prelude::*;
use simplelog::*;

use binance::api::*;
use binance::market::*;
use binance::model::{KlineSummaries, KlineSummary, Symbol};
use dotenv::dotenv;
use std::fs::File;

use colored::*;
use std::env;
mod db;
mod exchange;
mod helpers;
mod routes;
mod tick;
mod webhook;
#[async_std::main]
async fn main() {
    CombinedLogger::init(vec![
        TermLogger::new(
            LevelFilter::Warn,
            Config::default(),
            TerminalMode::Mixed,
            ColorChoice::Auto,
        ),
        WriteLogger::new(
            LevelFilter::Warn,
            Config::default(),
            File::create("sentry.log").unwrap(),
        ),
    ])
    .unwrap();

    warn!("program started!");
    let help = format!(
        "
    {} 
    {}
    {} - {}
    {} - {}
    ",
        "no command given",
        "cargo run [start] [init]",
        "start".red(),
        "start monitoring prices and volumes",
        "init".red(),
        "initialize db with monitored pairs"
    );

    let command = std::env::args().nth(1).expect(help.as_str());

    match command.as_str() {
        "start" => start().await,
        "init" => init().await,
        _ => warn!("{}", help.as_str()),
    };
}

async fn start() {
    dotenv().ok();

    //send_msg("Program started. sentry is watching ...").await;
    let pool = db::establish_connection()
        .await
        .expect("Can't connect to DB  😢");
    info!("connected to db 😃 : {:?}", pool);

    let pairs = match db::get_pairs(&pool).await {
        Ok(p) => p,
        Err(e) => panic!("Error geting pairs {:?}", e),
    };
    let mut hourly_counter: u8 = 0;
    let market: Market = Binance::new(None, None);

    let pairs_len = pairs.len();
    let mut avg_volumes: HashMap<i32, f64> = HashMap::new();
    for pair in pairs.clone() {
        let avg_volume: f64 = tick::get_avg(&pool, pair.clone()).await;
        avg_volumes.insert(pair.id + 0, avg_volume);
    }

    loop {
        let local: DateTime<Local> = Local::now();
        let loop_start_time = local.timestamp();
        let mut handles = vec![];
        let mut candles: HashMap<i32, KlineSummary> = HashMap::new();

        for pair in pairs.clone() {
            let avg_volume = *avg_volumes.get(&pair.id).unwrap();
            let p = pair.clone();

            handles.push( task::spawn(async move {
                let p = pair.clone();
                let r =  tick::main( p,avg_volume).await;

                (pair.id, r)

            }));
            // if handles.len()>=60 {
            //    let candles_results =  join_all(handles).await;
            //    for cr in candles_results {
            //        let (id, result) = cr;
            //        match result {
            //            Ok(candle)=>{
            //             candles.insert( id,  candle);
            //            }
            //            Err(e)=>{

            //            }
            //        };
            //    }

            //     handles =  vec![];
            // }
        }
        let candles_results =  join_all(handles).await;
        for cr in candles_results {
            let (id, result) = cr;
            match result {
                Ok(candle)=>{
                 candles.insert( id,  candle);
                }
                Err(e)=>{
                    warn!("error geting candle for {}", id);
                }
            };
        }

        let local: DateTime<Local> = Local::now();
        let loop_stop_time = local.timestamp();

        warn!(
            "{} candles collected in {} s", candles.len(),
            loop_stop_time - loop_start_time
        );

        for candle_pair in candles.clone().into_iter() {
            let (id, candle) = candle_pair;
            db::add_tick(
                &pool,
                id,
                candle.open_time,
                candle.open,
                candle.close,
                candle.high,
                candle.low,
                candle.volume,
                candle.number_of_trades as i32,
            )
            .await;
        }
        let local: DateTime<Local> = Local::now();
        let loop_stop2_time = local.timestamp();

        warn!("{} candles inserted in {} s", candles.len(), loop_stop2_time - loop_stop_time);
        candles.clear();

        //   join_all(handles).await;

        //     join_all(handles).await;

        let local: DateTime<Local> = Local::now();
        let loop_stop_time = local.timestamp();
        let time_to_wait = (60 - (loop_stop_time - loop_start_time));
        warn!(
            "{} {} CANDLES. it took {}s. {} {}s",
            "INSERTED".green().bold(),
            pairs_len,
            loop_stop_time - loop_start_time,
            "will wait for".blue(),
            time_to_wait
        );

        hourly_counter += 1;
        if hourly_counter >= 60 {
            hourly_counter = 0;
            warn!("HOURLY CHECK IN PROGRESS");
            for pair in pairs.clone() {
                avg_volumes.clear();
                let avg_volume: f64 = tick::get_avg(&pool, pair.clone()).await;
                avg_volumes.insert(pair.id, avg_volume);
            }

            hourlyCheck(&pool).await;
        } else {
            if time_to_wait > 0 {
                let sleep_time = Duration::from_secs(time_to_wait.unsigned_abs());
                task::sleep(sleep_time).await;
            }
        }
    }

    //routes::rocket().expect("Can't run http server");
}

async fn init() {
    let pool = db::establish_connection()
        .await
        .expect("Can't connect to DB  😢");
    info!("connected to db 😃 : {:?}", pool);

    let market_info = exchange::get_markets().await;

    for pair in market_info {
        match db::add_pair(
            &pool,
            pair.base_asset.to_uppercase().as_str(),
            pair.quote_asset.to_uppercase().as_str(),
            pair.symbol.to_uppercase().as_str(),
        )
        .await
        {
            Ok(_added) => warn!("pair {} added", &pair.symbol),
            Err(e) => error!("cant add {:?}", e),
        }
    }
}

async fn hourlyCheck(pool: &Pool<Postgres>) {
    let pairs = match db::get_pairs(&pool).await {
        Ok(p) => p,
        Err(e) => panic!("Error geting pairs {:?}", e),
    };
    let market: Market = Binance::new(None, None);
    for pair in pairs {
        let candles = match market.get_klines(pair.name.as_str(), "1h", 1000, None, None) {
            Ok(klines) => match klines {
                KlineSummaries::AllKlineSummaries(klines) => klines,
            },
            Err(e) => {
                warn!("{:?}", e);
                continue;
            }
        };
        let candles_len = candles.len() as f64;
        let mut last_volume = -1.;
        let mut last_candle: Option<KlineSummary> = None;
        let avg_volume: f64 = candles
            .into_iter()
            .map(|k| {
                last_volume = k.volume;
                last_candle = Some(k.clone());
                k
            })
            .map(|k| k.volume)
            .sum::<f64>()
            / candles_len;
        // warn!(" HOURLY pair {} avg {} now {}", pair.name.as_str(), avg_volume, last_volume);
        if last_volume > 0. {
            helpers::check_volume(
                "HOUR",
                avg_volume,
                5.,
                pair.name.as_str(),
                last_volume,
                &pair.base,
            )
            .await;
        }
        match last_candle {
            Some(candle) => {
                if avg_volume * 5. < last_volume && candle.open > candle.close {
                    match exchange::buy(&pair, String::from("5321172")).await {
                        Ok(r) => warn!("BOUGHT {}  \n{}", pair.name, r),
                        Err(e) => error!("error buying {} :\n{:?}", pair.name, e),
                    };
                }
                if avg_volume * 5. < last_volume && candle.open < candle.close {
                    match exchange::buy(&pair,  String::from("5321168" )).await {
                        Ok(r) => warn!("BOUGHT {}  \n{}", pair.name, r),
                        Err(e) => error!("error buying {} :\n{:?}", pair.name, e),
                    };
                }
                ()
            }
            None => (),
        };
    }
}
