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
    let raw_attestations = sp1_tee_host::attestations::get_raw_attestations()
        .await
        .map_err(|e| {
            tracing::error!("Failed to fetch attestations from S3: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let total = raw_attestations.len();
    tracing::debug!("Fetched {} raw attestations", total);

    let signers: Vec<_> = raw_attestations
        .iter()
        .filter_map(|raw| sp1_tee_host::attestations::verify_attestation(&raw.attestation).ok())
        .filter(|doc| {
            doc.user_data
                .as_ref()
                .map(|u| u == &SP1_TEE_VERSION.to_le_bytes())
                .unwrap_or(false)
        })
        .filter_map(|doc| sp1_tee_host::ethereum_address_from_sec1_bytes(doc.public_key.as_ref()?))
        .collect();

    let skipped = total.saturating_sub(signers.len());
    if skipped > 0 {
        tracing::warn!(
            "Skipped {} of {} attestations (verify failed, version mismatch, or bad pubkey)",
            skipped,
            total
        );
    }

    tracing::info!("Returning {} signers", signers.len());

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
