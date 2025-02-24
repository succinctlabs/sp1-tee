use sp1_tee_common::{EnclaveRequest, EnclaveResponse, VsockStream};

use clap::Parser;

#[derive(Parser)]
struct Args {
    /// The CID of the enclave to connect to.
    #[clap(short, long)]
    cid: Option<u32>,

    /// The port of the enclave to connect to.
    #[clap(short, long)]
    port: Option<u32>,
}

#[tokio::main]
async fn main() {
    let Args { cid, port } = Args::parse();

    // Accept connections from any CID, on port `VSOCK_PORT`.
    let mut stream = VsockStream::connect(cid.unwrap_or(10), port.unwrap_or(5005)).await.unwrap();

    let print = EnclaveRequest::Print("Hello from the host!".to_string());
    let get_public_key = EnclaveRequest::GetPublicKey;
    let attest_signing_key = EnclaveRequest::AttestSigningKey;

    stream.send(print).await.unwrap();
    stream.send(get_public_key).await.unwrap();
    stream.send(attest_signing_key).await.unwrap();

    while let Ok(msg) = stream.recv().await {
        match msg {
            EnclaveResponse::Ack => {
                println!("Received Ack");
            },
            EnclaveResponse::PublicKey(public_key) => {
                println!("Received public key: {:?}", public_key);
            },
            EnclaveResponse::SigningKeyAttestation(attestation) => {
                println!("Received attestation");

                let cose_message = aws_nitro_enclaves_cose::CoseSign1::from_bytes(&attestation).unwrap();

                let doc = aws_nitro_enclaves_nsm_api::api::AttestationDoc::from_binary(&cose_message.payload).unwrap();

                println!("Attestation doc: {:?}", doc);
            },
            _ => {
                println!("Received unexpected message: {:?}", msg.type_of());
            }
        }
    }

    stream.send(EnclaveRequest::CloseSession).await.unwrap();
}