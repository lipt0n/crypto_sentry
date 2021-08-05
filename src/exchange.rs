use binance::api::*;
use binance::general::*;
use binance::model::Symbol;
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use std::env;
use std::error::Error;
use std::str;
use std::str::from_utf8;

use hex;
use hmac::{Hmac, Mac, NewMac};
// Create alias for HMAC-SHA256
type HmacSha256 = Hmac<Sha256>;
use crate::db;

pub async fn get_markets() -> Vec<Symbol> {
    let general: General = Binance::new(None, None);
    let info = general
        .exchange_info()
        .expect("Cannot get base exchange info");
    let markets: Vec<Symbol> = info
        .symbols
        .into_iter()
        .filter(|s| s.quote_asset == "USDT")
        .filter(|s| !s.base_asset.contains("UP"))
        .filter(|s| !s.base_asset.contains("DOWN"))
        .filter(|s| !s.base_asset.contains("BULL"))
        .filter(|s| !s.base_asset.contains("BEAR"))
        .filter(|s| s.is_spot_trading_allowed)
        .collect();

    markets
}
pub async fn buy(pair: &db::Pair, bot_id:String) -> Result<String, ureq::Error> {
    // let bot_id = env::var("_3COMMAS_BOT_ID").expect("_3COMMAS_BOT_ID not set");
    let bot_key = env::var("_3COMMAS_BOT_KEY").expect("_3COMMAS_BOT_KEY not set");
    let bot_secret = env::var("_3COMMAS_BOT_SECRET").expect("_3COMMAS_BOT_SECRET not set");
    let base_url = format!("https://api.3commas.io");
    let param_url = format!(
        "/public/api/ver1/bots/{}/start_new_deal?pair={}_{}&bot_id={}",
        bot_id, pair.quote, pair.base, bot_id
    );
    let url = format!("{}{}", base_url, param_url);

    // Create HMAC-SHA256 instance which implements `Mac` trait
    let mut mac =
        HmacSha256::new_from_slice(bot_secret.as_bytes()).expect("HMAC can take key of any size");
    mac.update(param_url.as_bytes());

    let result = mac.finalize();

    let fancy_bytes = result.into_bytes();
    let code_bytes: &[u8] = fancy_bytes.as_slice();
    let signature = hex::encode(&code_bytes);

    #[derive(Debug, Serialize)]
    struct Data {
        pair: String,
        bot_id: String,
    }
    let data = Data {
        pair: format!("{}_{}", pair.quote, pair.base),
        bot_id: format!("{}", bot_id),
    };

    let resp: String = ureq::post(url.as_str())
        .set("APIKEY", bot_key.as_str())
        .set("Signature", signature.as_str())
        //     .query("pair", data.pair.as_str())
        //     .query("bot_id", bot_id.as_str())
        .call()?
        .into_string()?;

    Ok(resp)
}