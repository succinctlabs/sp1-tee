use std::process::Command;

use alloy::network::EthereumWallet;
use alloy::primitives::Address;
use alloy::providers::{Provider, ProviderBuilder};
use alloy::signers::local::PrivateKeySigner;
use serde::Deserialize;

use clap::Parser;

/// The folder containing the deployment (json) files.
const DEPLOY_FOLDER: &str = "contracts/deployments";

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
}

#[derive(Debug, Deserialize)]
struct Deployment {
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
}

type TEEVerifier<P, N> = _TEEVerifier::_TEEVerifierInstance<(), P, N>;

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
        command.current_dir("contracts");
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
    let deployment_path = format!("{}/{}.json", DEPLOY_FOLDER, chain_id);
    let deployment: Deployment = serde_json::from_reader(
        std::fs::File::open(deployment_path).expect("Failed to open deployment.json"),
    )
    .expect("Failed to parse deployment.json");

    ///////////////////////////////
    // Add the signers
    ///////////////////////////////
    
    // Loop over the address in the s3 bucket (and probably verify the attestations),
    // and add them to the contracts.

    // todo!()

    println!("Setup complete.");
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
        "--code-size-limit",
        "40000"
    ]);
}
