pub use sp1_sdk::network::tee::api::{EventPayload, GetAddressResponse, TEERequest, TEEResponse};

#[cfg(feature = "server")]
use {crate::server::ServerError, axum::response::sse::Event};

#[cfg(feature = "server")]
pub(crate) fn event_payload_to_event(payload: EventPayload) -> Event {
    Event::default().data(hex::encode(
        bincode::serialize(&payload).expect("Failed to serialize response"),
    ))
}

#[cfg(feature = "server")]
pub(crate) fn result_to_event_payload(response: Result<TEEResponse, ServerError>) -> EventPayload {
    match response {
        Ok(response) => EventPayload::Success(response),
        Err(error) => EventPayload::Error(error.to_string()),
    }
}

#[cfg(feature = "server")]
pub fn result_to_event(response: Result<TEEResponse, ServerError>) -> Event {
    event_payload_to_event(result_to_event_payload(response))
}
