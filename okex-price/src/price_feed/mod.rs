mod book;
mod config;
mod error;
mod tick;

use futures::{SinkExt, Stream, StreamExt};
use serde::Deserialize;
use tokio_tungstenite::{connect_async, tungstenite::Message};

pub use book::*;
pub use config::*;
pub use tick::*;

pub use error::PriceFeedError;

#[derive(Clone, Deserialize, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ChannelArgs {
    pub channel: String,
    pub inst_id: String,
}

pub async fn subscribe_btc_usd_swap(
    config: PriceFeedConfig,
) -> Result<std::pin::Pin<Box<dyn Stream<Item = OkexPriceTick> + Send>>, PriceFeedError> {
    let (ws_stream, _) = connect_async(config.url).await?;
    let (mut sender, receiver) = ws_stream.split();

    let subscribe_args = serde_json::json!({
        "op": "subscribe",
        "args": [
           {
                "channel": "tickers",
                "instId": "BTC-USD-SWAP"
            }
        ]
    })
    .to_string();
    let item = Message::Text(subscribe_args);

    sender.send(item).await?;

    Ok(Box::pin(receiver.filter_map(|message| async {
        if let Ok(msg) = message {
            if let Ok(msg_str) = msg.into_text() {
                if let Ok(tick) = serde_json::from_str::<OkexPriceTick>(&msg_str) {
                    return Some(tick);
                }
            }
        }
        None
    })))
}

pub async fn subscribe_order_book(
    config: PriceFeedConfig,
) -> Result<std::pin::Pin<Box<dyn Stream<Item = OkexOrderBook> + Send>>, PriceFeedError> {
    let (ws_stream, _) = connect_async(config.url).await?;
    let (mut sender, receiver) = ws_stream.split();

    let subscribe_args = serde_json::json!({
        "op": "subscribe",
        "args": [
           {
                "channel": "books",
                "instId": "BTC-USD-SWAP"
            }
        ]
    })
    .to_string();
    let item = Message::Text(subscribe_args);
    sender.send(item).await?;

    Ok(Box::pin(receiver.filter_map(|message| async {
        if let Ok(msg) = message {
            if let Ok(msg_str) = msg.into_text() {
                if let Ok(book) = serde_json::from_str::<OkexOrderBook>(&msg_str) {
                    return Some(book);
                }
            }
        }
        None
    })))
}
