use rust_decimal_macros::dec;
use std::fs;

use price_server::{app::*, ExchangePriceCacheConfig, OrderBookCacheError};
use shared::{payload::*, pubsub::*, time::*};

#[derive(serde::Deserialize)]
struct Fixture {
    payloads: Vec<OrderBookPayload>,
}

fn load_fixture() -> anyhow::Result<Fixture> {
    let contents = fs::read_to_string("./tests/fixtures/order-book-payload-real.json")
        .expect("Couldn't load fixtures");
    Ok(serde_json::from_str(&contents)?)
}

#[tokio::test]
async fn price_app() -> anyhow::Result<()> {
    let (tick_send, tick_recv) =
        memory::channel(chrono::Duration::from_std(std::time::Duration::from_secs(2)).unwrap());
    let publisher = tick_send.clone();
    let mut subscriber = tick_recv.resubscribe();

    let (_, recv) = futures::channel::mpsc::unbounded();

    let ex_cfgs = ExchangeWeights {
        okex: Some(dec!(1.0)),
        bitfinex: None,
    };

    let app = PriceApp::run(
        recv,
        PriceServerHealthCheckConfig::default(),
        FeeCalculatorConfig {
            base_fee_rate: dec!(0.001),
            immediate_fee_rate: dec!(0.01),
            delayed_fee_rate: dec!(0.1),
        },
        tick_recv,
        ExchangePriceCacheConfig::default(),
        ex_cfgs,
    )
    .await?;

    let err = app
        .get_cents_from_sats_for_immediate_buy(Sats::from_major(100_000_000))
        .await;
    if let Err(PriceAppError::ExchangePriceCacheError(ExchangePriceCacheError::OrderBookCache(
        OrderBookCacheError::NoSnapshotAvailable,
    ))) = err
    {
        assert!(true)
    } else {
        assert!(false)
    }

    let mut payloads = load_fixture()?.payloads.into_iter();
    let mut payload = payloads.next().unwrap();
    tick_send
        .publish(PriceStreamPayload::OkexBtcUsdSwapOrderBookPayload(
            payload.clone(),
        ))
        .await?;
    subscriber.next().await;
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    let err = app
        .get_cents_from_sats_for_immediate_buy(Sats::from_major(100_000_000))
        .await;
    if let Err(PriceAppError::ExchangePriceCacheError(ExchangePriceCacheError::OrderBookCache(
        OrderBookCacheError::OutdatedSnapshot(_),
    ))) = err
    {
        assert!(true)
    } else {
        assert!(false)
    }

    payload.timestamp = TimeStamp::now();
    publisher
        .publish(PriceStreamPayload::OkexBtcUsdSwapOrderBookPayload(payload))
        .await?;
    subscriber.next().await;
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    let cents = app
        .get_cents_from_sats_for_immediate_buy(Sats::from_major(100_000_000))
        .await?;
    assert_eq!(cents, UsdCents::from_major(12362500));
    let cents = app
        .get_cents_from_sats_for_immediate_buy(Sats::from_major(1))
        .await?;
    assert_eq!(cents, UsdCents::from_major(0));

    let cents = app
        .get_cents_from_sats_for_immediate_sell(Sats::from_major(100_000_000))
        .await?;
    assert_eq!(cents, UsdCents::from_major(25022249));
    let cents = app
        .get_cents_from_sats_for_immediate_sell(Sats::from_major(1))
        .await?;
    assert_eq!(cents, UsdCents::from_major(1));

    let cents = app
        .get_cents_from_sats_for_future_buy(Sats::from_major(100_000_000))
        .await?;
    assert_eq!(cents, UsdCents::from_major(11237500));
    let cents = app
        .get_cents_from_sats_for_future_buy(Sats::from_major(1))
        .await?;
    assert_eq!(cents, UsdCents::from_major(0));

    let cents = app
        .get_cents_from_sats_for_future_sell(Sats::from_major(100_000_000))
        .await?;
    assert_eq!(cents, UsdCents::from_major(27249749));
    let cents = app
        .get_cents_from_sats_for_future_sell(Sats::from_major(1))
        .await?;
    assert_eq!(cents, UsdCents::from_major(1));

    let sats = app
        .get_sats_from_cents_for_immediate_buy(UsdCents::from_major(1000000))
        .await?;
    assert_eq!(sats, Sats::from_major(5054996));

    let sats = app
        .get_sats_from_cents_for_immediate_sell(UsdCents::from_major(1000000))
        .await?;
    assert_eq!(sats, Sats::from_major(4945004));
    let sats = app
        .get_sats_from_cents_for_immediate_sell(UsdCents::from_major(1))
        .await?;
    assert_eq!(sats, Sats::from_major(9));

    let sats = app
        .get_sats_from_cents_for_future_buy(UsdCents::from_major(1000000))
        .await?;
    assert_eq!(sats, Sats::from_major(5504996));
    let sats = app
        .get_sats_from_cents_for_future_buy(UsdCents::from_major(1))
        .await?;
    assert_eq!(sats, Sats::from_major(2));

    let sats = app
        .get_sats_from_cents_for_future_sell(UsdCents::from_major(1000000))
        .await?;
    assert_eq!(sats, Sats::from_major(4495004));
    let sats = app
        .get_sats_from_cents_for_future_sell(UsdCents::from_major(1))
        .await?;
    assert_eq!(sats, Sats::from_major(8));

    Ok(())
}
