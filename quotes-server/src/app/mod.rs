mod config;

use chrono::{DateTime, Utc};
use futures::stream::StreamExt;
use rust_decimal::Decimal;
use tracing::{info_span, Instrument};

use shared::{
    health::HealthCheckTrigger,
    payload::{PriceStreamPayload, BITFINEX_EXCHANGE_ID, OKEX_EXCHANGE_ID},
    pubsub::*,
};

use ledger::*;

use crate::{cache::*, currency::*, error::*, price::*, quote::*};
pub use config::*;

pub struct QuotesApp {
    price_calculator: PriceCalculator,
    quotes: Quotes,
    ledger: Ledger,
    pool: sqlx::PgPool,
}

impl QuotesApp {
    pub async fn run(
        mut health_check_trigger: HealthCheckTrigger,
        health_check_cfg: QuotesServerHealthCheckConfig,
        fee_calc_cfg: QuotesFeeCalculatorConfig,
        subscriber: memory::Subscriber<PriceStreamPayload>,
        price_cache_config: QuotesExchangePriceCacheConfig,
        exchange_weights: ExchangeWeights,
        pool: sqlx::PgPool,
    ) -> Result<Self, QuotesAppError> {
        let health_subscriber = subscriber.resubscribe();
        tokio::spawn(async move {
            while let Some(check) = health_check_trigger.next().await {
                let _ = check.send(
                    health_subscriber
                        .healthy(health_check_cfg.unhealthy_msg_interval_price)
                        .await,
                );
            }
        });

        let mut price_mixer = PriceMixer::new();

        if let Some(weight) = exchange_weights.okex {
            if weight > Decimal::ZERO {
                let okex_order_book_cache = OrderBookCache::new(price_cache_config.clone());
                Self::subscribe_okex(subscriber.resubscribe(), okex_order_book_cache.clone())
                    .await?;
                price_mixer.add_provider(OKEX_EXCHANGE_ID, okex_order_book_cache, weight);
            }
        }

        if let Some(weight) = exchange_weights.bitfinex {
            if weight > Decimal::ZERO {
                let bitfinex_price_cache = ExchangeTickCache::new(price_cache_config.clone());
                Self::subscribe_bitfinex(subscriber.resubscribe(), bitfinex_price_cache.clone())
                    .await?;
                price_mixer.add_provider(BITFINEX_EXCHANGE_ID, bitfinex_price_cache, weight);
            }
        }

        let quotes = Quotes::new(&pool);
        let ledger = Ledger::init(&pool).await?;

        Ok(Self {
            price_calculator: PriceCalculator::new(fee_calc_cfg, price_mixer),
            quotes,
            ledger,
            pool,
        })
    }

    pub async fn quote_cents_from_sats_for_buy(
        &self,
        sats: Decimal,
        immediate_execution: bool,
    ) -> Result<Quote, QuotesAppError> {
        let sats = Satoshis::from(sats);
        let usd_amount = self
            .price_calculator
            .cents_from_sats_for_buy(sats.clone(), immediate_execution)
            .await?;
        let new_quote = NewQuote::builder()
            .direction(Direction::BuyCents)
            .immediate_execution(immediate_execution)
            .cent_amount(usd_amount)
            .sat_amount(sats)
            .expires_at(default_expiration_time()) //hardcoded for now
            .build()
            .expect("Could not build quote");
        let quote = self.quotes.create(new_quote).await?;
        if immediate_execution {
            self.accept_quote(quote.id).await?;
        }

        Ok(quote)
    }

    pub async fn quote_cents_from_sats_for_sell(
        &self,
        sats: Decimal,
        immediate_execution: bool,
    ) -> Result<Quote, QuotesAppError> {
        let sats = Satoshis::from(sats);
        let usd_amount = self
            .price_calculator
            .cents_from_sats_for_sell(sats.clone(), immediate_execution)
            .await?;
        let new_quote = NewQuote::builder()
            .direction(Direction::SellCents)
            .immediate_execution(immediate_execution)
            .cent_amount(usd_amount)
            .sat_amount(sats)
            .expires_at(default_expiration_time()) //hardcoded for now
            .build()
            .expect("Could not build quote");
        let quote = self.quotes.create(new_quote).await?;
        if immediate_execution {
            self.accept_quote(quote.id).await?;
        }

        Ok(quote)
    }

