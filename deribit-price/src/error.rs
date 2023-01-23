use serde_json::Error as SerdeError;
use thiserror::Error;
use tokio::sync::broadcast::error::SendError;
use tokio_tungstenite::tungstenite::error::Error as TungsteniteError;

use shared::{
    payload::*,
    pubsub::{Envelope, PublisherError},
};

#[allow(clippy::large_enum_variant)]
#[derive(Error, Debug)]
pub enum PriceFeedError {
    #[error("DeribitPriceFeedError - SerdeError: {0}")]
    SerializationError(#[from] SerdeError),

    #[error("DeribitPriceFeedError - PublisherError: {0}")]
    PublisherError(#[from] PublisherError),

    #[error("DeribitPriceFeedError - TungsteniteError: {0}")]
    TungsteniteError(#[from] TungsteniteError),

    #[error("PriceFeedError - PricePublish: {0}")]
    PricePublish(#[from] SendError<Envelope<PriceStreamPayload>>),

    #[error("PriceFeedError - EmptyPriceData: DeribitPriceTick.data was empty")]
    EmptyPriceData,
}
