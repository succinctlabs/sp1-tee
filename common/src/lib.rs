use serde::{Serialize, Deserialize};

mod communication;
pub use communication::{VsockStream, CommunicationError};

/// A VSOCK address is defined as the tuple of (CID, port).
///
/// So its OK to hardcode the port here.
pub const ENCLAVE_PORT: u16 = 5005;

/// The CID of the enclave.
pub const ENCLAVE_CID: u32 = 10;

#[derive(Debug, Serialize, Deserialize)]
pub enum EnclaveRequest {
    /// Print from the enclave to the debug console.
    Print(String),
    /// Request the enclave's public key.
    GetPublicKey,
    /// Request the enclave's signing key for crash tolerane.
    GetEncryptedSigningKey,
    /// Request the enclave to attest to the signing key.
    AttestSigningKey,
    /// An execution request, sent from the host to the enclave.
    Execute {
        stdin: sp1_sdk::SP1Stdin,
        program: Vec<u8>,
        cycle_limit: u32,
    },
    /// Set the enclave's signing key.
    SetSigningKey(Vec<u8>),
    /// Close the session, the enclave will drop the connection after this request.
    CloseSession,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum EnclaveResponse {
    PublicKey(k256::EncodedPoint),
    /// The enclave's signing key, encrypted with the host's public key.
    EncryptedSigningKey(Vec<u8>),
    /// An attestation document with the public key field set.
    SigningKeyAttestation(Vec<u8>),
    /// The result of an execution, sent from the enclave to the host.
    /// 
    /// The signature is of the form [ vkey || public_values ]
    SignedPublicValues {
        vkey: [u8; 32],
        public_values: Vec<u8>,
        signature: k256::ecdsa::Signature,
        recovery_id: u8,
    },
    /// The receiver of this variant should print this message to stdout.
    Error(String),
    /// Indicate to the host that the enclave has received the message.
    Ack,
}

impl EnclaveRequest {
    pub fn type_of(&self) -> &'static str {
        match self {
            EnclaveRequest::CloseSession => "CloseSession",
            EnclaveRequest::GetPublicKey => "GetPublicKey",
            EnclaveRequest::Print(_) => "Print",
            EnclaveRequest::GetEncryptedSigningKey => "GetEncryptedSigningKey",
            EnclaveRequest::Execute { .. } => "Execute",
            EnclaveRequest::SetSigningKey(_) => "SetSigningKey",
            EnclaveRequest::AttestSigningKey => "AttestSigningKey",
        }
    }
}

impl EnclaveResponse {
    pub fn type_of(&self) -> &'static str {
        match self {
            EnclaveResponse::PublicKey(_) => "PublicKey",
            EnclaveResponse::EncryptedSigningKey(_) => "EncryptedSigningKey",   
            EnclaveResponse::SigningKeyAttestation(_) => "SigningKeyAttestation",
            EnclaveResponse::SignedPublicValues { .. } => "SignedPublicValues",
            EnclaveResponse::Error(_) => "Error",
            EnclaveResponse::Ack => "Ack",
        }
    }
}