use std::path::PathBuf;

use alloy::{
    network::EthereumWallet,
    primitives::Address,
    providers::{Provider, ProviderBuilder},
    signers::local::PrivateKeySigner,
};
use serde::Deserialize;

use crate::{
    attestations::{retrieve_attestation_from_enclave, verify_attestation},
    ethereum_address_from_sec1_bytes,
    server::ServerArgs,
    TEEVerifier,
};

#[derive(Debug, thiserror::Error)]
pub enum RegisterSignerError {
    #[error("Failed to parse private key")]
    FailedToParsePrivateKey,

    #[error("Failed to parse RPC URL")]
    FailedToParseRpcUrl,

    #[error("Failed to open deployment.json")]
    FailedToOpenDeploymentJson,

    #[error("Failed to open deployment.json")]
    FailedToParseDeploymentJson,

    #[error("The public key is not set in attestation")]
    PublicKeyNotSet,

    #[error("Failed to derive address from public key")]
    FailedToDeriveAddress,

    #[error("Address mismatch expected: {expected}, got: {got}")]
    AddressMismatch { expected: Address, got: Address },

    #[error("Failed to retrieve attestations: {0}")]
    FailedToGetAttestations(#[from] crate::attestations::RetrieveAttestationError),

    #[error("Provider transport error: {0}")]
    TransportError(#[from] alloy::transports::TransportError),

    #[error("Provider contract error: {0}")]
    ContractError(#[from] alloy::contract::Error),

    #[error("Pending transaction error: {0}")]
    PendingTransactionError(#[from] alloy::providers::PendingTransactionError),

    #[error("Attest error: {0}")]
    AttestError(#[from] attestation_doc_validation::error::AttestError),
}

#[derive(Debug, Deserialize)]
struct Deployment {
    #[serde(rename = "SP1TeeVerifier")]
    sp1_tee_verifier: Address,
}

pub async fn register_signer(args: &ServerArgs, port: u16) -> Result<(), RegisterSignerError> {
    let signer = args
        .private_key
        .parse::<PrivateKeySigner>()
        .map_err(|_| RegisterSignerError::FailedToParsePrivateKey)?;

    let wallet = EthereumWallet::new(signer);
    let provider = ProviderBuilder::new().wallet(wallet).connect_http(
        args.rpc_url
            .parse()
            .map_err(|_| RegisterSignerError::FailedToParseRpcUrl)?,
    );
    let chain_id = provider.get_chain_id().await?;
    let sp1_tee_verifier_address = retrieve_tee_verifier_contract_address(chain_id)?;
    let verifier = TEEVerifier::new(sp1_tee_verifier_address, provider);
    let raw_attestation = retrieve_attestation_from_enclave(args.enclave_cid, port).await?;
    let doc = verify_attestation(&raw_attestation.attestation)?;

    // Derive the address from the public key.
    let pubkey_bytes = doc
        .public_key
        .ok_or_else(|| RegisterSignerError::PublicKeyNotSet)?;

    let derived_address = ethereum_address_from_sec1_bytes(&pubkey_bytes)
        .ok_or_else(|| RegisterSignerError::FailedToDeriveAddress)?;

    if derived_address != raw_attestation.address {
        return Err(RegisterSignerError::AddressMismatch {
            expected: raw_attestation.address,
            got: derived_address,
        });
    }

    verifier
        .addSigner(raw_attestation.address)
        .send()
        .await?
        .watch()
        .await?;

    Ok(())
}

pub fn retrieve_tee_verifier_contract_address(
    chain_id: u64,
) -> Result<Address, RegisterSignerError> {
    // Load the deployment from the path.
    let deployment_path = contracts_path(chain_id);
    let deployment: Deployment = serde_json::from_reader(
        std::fs::File::open(deployment_path)
            .map_err(|_| RegisterSignerError::FailedToOpenDeploymentJson)?,
    )
    .map_err(|_| RegisterSignerError::FailedToParseDeploymentJson)?;

    Ok(deployment.sp1_tee_verifier)
}

fn contracts_path(chain_id: u64) -> PathBuf {
    const MANIFEST_DIR: &str = env!("CARGO_MANIFEST_DIR");

    let mut path = PathBuf::from(MANIFEST_DIR);
    path = path
        .parent()
        .expect("Failed to get parent of manifest dir")
        .to_path_buf();

    path.push("contracts");
    path.push("deployments");
    path.push(format!("{}.json", chain_id));

    path
}
