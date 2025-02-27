use serde::{Deserialize, Serialize};
use sp1_sdk::SP1Stdin;

use k256::ecdsa::Signature;

#[cfg(feature = "server")]
use {
    crate::server::ServerError,
    axum::response::sse::Event,
};

#[derive(Debug, Serialize, Deserialize)]
pub struct TEERequest {
    pub id: [u8; 32],
    pub program: Vec<u8>,
    pub stdin: SP1Stdin,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TEEResponse {
    pub vkey: [u8; 32],
    pub public_values: Vec<u8>,
    pub signature: Signature,
    pub recovery_id: u8,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum EventPayload {
    Success(TEEResponse),
    Error(String),
}

#[cfg(feature = "server")]
impl EventPayload {
    pub fn to_event(self) -> Event {
        Event::default().data(serde_json::to_string(&self).expect("Failed to serialize response"))
    }
}

#[cfg(feature = "server")]
impl From<Result<TEEResponse, ServerError>> for EventPayload {
    fn from(response: Result<TEEResponse, ServerError>) -> Self {
        match response {
            Ok(response) => Self::Success(response),
            Err(error) => Self::Error(error.to_string()),
        }
    }
}
