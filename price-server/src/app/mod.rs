mod config;

use std::collections::HashMap;

use futures::stream::StreamExt;
use rust_decimal::Decimal;
use tracing::{info_span, instrument, Instrument};

use shared::{
    exchanges_config::ExchangeConfigAll,
    health::HealthCheckTrigger,
    payload::{OkexBtcUsdSwapPricePayload, OKEX_EXCHANGE_ID},
    pubsub::*,
};

use crate::{
    cache_config::ExchangePriceCacheConfig,
    exchange_tick_cache::ExchangeTickCache,
    price_mixer::{PriceMixer, PriceProvider},
};
pub use crate::{currency::*, error::*, fee_calculator::*};
pub use config::*;

pub struct PriceApp {
    price_mixer: PriceMixer,
    fee_calculator: FeeCalculator,
}

impl PriceApp {
    pub async fn run(
        mut health_check_trigger: HealthCheckTrigger,
        health_check_cfg: PriceServerHealthCheckConfig,
        fee_calc_cfg: FeeCalculatorConfig,
        subscriber: memory::Subscriber<OkexBtcUsdSwapPricePayload>,
        price_cache_config: ExchangePriceCacheConfig,
        exchanges_cfg: ExchangeConfigAll,
    ) -> Result<Self, PriceAppError> {
        let health_subscriber = subscriber.resubscribe();
        tokio::spawn(async move {
            while let Some(check) = health_check_trigger.next().await {
                check
                    .send(
                        health_subscriber
                            .healthy(health_check_cfg.unhealthy_msg_interval_price)
                            .await,
                    )
                    .expect("Couldn't send response");
            }
        });

        let mut providers: HashMap<String, (Box<dyn PriceProvider + Sync + Send>, Decimal)> =
            HashMap::new();

        if let Some(config) = exchanges_cfg.okex.as_ref() {
            let okex_price_cache = ExchangeTickCache::new(price_cache_config);
            Self::subscribe_okex(subscriber, okex_price_cache.clone()).await?;
            providers.insert(
                OKEX_EXCHANGE_ID.to_string(),
                (Box::new(okex_price_cache), config.weight),
            );
        }

        let fee_calculator = FeeCalculator::new(fee_calc_cfg);
        let app = Self {
            price_mixer: PriceMixer::new(providers),
            fee_calculator,
        };

        Ok(app)
    }

    async fn subscribe_okex(
        mut subscriber: memory::Subscriber<OkexBtcUsdSwapPricePayload>,
        price_cache: ExchangeTickCache,
    ) -> Result<(), PriceAppError> {
        let _ = tokio::spawn(async move {
            while let Some(msg) = subscriber.next().await {
                let span = info_span!(
                    "price_tick_received",
                    message_type = %msg.payload_type,
                    correlation_id = %msg.meta.correlation_id
                );
                shared::tracing::inject_tracing_data(&span, &msg.meta.tracing_data);

                async {
                    price_cache
                        .apply_update(msg.payload.0, msg.meta.correlation_id)
                        .await;
                }
                .instrument(span)
                .await;
            }
        });

        Ok(())
    }

    #[instrument(skip_all, fields(correlation_id, amount = %sats.amount()), ret, err)]
    pub async fn get_cents_from_sats_for_immediate_buy(
        &self,
        sats: Sats,
    ) -> Result<UsdCents, PriceAppError> {
        let cents = UsdCents::from_decimal(
            self.price_mixer
                .apply(|p| *p.buy_usd().cents_from_sats(sats.clone()).amount())
                .await?,
        );

        Ok(self.fee_calculator.decrease_by_immediate_fee(cents).floor())
    }

    #[instrument(skip_all, fields(correlation_id, amount = %sats.amount()), ret, err)]
    pub async fn get_cents_from_sats_for_immediate_sell(
        &self,
        sats: Sats,
    ) -> Result<UsdCents, PriceAppError> {
        let cents = UsdCents::from_decimal(
            self.price_mixer
                .apply(|p| *p.sell_usd().cents_from_sats(sats.clone()).amount())
                .await?,
        );
        Ok(self.fee_calculator.increase_by_immediate_fee(cents).ceil())
    }

    #[instrument(skip_all, fields(correlation_id, amount = %sats.amount()), ret, err)]
    pub async fn get_cents_from_sats_for_future_buy(
        &self,
        sats: Sats,
    ) -> Result<UsdCents, PriceAppError> {
        let cents = UsdCents::from_decimal(
            self.price_mixer
                .apply(|p| *p.buy_usd().cents_from_sats(sats.clone()).amount())
                .await?,
        );
        Ok(self.fee_calculator.decrease_by_delayed_fee(cents).floor())
    }

    #[instrument(skip_all, fields(correlation_id, amount = %sats.amount()), ret, err)]
    pub async fn get_cents_from_sats_for_future_sell(
        &self,
        sats: Sats,
    ) -> Result<UsdCents, PriceAppError> {
        let cents = UsdCents::from_decimal(
            self.price_mixer
                .apply(|p| *p.sell_usd().cents_from_sats(sats.clone()).amount())
                .await?,
        );
        Ok(self.fee_calculator.increase_by_delayed_fee(cents).ceil())
    }

    #[instrument(skip_all, fields(correlation_id, amount = %cents.amount()), ret, err)]
    pub async fn get_sats_from_cents_for_immediate_buy(
        &self,
        cents: UsdCents,
    ) -> Result<Sats, PriceAppError> {
        let sats = Sats::from_decimal(
            self.price_mixer
                .apply(|p| *p.buy_usd().sats_from_cents(cents.clone()).amount())
                .await?,
        );
        Ok(self.fee_calculator.increase_by_immediate_fee(sats).ceil())
    }

    #[instrument(skip_all, fields(correlation_id, amount = %cents.amount()), ret, err)]
    pub async fn get_sats_from_cents_for_immediate_sell(
        &self,
        cents: UsdCents,
    ) -> Result<Sats, PriceAppError> {
        let sats = Sats::from_decimal(
            self.price_mixer
                .apply(|p| *p.sell_usd().sats_from_cents(cents.clone()).amount())
                .await?,
        );

        Ok(self.fee_calculator.decrease_by_immediate_fee(sats).floor())
    }

    #[instrument(skip_all, fields(correlation_id, amount = %cents.amount()), ret, err)]
    pub async fn get_sats_from_cents_for_future_buy(
        &self,
        cents: UsdCents,
    ) -> Result<Sats, PriceAppError> {
        let sats = Sats::from_decimal(
            self.price_mixer
                .apply(|p| *p.buy_usd().sats_from_cents(cents.clone()).amount())
                .await?,
        );

        Ok(self.fee_calculator.increase_by_delayed_fee(sats).ceil())
    }

    #[instrument(skip_all, fields(correlation_id, amount = %cents.amount()), ret, err)]
    pub async fn get_sats_from_cents_for_future_sell(
        &self,
        cents: UsdCents,
    ) -> Result<Sats, PriceAppError> {
        let sats = Sats::from_decimal(
            self.price_mixer
                .apply(|p| *p.sell_usd().sats_from_cents(cents.clone()).amount())
                .await?,
        );
        Ok(self.fee_calculator.decrease_by_delayed_fee(sats).floor())
    }

    #[instrument(skip_all, fields(correlation_id), ret, err)]
    pub async fn get_cents_per_sat_exchange_mid_rate(&self) -> Result<f64, PriceAppError> {
        let cents_per_sat = self
            .price_mixer
            .apply(|p| *p.mid_price_of_one_sat().amount())
            .await?;
        Ok(f64::try_from(cents_per_sat)?)
    }
}
