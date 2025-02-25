use std::path::PathBuf;
use std::process::Command;

use alloy::network::EthereumWallet;
use alloy::primitives::{keccak256, Address};
use alloy::providers::{Provider, ProviderBuilder};
use alloy::signers::local::PrivateKeySigner;
use aws_nitro_enclaves_cose::CoseSign1;
use aws_nitro_enclaves_nsm_api::api::AttestationDoc;
use serde::Deserialize;

use clap::Parser;

/// The folder containing the deployment (json) files.
const DEPLOYMNET_FOLDER: &str = "../contracts/deployments";

#[derive(Parser)]
#[clap(about = "
    Setup the SP1 TEE contracts.

    This command will deploy the contracts if the `deploy` flag is set to true.

    Otherwise, it will only add the PCRs to the existing contracts, and attempt to register the known certificates.
")]
struct Args {
    /// Whether or not to deploy the contracts.
    #[clap(long)]
    deploy: bool,

    /// The RPC_URL to use.
    #[clap(long)]
    rpc_url: String,

    /// The SP1 verifier gateway address.
    #[clap(long)]
    gateway: Option<String>,

    /// The private key to use.
    #[clap(long)]
    private_key: Option<String>,

    /// The etherscan API key to use.
    #[clap(long)]
    etherscan_api_key: Option<String>,

    /// The etherscan URL to use.
    #[clap(long)]
    etherscan_url: Option<String>,

    /// The path to the attestation document.
    #[clap(long)]
    cose_doc_path: PathBuf,
}

#[derive(Debug, Deserialize)]
struct Deployment {
    #[serde(rename = "CertManager")]
    cert_manager: Address,

    #[serde(rename = "SP1TeeVerifier")]
    sp1_tee_verifier: Address,
}

alloy::sol! {
    #[sol(rpc)]
    contract _TEEVerifier {
        /// @notice Sets a valid PCR0 corresponding to a program that runs an SP1 executor.
        ///
        /// @dev Only the owner can set a valid PCR0.
        function setValidPCR0(bytes memory pcr0) external;

        /// @notice Adds a signer to the list of signers, after validating an attestation.
        ///
        /// @dev Only the owner or the manager can add a signer.
        function addSigner(bytes memory attestationTbs, bytes memory signature) external;
    }

    #[sol(rpc)]
    contract _CertManager {
        function verified(bytes32 certHash) external view returns (bool);

        function verifyCACert(bytes memory cert, bytes32 parentCertHash) external;

        function verifyClientCert(bytes memory cert, bytes32 parentCertHash) external;
    }
}

type TEEVerifier<P, N> = _TEEVerifier::_TEEVerifierInstance<(), P, N>;
type CertManager<P, N> = _CertManager::_CertManagerInstance<(), P, N>;

