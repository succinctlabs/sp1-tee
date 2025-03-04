use clap::Parser;
use std::{path::Path, sync::Arc, time::Duration};
use axum::{response::IntoResponse, http::StatusCode, response::Response};

pub mod stream;

/// The directory of the manifest file.
///
/// Used for locating the enclave.sh script.
const MANIFEST_DIR: &str = env!("CARGO_MANIFEST_DIR");

pub struct Server {
    pub execution_mutex: tokio::sync::Mutex<()>,
    pub cid: u32,
}

impl Server {
    /// Create a new server.
    ///
    /// This function will block and start the enclave and spawn a task to save attestations to S3.
    pub fn new(args: &ServerArgs) -> Arc<Self> {
        #[cfg(feature = "production")]
        {
            if args.debug {
                panic!("Debug mode is not allowed when the program is built for production.");
            }
        }

        // Blocking start the enclave.
        start_enclave(args);

        // Spawn a task to save attestations to S3.
        spawn_attestation_task(
            args.enclave_cid,
            sp1_tee_common::ENCLAVE_PORT,
            crate::attestations::ATTESTATION_INTERVAL,
        );

        Arc::new(Self {
            execution_mutex: tokio::sync::Mutex::new(()),
            cid: args.enclave_cid,
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
    #[clap(long, default_value_t = sp1_tee_common::ENCLAVE_CID)]
    pub enclave_cid: u32,

    /// The number of cores to use for the enclave.
    #[clap(long, default_value = "16")]
    pub enclave_cores: u32,

    /// The memory to use for the enclave.
    #[clap(short, long, default_value = "5000")]
    pub enclave_memory: u32,

    /// Run the enclave in debug mode.
    #[clap(short, long)]
    pub debug: bool,
}

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

    #[error("Failed to convert public key to address")]
    FailedToConvertPublicKeyToAddress,

    #[error("Enclave error: {0}")]
    EnclaveError(String),

    #[error("Stdin is too large, found {0} bytes")]
    StdinTooLarge(usize),

    #[error("Program is too large, found {0} bytes")]
    ProgramTooLarge(usize),
}

impl IntoResponse for ServerError {
    fn into_response(self) -> Response {
        match self {
            ServerError::FailedToConnectToEnclave => (StatusCode::INTERNAL_SERVER_ERROR, "Failed to connect to enclave").into_response(),
            ServerError::FailedToSendRequestToEnclave => (StatusCode::INTERNAL_SERVER_ERROR, "Failed to send request to enclave").into_response(),
            ServerError::FailedToReceiveResponseFromEnclave => (StatusCode::INTERNAL_SERVER_ERROR, "Failed to receive response from enclave").into_response(),
            ServerError::UnexpectedResponseFromEnclave => (StatusCode::INTERNAL_SERVER_ERROR, "Unexpected response from enclave").into_response(),
            ServerError::FailedToConvertPublicKeyToAddress => (StatusCode::INTERNAL_SERVER_ERROR, "Failed to convert public key to address, this is a bug.").into_response(),
            ServerError::EnclaveError(e) => (StatusCode::INTERNAL_SERVER_ERROR, e).into_response(),
            ServerError::StdinTooLarge(size) => (StatusCode::PAYLOAD_TOO_LARGE, format!("Stdin is too large, found {} bytes", size)).into_response(),
            ServerError::ProgramTooLarge(size) => (StatusCode::PAYLOAD_TOO_LARGE, format!("Program is too large, found {} bytes", size)).into_response(),
        }
    }
}

/// Start the enclave.
///
/// This function will block until the enclave is started or force the program to exit with an error code.
///
/// This function utilizes the `enclave.sh` script to start the enclave.
pub fn start_enclave(args: &ServerArgs) {
    // Run the enclave.sh script.
    let mut command = std::process::Command::new("sh");
    command.current_dir(Path::new(MANIFEST_DIR).parent().unwrap());
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

/// Terminate the enclave.
///
/// This function will block until the enclave is terminated or force the program to exit with an error code.
///
/// This function utilizes the `enclave.sh` script to terminate the enclave.
pub fn terminate_enclaves() {
    // Run the enclave.sh script.
    let mut command = std::process::Command::new("sh");
    command.current_dir(Path::new(MANIFEST_DIR).parent().unwrap());

    // Pipe the output to the parent process.
    command.stderr(std::process::Stdio::inherit());
    command.stdout(std::process::Stdio::inherit());

    command.arg("enclave.sh");
    command.arg("terminate");

    let output = command.output().expect("Failed to run enclave.sh");
    if !output.status.success() {
        tracing::error!("Failed to terminate enclaves");
        std::process::exit(1);
    }
}

/// Spawn a task that will save attestations to S3.
///
/// This function will run until the program is killed.
pub fn spawn_attestation_task(cid: u32, port: u16, interval: Duration) {
    tokio::spawn(async move {
        // If the attestation fails, we try again sooner.
        const TRY_AGAIN_INTERVAL: Duration = Duration::from_secs(5);

        // Sleep for a bit before starting the loop, this allows the enclave to start.
        tokio::time::sleep(TRY_AGAIN_INTERVAL).await;

        loop {
            if let Err(e) =
                crate::attestations::save_attestation(crate::attestations::SaveAttestationArgs {
                    cid,
                    port,
                    ..Default::default()
                })
                .await
            {
                tracing::error!("Failed to save attestation: {}", e);

                tokio::time::sleep(TRY_AGAIN_INTERVAL).await;
                continue;
            }

            tokio::time::sleep(interval).await;
        }
    });
}
