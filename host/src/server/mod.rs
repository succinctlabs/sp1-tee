use std::{sync::Arc, time::Duration};

use clap::Parser;

use axum::{http::StatusCode, response::IntoResponse};

#[derive(Debug, thiserror::Error)]
pub enum ServerError {
    #[error("Failed to connect to enclave")]
    FailedToConnectToEnclave,

    #[error("Failed to send request to enclave")]
    FailedToSendRequestToEnclave,

    #[error("Failed to receive response from enclave")]
    FailedToReceiveResponseFromEnclave,

    #[error("Unexpected response from enclave")]
    UnexpectedResponseFromEnclave,

    #[error("Enclave error: {0}")]
    EnclaveError(String),

    #[error("Stdin is too large, found {0} bytes")]
    StdinTooLarge(usize),

    #[error("Program is too large, found {0} bytes")]
    ProgramTooLarge(usize),
}

impl IntoResponse for ServerError {
    fn into_response(self) -> axum::response::Response {
        match self {
            ServerError::StdinTooLarge(_) => {
                (StatusCode::PAYLOAD_TOO_LARGE, self.to_string()).into_response()
            }
            ServerError::ProgramTooLarge(_) => {
                (StatusCode::PAYLOAD_TOO_LARGE, self.to_string()).into_response()
            }
            ServerError::EnclaveError(_) => {
                (StatusCode::BAD_REQUEST, self.to_string()).into_response()
            }
            _ => (StatusCode::INTERNAL_SERVER_ERROR, self.to_string()).into_response(),
        }
    }
}

pub struct Server {
    pub execution_mutex: tokio::sync::Mutex<()>,
    pub cid: u32,
}

impl Server {
    pub fn new(cid: u32) -> Arc<Self> {
        Arc::new(Self {
            execution_mutex: tokio::sync::Mutex::new(()),
            cid,
        })
    }
}

#[derive(Parser)]
pub struct ServerArgs {
    /// The port to listen on.
    #[clap(short, long, default_value = "3000")]
    pub port: u16,

    /// The address to listen on.
    #[clap(short, long, default_value = "0.0.0.0")]
    pub address: String,

    /// The CID and port of the enclave to connect to.
    #[clap(long, default_value = "10")]
    pub enclave_cid: u32,

    /// The number of cores to use for the enclave.
    #[clap(long, default_value = "2")]
    pub enclave_cores: u32,

    /// The memory to use for the enclave.
    #[clap(short, long, default_value = "5000")]
    pub enclave_memory: u32,

    /// Run the enclave in debug mode.
    #[clap(short, long)]
    pub debug: bool,
}

/// Start the enclave.
/// 
/// This function will block until the enclave is started or force the program to exit with an error code.
pub fn start_enclave(args: &ServerArgs) {
    // Run the enclave.sh script.
    let mut command = std::process::Command::new("sh");
    command.current_dir("../");
    command.arg("enclave.sh");
    command.arg("run");
    if args.debug {
        command.arg("--debug");
    }

    // Set the environment variables.
    command.env("ENCLAVE_CID", args.enclave_cid.to_string());
    command.env("ENCLAVE_CPU_COUNT", args.enclave_cores.to_string());
    command.env("ENCLAVE_MEMORY", args.enclave_memory.to_string());

    // Pipe the output to the parent process.
    command.stdout(std::process::Stdio::inherit());
    command.stderr(std::process::Stdio::inherit());

    let output = command.output().expect("Failed to run enclave.sh");
    if !output.status.success() {
        tracing::error!("Failed to start enclave");
        std::process::exit(1);
    }

    tracing::info!(
        "Enclave started on CID: {} with {} cores and {}MB of memory",
        args.enclave_cid,
        args.enclave_cores,
        args.enclave_memory
    );
}

/// Spawn a task that will save attestations to S3.
/// 
/// This function will run until the program is killed.
pub fn spawn_attestation_task(cid: u32, port: u32, interval: Duration) {
    tokio::spawn(async move {
        loop {
            if let Err(e) = crate::attestations::save_attestation(
                crate::attestations::SaveAttestationArgs {
                    cid,
                    port,
                    ..Default::default()
                },
            )
            .await
            {
                tracing::error!("Failed to save attestation: {}", e);
            }

            tokio::time::sleep(interval).await;
        }
    });
}
