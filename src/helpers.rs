use std::env;
use std::error::Error;

use crate::webhook;

pub async fn check_volume(
    interval: &str,
    avg_vol: f64,
    avg_price:f64,
    avg_trades:i64,
    multiplier: f64,
    pair_name: &str,
    candle_volume: f64,
    base_asset: &str,
    bull:bool,
    no_of_transactions:i64
) {
    if avg_vol * multiplier < candle_volume && avg_vol > 0. {
        let mut ccolor = "RED";
        if bull {
            ccolor = "GREEN";
        }
        let avg_transaction_per_volume = avg_vol / avg_trades as f64;
        let transactions_per_volume = candle_volume / no_of_transactions as f64;
        let transaction_per_vol_diff =  percentage(transactions_per_volume, avg_transaction_per_volume);
        let msg = format!(
            "
        1 {} INTERVAL
        VOLUME MOVE DETECTED FOR {} by {:.1} %
        NO OF TRANSACTIONS PER VOL CHANGE: {:.1} %
        AVG VOLUME: {:.1}
        CUR VOLUME: {:.1}
        CANDLE: {}
        NO OF TRANSACTIONS: {}
        AVG NO OF TRANSACTIONS: {}
        https://www.tradingview.com/chart?symbol=BINANCE%3A{}
        ",
            interval,
            pair_name,
            percentage(candle_volume, avg_vol),
            transaction_per_vol_diff,
            avg_vol,
            candle_volume,
            ccolor,
            no_of_transactions,
            avg_trades,
            pair_name,
       
        );
        warn!("{}", msg.as_str());
        if interval == "HOUR" {
            send_msg(msg.as_str(), base_asset, interval).await;
        } else {
            send_msg(msg.as_str(), base_asset, interval).await;
        }
    }
}
pub async fn send_msg(msg: &str, base_asset: &str, interval: &str) -> Result<String, Box<dyn Error>> {
    let webhook_url = env::var("DISCORD_WEBHOOK").expect("no DISCORD_WEBHOOK");
    let mut new_msg = webhook::Message::new();

    let message = new_msg
        .content(msg)
        .tts(true)
        .username(format!("{} SENTRY for {}", interval, base_asset).as_str())
        .avatar_url(
            format!(
                "https://github.com/spothq/cryptocurrency-icons/raw/master/32/icon/{}.png",
                base_asset.to_lowercase()
            )
            .as_str(),
        );

    let resp: String = ureq::post(webhook_url.as_str())
        .send_json(ureq::json!(&new_msg))?
        .into_string()?;

    Ok(resp)
}

pub fn percentage(a: f64, b: f64) -> f64 {
    (a / b) * 100.
}
