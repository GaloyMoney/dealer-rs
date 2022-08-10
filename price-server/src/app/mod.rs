mod error;

use chrono::Duration;
use futures::stream::StreamExt;

use shared::{currency::*, payload::OkexBtcUsdSwapPricePayload, pubsub::*};

use super::exchange_price_cache::ExchangePriceCache;

pub use crate::fee_calculator::*;
pub use error::*;

pub struct PriceApp {
    price_cache: ExchangePriceCache,
    fee_calculator: FeeCalculator,
}

impl PriceApp {
    pub async fn run(
        fee_calc_cfg: FeeCalculatorConfig,
        pubsub_cfg: PubSubConfig,
    ) -> Result<Self, PriceAppError> {
        let subscriber = Subscriber::new(pubsub_cfg).await?;
        let mut stream = subscriber.subscribe::<OkexBtcUsdSwapPricePayload>().await?;

        let price_cache = ExchangePriceCache::new(Duration::seconds(30));
        let fee_calculator = FeeCalculator::new(fee_calc_cfg);
        let app = Self {
            price_cache: price_cache.clone(),
            fee_calculator,
        };
        let _ = tokio::spawn(async move {
            while let Some(msg) = stream.next().await {
                let payload = msg.payload;
                price_cache.apply_update(payload).await;
            }
        });
        Ok(app)
    }

    pub async fn get_cents_from_sats_for_immediate_buy(
        &self,
        sats: impl Into<Sats>,
    ) -> Result<u64, PriceAppError> {
        let price_of_one_sat = self.price_cache.latest_tick().await?.bid_price_of_one_sat;
        Ok(u64::try_from(self.fee_calculator.apply_immediate_fee(
            price_of_one_sat * *sats.into().amount(),
        ))?)
    }

    pub async fn get_cents_from_sats_for_immediate_sell(
        &self,
        sats: impl Into<Sats>,
    ) -> Result<u64, PriceAppError> {
        let price_of_one_sat = self.price_cache.latest_tick().await?.ask_price_of_one_sat;
        Ok(u64::try_from(self.fee_calculator.apply_immediate_fee(
            price_of_one_sat * *sats.into().amount(),
        ))?)
    }

    pub async fn get_cents_from_sats_for_future_buy(
        &self,
        sats: impl Into<Sats>,
    ) -> Result<u64, PriceAppError> {
        let price_of_one_sat = self.price_cache.latest_tick().await?.bid_price_of_one_sat;
        Ok(u64::try_from(self.fee_calculator.apply_delayed_fee(
            price_of_one_sat * *sats.into().amount(),
        ))?)
    }

    pub async fn get_cents_from_sats_for_future_sell(
        &self,
        sats: impl Into<Sats>,
    ) -> Result<u64, PriceAppError> {
        let price_of_one_sat = self.price_cache.latest_tick().await?.ask_price_of_one_sat;
        Ok(u64::try_from(self.fee_calculator.apply_delayed_fee(
            price_of_one_sat * *sats.into().amount(),
        ))?)
    }

    pub async fn get_sats_from_cents_for_immediate_buy(
        &self,
        cents: UsdCents,
    ) -> Result<u64, PriceAppError> {
        let price_of_one_sat = self.price_cache.latest_tick().await?.ask_price_of_one_sat;
        let fee_on_one_sat = self.fee_calculator.apply_immediate_fee(price_of_one_sat);
        Ok(u64::try_from(cents.amount() / fee_on_one_sat.amount()).unwrap())
    }

    pub async fn get_sats_from_cents_for_immediate_sell(
        &self,
        cents: UsdCents,
    ) -> Result<u64, PriceAppError> {
        let price_of_one_sat = self.price_cache.latest_tick().await?.bid_price_of_one_sat;
        let fee_on_one_sat = self.fee_calculator.apply_immediate_fee(price_of_one_sat);
        Ok(u64::try_from(cents.amount() / fee_on_one_sat.amount()).unwrap())
    }

    pub async fn get_cents_per_sats_exchange_mid_rate(
        &self,
        sats: impl Into<Sats>,
    ) -> Result<u64, PriceAppError> {
        let cents = self.price_cache.latest_tick().await?.mid_price_of_one_sat();
        Ok(u64::try_from(cents * *sats.into().amount())?)
    }

    pub async fn get_sats_from_cents_for_future_buy(
        &self,
        cents: UsdCents,
    ) -> Result<u64, PriceAppError> {
        let price_of_one_sat = self.price_cache.latest_tick().await?.ask_price_of_one_sat;
        let fee_on_sat = self.fee_calculator.apply_delayed_fee(price_of_one_sat);
        Ok(u64::try_from(cents.amount() / fee_on_sat.amount()).unwrap())
    }

    pub async fn get_sats_from_cents_for_future_sell(
        &self,
        cents: UsdCents,
    ) -> Result<u64, PriceAppError> {
        let price_of_one_sat = self.price_cache.latest_tick().await?.bid_price_of_one_sat;
        let fee_per_sat = self.fee_calculator.apply_delayed_fee(price_of_one_sat);
        Ok(u64::try_from(cents.amount() / fee_per_sat.amount()).unwrap())
    }
}
