//! Regenerate the durable `/signers` snapshot from raw attestations in S3.
//!
//! This binary is invoked manually on the versioned enclave host
//! (`Sp1TeeVersionedStack-v{N}` ASG) after that ASG has been temporarily
//! scaled up and `save_attestation` has had time to refresh raw
//! attestations under `sp1-tee-attestations`. It then:
//!
//! 1. Reads fresh raw attestations from the legacy public attestations
//!    bucket (anonymous read).
//! 2. Runs each through Nitro CA verification, the SP1 TEE version filter,
//!    and the SEC1 → Ethereum-address derivation.
//! 3. Refuses to write if zero valid signers are derived (defense against
//!    overwriting a known-good snapshot with an empty one).
//! 4. Writes a [`sp1_tee_host::attestations::SignersSnapshot`] to the
//!    private snapshots bucket. Only the inner `Vec<Address>` is served by
//!    `/signers`; the wrapper carries metadata for ops/debug.
//!
//! After the snapshot is in place the operator can scale the versioned
//! ASG back to `desired=0`. Stub `/signers` traffic remains served from
//! the snapshot indefinitely (the snapshot key is excluded from the
//! attestations bucket's daily-refresh lifecycle by design).

use clap::Parser;
use sp1_sdk::network::tee::SP1_TEE_VERSION;

#[derive(Parser, Debug)]
#[command(about = "Regenerate the durable /signers snapshot in S3")]
struct Args {
    /// Destination snapshots bucket (private, CDK-managed). May also be set
    /// via the `SP1_TEE_SNAPSHOTS_BUCKET` env var.
    #[clap(long, env = "SP1_TEE_SNAPSHOTS_BUCKET")]
    bucket: String,

    /// Print what would be written without uploading.
    #[clap(long)]
    dry_run: bool,
}

/// Operator-grep-friendly error type. Each variant is a stable, searchable
/// log prefix tied to one stage of the snapshot regeneration pipeline.
#[derive(Debug, thiserror::Error)]
#[allow(clippy::large_enum_variant)]
enum GenerateError {
    #[error("snapshot-generate/list-attestations: {0}")]
    ListAttestations(#[from] sp1_tee_host::attestations::GetAttestationError),

    #[error("snapshot-generate/no-valid-signers: 0 valid signers derived from {0} raw attestations (refusing to overwrite a known-good snapshot)")]
    NoValidSigners(usize),

    #[error("snapshot-generate/upload: {0}")]
    Upload(#[from] sp1_tee_host::attestations::WriteSnapshotError),
}

#[tokio::main]
#[allow(clippy::result_large_err)]
async fn main() -> Result<(), GenerateError> {
    rustls::crypto::aws_lc_rs::default_provider()
        .install_default()
        .expect("Failed to install rustls provider");

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    let args = Args::parse();

    let raw_attestations = sp1_tee_host::attestations::get_raw_attestations().await?;
    let source_count = raw_attestations.len();
    tracing::info!("fetched {source_count} raw attestations");

    // Re-verify every raw attestation. Same code path the prior live
    // `/signers` handler ran on every request, so the snapshot is the
    // exact verified set as-of generation time.
    let signers = sp1_tee_host::attestations::derive_signers(&raw_attestations, SP1_TEE_VERSION);
    if signers.is_empty() {
        return Err(GenerateError::NoValidSigners(source_count));
    }
    tracing::info!(
        "verified {} signers from {} raw attestations",
        signers.len(),
        source_count
    );

    let snapshot =
        sp1_tee_host::attestations::build_snapshot(SP1_TEE_VERSION, signers, source_count);

    let key = sp1_tee_host::attestations::snapshot_key(SP1_TEE_VERSION);

    if args.dry_run {
        let body = bincode::serialize(&snapshot).expect("snapshot serialization is infallible");
        tracing::info!(
            "dry-run: would write {} bytes ({} signers, source_count={}) to {}/{}",
            body.len(),
            snapshot.signers.len(),
            snapshot.source_count,
            args.bucket,
            key,
        );
        return Ok(());
    }

    sp1_tee_host::attestations::write_signers_snapshot(&args.bucket, &snapshot).await?;
    tracing::info!(
        "wrote snapshot to {}/{} ({} signers, generated_at_unix={})",
        args.bucket,
        key,
        snapshot.signers.len(),
        snapshot.generated_at_unix,
    );
    Ok(())
}
