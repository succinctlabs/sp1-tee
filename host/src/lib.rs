#[cfg(feature = "attestations")]
use alloy::primitives::Address;

/// The functionality for saving and verifying attestations.
#[cfg(feature = "attestations")]
pub mod attestations;
#[cfg(feature = "attestations")]
pub use attestations::{save_attestation, SaveAttestationArgs, SaveAttestationError};

/// The SP1 TEE verifier contract.
#[cfg(feature = "attestations")]
pub mod contract;
#[cfg(feature = "attestations")]
pub use contract::TEEVerifier;

/// The API for interacting with the host server.
#[cfg(any(feature = "server", feature = "client"))]
pub mod api;

/// The host server implementation.
#[cfg(feature = "server")]
pub mod server;
#[cfg(feature = "server")]
pub use server::stream::HostStream;

#[cfg(feature = "client")]
pub use sp1_sdk::network::tee::client::{Client, ClientError};

#[cfg(feature = "production")]
pub const S3_BUCKET: &str = "sp1-tee-attestations";
#[cfg(not(feature = "production"))]
pub const S3_BUCKET: &str = "sp1-tee-attestations-testing";

/// Converts a K256 encoded point to an Ethereum address.
/// 
/// Ethereum address are derived as `keccack256([x || y])[12..]`
#[cfg(feature = "attestations")]
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
#[cfg(feature = "attestations")]
pub fn ethereum_address_from_sec1_bytes(public_key: &[u8]) -> Address {
    Address::from_raw_public_key(&public_key[1..])
}