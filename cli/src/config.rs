use anyhow::Context;
use serde::{Deserialize, Serialize};
use std::path::Path;

use bria_client::BriaClientConfig;
use galoy_client::GaloyClientConfig;
use hedging::{ExchangesConfig, HedgingAppConfig};
use price_server::{
    ExchangePriceCacheConfig, FeeCalculatorConfig, PriceServerConfig, PriceServerHealthCheckConfig,
};
use user_trades::UserTradesConfig;

use super::{db::DbConfig, tracing::TracingConfig};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub db: DbConfig,
    #[serde(default)]
    pub tracing: TracingConfig,
    #[serde(default)]
    pub price_server: PriceServerWrapper,
    #[serde(default)]
    pub bitfinex_price_feed: BitfinexPriceFeedConfigWrapper,
    #[serde(default)]
    pub user_trades: UserTradesConfigWrapper,
    #[serde(default)]
    pub galoy: GaloyClientConfig,
    #[serde(default)]
    pub hedging: HedgingConfigWrapper,
    #[serde(default)]
    pub exchanges: ExchangesConfig,
    #[serde(default)]
    pub bria: BriaClientConfig,
}

pub struct EnvOverride {
    pub pg_con: String,
    pub okex_secret_key: String,
    pub okex_passphrase: String,
    pub galoy_phone_code: String,
    pub bitfinex_secret_key: String,
    pub bria_url: String,
    pub bria_key: String,
    pub bria_wallet_name: String,
    pub bria_payout_queue_name: String,
    pub bria_external_id: String,
}

impl Config {
    pub fn from_path(
        path: impl AsRef<Path>,
        EnvOverride {
            galoy_phone_code,
            okex_passphrase,
            okex_secret_key,
            pg_con: stablesats_pg_con,
            bitfinex_secret_key: _,
            bria_url,
            bria_key,
            bria_wallet_name,
            bria_payout_queue_name,
            bria_external_id,
        }: EnvOverride,
    ) -> anyhow::Result<Self> {
        let config_file = std::fs::read_to_string(path).context("Couldn't read config file")?;
        let mut config: Config =
            serde_yaml::from_str(&config_file).context("Couldn't parse config file")?;

        config.galoy.auth_code = galoy_phone_code;

        if let Some(okex) = config.exchanges.okex.as_mut() {
            okex.config.client.secret_key = okex_secret_key;
            okex.config.client.passphrase = okex_passphrase;
        };

        config.db.pg_con = stablesats_pg_con;
        config.bria.url = bria_url;
        config.bria.key = bria_key;
        config.bria.wallet_name = bria_wallet_name;
        config.bria.payout_queue_name = bria_payout_queue_name;
        config.bria.external_id = bria_external_id;
        Ok(config)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceServerWrapper {
    #[serde(default = "bool_true")]
    pub enabled: bool,
    #[serde(default)]
    pub health: PriceServerHealthCheckConfig,
    #[serde(default)]
    pub server: PriceServerConfig,
    #[serde(default)]
    pub fees: FeeCalculatorConfig,
    #[serde(default)]
    pub price_cache: ExchangePriceCacheConfig,
}
impl Default for PriceServerWrapper {
    fn default() -> Self {
        Self {
            enabled: true,
            server: PriceServerConfig::default(),
            health: PriceServerHealthCheckConfig::default(),
            fees: FeeCalculatorConfig::default(),
            price_cache: ExchangePriceCacheConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BitfinexPriceFeedConfigWrapper {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub config: bitfinex_price::PriceFeedConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserTradesConfigWrapper {
    #[serde(default = "bool_true")]
    pub enabled: bool,
    #[serde(default)]
    pub config: UserTradesConfig,
}
impl Default for UserTradesConfigWrapper {
    fn default() -> Self {
        Self {
            enabled: true,
            config: UserTradesConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HedgingConfigWrapper {
    #[serde(default = "bool_true")]
    pub enabled: bool,
    #[serde(default)]
    pub config: HedgingAppConfig,
}
impl Default for HedgingConfigWrapper {
    fn default() -> Self {
        Self {
            enabled: true,
            config: HedgingAppConfig::default(),
        }
    }
}

fn bool_true() -> bool {
    true
}
