//! Regenerate the durable `/signers` snapshot from raw attestations in S3.
//!
//! Intended to run on deploy and on key rotation. Reads the current set of
//! raw attestations from `S3_BUCKET`, runs them through the same
//! verify/filter/derive pipeline that the live `/signers` handlers use, and
//! writes the resulting `Vec<Address>` (bincode) to the snapshot key.
//!
//! With this snapshot in place, the `signers-stub` and versioned
//! `/signers` handlers can serve cached signers without needing the
//! versioned enclave fleet running. The versioned enclave only needs to be
//! up briefly when this generator runs.

use clap::Parser;
use sp1_sdk::network::tee::SP1_TEE_VERSION;

#[derive(Parser)]
struct Args {
    /// Print what would be written without uploading.
    #[clap(long)]
    dry_run: bool,
}

/// Operator-grep-friendly error type. Each variant is a stable, searchable
/// log prefix tied to one stage of the snapshot regeneration pipeline.
#[derive(Debug, thiserror::Error)]
enum GenerateError {
    #[error("snapshot-generate/list-attestations: {0}")]
    ListAttestations(#[from] sp1_tee_host::attestations::GetAttestationError),

    #[error("snapshot-generate/empty: 0 valid signers from {0} raw attestations (refusing to overwrite a known-good snapshot)")]
    NoValidSigners(usize),

    #[error("snapshot-generate/encode: {0}")]
    Encode(#[from] bincode::Error),

    #[error("snapshot-generate/upload: {0}")]
    Upload(#[from] sp1_tee_host::attestations::WriteSnapshotError),
}

#[tokio::main]
async fn main() -> Result<(), GenerateError> {
    rustls::crypto::aws_lc_rs::default_provider()
        .install_default()
        .expect("failed to install rustls provider");

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    let args = Args::parse();

    let raw_attestations = sp1_tee_host::attestations::get_raw_attestations().await?;
    tracing::info!("fetched {} raw attestations", raw_attestations.len());

    // Sanity-check: refuse to ship a snapshot that contains no valid
    // attestations. `derive_signers` runs the same Nitro CA + 3h cert
    // expiry checks the handlers will run on every read.
    let signers = sp1_tee_host::attestations::derive_signers(&raw_attestations, SP1_TEE_VERSION);
    if signers.is_empty() {
        return Err(GenerateError::NoValidSigners(raw_attestations.len()));
    }
    tracing::info!(
        "verified {} signers from {} raw attestations; writing snapshot",
        signers.len(),
        raw_attestations.len()
    );

    let key = sp1_tee_host::attestations::signers_snapshot_key(SP1_TEE_VERSION);

    if args.dry_run {
        let body_len = bincode::serialize(&raw_attestations)?.len();
        tracing::info!(
            "dry-run: would write {} bytes ({} attestations) to {}",
            body_len,
            raw_attestations.len(),
            key
        );
        return Ok(());
    }

    sp1_tee_host::attestations::write_signers_snapshot(SP1_TEE_VERSION, &raw_attestations).await?;
    tracing::info!("wrote snapshot to {}", key);
    Ok(())
}
