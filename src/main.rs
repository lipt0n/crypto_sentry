#[macro_use]
extern crate log;
#[macro_use]
extern crate rocket;

extern crate simplelog;
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
            LevelFilter::Info,
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
    loop {
        let local: DateTime<Local> = Local::now();
        let loop_start_time = local.timestamp();
        for pair in pairs.clone() {
            let pool = pool.clone();
            let candles = match market.get_klines(pair.name.as_str(), "1m", 1, None, None) {
                Ok(klines) => match klines {
                    KlineSummaries::AllKlineSummaries(klines) => klines,
                },
                Err(e) => {
                    warn!("{:?}", e);
                    continue;
                    //    continue;
                }
            };
            let candle = candles.clone().into_iter().nth(0);
            let p = pair.clone();
            match candle {
                Some(candle) => {
                    task::spawn(async move {
                        tick::main(pool, p, candle).await;
                    });

                    ()
                }
                None => (),
            }
        }
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
        if hourly_counter >= 30 {
            hourly_counter = 0;
            warn!("HOURLY CHECK IN PROGRESS");

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
        let avg_volume: f64 = candles
            .into_iter()
            .map(|k| {
                last_volume = k.volume;
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
    }
}
