use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx_ledger::{tx_template::*, SqlxLedger, SqlxLedgerError};
use tracing::instrument;

use crate::{constants::*, error::*};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IncreaseExchangeAllocationMeta {
    #[serde(with = "chrono::serde::ts_seconds")]
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct IncreaseExchangeAllocationParams {
    pub okex_allocation_usd_cents_amount: Decimal,
    pub meta: IncreaseExchangeAllocationMeta,
}

impl IncreaseExchangeAllocationParams {
    pub fn defs() -> Vec<ParamDefinition> {
        vec![
            ParamDefinition::builder()
                .name("usd_amount")
                .r#type(ParamDataType::DECIMAL)
                .build()
                .unwrap(),
            ParamDefinition::builder()
                .name("meta")
                .r#type(ParamDataType::JSON)
                .build()
                .unwrap(),
            ParamDefinition::builder()
                .name("effective")
                .r#type(ParamDataType::DATE)
                .build()
                .unwrap(),
        ]
    }
}

impl From<IncreaseExchangeAllocationParams> for TxParams {
    fn from(
        IncreaseExchangeAllocationParams {
            okex_allocation_usd_cents_amount,
            meta,
        }: IncreaseExchangeAllocationParams,
    ) -> Self {
        let effective = meta.timestamp.naive_utc().date();
        let meta = serde_json::to_value(meta).expect("Couldn't serialize meta");
        let mut params = Self::default();
        params.insert(
            "usd_amount",
            okex_allocation_usd_cents_amount / CENTS_PER_USD,
        );
        params.insert("meta", meta);
        params.insert("effective", effective);
        params
    }
}

pub struct IncreaseExchangeAllocation {}

impl IncreaseExchangeAllocation {
    #[instrument(name = "ledger.increase_exchange_allocation.init", skip_all)]
    pub async fn init(ledger: &SqlxLedger) -> Result<(), LedgerError> {
        let tx_input = TxInput::builder()
            .journal_id(format!("uuid('{STABLESATS_JOURNAL_ID}')"))
            .effective("params.effective")
            .metadata("params.meta")
            .description("'Increase exchange allocation'")
            .build()
            .expect("Couldn't build TxInput");

        let entries = vec![
            EntryInput::builder()
                .entry_type("'INCREASE_EXCHANGE_ALLOCATION_USD_CR'")
                .currency("'USD'")
                .account_id(format!("uuid('{OKEX_ALLOCATION_ID}')"))
                .direction("CREDIT")
                .layer("SETTLED")
                .units("params.usd_amount")
                .build()
                .expect("Couldn't build INCREASE_EXCHANGE_ALLOCATION_USD_CR entry"),
            EntryInput::builder()
                .entry_type("'INCREASE_EXCHANGE_ALLOCATION_USD_DR'")
                .currency("'USD'")
                .account_id(format!("uuid('{STABLESATS_LIABILITY_ID}')"))
                .direction("DEBIT")
                .layer("SETTLED")
                .units("params.usd_amount")
                .build()
                .expect("Couldn't build INCREASE_EXCHANGE_ALLOCATION_USD_DR entry"),
        ];

        let params = IncreaseExchangeAllocationParams::defs();
        let template = NewTxTemplate::builder()
            .id(INCREASE_EXCHANGE_ALLOCATION_ID)
            .code(INCREASE_EXCHANGE_ALLOCATION_CODE)
            .tx_input(tx_input)
            .entries(entries)
            .params(params)
            .build()
            .expect("Couldn't build INCREASE_EXCHANGE_ALLOCATION_CODE");

        match ledger.tx_templates().create(template).await {
            Ok(_) | Err(SqlxLedgerError::DuplicateKey(_)) => Ok(()),
            Err(e) => Err(e.into()),
        }
    }
}