#[tokio::main]
async fn main() {
    ///////////////////////////////
    // Global args.
    ///////////////////////////////

    let args = Args::parse();

    let pk = unwrap_or_env(&args.private_key, "PRIVATE_KEY");

    let signer = pk
        .parse::<PrivateKeySigner>()
        .expect("Invalid private key provided");
    let wallet = EthereumWallet::new(signer);

    let provider = ProviderBuilder::new()
        .wallet(wallet)
        .on_http(args.rpc_url.parse().expect("Failed to parse RPC url"));

    ///////////////////////////////
    // Deploy the contracts.
    ///////////////////////////////

    // Deploy the contracts if the flag is set.
    // Otherwise, we will only add the PCRs to the existing contracts.
    if args.deploy {
        println!("Deploying contracts..");

        let gateway = unwrap_or_env(&args.gateway, "SP1_VERIFIER_GATEWAY")
            .parse::<Address>()
            .expect("Failed to parse SP1_VERIFIER_GATEWAY");

        let mut command = Command::new("forge");
        command.current_dir("../contracts");
        command.env("SP1_VERIFIER_GATEWAY", gateway.to_string());

        // If the RPC url is anvil, we need to use the anvil deploy args
        if args.rpc_url.starts_with("http") {
            anvil_deploy_args(&mut command, &args);
        } else {
            deploy_args(&mut command, &args);
        }

        // Run the command.
        let output = command
            .arg("--private-key")
            .arg(pk)
            .output()
            .expect("Failed to run forge script");

        println!("{}", String::from_utf8_lossy(&output.stdout));

        // If panic, early exit with the output.
        if !output.status.success() {
            panic!(
                "Failed to deploy contracts, output: {:?}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
    }

    let chain_id = provider
        .get_chain_id()
        .await
        .expect("Failed to get chain id");

    // Load the deployment from the path.
    let deployment_path = format!("{}/{}.json", DEPLOYMNET_FOLDER, chain_id);
    let deployment: Deployment = serde_json::from_reader(
        std::fs::File::open(deployment_path).expect("Failed to open deployment.json"),
    )
    .expect("Failed to parse deployment.json");

    println!("Setting up the contracts...");

    ///////////////////////////////
    // Verify CA certs.
    ///////////////////////////////

    // Read the attestation doc from the path.
    let cose_doc = std::fs::read(args.cose_doc_path).expect("Failed to read attestation doc");
    let cose_doc = CoseSign1::from_bytes(&cose_doc).expect("Failed to parse COSE doc");
    let attestation_doc = AttestationDoc::from_binary(&cose_doc.payload).expect("Failed to parse attestation document");

    // Add the CA certs to the cert manager.
    let cert_manager = CertManager::new(deployment.cert_manager, &provider);

    // The root of trust is self signed, so we can just use the first CA in the bundle.
    let mut parent_cert_hash = keccak256(attestation_doc.cabundle[0].clone());
    for ca in attestation_doc.cabundle {
        // Dont verify a cert twice.
        if cert_manager
            .verified(keccak256(&ca))
            .call()
            .await
            .expect("Failed to check if CA cert is verified")
            ._0
        {
            println!("CA cert already verified: {}", keccak256(&ca));
            continue;
        }

        // Hash the cert, this is the next parent cert hash.
        let hash = keccak256(&ca);

        // Submit the TX to verify the CA cert.
        let _ = cert_manager
            .verifyCACert(ca.into_vec().into(), parent_cert_hash)
            .send()
            .await
            .expect("Failed to verify CA cert")
            .watch()
            .await
            .expect("Failed to get tx hash while verifying CA cert");

        parent_cert_hash = hash;
    }
    println!("All CA certs verified.");

    ///////////////////////////////
    // Verify client cert.
    ///////////////////////////////

    // Add the client certs to the cert manager.
    let _ = cert_manager
        .verifyClientCert(attestation_doc.certificate.into_vec().into(), parent_cert_hash)
        .send()
        .await
        .expect("Failed to verify client cert")
        .watch()
        .await
        .expect("Failed to get tx hash while verifying client cert");

    println!("Client cert verified.");

    ///////////////////////////////
    // Set the valid PCR0s.
    ///////////////////////////////

    // Finally, we can set the valid PCR0s.
    let tee_verifier = TEEVerifier::new(deployment.sp1_tee_verifier, &provider);

    let _ = tee_verifier
        .setValidPCR0(attestation_doc.pcrs.get(&0).expect("Failed to get PCR0").clone().into_vec().into())
        .send()
        .await
        .expect("Failed to set valid PCR0")
        .watch()
        .await;

    println!("Valid PCR0 set.");

    ///////////////////////////////
    // Register the signer.
    ///////////////////////////////

    let _ = tee_verifier.addSigner(cose_doc.payload.into_vec().into(), cose_doc.signature.into_vec().into())
        .send()
        .await
        .expect("Failed to register signer")
        .watch()
        .await;

    println!("Signer registered.");
    println!("Setup complete, dont forget to change the owner and manager of the contracts!");
}

fn unwrap_or_env(value: &Option<String>, env_var: &str) -> String {
    match value {
        Some(value) => value.clone(),
        None => std::env::var(env_var).expect(format!("{} env var is not set", env_var).as_str()),
    }
}

fn deploy_args(cmd: &mut Command, args: &Args) {
    let etherscan_url = unwrap_or_env(&args.etherscan_url, "ETHERSCAN_URL");
    let etherscan_api_key = unwrap_or_env(&args.etherscan_api_key, "ETHERSCAN_API_KEY");

    cmd.args(&[
        "script",
        "script/Deploy.s.sol",
        "--rpc-url",
        &args.rpc_url,
        "--verify",
        "--verifier",
        "etherscan",
        "--verifier-url",
        &etherscan_url,
        "--verifier-api-key",
        &etherscan_api_key,
        "--broadcast",
    ])
    .output()
    .expect("Failed to run forge script");
}

fn anvil_deploy_args(cmd: &mut Command, args: &Args) {
    cmd.args(&[
        "script",
        "script/Deploy.s.sol",
        "--rpc-url",
        &args.rpc_url,
        "--broadcast",
    ]);
}
