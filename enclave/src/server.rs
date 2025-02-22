use crate::EnclaveArgs;

use k256::ecdsa::SigningKey;
use rand_core::OsRng;
use tokio_vsock::{VsockListener, VsockStream as VsockStreamRaw, VMADDR_CID_ANY, VsockAddr};

use std::sync::Arc;

use parking_lot::Mutex;

use sp1_tee_common::{EnclaveRequest, EnclaveResponse, EnclaveStream};

pub struct Server {
    /// The arguments passed to the enclave at startup.
    args: EnclaveArgs,
    /// The signing key for the enclave.
    /// 
    /// Wrapped in a [`parking_lot::Mutex`] as the host may change it.
    signing_key: Mutex<SigningKey>,
    /// Note: Only one execution can be running at a time, as it allocates a significant amount of memory.
    /// 
    /// In the enclave, memory MUST be specified up front, so extra consideration is required to ensure we dont OOM.
    /// 
    /// This is a [`parking_lot::Mutex`] to avoid priority inversion.
    execution_guard: Mutex<()>,
}

impl Server {
    pub fn new(args: EnclaveArgs) -> Self {
        let signing_key = SigningKey::random(&mut OsRng);

        Self {
            signing_key: Mutex::new(signing_key),
            args,
            execution_guard: Mutex::new(()),
        }
    }

    pub async fn run(self) {
        let this = Arc::new(self);

        let addr = VsockAddr::new(this.args.cid.unwrap_or(VMADDR_CID_ANY), this.args.port);

        let listener = VsockListener::bind(addr).expect("Failed to bind to vsock");

        loop {
            let (stream, _) = listener.accept().await.expect("Failed to accept connection");

            // Spawn a new (blocking) thread to handle the request.
            //
            // Tokio tasks aren't preferable here as exeuction (the most likely request type) should be considered blocking.
            std::thread::spawn({
                let this = this.clone();

                move || {
                    this.handle_connection(stream);
                }
            });
        }
    }

    pub fn handle_connection(&self, stream: VsockStreamRaw) {
        let mut stream = EnclaveStream::<EnclaveRequest, EnclaveResponse>::new(stream);

        let message = stream.blocking_recv().unwrap();

        match message {
            EnclaveRequest::Print(message) => {
                println!("{}", message);

                stream.blocking_send(EnclaveResponse::Ack).unwrap();
            },
            EnclaveRequest::AttestSigningKey => {
                let attestation = self.attest_signing_key();

                stream.blocking_send(EnclaveResponse::SigningKeyAttestation(attestation)).unwrap();
            },
            EnclaveRequest::GetEncryptedSigningKey => {
                let ciphertext = self.get_signing_key();

                stream.blocking_send(EnclaveResponse::EncryptedSigningKey(ciphertext)).unwrap();
            },
            EnclaveRequest::SetSigningKey(ciphertext) => {
                self.set_signing_key(ciphertext);

                stream.blocking_send(EnclaveResponse::Ack).unwrap();
            },
            EnclaveRequest::Execute { stdin, program } => {
                let _res = self.execute(stdin, program);
            },
        }
    }

    /// Decrypts the signing key (using KMS) and sets it on the server.
    pub fn set_signing_key(&self, ciphertext: Vec<u8>) {
        todo!()
    }

    /// Encrypts the servers signing key (using KMS) and sends it to the host.
    pub fn get_signing_key(&self) -> Vec<u8> {
        todo!()
    }
    
    /// Attests to the signing key.
    pub fn attest_signing_key(&self) -> Vec<u8> {
        todo!()
    }

    /// Executes a program with the given stdin and program.
    /// 
    /// Sends a signature over the public values (and the vkey) to the host.
    pub fn execute(&self, stdin: Vec<u8>, program: Vec<u8>) {
        // Take the guard to ensure only one execution can be running at a time.
        let _guard = self.execution_guard.lock();

        todo!()
    }
}
