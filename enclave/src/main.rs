use clap::Parser;

pub mod server;
pub mod executor;
pub mod ffi;

pub const VSOCK_PORT: u32 = 5005;

#[derive(clap::Parser)]
pub struct EnclaveArgs {
    /// The port to listen on for vsock connections.
    #[clap(short, long)]
    port: u32,

    /// The ARN of the KMS key used for sealing.
    #[clap(short, long)]
    enc_key_arn: String,

    /// The CID of the enclave.
    #[clap(short, long)]
    cid: Option<u32>,
}

fn main() {
    // Initialize the Nitro Enclaves SDK.
    unsafe { ffi::aws_nitro_enclaves_library_init(std::ptr::null_mut()); }

    std::panic::catch_unwind(|| {
        let args = EnclaveArgs::parse();

        // Initialize the server.
        let server = server::Server::new(args);

        // Run the server, indefinitely.
        server.run();
    }).unwrap_or_else(|e| {
        eprintln!("Panic: {:?}", e);
    });
    
    // Loop forever so we can see logs if it panics.
    loop {}
}
