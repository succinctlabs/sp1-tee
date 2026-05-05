//! HTTP stub serving `/signers` from cached S3 attestations.
//!
//! Mirrors the `/signers` handler in `bin/server.rs` but does NOT start a
//! Nitro Enclave or accept `/execute` requests. Designed to run on tiny
//! instances (e.g. Lambda via lambda-web-adapter, or `t4g.nano`) so the
//! SDK cold-start fetch path stays cheap and reachable while the
//! production enclave fleet is scaled to zero between SP1 upgrades.
//!
//! Anyone with read-only access to the public `sp1-tee-attestations` S3
//! bucket can run this — no enclave, no signing keys, no secrets needed.

use axum::{
    body::Bytes,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use clap::Parser;
use sp1_sdk::network::tee::SP1_TEE_VERSION;
use std::net::SocketAddr;
use tokio::net::TcpListener;

#[derive(Parser)]
struct Args {
    #[clap(short, long, default_value = "0.0.0.0")]
    address: String,

    #[clap(short, long, default_value = "8080")]
    port: u16,
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

    let app = Router::new()
        .route("/signers", get(get_signers))
        .route("/health", get(health));

    let addr = SocketAddr::new(args.address.parse().expect("Invalid address"), args.port);

    let listener = TcpListener::bind(addr)
        .await
        .expect("Failed to bind to address");

    tracing::info!("signers-stub listening on {}", addr);

    axum::serve(listener, app.into_make_service())
        .await
        .expect("Server error");
}

async fn health() -> &'static str {
    "ok"
}

#[tracing::instrument(skip_all)]
async fn get_signers() -> Result<Response, StatusCode> {
    // Snapshot path. The snapshot stores `Vec<RawAttestation>`; we re-run
    // `derive_signers` on every request so the Nitro CA chain + 3h cert
    // expiry are still enforced — i.e., S3 PutObject on the snapshot key is
    // not by itself sufficient to forge the served signer set.
    let raw = match sp1_tee_host::attestations::read_signers_snapshot(SP1_TEE_VERSION).await {
        Ok(raw) => raw,
        Err(sp1_tee_host::attestations::ReadSnapshotError::NotFound(key)) => {
            tracing::warn!("snapshot {key} not found; falling back to live derivation (bootstrap)");
            return live_derivation_response().await;
        }
        Err(e) => {
            tracing::error!(alert = true, "snapshot fetch failed: {e}");
            return Err(StatusCode::SERVICE_UNAVAILABLE);
        }
    };

    let signers = sp1_tee_host::attestations::derive_signers(&raw, SP1_TEE_VERSION);
    if signers.is_empty() {
        tracing::error!(
            alert = true,
            "snapshot decoded with {} entries but produced 0 valid signers (all expired or version mismatch)",
            raw.len()
        );
        return Err(StatusCode::SERVICE_UNAVAILABLE);
    }

    tracing::info!(
        "Serving {} signers from snapshot ({} raw attestations)",
        signers.len(),
        raw.len()
    );

    let body = bincode::serialize(&signers).map_err(|e| {
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

/// Bootstrap-only fallback: derive signers from raw attestations at the
/// bucket root. Used when the snapshot key doesn't exist yet (first deploy
/// or pre-snapshot environments). Any other snapshot read failure returns
/// 503 instead of taking this path, so an attacker cannot force-downgrade
/// production to live derivation by perturbing the snapshot fetch.
async fn live_derivation_response() -> Result<Response, StatusCode> {
    let raw_attestations = sp1_tee_host::attestations::get_raw_attestations()
        .await
        .map_err(|e| {
            tracing::error!("Failed to fetch attestations from S3: {e}");
            StatusCode::SERVICE_UNAVAILABLE
        })?;

    let signers = sp1_tee_host::attestations::derive_signers(&raw_attestations, SP1_TEE_VERSION);
    if signers.is_empty() {
        tracing::error!(
            alert = true,
            "live derivation produced 0 valid signers from {} raw attestations",
            raw_attestations.len()
        );
        return Err(StatusCode::SERVICE_UNAVAILABLE);
    }

    tracing::info!("Returning {} signers (live derivation)", signers.len());

    let body = bincode::serialize(&signers).map_err(|e| {
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
