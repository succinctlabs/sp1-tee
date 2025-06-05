#[cfg(feature = "attestations")]
use alloy::primitives::Address;

use tracing_subscriber::EnvFilter;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, Layer};

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

#[cfg(feature = "metrics")]
pub mod metrics;

/// Initialize the tracing subscriber.
///
/// The default filter is `sp1-tee-server=debug,info`.
pub fn init_tracing() {
    let default_env_filter = EnvFilter::try_from_default_env().unwrap_or(EnvFilter::from(
        "sp1_tee_server=debug,sp1_tee_host=debug,info",
    ));

    let fmt_layer = tracing_subscriber::fmt::layer()
        .with_line_number(true)
        .with_file(true)
        .with_filter(default_env_filter);

    let alert_layer = if std::env::var("DISABLE_ALERTS").is_ok() {
        None
    } else {
        Some(alert_subscriber::seal_layer())
    };

    tracing_subscriber::Registry::default()
        .with(fmt_layer)
        .with(alert_layer)
        .init();
}

/// Converts a K256 encoded point to an Ethereum address.
///
/// Ethereum address are derived as `keccack256([x || y])[12..]`
///
/// Returns `None` if the point is not `uncompressed`.
#[cfg(feature = "attestations")]
pub fn ethereum_address_from_encoded_point(encoded_point: &k256::EncodedPoint) -> Option<Address> {
    if encoded_point.is_identity() || encoded_point.is_compact() || encoded_point.is_compressed() {
        return None;
    }

    ethereum_address_from_sec1_bytes(encoded_point.as_bytes())
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
///
/// Returns `None` if the format is invalid.
#[cfg(feature = "attestations")]
pub fn ethereum_address_from_sec1_bytes(public_key: &[u8]) -> Option<Address> {
    if public_key.len() != 65 {
        return None;
    }

    if public_key[0] != 0x04 {
        return None;
    }

    Some(Address::from_raw_public_key(&public_key[1..]))
}
