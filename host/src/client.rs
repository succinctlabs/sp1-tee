use reqwest::{Client as HttpClient, Error as HttpError};
use eventsource_stream::{Eventsource, EventStreamError};
use futures::stream::StreamExt;

use crate::api::{TEERequest, TEEResponse, EventPayload};

#[derive(Debug, thiserror::Error)]
pub enum ClientError {
    #[error("Http Error: {0}")]
    Http(#[from] HttpError),

    #[error("Event Error: {0}")]
    Event(#[from] EventStreamError<HttpError>),

    #[error("Failed to parse response: {0}")]
    Parse(#[from] serde_json::Error),

    #[error("Error recieved from server: {0}")]
    ServerError(String),

    #[error("No response received")]
    NoResponse,
}

/// Internally, the client uses [SSE](https://en.wikipedia.org/wiki/Server-sent_events) 
/// to receive the response from the host, without having to poll the server or worry about
/// the connection being closed.
pub struct Client {
    client: HttpClient,
    url: String,
}

impl Client {
    pub fn new(url: &str) -> Self {
        Self {
            client: HttpClient::new(),
            url: url.to_string(),
        }
    }

    pub async fn execute(&self, request: TEERequest) -> Result<TEEResponse, ClientError> {
        // Only one response is expected,
        // 
        // So take the first item returned from the "next".
        let payload: EventPayload = self.client.post(format!("{}/execute", self.url))
            .json(&request)
            .send()
            .await?
            .bytes_stream()
            .eventsource()
            .map(|event| {
                match event {
                    Ok(event) => Ok(serde_json::from_str(&event.data)?),
                    Err(e) => Err(ClientError::Event(e)),
                }
            })
            .next()
            .await
            .ok_or(ClientError::NoResponse)??;

        // Everything worked as expected, but the handle the case where execution failed.
        match payload {
            EventPayload::Success(response) => Ok(response),
            EventPayload::Error(error) => Err(ClientError::ServerError(error)),
        }
    }

    pub async fn get_address(&self) -> Result<Address, ClientError> {
        let response = self.client.get(format!("{}/address", self.url))
            .send()
            .await?
            .json::<GetAddressResponse>()
            .await?;

        Ok(response.address)
    }
}
