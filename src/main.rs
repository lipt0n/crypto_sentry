#[macro_use]
extern crate log;
#[macro_use]
extern crate rocket;

use db::Pair;
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
use binance::websockets::*;
use colored::*;
use dotenv::dotenv;
use std::env;
use std::fs::File;
use std::sync::atomic::AtomicBool;
mod db;
mod exchange;
mod helpers;
mod routes;
mod tick;
mod webhook;
#[async_std::main]
async fn main() {
    let period = env::var("PERIOD").unwrap_or_else(|_| String::from("5m"));
    let diff = env::var("DIFF").unwrap_or_else(|_| String::from("30")).parse::<f64>().unwrap_or_else(|_| 30.);


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
            File::create(format!("sentry_{}.log",period)).unwrap(),
        ),
    ])
    .unwrap();

    warn!("program started for {} candle and {} diff !", period, diff);
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
    // filter pairs with low 24h volume
    let min_usd_24h_volume = 100_000;
 





    let period = env::var("PERIOD").unwrap_or_else(|_| String::from("5m"));
    let diff = env::var("DIFF").unwrap_or_else(|_| String::from("30")).parse::<f64>().unwrap_or_else(|_| 30.);
    let mut avg_volumes: HashMap<i32, (f64, f64, i64)> = get_avg_volumes(pairs.clone(), period.clone());
    warn!("Getting avgs");
    // for pair in pairs.clone() {
    //     let avg_volume = tick::get_avg(&pool, pair.clone()).await;
    //     avg_volumes.insert(pair.id + 0, avg_volume);
    // }
    let avg_volumes_copy = avg_volumes.clone();

    let mut endpoints: Vec<String> = Vec::new();


    let keep_running = AtomicBool::new(true);
    let mut queueue :HashMap<i32,i32> = HashMap::new();
    let mut web_socket: WebSockets<'_> = WebSockets::new(|event: WebsocketEvent| {
        match event {
            WebsocketEvent::Kline(kline_event) => {
               
                let open = kline_event.kline.open.parse::<f64>().unwrap();
                let close = kline_event.kline.close.parse::<f64>().unwrap();
                let high = kline_event.kline.high.parse::<f64>().unwrap();
                let low = kline_event.kline.low.parse::<f64>().unwrap();
                let volume = kline_event.kline.volume.parse::<f64>().unwrap();
                let orders = kline_event.kline.number_of_trades as i32;

                let mut iter = pairs.clone().into_iter();
                let pair = iter.find(|p| p.name == kline_event.kline.symbol).unwrap();
                let candle = kline_event.kline.clone();
                let p = pair.clone();
                let (avg_volume, avg_price, avg_trades) =
                            *avg_volumes_copy.get(&pair.id).unwrap();
                if queueue.contains_key(&pair.id) {
                    let counter = queueue.get(&pair.id).unwrap() - 1;
                    queueue.remove(&pair.id);
                    if counter > 0 {
                        queueue.insert(pair.id, counter);
                    }
                    
                    
                }
                if !queueue.contains_key(&pair.id) && avg_volume * diff < candle.volume.parse::<f64>().unwrap() && candle.open.parse::<f64>().unwrap() < candle.close.parse::<f64>().unwrap() &&  candle.close.parse::<f64>().unwrap() >=  candle.high.parse::<f64>().unwrap() {
                    println!(
                        "Symbol: {}, open: {} close: {} high: {}, low: {}, volume: {}, is closed: {}",
                        kline_event.kline.symbol,
                        kline_event.kline.open,
                        kline_event.kline.close,
                        kline_event.kline.low,
                        kline_event.kline.high,
                        kline_event.kline.volume,
                        kline_event.kline.is_final_bar
                    );
                    task::block_on(async {
                        
                        let r = tick::main(p, avg_volume, avg_price, avg_trades, kline_event.clone()).await;
                    });
                        queueue.insert(pair.id, 10);
                }
                if kline_event.kline.is_final_bar  && period.clone() == "5m" {
                    // println!(
                    //     "Symbol: {}, high: {}, low: {}, volume: {}, is closed: {}",
                    //     kline_event.kline.symbol,
                    //     kline_event.kline.low,
                    //     kline_event.kline.high,
                    //     kline_event.kline.volume,
                    //     kline_event.kline.is_final_bar
                    // );
    
                    let x = task::block_on(async {
                        db::add_tick(
                            &pool,
                            pair.id,
                            kline_event.kline.end_time,
                            open,
                            close,
                            high,
                            low,
                            volume,
                            orders,
                        )
                        .await;
                    });
                }
            }
            _ => (),
        };
        Ok(())
    });

    match web_socket.connect_multiple_streams(&endpoints) {
        Ok(msg) => {
            warn!("connected to binance!");
        }
        Err(e) => {
            error!("Error connecting to binance : \n {:?}", e);
        }
    } // check error
    if let Err(e) = web_socket.event_loop(&keep_running) {
        error!("Error in BINANCE EVENT LOOP: {:?}", e);
    }
    web_socket.disconnect().unwrap();

    // loop {
    //     let local: DateTime<Local> = Local::now();
    //     let loop_start_time = local.timestamp();
    //     let mut handles = vec![];
    //     let mut candles: HashMap<i32, KlineSummary> = HashMap::new();

    //     for pair in pairs.clone() {
    //         let (avg_volume, avg_price, avg_trades) = *avg_volumes.get(&pair.id).unwrap();
    //         let p = pair.clone();

    //         handles.push(task::spawn(async move {
    //             let p = pair.clone();
    //             let r = tick::main(p, avg_volume, avg_price, avg_trades).await;

    //             (pair.id, r)
    //         }));
    //         // if handles.len()>=60 {
    //         //    let candles_results =  join_all(handles).await;
    //         //    for cr in candles_results {
    //         //        let (id, result) = cr;
    //         //        match result {
    //         //            Ok(candle)=>{
    //         //             candles.insert( id,  candle);
    //         //            }
    //         //            Err(e)=>{

    //         //            }
    //         //        };
    //         //    }

    //         //     handles =  vec![];
    //         // }
    //     }
    //     let candles_results = join_all(handles).await;
    //     for cr in candles_results {
    //         let (id, result) = cr;
    //         match result {
    //             Ok(candle) => {
    //                 candles.insert(id, candle);
    //             }
    //             Err(e) => {
    //                 warn!("error geting candle for {}", id);
    //             }
    //         };
    //     }

    //     let local: DateTime<Local> = Local::now();
    //     let loop_stop_time = local.timestamp();

    //     warn!(
    //         "{} candles collected in {} s",
    //         candles.len(),
    //         loop_stop_time - loop_start_time
    //     );

    //     for candle_pair in candles.clone().into_iter() {
    //         let (id, candle) = candle_pair;
    //         db::add_tick(
    //             &pool,
    //             id,
    //             candle.open_time,
    //             candle.open,
    //             candle.close,
    //             candle.high,
    //             candle.low,
    //             candle.volume,
    //             candle.number_of_trades as i32,
    //         )
    //         .await;
    //     }
    //     let local: DateTime<Local> = Local::now();
    //     let loop_stop2_time = local.timestamp();

    //     warn!(
    //         "{} candles inserted in {} s",
    //         candles.len(),
    //         loop_stop2_time - loop_stop_time
    //     );
    //     candles.clear();

    //     //   join_all(handles).await;

    //     //     join_all(handles).await;

    //     let local: DateTime<Local> = Local::now();
    //     let loop_stop_time = local.timestamp();
    //     let time_to_wait = (60 - (loop_stop_time - loop_start_time));
    //     warn!(
    //         "{} {} CANDLES. it took {}s. {} {}s",
    //         "INSERTED".green().bold(),
    //         pairs_len,
    //         loop_stop_time - loop_start_time,
    //         "will wait for".blue(),
    //         time_to_wait
    //     );

    //     hourly_counter += 1;
    //     if hourly_counter >= 60 {
    //         hourly_counter = 0;
    //         warn!("HOURLY CHECK IN PROGRESS");
    //         for pair in pairs.clone() {
    //             avg_volumes.clear();
    //             let avg_volume = tick::get_avg(&pool, pair.clone()).await;
    //             avg_volumes.insert(pair.id, avg_volume);
    //         }

    //         hourlyCheck(&pool).await;
    //     } else {
    //         if time_to_wait > 0 {
    //             let sleep_time = Duration::from_secs(time_to_wait.unsigned_abs());
    //             task::sleep(sleep_time).await;
    //         }
    //     }
    // }

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

        match last_candle {
            Some(candle) => {
                if last_volume > 0. {
                    helpers::check_volume(
                        "HOUR",
                        avg_volume,
                        0.,
                        1,
                        5.,
                        pair.name.as_str(),
                        last_volume,
                        &pair.base,
                        candle.open < candle.close,
                        candle.number_of_trades,
                    )
                    .await;
                }
                if avg_volume * 5. < last_volume && candle.open > candle.close {
                    match exchange::buy(&pair, String::from("5321172")).await {
                        Ok(r) => warn!("BOUGHT {}  \n{}", pair.name, r),
                        Err(e) => error!("error buying {} :\n{:?}", pair.name, e),
                    };
                }
                if avg_volume * 5. < last_volume && candle.open < candle.close {
                    match exchange::buy(&pair, String::from("5321168")).await {
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


fn get_avg_volumes(pairs:Vec<Pair>, period:String) -> HashMap<i32,(f64,f64,i64)> {

    let market: Market = Binance::new(None, None);
    let mut avg_volumes: HashMap<i32, (f64, f64, i64)> = HashMap::new();
    for pair in pairs {
        let candles = match market.get_klines(pair.name.as_str(), period.as_str(), 1000, None, None) {
            Ok(klines) => match klines {
                KlineSummaries::AllKlineSummaries(klines) => klines,
            },
            Err(e) => {
                warn!("{:?}", e);
                continue;
            }
        };
        let candles_len = candles.len() as f64;
        let candles_len_i64 = candles.len() as i64;
        let mut last_volume = -1.;
        let mut last_candle: Option<KlineSummary> = None;
        let avg_volume: f64 = candles.clone()
            .into_iter()
            .map(|k| k.volume)
            .sum::<f64>()
            / candles_len;
        let avg_close: f64 = candles.clone()
        .into_iter()
        .map(|k| k.close)
        .sum::<f64>()
        / candles_len;
        let avg_orders: i64 = candles.clone()
        .into_iter()
        .map(|k| k.number_of_trades)
        .sum::<i64>() / candles_len_i64;
        avg_volumes.insert(pair.id, (avg_volume,avg_close,avg_orders));
        
    }
    avg_volumes
}
