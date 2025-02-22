use sp1_tee_common::{EnclaveMessage, VsockStream};

fn main() {
    // Accept connections from any CID, on port `VSOCK_PORT`.
    let mut stream = VsockStream::connect(10, 5005).unwrap();

    let msg = EnclaveMessage::PrintMe("Hello, enclave!".to_string());

    stream.send_message(msg).unwrap();

    let msg = stream.block_on_message().unwrap();
    match msg {
        EnclaveMessage::PrintMe(msg) => {
            println!("Received message: {}", msg);
        }
        _ => {
            panic!("Received unexpected message: {:?}", msg);
        }
    }
}