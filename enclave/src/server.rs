use crate::EnclaveArgs;

use aws_nitro_enclaves_nsm_api::{
    api::{Request, Response},
    driver::{nsm_exit, nsm_init, nsm_process_request},
};
use k256::ecdsa::SigningKey;
use parking_lot::Mutex;
use rand_core::OsRng;
use sha3::Digest;
use sp1_sdk::{CpuProver, HashableKey, Prover, SP1Stdin};
use sp1_tee_common::{EnclaveRequest, EnclaveResponse, VsockStream};
use std::sync::Arc;
use tokio_vsock::{VsockAddr, VsockListener, VsockStream as VsockStreamRaw, VMADDR_CID_ANY};

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
    /// The prover instance to use.
    prover: Arc<CpuProver>,
}

impl Server {
    pub fn new(args: EnclaveArgs) -> Self {
        let signing_key = SigningKey::random(&mut OsRng);

        println!(
            "Server started with public key: {:?}",
            signing_key.verifying_key()
        );

        Self {
            signing_key: Mutex::new(signing_key),
            args,
            execution_guard: Mutex::new(()),
            prover: Arc::new(CpuProver::new()),
        }
    }

    pub async fn run(self) {
        let this = Arc::new(self);

        let addr = VsockAddr::new(this.args.cid.unwrap_or(VMADDR_CID_ANY), sp1_tee_common::ENCLAVE_PORT as u32);

        let listener = VsockListener::bind(addr).expect("Failed to bind to vsock");

        loop {
            let (stream, _) = listener
                .accept()
                .await
                .expect("Failed to accept connection");

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

    /// Handles a connection from the host.
    ///
    /// NOTE: unwraps are used here on recv as this is only ran in a spawned thread.
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
    ///
    /// NOTE: unwraps are used here on sends as this is only ran in a spawned thread.
    fn handle_message(
        &self,
        message: EnclaveRequest,
        stream: &mut VsockStream<EnclaveRequest, EnclaveResponse>,
    ) -> ConnectionState {
        match message {
            EnclaveRequest::Print(message) => {
                println!("{}", message);

                let _ =stream.blocking_send(EnclaveResponse::Ack);
            }
            EnclaveRequest::AttestSigningKey => match self.attest_signing_key() {
                Ok(attestation) => {
                    stream
                        .blocking_send(EnclaveResponse::SigningKeyAttestation(attestation))
                        .unwrap();
                }
                Err(e) => {
                    stream
                        .blocking_send(EnclaveResponse::Error(format!(
                            "Failed to attest to the signing key: {:?}",
                            e
                        )))
                        .unwrap();
                }
            },
            EnclaveRequest::GetPublicKey => {
                let public_key = self.get_public_key();

                stream
                    .blocking_send(EnclaveResponse::PublicKey(public_key))
                    .unwrap();
            }
            EnclaveRequest::Execute { stdin, program } => {
                stream.blocking_send(self.execute(stdin, program)).unwrap();
            }
            EnclaveRequest::CloseSession => {
                return ConnectionState::Close;
            }
            EnclaveRequest::GetEncryptedSigningKey => {
                stream
                    .blocking_send(EnclaveResponse::Error("Not implemented".to_string()))
                    .unwrap();
            }
            EnclaveRequest::SetSigningKey(_) => {
                stream
                    .blocking_send(EnclaveResponse::Error("Not implemented".to_string()))
                    .unwrap();
            }
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
        self.signing_key
            .lock()
            .verifying_key()
            .to_encoded_point(false)
    }

    /// Attests to the signing key.
    fn attest_signing_key(&self) -> Result<Vec<u8>, ServerError> {
        let fd = nsm_init();

        if fd < 0 {
            return Err(ServerError::FailedToInitNSM);
        }

        // SEC1 encoded public key.
        //
        // This is of the form [0x04 || X || Y]
        let public_key_bytes = self.get_public_key().to_bytes().to_vec();

        let request = Request::Attestation {
            user_data: None,
            nonce: None,
            public_key: Some(public_key_bytes.into()),
        };

        let response = nsm_process_request(fd, request);

        nsm_exit(fd);

        match response {
            Response::Attestation { document, .. } => Ok(document),
            _ => Err(ServerError::UnexpectedResponseType),
        }
    }

    /// Executes a program with the given stdin and program.
    ///
    /// Sends a signature over the public values (and the vkey) to the host.
    fn execute(&self, stdin: SP1Stdin, program: Vec<u8>) -> EnclaveResponse {
        // Take the guard to ensure only one execution can be running at a time.
        let _guard = self.execution_guard.lock();

        println!("Setup start");
        let (_, vk) = self.prover.setup(&program);
        println!("Setup complete");

        match self.prover.execute(&program, &stdin).run() {
            Ok((public_values, _)) => {
                println!("Execute complete");

                let vkey_raw = vk.bytes32_raw();

                let to_sign = [vkey_raw.to_vec(), public_values.to_vec()].concat();

                let hasher = sha3::Keccak256::new_with_prefix(to_sign.as_slice());

                let Ok((signature, recovery_id)) =
                    self.signing_key.lock().sign_digest_recoverable(hasher)
                else {
                    return EnclaveResponse::Error(
                        "Failed to sign public values, this is a bug.".to_string(),
                    );
                };

                EnclaveResponse::SignedPublicValues {
                    vkey: vkey_raw,
                    public_values: public_values.to_vec(),
                    signature,
                    recovery_id: recovery_id.into(),
                }
            }
            Err(e) => EnclaveResponse::Error(format!("Failed to execute program: {:?}", e)),
        }
    }
}

#[derive(Debug, thiserror::Error)]
enum ServerError {
    #[error("Failed to initialize NSM")]
    FailedToInitNSM,

    #[error("Unexpected response type from NSM, this is a bug.")]
    UnexpectedResponseType,
}
