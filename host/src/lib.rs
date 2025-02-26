use alloy::primitives::Address;
use aws_config::{BehaviorVersion, Region};
use sp1_tee_common::{CommunicationError, EnclaveRequest, EnclaveResponse, VsockStream};

pub mod attestations;
pub use attestations::{save_attestation, SaveAttestationArgs, SaveAttestationError};

pub mod contract;
pub use contract::TEEVerifier;

pub mod api;

pub mod server;

#[cfg(feature = "production")]
pub const S3_BUCKET: &str = "sp1-tee-attestations";

#[cfg(not(feature = "production"))]
pub const S3_BUCKET: &str = "sp1-tee-attestations-testing";

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
/// Ethereum address are derived as `keccack256([x || y])[12..]`
pub fn ethereum_address_from_encoded_point(encoded_point: &k256::EncodedPoint) -> Option<Address> {
    if encoded_point.is_identity() || encoded_point.is_compact() || encoded_point.is_compressed() {
        return None;
    }

    Some(ethereum_address_from_sec1_bytes(encoded_point.as_bytes()))
}

/// Converts a K256 SEC1 encoded public key to an Ethereum address.
/// 
/// SEC1 bytes are of the form:
/// 
/// ```
/// [ 0x04 || x || y ]
/// ```
/// 
/// Ethereum address are derived as `keccack256([x || y])[12..]`
pub fn ethereum_address_from_sec1_bytes(public_key: &[u8]) -> Address {
    Address::from_raw_public_key(&public_key[1..])
}

pub async fn s3_client() -> aws_sdk_s3::Client {
    // Loads from environment variables.
    let aws_config = aws_config::defaults(BehaviorVersion::latest())
        // buckets are in us-east-1
        .region(Region::new("us-east-1"))
        .load()
        .await;

    // Create the S3 client.
    aws_sdk_s3::Client::new(&aws_config)
}