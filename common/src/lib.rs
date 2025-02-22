use serde::{Serialize, Deserialize};

mod vsock;
pub use vsock::{VsockStream, CommunicationError};

/// A message sent between the enclave and the host.
/// 
/// Depending on the context, this type may have been sent from the host or form the enclave.
#[derive(Serialize, Deserialize)]
pub enum EnclaveMessage {
    GetEncryptedSigningKey,
    EncryptedSigningKey(Vec<u8>),
    SignedPublicValues {
        // todo
        a: Vec<u8>,
    },
    Execute {
        // todo
        stdin: Vec<u8>,
        program: Vec<u8>,
    },
    PrintMe(String),
}

impl EnclaveMessage {
    pub fn type_of(&self) -> &'static str {
        match self {
            EnclaveMessage::GetEncryptedSigningKey => "GetEncryptedSigningKey",
            EnclaveMessage::EncryptedSigningKey(_) => "EncryptedSigningKey",
            EnclaveMessage::SignedPublicValues { .. } => "SignedPublicValues",
            EnclaveMessage::Execute { .. } => "Execute",
            EnclaveMessage::PrintMe(_) => "PrintMe",
        }
    }
}