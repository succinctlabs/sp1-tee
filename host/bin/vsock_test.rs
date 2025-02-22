use sp1_tee_common::{EnclaveMessage, VsockStream};

fn main() {
    // Accept connections from any CID, on port `VSOCK_PORT`.
    let mut stream = VsockStream::connect(10, 5005).unwrap();

    let msg = EnclaveMessage::PrintMe("Hello, world!".to_string());

    stream.send_message(msg).unwrap();
}