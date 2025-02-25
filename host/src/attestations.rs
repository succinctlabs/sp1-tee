use aws_config::{BehaviorVersion, Region};
use aws_sdk_s3::{error::SdkError, operation::put_object::PutObjectError};
use sp1_tee_common::{CommunicationError, EnclaveRequest, EnclaveResponse, VsockStream};
use crate::HostStream;

use crate::ethereum_address_from_encoded_point;

#[derive(Debug)]
pub struct SaveAttestationArgs {
    /// The CID of the enclave to connect to.
    pub cid: u32,

    /// The port of the enclave to connect to.
    pub port: u32,

    /// The S3 Bucket to write to
    pub bucket: String,
}

impl Default for SaveAttestationArgs {
    fn default() -> Self {
        Self {
            cid: 10,
            port: 5005,
            #[cfg(feature = "production")]
            bucket: "sp1-tee-attestations".to_string(),
            #[cfg(not(feature = "production"))]
            bucket: "sp1-tee-attestations-testing".to_string(),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SaveAttestationError {
    #[error("Failed to communicate with enclave: {0}")]
    VsockError(#[from] CommunicationError),

    #[error("Failed to put object: {0}")]
    S3PutObjectError(#[from] SdkError<PutObjectError>),

    #[error("Got a bad public key from the enclave, this is a bug.")]
    BadPublicKey,

    #[error("Unexpected message from enclave, expected signing key attestation, got {0}")]
    UnexpectedMessage(&'static str),
}

/// Save the attestation to S3.
///
/// This function will connect to the enclave, request the signing key attestation, and save it to S3.
pub async fn save_attestation(args: SaveAttestationArgs) -> Result<(), SaveAttestationError> {
    tracing::debug!("Save attestation args: {:#?}", args);

    let SaveAttestationArgs { cid, port, bucket } = args;

    // Loads from environment variables.
    let aws_config = aws_config::defaults(BehaviorVersion::latest())
        // buckets are in us-east-1
        .region(Region::new("us-east-1"))
        .load()
        .await;

    // Create the S3 client.
    let s3_client = aws_sdk_s3::Client::new(&aws_config);

    // Connect to the enclave.
    let mut stream = HostStream::new(cid, port).await?;

    // Request the signing key attestation.
    let attest_signing_key = EnclaveRequest::AttestSigningKey;
    let get_public_key = EnclaveRequest::GetPublicKey;

    stream.send(attest_signing_key).await?;
    stream.send(get_public_key).await?;

    let attestation = match stream.recv().await? {
        EnclaveResponse::SigningKeyAttestation(attestation) => {
            attestation
        }
        msg => {
            return Err(SaveAttestationError::UnexpectedMessage(msg.type_of()));
        }
    };

    let public_key = match stream.recv().await? {
        EnclaveResponse::PublicKey(public_key) => {
            public_key
        }
        msg => {
            return Err(SaveAttestationError::UnexpectedMessage(msg.type_of()));
        }
    };

    // The address of the enclave is the S3 bucket key.
    let key = ethereum_address_from_encoded_point(&public_key).ok_or(SaveAttestationError::BadPublicKey)?;
    let key = key.to_string();

    // Write the attestation to S3.
    s3_client
        .put_object()
        .bucket(bucket)
        .key(key)
        .body(attestation.into())
        .send()
        .await?;

    Ok(())
}