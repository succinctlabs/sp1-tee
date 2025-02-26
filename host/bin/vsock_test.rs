use sp1_tee_common::{EnclaveRequest, EnclaveResponse, VsockStream};
use sp1_tee_host::{attestations::verify_attestation, HostStream};

use clap::Parser;

#[derive(Parser)]
struct Args {
    /// The CID of the enclave to connect to.
    #[clap(short, long)]
    cid: Option<u32>,

    /// The port of the enclave to connect to.
    #[clap(short, long)]
    port: Option<u16>,
}

#[tokio::main]
async fn main() {
    let Args { cid, port } = Args::parse();

    for _ in 0..10 {
        let mut stream = HostStream::new(cid.unwrap_or(10), port.unwrap_or(5005))
            .await
            .unwrap();

        let message = EnclaveRequest::Print("Hello from the host!".to_string());

        stream.send(message).await.unwrap();
        stream.send(EnclaveRequest::CloseSession).await.unwrap();

        drop(stream);
    }

    // Accept connections from any CID, on port `VSOCK_PORT`.
    let mut stream = VsockStream::connect(cid.unwrap_or(10), port.unwrap_or(5005) as u32)
        .await
        .unwrap();


    let print = EnclaveRequest::Print("Hello from the host!".to_string());
    let get_public_key = EnclaveRequest::GetPublicKey;
    let attest_signing_key = EnclaveRequest::AttestSigningKey;

    stream.send(print).await.unwrap();
    stream.send(get_public_key).await.unwrap();
    stream.send(attest_signing_key).await.unwrap();

    for _ in 0..3 {
        match stream.recv().await.unwrap() {
            EnclaveResponse::Ack => {
                println!("Received Ack");
            }
            EnclaveResponse::PublicKey(public_key) => {
                println!("Received public key: {:?}", public_key);
            }
            EnclaveResponse::SigningKeyAttestation(attestation) => {
                println!("Received attestation");

                let doc = verify_attestation(&attestation).unwrap();

                println!("Attestation doc: {:?}", doc);
            }
            msg => {
                println!("Received unexpected message: {:?}", msg.type_of());
            }
        }
    }

    stream.send(EnclaveRequest::CloseSession).await.unwrap();
}
