//! HTTP stub serving `/signers` from the durable signers snapshot in S3.
//!
//! Mirrors the SDK-facing wire format of the original `/signers` handler in
//! `bin/server.rs` (bincode `Vec<Address>`) but does NOT start a Nitro
//! Enclave or accept `/execute`/`/address` requests. The serving artifact
//! is the snapshot written by `sp1-tee-snapshot-generate`, so the stub does
//! not depend on raw-attestation freshness or on the versioned enclave fleet
//! being up.
//!
//! Snapshot is read from a private CDK-managed bucket using ambient AWS
//! credentials (the EC2 instance role grants scoped `s3:GetObject` on the
//! snapshot key). The legacy anonymous-read attestations bucket is not
//! consulted on the steady-state path — the snapshot is the complete trust
//! artifact.

use axum::{
    body::Bytes,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use clap::Parser;
use sp1_sdk::network::tee::SP1_TEE_VERSION;
use sp1_tee_host::attestations::read_signers_snapshot;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;

#[derive(Parser, Debug)]
struct Args {
    #[clap(short, long, default_value = "0.0.0.0")]
    address: String,

    #[clap(short, long, default_value = "8080")]
    port: u16,

    /// Snapshots bucket (private, CDK-managed). May also be set via the
    /// `SP1_TEE_SNAPSHOTS_BUCKET` env var (which is what the systemd unit
    /// uses in production).
    #[clap(long, env = "SP1_TEE_SNAPSHOTS_BUCKET")]
    bucket: String,
}

#[derive(Clone)]
struct AppState {
    bucket: Arc<String>,
}

#[tokio::main]
async fn main() {
    rustls::crypto::aws_lc_rs::default_provider()
        .install_default()
        .expect("Failed to install rustls provider");

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    let args = Args::parse();

    let state = AppState {
        bucket: Arc::new(args.bucket.clone()),
    };

    let app = Router::new()
        .route("/signers", get(get_signers))
        .route("/health", get(health))
        .with_state(state);

    let addr = SocketAddr::new(args.address.parse().expect("Invalid address"), args.port);

    let listener = TcpListener::bind(addr)
        .await
        .expect("Failed to bind to address");

    tracing::info!(
        "signers-stub listening on {addr}, snapshot bucket={}",
        args.bucket
    );

    axum::serve(listener, app.into_make_service())
        .await
        .expect("Server error");
}

// Process-up health check only. Deliberately does NOT validate snapshot
// availability — making ALB health depend on snapshot reads would create
// a replacement loop on cold starts before the snapshot exists, and on
// any S3 transient. Snapshot freshness/correctness is observed via the
// stub target group's 5xx rate alarm and a synthetic `/signers` canary
// (tracked as follow-up; see PR description).
async fn health() -> &'static str {
    "ok"
}

#[tracing::instrument(skip_all)]
async fn get_signers(
    axum::extract::State(state): axum::extract::State<AppState>,
) -> Result<Response, StatusCode> {
    let snapshot = match read_signers_snapshot(&state.bucket, SP1_TEE_VERSION).await {
        Ok(s) => s,
        Err(e) => {
            // Every failure mode (missing/corrupt/oversize/wrong-version/empty)
            // surfaces as 503. There is intentionally no live-derivation
            // fallback — the snapshot is the durable serving artifact, and
            // silently falling back would re-couple `/signers` to raw
            // attestation freshness.
            tracing::error!(alert = true, "snapshot read failed: {e}");
            return Err(StatusCode::SERVICE_UNAVAILABLE);
        }
    };

    tracing::info!(
        "Serving {} signers from snapshot (sp1_tee_version={}, generated_at_unix={}, source_count={})",
        snapshot.signers.len(),
        snapshot.sp1_tee_version,
        snapshot.generated_at_unix,
        snapshot.source_count,
    );

    // Wire format: bincode::serialize(&Vec<Address>). Identical to the
    // historical `/signers` response from the versioned enclave fleet, so
    // SDK clients see no change.
    let body = bincode::serialize(&snapshot.signers).map_err(|e| {
        tracing::error!("Failed to serialize signers: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok((
        StatusCode::OK,
        [("content-type", "application/octet-stream")],
        Bytes::from(body),
    )
        .into_response())
}