    pub async fn quote_sats_from_cents_for_sell(
        &self,
        cents: Decimal,
        immediate_execution: bool,
    ) -> Result<Quote, QuotesAppError> {
        let cents = UsdCents::from(cents);
        let sat_amount = self
            .price_calculator
            .sats_from_cents_for_sell(cents.clone(), immediate_execution)
            .await?;
        let new_quote = NewQuote::builder()
            .direction(Direction::SellCents)
            .immediate_execution(immediate_execution)
            .cent_amount(cents)
            .sat_amount(sat_amount)
            .expires_at(default_expiration_time()) //hardcoded for now
            .build()
            .expect("Could not build quote");
        let quote = self.quotes.create(new_quote).await?;
        if immediate_execution {
            self.accept_quote(quote.id).await?;
        }

        Ok(quote)
    }

    pub async fn quote_sats_from_cents_for_buy(
        &self,
        cents: Decimal,
        immediate_execution: bool,
    ) -> Result<Quote, QuotesAppError> {
        let cents = UsdCents::from(cents);
        let sat_amount = self
            .price_calculator
            .sats_from_cents_for_buy(cents.clone(), immediate_execution)
            .await?;
        let new_quote = NewQuote::builder()
            .direction(Direction::BuyCents)
            .immediate_execution(immediate_execution)
            .cent_amount(cents)
            .sat_amount(sat_amount)
            .expires_at(default_expiration_time()) //hardcoded for now
            .build()
            .expect("Could not build quote");
        let quote = self.quotes.create(new_quote).await?;
        if immediate_execution {
            self.accept_quote(quote.id).await?;
        }

        Ok(quote)
    }

    pub async fn accept_quote(&self, id: QuoteId) -> Result<(), QuotesAppError> {
        let mut quote = self.quotes.find_by_id(id).await?;
        if !quote.is_accepted() {
            quote.accept();
            let tx = self.pool.begin().await?;
            if quote.direction == Direction::SellCents {
                self.ledger
                    .sell_usd_quote_accepted(
                        tx,
                        LedgerTxId::new(),
                        SellUsdQuoteAcceptedParams {
                            usd_cents_amount: Decimal::from(quote.cent_amount.clone()),
                            satoshi_amount: Decimal::from(quote.sat_amount.clone()),
                            meta: SellUsdQuoteAcceptedMeta {
                                timestamp: Utc::now(),
                            },
                        },
                    )
                    .await?;
            } else {
                self.ledger
                    .buy_usd_quote_accepted(
                        tx,
                        LedgerTxId::new(),
                        BuyUsdQuoteAcceptedParams {
                            usd_cents_amount: Decimal::from(quote.cent_amount.clone()),
                            satoshi_amount: Decimal::from(quote.sat_amount.clone()),
                            meta: BuyUsdQuoteAcceptedMeta {
                                timestamp: Utc::now(),
                            },
                        },
                    )
                    .await?;
            }
            self.quotes.update(quote).await?;
        }
        Ok(())
    }

    async fn subscribe_okex(
        mut subscriber: memory::Subscriber<PriceStreamPayload>,
        order_book_cache: OrderBookCache,
    ) -> Result<(), QuotesAppError> {
        tokio::spawn(async move {
            while let Some(msg) = subscriber.next().await {
                if let PriceStreamPayload::OkexBtcUsdSwapOrderBookPayload(price_msg) = msg.payload {
                    let span = info_span!(
                        "quotes_server.okex_order_book_received",
                        message_type = %msg.payload_type,
                        correlation_id = %msg.meta.correlation_id
                    );
                    shared::tracing::inject_tracing_data(&span, &msg.meta.tracing_data);
                    async {
                        order_book_cache.apply_update(price_msg).await;
                    }
                    .instrument(span)
                    .await;
                }
            }
        });

        Ok(())
    }

    async fn subscribe_bitfinex(
        mut subscriber: memory::Subscriber<PriceStreamPayload>,
        price_cache: ExchangeTickCache,
    ) -> Result<(), QuotesAppError> {
        tokio::spawn(async move {
            while let Some(msg) = subscriber.next().await {
                if let PriceStreamPayload::BitfinexBtcUsdSwapPricePayload(price_msg) = msg.payload {
                    let span = info_span!(
                        "quotes_server.bitfinex_price_tick_received",
                        message_type = %msg.payload_type,
                        correlation_id = %msg.meta.correlation_id
                    );
                    shared::tracing::inject_tracing_data(&span, &msg.meta.tracing_data);
                    async {
                        price_cache
                            .apply_update(price_msg, msg.meta.correlation_id)
                            .await;
                    }
                    .instrument(span)
                    .await;
                }
            }
        });

        Ok(())
    }
}

// helper fn. remove later
fn default_expiration_time() -> DateTime<Utc> {
    Utc::now() + chrono::Duration::minutes(2)
}
