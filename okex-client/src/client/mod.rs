mod error;
mod okex_response;

use std::collections::HashMap;

use chrono::{SecondsFormat, Utc};
use data_encoding::BASE64;
use ring::hmac;

use reqwest::header::{HeaderMap, HeaderValue, CONTENT_TYPE};
use reqwest::Client as ReqwestClient;

pub use error::*;
use okex_response::*;

const OKEX_API_URL: &str = "https://www.okex.com";
pub const OKEX_MINIMUM_WITHDRAWAL_AMOUNT: f64 = 0.001;
pub const OKEX_MINIMUM_WITHDRAWAL_FEE: f64 = 0.0002;

#[derive(Debug, PartialEq)]
pub struct DepositAddress {
    pub value: String,
}

#[derive(Debug)]
pub struct TransferId {
    pub value: String,
}

#[derive(Debug)]
pub struct AvailableBalance {
    pub value: String,
}

#[derive(Debug)]
pub struct TransferState {
    pub value: String,
}

#[derive(Debug)]
pub struct WithdrawId {
    pub value: String,
}

#[derive(Debug)]
pub struct OrderId {
    pub value: String,
}

pub struct OkexClientConfig {
    pub api_key: String,
    pub passphrase: String,
    pub secret_key: String,
}

pub struct OkexClient {
    client: ReqwestClient,
    config: OkexClientConfig,
}

impl OkexClient {
    pub fn new(config: OkexClientConfig) -> Self {
        Self {
            client: ReqwestClient::new(),
            config,
        }
    }

    pub async fn get_funding_deposit_address(&self) -> Result<DepositAddress, OkexClientError> {
        let request_path = "/api/v5/asset/deposit-address?ccy=BTC";
        let url = format!("{}{}", OKEX_API_URL, request_path);

        let timestamp = Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true);
        let pre_hash = format!("{}GET{}", timestamp, request_path);

        let headers = self.request_headers(timestamp.as_str(), pre_hash)?;

        let response = self.client.get(url).headers(headers).send().await?;

