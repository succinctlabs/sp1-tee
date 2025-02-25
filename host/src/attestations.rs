use aws_config::{BehaviorVersion, Region};
use aws_sdk_s3::{error::SdkError, operation::put_object::PutObjectError};
use sp1_tee_common::{CommunicationError, EnclaveRequest, EnclaveResponse, VsockStream};

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
            bucket: "sp1-tee-attestations".to_string(),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SaveAttestationError {
    #[error("Failed to communicate with enclave: {0}")]
    VsockError(#[from] CommunicationError),

    #[error("Failed to put object: {0}")]
    S3PutObjectError(#[from] SdkError<PutObjectError>),

    #[error("Unexpected message from enclave, expected signing key attestation, got {0}")]
    UnexpectedMessage(&'static str),
}

pub async fn save_attestation(args: SaveAttestationArgs) -> Result<(), SaveAttestationError> {
    tracing::debug!("Save attestation args: {:#?}", args);

    let SaveAttestationArgs { cid, port, bucket } = args;

    // Save the attestation to S3, with the key being the host name and the CID.
    let key = format!("{}-{}", host_name(), cid);

    // Loads from environment variables.
    let aws_config = aws_config::defaults(BehaviorVersion::latest())
        .region(Region::new("us-east-1"))
        .load()
        .await;

    // Create the S3 client.
    let s3_client = aws_sdk_s3::Client::new(&aws_config);

    // Accept connections from any CID, on port `VSOCK_PORT`.
    let mut stream = VsockStream::connect(cid, port).await.unwrap();

    let attest_signing_key = EnclaveRequest::AttestSigningKey;

    stream.send(attest_signing_key).await?;

    match stream.recv().await? {
        EnclaveResponse::SigningKeyAttestation(attestation) => {
            s3_client
                .put_object()
                .bucket(bucket)
                .key(key)
                .body(attestation.into())
                .send()
                .await?;
        }
        msg => {
            return Err(SaveAttestationError::UnexpectedMessage(msg.type_of()));
        }
    }

    Ok(())
}

fn host_name() -> String {
    let raw = std::process::Command::new("hostname")
        .output()
        .unwrap()
        .stdout;

    String::from_utf8(raw).unwrap()
}

fn get_region() -> Region {
    let raw = std::process::Command::new("ec2-metadata")
        .arg("--region")
        .output()
        .unwrap()
        .stdout;

    let region_output_raw = String::from_utf8(raw).unwrap();

    // Remove the "region: " prefix.
    let region = region_output_raw.replace("region: ", "");

    Region::new(region.trim().to_string())
}
