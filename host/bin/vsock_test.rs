use sp1_tee_common::{EnclaveRequest, EnclaveResponse, EnclaveStream};

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
    let mut stream = EnclaveStream::connect(cid.unwrap_or(10), port.unwrap_or(5005)).await.unwrap();

    let msg = EnclaveRequest::Print("Hello from the host!".to_string());

    stream.send(msg).await.unwrap();

    let msg = stream.recv().await.unwrap();
    match msg {
        EnclaveResponse::Print(msg) => {
            println!("Received message: {}", msg);
        }
        EnclaveResponse::Ack => {
            println!("Received Ack");
        }
        _ => {
            panic!("Received unexpected message: {:?}", msg.type_of());
        }
    }
}