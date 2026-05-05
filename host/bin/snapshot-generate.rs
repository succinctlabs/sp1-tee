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

    /// Allow the new snapshot's signer count to be lower than the
    /// previous snapshot's. Off by default — the generator refuses to
    /// shrink the served signer set, since that would let a partial
    /// attestation refresh clobber a known-good snapshot with degraded
    /// data. Pass this flag for legitimate signer-removal events
    /// (decommission, key rotation that drops a signer).
    #[clap(long)]
    allow_shrink: bool,
}

/// Operator-grep-friendly error type. Each variant is a stable, searchable
/// log prefix tied to one stage of the snapshot regeneration pipeline.
#[derive(Debug, thiserror::Error)]
#[allow(clippy::large_enum_variant)]
enum GenerateError {
    #[error("snapshot-generate/fetch-raw-attestations: {0}")]
    FetchRawAttestations(#[from] sp1_tee_host::attestations::GetAttestationError),

    #[error("snapshot-generate/no-valid-signers: 0 valid signers derived from {0} raw attestations (refusing to overwrite a known-good snapshot)")]
    NoValidSigners(usize),

    #[error("snapshot-generate/shrink-refused: previous snapshot had {previous} signers, new derivation has {current}. \
        Wait at least one ATTESTATION_INTERVAL (30 min) after host boot so all signers refresh, then re-run. \
        Pass --allow-shrink only if this is an intentional signer removal (decommission/key rotation)")]
    ShrinkRefused { previous: usize, current: usize },

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

    // Shrink protection. Read the existing snapshot at the same
    // SP1_TEE_VERSION; if it was a valid same-version snapshot with more
    // signers than we just derived, refuse to overwrite unless the
    // operator explicitly opts in via `--allow-shrink`. Defends against
    // a partial save_attestation cycle producing a degraded (but
    // non-empty) snapshot that would still pass the read-side
    // empty-check.
    //
    // NotFound (first deploy of this version) and any validation error
    // (corrupt / schema bump / version mismatch / empty) are intentionally
    // treated as "no comparable previous snapshot, write fresh" — those
    // are the cases the operator actually wants to recover from.
    //
    // The ETag from a successful read is threaded into write_signers_snapshot
    // as `If-Match` so a concurrent writer between this read and our write
    // is rejected with PreconditionFailed instead of silently clobbered.
    let expected_etag: Option<String> = match sp1_tee_host::attestations::read_signers_snapshot(
        &args.bucket,
        SP1_TEE_VERSION,
    )
    .await
    {
        Ok((previous, etag)) => {
            if previous.signers.len() > signers.len() && !args.allow_shrink {
                return Err(GenerateError::ShrinkRefused {
                    previous: previous.signers.len(),
                    current: signers.len(),
                });
            }
            tracing::info!(
                "previous snapshot had {} signers (etag={:?}); proceeding (allow_shrink={})",
                previous.signers.len(),
                etag,
                args.allow_shrink,
            );
            etag
        }
        Err(e) => {
            // `--allow-shrink` only meaningfully relaxes a check that
            // requires a comparable previous snapshot to exist. When
            // none does (NotFound / corrupt / wrong-version / empty),
            // the flag is a no-op — but operators who pass it are
            // expressing the expectation that one exists, so surface
            // the mismatch rather than silently proceeding.
            if args.allow_shrink {
                tracing::warn!(
                    "--allow-shrink supplied but no comparable previous snapshot exists ({e}); \
                     flag has no effect — proceeding with first/recovery write"
                );
            } else {
                tracing::info!(
                    "no comparable previous snapshot ({e}); proceeding without shrink check or If-Match"
                );
            }
            None
        }
    };

    let snapshot =
        sp1_tee_host::attestations::build_snapshot(SP1_TEE_VERSION, signers, source_count);

    let key = sp1_tee_host::attestations::snapshot_key(SP1_TEE_VERSION);

    if args.dry_run {
        let body = bincode::serialize(&snapshot).expect("snapshot serialization is infallible");
        tracing::info!(
            "dry-run: would write {} bytes ({} signers, source_count={}, if_match={:?}) to {}/{}",
            body.len(),
            snapshot.signers.len(),
            snapshot.source_count,
            expected_etag,
            args.bucket,
            key,
        );
        return Ok(());
    }

    sp1_tee_host::attestations::write_signers_snapshot(
        &args.bucket,
        &snapshot,
        expected_etag.as_deref(),
    )
    .await?;
    tracing::info!(
        "wrote snapshot to {}/{} ({} signers, generated_at_unix={})",
        args.bucket,
        key,
        snapshot.signers.len(),
        snapshot.generated_at_unix,
    );
    Ok(())
}