        let addr_data = Self::extract_response_data::<DepositAddressData>(response).await?;
        Ok(DepositAddress {
            value: addr_data.addr,
        })
    }

    pub async fn transfer_funding_to_trading(
        &self,
        amt: f64,
    ) -> Result<TransferId, OkexClientError> {
        let request_path = "/api/v5/asset/transfer";
        let url = format!("{}{}", OKEX_API_URL, request_path);

        let mut body: HashMap<String, String> = HashMap::new();
        body.insert("ccy".to_string(), "BTC".to_string());
        body.insert("amt".to_string(), amt.to_string());
        body.insert("from".to_string(), "6".to_string());
        body.insert("to".to_string(), "18".to_string());
        let request_body = serde_json::to_string(&body)?;

        let timestamp = Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true);
        let pre_hash = format!("{}POST{}{}", timestamp, request_path, request_body);

        let headers = self.request_headers(timestamp.as_str(), pre_hash)?;

        let response = self
            .client
            .post(url)
            .headers(headers)
            .body(request_body)
            .send()
            .await?;

        let transfer_data = Self::extract_response_data::<TransferData>(response).await?;
        Ok(TransferId {
            value: transfer_data.trans_id,
        })
    }

    pub async fn transfer_trading_to_funding(
        &self,
        amt: f64,
    ) -> Result<TransferId, OkexClientError> {
        let request_path = "/api/v5/asset/transfer";
        let url = format!("{}{}", OKEX_API_URL, request_path);

        let mut body: HashMap<String, String> = HashMap::new();
        body.insert("ccy".to_string(), "BTC".to_string());
        body.insert("amt".to_string(), amt.to_string());
        body.insert("from".to_string(), "18".to_string());
        body.insert("to".to_string(), "6".to_string());
        let request_body = serde_json::to_string(&body)?;

        let timestamp = Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true);
        let pre_hash = format!("{}POST{}{}", timestamp, request_path, request_body);

        let headers = self.request_headers(timestamp.as_str(), pre_hash)?;

        let response = self
            .client
            .post(url)
            .headers(headers)
            .body(request_body)
            .send()
            .await?;

        let transfer_data = Self::extract_response_data::<TransferData>(response).await?;
        Ok(TransferId {
            value: transfer_data.trans_id,
        })
    }

    pub async fn funding_account_balance(&self) -> Result<AvailableBalance, OkexClientError> {
        let request_path = "/api/v5/asset/balances?ccy=BTC";
        let url = format!("{}{}", OKEX_API_URL, request_path);

        let timestamp = Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true);
        let pre_hash = format!("{}GET{}", timestamp, request_path);

        let headers = self.request_headers(timestamp.as_str(), pre_hash)?;

        let response = self.client.get(url).headers(headers).send().await?;

        let funding_balance = Self::extract_response_data::<FundingBalanceData>(response).await?;
        Ok(AvailableBalance {
            value: funding_balance.avail_bal,
        })
    }

    pub async fn trading_account_balance(&self) -> Result<AvailableBalance, OkexClientError> {
        let request_path = "/api/v5/account/balance?ccy=BTC";
        let url = format!("{}{}", OKEX_API_URL, request_path);

        let timestamp = Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true);
        let pre_hash = format!("{}GET{}", timestamp, request_path);

        let headers = self.request_headers(timestamp.as_str(), pre_hash)?;

        let response = self.client.get(url).headers(headers).send().await?;

        let trading_balance = Self::extract_response_data::<TradingBalanceData>(response).await?;
        Ok(AvailableBalance {
            value: trading_balance.details[0].avail_bal.clone(),
        })
    }

    pub async fn transfer_state(
        &self,
        transfer_id: TransferId,
    ) -> Result<TransferState, OkexClientError> {
        let request_path = format!(
            "/api/v5/asset/transfer-state?ccy=BTC&transId={}",
            transfer_id.value
        );
        let url = format!("{}{}", OKEX_API_URL, request_path);

        let timestamp = Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true);
        let pre_hash = format!("{}GET{}", timestamp, request_path);

        let headers = self.request_headers(timestamp.as_str(), pre_hash)?;

        let response = self.client.get(url).headers(headers).send().await?;

        let state_data = Self::extract_response_data::<TransferStateData>(response).await?;

        Ok(TransferState {
            value: state_data.state,
        })
    }

    pub async fn withdraw_btc_onchain(
        &self,
        amt: f64,
        fee: f64,
        btc_address: String,
    ) -> Result<WithdrawId, OkexClientError> {
        let request_path = "/api/v5/asset/withdrawal?ccy=BTC";
        let url = format!("{}{}", OKEX_API_URL, request_path);

        let mut body: HashMap<String, String> = HashMap::new();
        body.insert("ccy".to_string(), "BTC".to_string());
        body.insert("amt".to_string(), amt.to_string());
        body.insert("dest".to_string(), "4".to_string());
        body.insert("fee".to_string(), fee.to_string());
        body.insert("chain".to_string(), "BTC-Bitcoin".to_string());
        body.insert("toAddr".to_string(), btc_address);
        let request_body = serde_json::to_string(&body)?;

        let timestamp = Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true);
        let pre_hash = format!("{}POST{}{}", timestamp, request_path, request_body);
        let headers = self.request_headers(timestamp.as_str(), pre_hash)?;

        let response = self
            .client
            .post(url)
            .headers(headers)
            .body(request_body)
            .send()
            .await?;

        let withdraw_data = Self::extract_response_data::<WithdrawData>(response).await?;

        Ok(WithdrawId {
            value: withdraw_data.wd_id,
        })
    }

    pub async fn place_order(
        &self,
        inst_id: String,
        trade_mode: String,
        side: String,
        pos_side: String,
        order_type: String,
        size: u64,
    ) -> Result<OrderId, OkexClientError> {
        let request_path = "/api/v5/trade/order";
        let url = format!("{}{}", OKEX_API_URL, request_path);

        let mut body: HashMap<String, String> = HashMap::new();
        body.insert("ccy".to_string(), "BTC".to_string());
        body.insert("instId".to_string(), inst_id);
        body.insert("tdMode".to_string(), trade_mode);
        body.insert("side".to_string(), side);
        body.insert("ordType".to_string(), order_type);
        body.insert("posSide".to_string(), pos_side);
        body.insert("sz".to_string(), size.to_string());
        let request_body = serde_json::to_string(&body)?;

        let timestamp = Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true);
        let pre_hash = format!("{}POST{}{}", timestamp, request_path, request_body);
        let headers = self.request_headers(timestamp.as_str(), pre_hash)?;

        let response = self
            .client
            .post(url)
            .headers(headers)
            .body(request_body)
            .send()
            .await?;

        let order_data = Self::extract_response_data::<OrderData>(response).await?;
        Ok(OrderId {
            value: order_data.ord_id,
        })
    }

    pub async fn position(&self) -> Result<String, OkexClientError> {
        let request_path = "/api/v5/account/positions?instId=BTC-USD-SWAP";
        let url = format!("{}{}", OKEX_API_URL, request_path);

        let timestamp = Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true);
        let pre_hash = format!("{}GET{}", timestamp, request_path);

        let headers = self.request_headers(timestamp.as_str(), pre_hash)?;

        let response = self.client.get(url).headers(headers).send().await?;

        let positions_data = Self::extract_response_data::<PositionData>(response).await?;

        Ok("position".to_string())
    }

    async fn extract_response_data<T: serde::de::DeserializeOwned>(
        response: reqwest::Response,
    ) -> Result<T, OkexClientError> {
        let response_text = response.text().await?;

        println!("{:#?}", response_text);

        let OkexResponse { code, msg, data } =
            serde_json::from_str::<OkexResponse<T>>(&response_text)?;
        if let Some(data) = data {
            if let Some(first) = data.into_iter().next() {
                return Ok(first);
            }
        }
        Err(OkexClientError::UnexpectedResponse { msg, code })
    }

    fn sign_okex_request(&self, pre_hash: String) -> String {
        let key = hmac::Key::new(hmac::HMAC_SHA256, self.config.secret_key.as_bytes());
        let signature = hmac::sign(&key, pre_hash.as_bytes());
        BASE64.encode(signature.as_ref())
    }

    fn request_headers(
        &self,
        formatted_timestamp: &str,
        pre_hash: String,
    ) -> Result<HeaderMap, OkexClientError> {
        let sign_base64 = self.sign_okex_request(pre_hash);

        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_str("application/json")?);
        headers.insert(
            "OK-ACCESS-KEY",
            HeaderValue::from_str(self.config.api_key.as_str())?,
        );
        headers.insert(
            "OK-ACCESS-SIGN",
            HeaderValue::from_str(sign_base64.as_str())?,
        );
        headers.insert(
            "OK-ACCESS-TIMESTAMP",
            HeaderValue::from_str(formatted_timestamp)?,
        );
        headers.insert(
            "OK-ACCESS-PASSPHRASE",
            HeaderValue::from_str(self.config.passphrase.as_str())?,
        );
        Ok(headers)
    }
}
