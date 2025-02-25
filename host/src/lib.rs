use alloy::primitives::Address;
use sp1_tee_common::{CommunicationError, EnclaveRequest, EnclaveResponse, VsockStream};

mod attestations;
pub use attestations::{save_attestation, SaveAttestationArgs, SaveAttestationError};

/// A wrapper around [`VsockStream`] that allows for sending messages to the enclave.
///
/// This stream is bi-directional, and it will automatically close the connection when the stream is dropped.
pub struct HostStream {
    stream: VsockStream<EnclaveResponse, EnclaveRequest>,
}

impl HostStream {
    /// Connects to the enclave on the given CID and port.
    pub async fn new(cid: u32, port: u32) -> Result<Self, CommunicationError> {
        let stream = VsockStream::connect(cid, port).await?;

        Ok(Self { stream })
    }

    /// Sends a request to the enclave.
    pub async fn send(&mut self, request: EnclaveRequest) -> Result<(), CommunicationError> {
        self.stream.send(request).await
    }

    /// Receives a response from the enclave.
    pub async fn recv(&mut self) -> Result<EnclaveResponse, CommunicationError> {
        self.stream.recv().await
    }
}

impl Drop for HostStream {
    fn drop(&mut self) {
        // Make sure the enclave drops the connection when the host drops the stream.
        //
        // Ignore any errors as they stream may already be closed.
        let _ = self.stream.blocking_send(EnclaveRequest::CloseSession);
    }
}

/// Converts a K256 encoded point to an Ethereum address.
/// 
/// Returns `None` if the point is the identity, compact or compressed, as we need the full public key
/// of the form [ x || y] to compute the address correctly.
pub fn ethereum_address_from_encoded_point(encoded_point: &k256::EncodedPoint) -> Option<Address> {
    if encoded_point.is_identity() || encoded_point.is_compact() || encoded_point.is_compressed() {
        return None;
    }

    // Note: The leading 0x04 is an indentifier, and should be skipped for the hashing.
    Some(Address::from_raw_public_key(&encoded_point.as_bytes()[1..]))
}
