use axum::{extract::State, routing::post, Json, Router};
use clap::Parser;
use sp1_tee_common::{EnclaveRequest, EnclaveResponse};
use sp1_tee_host::server::{Server, ServerArgs, ServerError};
use sp1_tee_host::{
    api::{TEERequest, TEEResponse},
    HostStream,
};
use std::{sync::Arc, time::Duration};
use tokio::net::TcpListener;

/// A VSOCK address is defined as the tuple of (CID, port).
///
/// So its OK to hardcode the port here.
const ENCLAVE_PORT: u32 = 5005;

// Resubmit the attestation every 12 hours
const ATTESTATION_INTERVAL: Duration = Duration::from_secs(12 * 60 * 60);

#[tokio::main]
async fn main() {
    // todo: improve beyond default.
    tracing_subscriber::fmt::init();

    let args = ServerArgs::parse();

    // Start the enclave.
    sp1_tee_host::server::start_enclave(&args);

    // Spawn the attestation task.
    sp1_tee_host::server::spawn_attestation_task(
        args.enclave_cid,
        ENCLAVE_PORT,
        ATTESTATION_INTERVAL,
    );

    let server = Server::new(args.enclave_cid);

    let app = Router::new()
        .route("/execute", post(execute))
        .with_state(server);

    let listener = TcpListener::bind((args.address.clone(), args.port))
        .await
        .expect("Failed to bind to address");

    tracing::info!("Listening on {}:{}", args.address, args.port);

    axum::serve(listener, app.into_make_service())
        .await
        .expect("Failed to serve");
}

/// Execute a program on the enclave.
///
/// In order to avoid OOM in the enclave, we run only one program at a time.
async fn execute(
    State(server): State<Arc<Server>>,
    Json(request): Json<TEERequest>,
) -> Result<Json<TEEResponse>, ServerError> {
    let _guard = server.execution_mutex.lock().await;

    // Open a connection to the enclave.
    let mut stream = HostStream::new(server.cid, ENCLAVE_PORT)
        .await
        .map_err(|e| {
            tracing::error!("Failed to connect to enclave: {}", e);
            ServerError::FailedToConnectToEnclave
        })?;

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

    // Receive the response from the enclave.
    let response = stream.recv().await.map_err(|e| {
        tracing::error!("Failed to receive response from enclave: {}", e);
        ServerError::FailedToReceiveResponseFromEnclave
    })?;

    match response {
        EnclaveResponse::SignedPublicValues {
            vkey,
            public_values,
            signature,
            recovery_id,
        } => {
            // Return the response.
            Ok(Json(TEEResponse {
                vkey,
                public_values,
                signature,
                recovery_id,
            }))
        }
        EnclaveResponse::Error(error) => {
            tracing::error!("Error from enclave: {}", error);
            Err(ServerError::EnclaveError(error))
        }
        _ => {
            tracing::error!("Unexpected response from enclave: {:?}", response);
            Err(ServerError::UnexpectedResponseFromEnclave)
        }
    }
}
