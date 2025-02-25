use crate::EnclaveArgs;

use k256::ecdsa::SigningKey;
use rand_core::OsRng;
use tokio_vsock::{VsockListener, VsockStream as VsockStreamRaw, VMADDR_CID_ANY, VsockAddr};
use sp1_tee_common::{EnclaveRequest, EnclaveResponse, VsockStream};
use aws_nitro_enclaves_nsm_api::{
    driver::{nsm_init, nsm_process_request, nsm_exit},
    api::{Request, Response},
};
use parking_lot::Mutex;
use std::sync::Arc;

enum ConnectionState {
    Continue,
    Close,
}

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

        println!("Server started with public key: {:?}", signing_key.verifying_key());

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

    fn handle_connection(&self, stream: VsockStreamRaw) {
        let mut stream = VsockStream::<EnclaveRequest, EnclaveResponse>::new(stream);

        loop {
            let message = stream.blocking_recv().unwrap();

            match self.handle_message(message, &mut stream) {
                ConnectionState::Continue => {}
                ConnectionState::Close => {
                    println!("Connection closed.");
                    break;
                }
            }
        }
    }

    /// Handles a message from the host.
    /// 
    /// Returns false if the connection should be closed.
    fn handle_message(&self, message: EnclaveRequest, stream: &mut VsockStream<EnclaveRequest, EnclaveResponse>) -> ConnectionState {
        match message {
            EnclaveRequest::Print(message) => {
                println!("{}", message);

                stream.blocking_send(EnclaveResponse::Ack).unwrap();
            },
            EnclaveRequest::AttestSigningKey => {
                match self.attest_signing_key() {
                    Ok(attestation) => {
                        stream.blocking_send(EnclaveResponse::SigningKeyAttestation(attestation)).unwrap();
                    },
                    Err(e) => {
                        stream.blocking_send(EnclaveResponse::Error(format!("Failed to attest to the signing key: {:?}", e))).unwrap();
                    },
                }
            },
            EnclaveRequest::GetPublicKey => {
                let public_key = self.get_public_key();

                stream.blocking_send(EnclaveResponse::PublicKey(public_key)).unwrap();
            },
            EnclaveRequest::Execute { stdin, program } => {
                let _res = self.execute(stdin, program);
            },
            EnclaveRequest::CloseSession => {
                return ConnectionState::Close;
            },
            EnclaveRequest::GetEncryptedSigningKey => {
                stream.blocking_send(EnclaveResponse::Error("Not implemented".to_string())).unwrap();
            },
            EnclaveRequest::SetSigningKey(_) => {
                stream.blocking_send(EnclaveResponse::Error("Not implemented".to_string())).unwrap();
            },
        }

        ConnectionState::Continue
    }

    /// Decrypts the signing key (using KMS) and sets it on the server.
    #[allow(unused)]
    fn set_signing_key(&self, ciphertext: Vec<u8>) {
        todo!()
    }

    /// Encrypts the servers signing key (using KMS) and sends it to the host.
    #[allow(unused)]
    fn get_signing_key(&self) -> Vec<u8> {
        todo!()
    }

    fn get_public_key(&self) -> k256::EncodedPoint {
        self.signing_key.lock().verifying_key().to_encoded_point(false)
    }
    
    /// Attests to the signing key.
    fn attest_signing_key(&self) -> Result<Vec<u8>, ServerError> {
        let fd = nsm_init();

        if fd < 0 {
            return Err(ServerError::FailedToInitNSM);
        }

        // SEC1 encoded public key.
        // Explicitly use compression as only the X-coordinate is used in the contract.
        let public_key_bytes = self.get_public_key().to_bytes().to_vec();

        let request = Request::Attestation {
            user_data: None,
            nonce: None,
            public_key: Some(public_key_bytes.into()),
        };

        let response = nsm_process_request(fd, request);

        nsm_exit(fd);

        match response {
            Response::Attestation { document, .. } => {
                Ok(document)
            },
            _ => Err(ServerError::UnexpectedResponseType),
        }
    }

    /// Executes a program with the given stdin and program.
    /// 
    /// Sends a signature over the public values (and the vkey) to the host.
    fn execute(&self, stdin: Vec<u8>, program: Vec<u8>) {
        // Take the guard to ensure only one execution can be running at a time.
        let _guard = self.execution_guard.lock();

        todo!()
    }
}

#[derive(Debug, thiserror::Error)]
enum ServerError {
    #[error("Failed to initialize NSM")]
    FailedToInitNSM,

    #[error("Unexpected response type from NSM, this is a bug.")]
    UnexpectedResponseType,
}