use std::env;
use std::error::Error;

use crate::webhook;

pub async fn check_volume(
    interval: &str,
    avg_vol: f64,
    multiplier: f64,
    pair_name: &str,
    candle_volume: f64,
    base_asset: &str,
) {
    if avg_vol * multiplier < candle_volume && avg_vol > 0. {
        let msg = format!(
            "
        1 {} INTERVAL
        VOLUME MOVE DETECTED FOR {} by {:.1} %
        AVG VOLUME: {:.1}
        CUR VOLUME: {:.1}
        https://www.tradingview.com/chart?symbol=BINANCE%3A{}
        ",
            interval,
            pair_name,
            percentage(candle_volume, avg_vol),
            avg_vol,
            candle_volume,
            pair_name
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
