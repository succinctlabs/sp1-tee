use vsock::{VsockListener, VMADDR_CID_ANY, VsockStream};

use std::io::{BufReader, Read, Write};

fn main() {
    // Accept connections from any CID, on port `VSOCK_PORT`.
    let mut stream = VsockStream::connect_with_cid_port(10, 5005).unwrap();

    let msg = b"Hello, world!";
    stream.write(msg).unwrap();
}