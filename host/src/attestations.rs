use std::time::Duration;

use alloy::hex::FromHexError;
use alloy::primitives::Address;
use attestation_doc_validation::error::{AttestError, AttestResult};
use aws_config::{BehaviorVersion, Region};
use aws_sdk_s3::operation::get_object::GetObjectError;
use aws_sdk_s3::operation::list_objects_v2::ListObjectsV2Error;
use aws_sdk_s3::primitives::ByteStreamError;
use aws_sdk_s3::{error::SdkError, operation::put_object::PutObjectError};
use sp1_tee_common::{CommunicationError, EnclaveRequest, EnclaveResponse};

use aws_nitro_enclaves_nsm_api::api::AttestationDoc;

use crate::ethereum_address_from_encoded_point;
use crate::HostStream;

// Attestations expire every 3 hours and we update every 30 mins.
pub const ATTESTATION_INTERVAL: Duration = Duration::from_secs(30 * 60);

/// Creates an S3 client from the environment variables.
///
/// For EC2 instances, the environment variables are set automatically.
///
/// # Panics
///
/// This function will panic if the environment variables are not set.
pub(crate) async fn s3_client_write() -> aws_sdk_s3::Client {
    // Loads from environment variables.
    let aws_config = aws_config::defaults(BehaviorVersion::latest())
        // buckets are in us-east-1
        .region(Region::new("us-east-1"))
        .load()
        .await;

    // Create the S3 client.
    aws_sdk_s3::Client::new(&aws_config)
}

/// Creates a client that doesnt sign requests
pub(crate) async fn s3_client_read_only() -> aws_sdk_s3::Client {
    // Loads from environment variables.
    let aws_config = aws_config::defaults(BehaviorVersion::latest())
        // buckets are in us-east-1
        .region(Region::new("us-east-1"))
        .no_credentials()
        .load()
        .await;

    // Create the S3 client.
    aws_sdk_s3::Client::new(&aws_config)
}

#[derive(Debug)]
pub struct SaveAttestationArgs {
    /// The CID of the enclave to connect to.
    pub cid: u32,

    /// The port of the enclave to connect to.
    pub port: u16,

    /// The S3 Bucket to write to
    pub bucket: String,
}

impl Default for SaveAttestationArgs {
    fn default() -> Self {
        Self {
            cid: 10,
            port: sp1_tee_common::ENCLAVE_PORT,
            bucket: crate::S3_BUCKET.to_string(),
        }
    }
}

