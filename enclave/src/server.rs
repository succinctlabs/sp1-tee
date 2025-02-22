use crate::EnclaveArgs;

use k256::ecdsa::SigningKey;
use rand_core::OsRng;
use vsock::{VsockListener, VsockStream as VsockStreamRaw, VMADDR_CID_ANY};

use std::sync::atomic::AtomicUsize;
use std::sync::{Arc, Mutex};

use sp1_tee_common::{EnclaveMessage, VsockStream};

const ALLOCATED_CPUS: usize = 4;

/// The number of executors currently running.
/// 
/// An executor allocated a signinficant amount of memory, to ensure an OOM doesnt occur, 
/// we keep track here.
static EXECUTOR_COUNT: AtomicUsize = AtomicUsize::new(0);

pub struct Server {
    signing_key: Mutex<SigningKey>,
    args: EnclaveArgs,
}

impl Server {
    pub fn new(args: EnclaveArgs) -> Self {
        let signing_key = SigningKey::random(&mut OsRng);

        Self {
            signing_key: Mutex::new(signing_key),
            args,
        }
    }

    pub fn run(self) {
        let this = Arc::new(self);

        let listener =
            VsockListener::bind_with_cid_port(this.args.cid.unwrap_or(VMADDR_CID_ANY), this.args.port)
                .expect("Failed to bind to vsock");

        loop {
            let (stream, _) = listener.accept().expect("Failed to accept connection");

            std::thread::spawn({
                let this = this.clone();

                move || {
                    this.handle_connection(stream);
                }
            });
        }
    }

    pub fn handle_connection(&self, stream: VsockStreamRaw) {
        let mut stream = VsockStream::new(stream);

        let message = stream.block_on_message().unwrap();

        match message {
            EnclaveMessage::PrintMe(message) => {
                println!("{}", message);

                stream.send_message(EnclaveMessage::PrintMe("Hello, host!".to_string())).unwrap();
            },
            _ => {
                println!("Received unimplented message type: {}", message.type_of());
            }
        }
    }

    pub fn set_signing_key(&self) {
        todo!()
    }
}
