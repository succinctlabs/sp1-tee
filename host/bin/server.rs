use axum::{
    extract::State,
    response::{
        sse::{Event, Sse},
    },
    routing::{get, post},
    Json, Router,
};
use clap::Parser;
use sp1_tee_common::{EnclaveRequest, EnclaveResponse};
use sp1_tee_host::{
    api::{EventPayload, GetAddressResponse},
    server::{Server, ServerArgs, ServerError},
};
use sp1_tee_host::{
    api::{TEERequest, TEEResponse},
    HostStream,
};
use std::sync::Arc;
use std::{convert::Infallible, str::FromStr};
use tokio::net::TcpListener;
use tracing_subscriber::EnvFilter;

use futures::stream::{self, Stream, StreamExt};

/// Initialize the tracing subscriber.
///
/// The default filter is `sp1-tee-server=debug,info`.
fn init_tracing() {
    let default_env_filter = EnvFilter::try_from_default_env().unwrap_or(
        EnvFilter::from_str("sp1_tee_server=debug,sp1_tee_host=debug,info")
            .expect("Failed to server default env filter"),
    );

    tracing_subscriber::fmt()
        .with_env_filter(default_env_filter)
        .with_line_number(true)
        .with_file(true)
        .init();
}

#[tokio::main]
async fn main() {
    init_tracing();

    let args = ServerArgs::parse();

    // Start the server.
    //
    // This function also starts the enclave and spawns a task to save attestations to S3.
    let server = Server::new(&args);

    let app = Router::new()
        .route("/execute", post(execute))
        .route("/address", get(get_address))
        .with_state(server);

    let listener = TcpListener::bind((args.address.clone(), args.port))
        .await
        .expect("Failed to bind to address");

    tracing::info!("Listening on {}:{}", args.address, args.port);

    // Run the server indefinitely or wait for a Ctrl-C.
    tokio::select! {
        e = axum::serve(listener, app.into_make_service()) => {
            if let Err(e) = e {
                tracing::error!("Server error: {}", e);
            }
        }
        _ = tokio::signal::ctrl_c() => {
            tracing::info!("Ctrl-C received, terminating enclaves");

            sp1_tee_host::server::terminate_enclaves();
            std::process::exit(0);
        }
    }
}

async fn get_address(
    State(server): State<Arc<Server>>,
) -> Result<Json<GetAddressResponse>, ServerError> {
    let mut stream = HostStream::new(server.cid, sp1_tee_common::ENCLAVE_PORT)
        .await
        .map_err(|e| {
            tracing::error!("Failed to connect to enclave: {}", e);

            ServerError::FailedToConnectToEnclave
        })?;

    stream
        .send(EnclaveRequest::GetPublicKey)
        .await
        .map_err(|e| {
            tracing::error!("Failed to send request to enclave: {}", e);

            ServerError::FailedToSendRequestToEnclave
        })?;

    let response = stream.recv().await.map_err(|e| {
        tracing::error!("Failed to receive response from enclave: {}", e);

        ServerError::FailedToReceiveResponseFromEnclave
    })?;

    match response {
        EnclaveResponse::PublicKey(public_key) => {
            let Some(address) = sp1_tee_host::ethereum_address_from_encoded_point(&public_key)
            else {
                tracing::error!("Failed to convert public key to address");

                return Err(ServerError::FailedToConvertPublicKeyToAddress);
            };

            Ok(Json(GetAddressResponse { address }))
        }
        _ => {
            tracing::error!("Unexpected response from enclave: {:?}", response);

            Err(ServerError::UnexpectedResponseFromEnclave)
        }
    }
}

/// Execute a program on the enclave.
///
/// In order to avoid OOM in the enclave, we run only one program at a time.
#[tracing::instrument(skip_all, fields(id = ?request.id))]
async fn execute(
    State(server): State<Arc<Server>>,
    Json(request): Json<TEERequest>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let _guard = server.execution_mutex.lock().await;

    tracing::info!("Executing request");

    let response = execute_inner(server.clone(), request);
    let response =
        stream::once(response).map(|response| Ok(sp1_tee_host::api::result_to_event(response)));

    Sse::new(response)
}

async fn execute_inner(
    server: Arc<Server>,
    request: TEERequest,
) -> Result<TEEResponse, ServerError> {
    // Open a connection to the enclave.
    let mut stream = HostStream::new(server.cid, sp1_tee_common::ENCLAVE_PORT)
        .await
        .map_err(|e| {
            tracing::error!("Failed to connect to enclave: {}", e);
            ServerError::FailedToConnectToEnclave
        })?;

    tracing::debug!("Successfully connected to enclave");

    // Setup the request.
    let request = EnclaveRequest::Execute {
        program: request.program,
        stdin: request.stdin,
    };

    // Send the request to the enclave.
    stream.send(request).await.map_err(|e| {
        tracing::error!("Failed to send request to enclave: {}", e);
        ServerError::FailedToSendRequestToEnclave
    })?;

    tracing::debug!("Successfully sent request to enclave");

    // Receive the response from the enclave.
    let response = stream.recv().await.map_err(|e| {
        tracing::error!("Failed to receive response from enclave: {:?}", e);

        ServerError::FailedToReceiveResponseFromEnclave
    })?;

    tracing::debug!("Successfully received response from enclave");

    match response {
        EnclaveResponse::SignedPublicValues {
            vkey,
            public_values,
            signature,
            recovery_id,
        } => {
            Ok(TEEResponse {
                vkey,
                public_values,
                signature,
                // Add 27 to the recovery id, as this is required by Ethereum.
                recovery_id: recovery_id + 27,
            })
        }
        EnclaveResponse::Error(error) => {
            tracing::error!("Error from enclave: {:?}", error);

            Err(ServerError::EnclaveError(error))
        }
        _ => {
            tracing::error!("Unexpected response from enclave: {:?}", response);

            Err(ServerError::UnexpectedResponseFromEnclave)
        }
    }
}
