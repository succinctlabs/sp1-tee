use sp1_tee_common::{EnclaveRequest, EnclaveResponse, VsockStream};

use clap::Parser;
use tokio::io::AsyncWriteExt;

#[derive(Parser)]
struct Args {
    /// The CID of the enclave to connect to.
    #[clap(short, long)]
    cid: Option<u32>,

    /// The port of the enclave to connect to.
    #[clap(short, long)]
    port: Option<u32>,

    /// The path to the output file.
    #[clap(short, long)]
    out_file: Option<String>,
}

#[tokio::main]
async fn main() {
    let Args { cid, port, out_file } = Args::parse();

    // Accept connections from any CID, on port `VSOCK_PORT`.
    let mut stream = VsockStream::connect(cid.unwrap_or(10), port.unwrap_or(5005)).await.unwrap();

    let out_file = out_file.unwrap_or("attestation.bin".to_string());
    let mut out_file = tokio::fs::File::create(out_file).await.unwrap();

    let attest_signing_key = EnclaveRequest::AttestSigningKey;

    stream.send(attest_signing_key).await.unwrap();

    while let Ok(msg) = stream.recv().await {
        match msg {
            EnclaveResponse::SigningKeyAttestation(attestation) => {
                out_file.write_all(&attestation).await.unwrap();
            },
            _ => {
                println!("Received unexpected message: {:?}", msg.type_of());
            }
        }
    }

    stream.send(EnclaveRequest::CloseSession).await.unwrap();
}