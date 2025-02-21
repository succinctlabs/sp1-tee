use vsock::{VsockListener, VMADDR_CID_ANY, VsockStream};

use std::io::{BufReader, Read};

pub mod server;
pub mod executor;
pub mod ffi;

pub const VSOCK_PORT: u32 = 5005;

/// The CID of the host, as seen by the enclave.
/// 
/// This is always 3.
/// 
/// <https://docs.aws.amazon.com/enclaves/latest/user/nitro-enclave-concepts.html#term-socket>
pub const HOST_CID: u32 = 3;

fn main() {
    unsafe { ffi::aws_nitro_enclaves_library_init(std::ptr::null_mut()); }

    println!("Hello, world!");

    // Accept connections from any CID, on port `VSOCK_PORT`.
    
    loop {
        let listener = match VsockListener::bind_with_cid_port(10, VSOCK_PORT) {
            Ok(listener) => listener,
            Err(e) => {
                println!("Failed to bind to socket: {:?}", e);
                continue;
            }
        };
         
        let (stream, addr) = match listener.accept() {
            Ok((stream, addr)) => (stream, addr),
            Err(e) => {
                println!("Failed to accept connection: {:?}", e);
                continue;
            }
        };

        println!("Accepted connection from {:?}", addr);

        let mut stream = BufReader::new(stream);

        std::thread::spawn(move || {
            handle_connection(&mut stream);
        });
    }
}

fn handle_connection(stream: &mut BufReader<VsockStream>) {
    let mut buf = [0; 1024];

    loop {
        let n = stream.read(&mut buf).unwrap();

        if n == 0 {
            println!("Connection closed");
            break;
        }
    
        println!("Received message: {:?}", String::from_utf8_lossy(&buf[..n]));
        
        buf = [0; 1024];
    }
}
