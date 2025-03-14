use axum::{
    body::Bytes,
    extract::{DefaultBodyLimit, State},
    response::sse::{Event, Sse},
    routing::{get, post},
    Json, Router,
};
use clap::Parser;
use sp1_tee_common::{EnclaveRequest, EnclaveResponse};
use sp1_tee_host::{
    api::GetAddressResponse,
    server::{Server, ServerArgs, ServerError},
};
use sp1_tee_host::{
    api::{TEERequest, TEEResponse},
    HostStream,
};
use std::convert::Infallible;
use std::sync::Arc;
use tokio::net::TcpListener;

use futures::stream::{self, Stream, StreamExt};

#[tokio::main]
async fn main() {
    sp1_tee_host::init_tracing();

    let args = ServerArgs::parse();

    // First, kill any existing enclaves.
    //
    // Just in case the server was killed uncleanly last time.
    sp1_tee_host::server::terminate_enclaves();

    // Start the server.
    //
    // This function also starts the enclave and spawns a task to save attestations to S3.
    let server = Server::new(&args);

    let app = Router::new()
        .route("/execute", post(execute).layer(DefaultBodyLimit::disable()))
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
    tracing::debug!("Handling get address request");

    let mut stream = HostStream::new(server.cid, sp1_tee_common::ENCLAVE_PORT)
        .await
        .map_err(|e| {
            tracing::error!(alert = true, "Failed to connect to enclave: {}", e);

            ServerError::FailedToConnectToEnclave
        })?;

    stream
        .send(EnclaveRequest::GetPublicKey)
        .await
        .map_err(|e| {
            tracing::error!(alert = true, "Failed to send request to enclave: {}", e);

            ServerError::FailedToSendRequestToEnclave
        })?;

    let response = stream.recv().await.map_err(|e| {
        tracing::error!(
            alert = true,
            "Failed to receive response from enclave: {}",
            e
        );

        ServerError::FailedToReceiveResponseFromEnclave
    })?;

    match response {
        EnclaveResponse::PublicKey(public_key) => {
            let Some(address) = sp1_tee_host::ethereum_address_from_encoded_point(&public_key)
            else {
                tracing::error!(alert = true, "Failed to convert public key to address");

                return Err(ServerError::FailedToConvertPublicKeyToAddress);
            };

            Ok(Json(GetAddressResponse { address }))
        }
        _ => {
            tracing::error!(
                alert = true,
                "Unexpected response from enclave: {:?}",
                response
            );

            Err(ServerError::UnexpectedResponseFromEnclave)
        }
    }
}

/// Execute a program on the enclave.
///
/// In order to avoid OOM in the enclave, we run only one program at a time.
async fn execute(
    State(server): State<Arc<Server>>,
    req: Bytes,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, ServerError> {
    let request = bincode::deserialize::<TEERequest>(&req).map_err(|e| {
        tracing::error!("Failed to deserialize request: {}", e);

        ServerError::FailedToDeserializeRequest(e)
    })?;

    #[cfg(feature = "production")]
    {
        let signer = request
            .signature
            .recover_address_from_msg(request.id)
            .map_err(|_| {
                tracing::error!(
                    "Failed to recover signer address, request id: {}",
                    hex::encode(request.id)
                );

                ServerError::FailedToAuthenticateRequest
            })?;

        match server.auth_client.is_whitelisted(signer).await {
            Ok(true) => (),
            Ok(false) => {
                tracing::error!(
                    "Failed to authenticate request by {:?}: Not whitelisted",
                    signer
                );

                return Err(ServerError::FailedToAuthenticateRequest)
            },
            Err(e) => {
                tracing::error!(
                    alert = true,
                    "Failed to authenticate request by {:?}: {}",
                    signer,
                    e
                );

                return Err(ServerError::FailedToAuthenticateRequest);
            }
        }
    }

    let response = execute_inner(server.clone(), request);
    let response =
        stream::once(response).map(|response| Ok(sp1_tee_host::api::result_to_event(response)));

    Ok(Sse::new(response))
}

#[tracing::instrument(skip_all, fields(id = hex::encode(request.id)))]
async fn execute_inner(
    server: Arc<Server>,
    request: TEERequest,
) -> Result<TEEResponse, ServerError> {
    tracing::info!("Got execution request");

    let _guard = server.execution_mutex.lock().await;

    tracing::info!("Acquired execution gurad");

    // Open a connection to the enclave.
    let mut stream = HostStream::new(server.cid, sp1_tee_common::ENCLAVE_PORT)
        .await
        .map_err(|e| {
            tracing::error!(alert = true, "Failed to connect to enclave: {}", e);

            ServerError::FailedToConnectToEnclave
        })?;

    tracing::debug!("Successfully connected to enclave");

    // Setup the request.
    let request = EnclaveRequest::Execute {
        program: request.program,
        stdin: request.stdin,
    };

    // Send the request to the enclave.
    let execution_start = std::time::Instant::now();
    stream.send(request).await.map_err(|e| {
        tracing::error!(alert = true, "Failed to send request to enclave: {}", e);

        ServerError::FailedToSendRequestToEnclave
    })?;

    tracing::debug!("Successfully sent request to enclave");

    // Receive the response from the enclave.
    let response = stream.recv().await.map_err(|e| {
        tracing::error!(
            alert = true,
            "Failed to receive response from enclave: {:?}",
            e
        );

        ServerError::FailedToReceiveResponseFromEnclave
    })?;

    let execution_duration = execution_start.elapsed();
    tracing::info!("Execution duration: {:?} seconds", execution_duration.as_secs());

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
            // This error type is expected, it can happen if the execution fails.
            tracing::error!("Error during execution from enclave: {:?}", error);

            Err(ServerError::EnclaveError(error))
        }
        _ => {
            tracing::error!(
                alert = true,
                "Unexpected response from enclave: {:?}",
                response
            );

            Err(ServerError::UnexpectedResponseFromEnclave)
        }
    }
}
