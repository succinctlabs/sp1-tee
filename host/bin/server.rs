use axum::{extract::State, routing::post, Json, Router};
use clap::Parser;
use sp1_tee_common::{EnclaveRequest, EnclaveResponse};
use sp1_tee_host::server::{Server, ServerArgs, ServerError};
use sp1_tee_host::{
    api::{TEERequest, TEEResponse},
    HostStream,
};
use std::sync::Arc;
use tokio::net::TcpListener;

#[tokio::main]
async fn main() {
    // todo: improve beyond default.
    tracing_subscriber::fmt::init();

    let args = ServerArgs::parse();

    // Start the server.
    //
    // This function also starts the enclave and spawns a task to save attestations to S3.
    let server = Server::new(&args);

    let app = Router::new()
        .route("/execute", post(execute))
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
        }
    }
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
    let mut stream = HostStream::new(server.cid, sp1_tee_common::ENCLAVE_PORT)
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