#[derive(Debug, thiserror::Error)]
#[allow(clippy::large_enum_variant)]
pub enum SaveAttestationError {
    #[error("Failed to communicate with enclave: {0:?}")]
    VsockError(#[from] CommunicationError),

    #[error("Failed to put object: {0:?}")]
    S3PutObjectError(#[from] SdkError<PutObjectError>),

    #[error("Got a bad public key from the enclave, this is a bug.")]
    BadPublicKey,

    #[error("Unexpected message from enclave, expected signing key attestation, got {0:?}")]
    UnexpectedMessage(&'static str),
}

/// Save the attestation to S3.
///
/// This function will connect to the enclave, request the signing key attestation, and save it to S3.
pub async fn save_attestation(args: SaveAttestationArgs) -> Result<(), SaveAttestationError> {
    tracing::debug!("Save attestation args: {:#?}", args);

    let SaveAttestationArgs { cid, port, bucket } = args;

    let s3_client = s3_client_write().await;

    // Connect to the enclave.
    let mut stream = HostStream::new(cid, port).await?;

    // Request the signing key attestation.
    let attest_signing_key = EnclaveRequest::AttestSigningKey;
    let get_public_key = EnclaveRequest::GetPublicKey;

    stream.send(attest_signing_key).await?;
    stream.send(get_public_key).await?;

    let attestation = match stream.recv().await? {
        EnclaveResponse::SigningKeyAttestation(attestation) => attestation,
        msg => {
            return Err(SaveAttestationError::UnexpectedMessage(msg.type_of()));
        }
    };

    let public_key = match stream.recv().await? {
        EnclaveResponse::PublicKey(public_key) => public_key,
        msg => {
            return Err(SaveAttestationError::UnexpectedMessage(msg.type_of()));
        }
    };

    // The address of the enclave is the S3 bucket key.
    let key = ethereum_address_from_encoded_point(&public_key)
        .ok_or(SaveAttestationError::BadPublicKey)?;

    tracing::info!("Saving attestation to S3 for address: {}", key);

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

#[derive(Debug, thiserror::Error)]
pub enum GetAttestationError {
    #[error("Failed to list attestations: {0}")]
    ListAttestationsError(#[from] SdkError<ListObjectsV2Error>),

    #[error("Failed to get object: {0}")]
    S3GetObjectError(#[from] SdkError<GetObjectError>),

    #[error("Failed to parse key as address: {0}")]
    ParseAddressError(#[from] FromHexError),

    #[error("Failed to recieve bytestream: {0}")]
    ByteStreamError(#[from] ByteStreamError),
}

pub struct RawAttestation {
    pub address: Address,
    pub attestation: Vec<u8>,
}

/// Tries to fetch all attestations from S3.
///
/// # Errors
/// - [`GetAttestationError::ListAttestationsError`] - Failed to list attestations.
/// - [`GetAttestationError::S3GetObjectError`] - Failed to get an object.
/// - [`GetAttestationError::ParseAddressError`] - Failed to parse an address.
/// - [`GetAttestationError::ByteStreamError`] - Failed to collect the byte stream.
///
/// # Panics
///
/// See [`s3_client`] for more details.
pub async fn get_raw_attestations() -> Result<Vec<RawAttestation>, GetAttestationError> {
    let s3_client = s3_client_read_only().await;

    let attestation_s3_objs = s3_client
        .list_objects_v2()
        .bucket(crate::S3_BUCKET.to_string())
        .send()
        .await?;

    let mut attestations = Vec::new();

    for metadata in &attestation_s3_objs
        .contents
        .expect("No contents found in attestations")
    {
        let key = metadata.key.clone().expect("No key found in attestations");

        let key_as_address = key
            .parse::<Address>()
            .expect("Failed to parse key as address");

        // Fetch the actual object from S3.
        let object = s3_client
            .get_object()
            .bucket(crate::S3_BUCKET.to_string())
            .key(key)
            .send()
            .await?;

        let bytes = object.body.collect().await?.to_vec();

        attestations.push(RawAttestation {
            address: key_as_address,
            attestation: bytes,
        });
    }

    Ok(attestations)
}

#[derive(Debug, thiserror::Error)]
#[allow(clippy::large_enum_variant)]
pub enum AttestationVerificationError {
    #[error("Failed to verify attestation: {0}")]
    VerificationError(#[from] AttestError),

    #[error("Failed to verify PCR0 expected: {0} actual: {1}")]
    Pcr0VerificationError(String, String),

    #[error("Failed to get attestations: {0}")]
    GetAttestationError(#[from] GetAttestationError),

    #[error("Missing or invalid pubkey field on attestation docment")]
    MissingRequiredField(&'static str),

    #[error(
        "Invalid associated attestation for signer, expected: {0} found: {1} \n this is a bug."
    )]
    AddressMismatch(Address, Address),

    #[error("Version mismatch, expected: {0} found: {1}")]
    VersionMismatch(String, String),
}

/// Verifies an attestation for a given signer and hex-encoded PCR0 value.
///
/// # Errors
/// - [`AttestationVerificationError::GetAttestationError`] - Failed to get the attestation.
/// - [`AttestationVerificationError::Pcr0VerificationError`] - Failed to verify the PCR0 value.
/// - [`AttestationVerificationError::VerificationError`] - Failed to verify the attestation.
pub async fn verify_attestation_for_signer(
    signer: Address,
    version: u32,
    pcr0: &str,
) -> Result<(), AttestationVerificationError> {
    let client = s3_client_read_only().await;

    // Fetch the attestation from S3.
    let attestation = client
        .get_object()
        .bucket(crate::S3_BUCKET.to_string())
        .key(signer.to_string())
        .send()
        .await
        .map_err(GetAttestationError::from)?;

    let bytes = attestation
        .body
        .collect()
        .await
        .map_err(GetAttestationError::from)?
        .to_vec();

    // Verify the attestations root of trust.
    let doc = verify_attestation(bytes.as_ref())?;

    // Verify the PCR0 value.
    let doc_pcr0 = hex::encode(doc.pcrs[&0].as_ref());
    if doc_pcr0 != pcr0.replace("0x", "") {
        return Err(AttestationVerificationError::Pcr0VerificationError(
            pcr0.to_string(),
            doc_pcr0,
        ));
    }

    // Verify the version of the attestation.
    if doc
        .user_data
        .as_ref()
        .ok_or(AttestationVerificationError::MissingRequiredField(
            "user_data",
        ))?
        != &version.to_le_bytes()
    {
        return Err(AttestationVerificationError::VersionMismatch(
            version.to_string(),
            String::from_utf8(doc.user_data.as_ref().unwrap().to_vec()).unwrap(),
        ));
    }

    // Verify the address of the attestation.
    let derived_address = doc
        .public_key
        .and_then(|b| crate::ethereum_address_from_sec1_bytes(b.as_ref()))
        .ok_or(AttestationVerificationError::MissingRequiredField(
            "public_key",
        ))?;

    if derived_address != signer {
        return Err(AttestationVerificationError::AddressMismatch(
            signer,
            derived_address,
        ));
    }

    Ok(())
}

/// Verifies an attestation, this should be the COSESign1 attestation from the enclave.
///
/// This function will:
/// - Decode the COSESign1 structure.
/// - Validate all CA certs against the root of trust
/// - Verify the signature of the payload
/// - Return the payload as a deserialized [`AttestationDoc`].
///
/// This function is the "low-level" verification of the root of trust of the attestation.
/// For protocol level verification, see [`verify_attestation_for_signer`].
pub fn verify_attestation(attestation: &[u8]) -> AttestResult<AttestationDoc> {
    attestation_doc_validation::validate_and_parse_attestation_doc(attestation)
}
