//! Deploy the SP1 TEE contracts.
//!
//! Otherwise, it will just try to add signers to the existing contracts.
use std::path::PathBuf;
use std::process::{Command, Stdio};

use alloy::network::EthereumWallet;
use alloy::primitives::Address;
use alloy::providers::{Provider, ProviderBuilder, WalletProvider};
use alloy::signers::local::PrivateKeySigner;
use serde::Deserialize;

use clap::Parser;

use sp1_tee_host::attestations::RawAttestation;
use sp1_tee_host::contract::TEEVerifier;
use sp1_tee_host::ethereum_address_from_sec1_bytes;
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
#[clap(about = "
    Setup the SP1 TEE contracts.

    This command will deploy the contracts if the `deploy` flag is set to true.

    Otherwise, it will only add the PCRs to the existing contracts, and attempt to register the known certificates.
")]
struct Args {
    /// Whether or not to deploy the contracts.
    #[clap(long, default_value_if("anvil", "true", "true"))]
    deploy: bool,

    /// Deploy to anvil.
    #[clap(long, requires_if("false", "rpc_url"))]
    anvil: bool,

    /// The RPC_URL to use, if anvil modes uses the default anvil port.
    #[clap(
        long,
        required(false),
        default_value_if("anvil", "true", "http://localhost:8545")
    )]
    rpc_url: String,

    /// The private key to use.
    ///
    /// This defaults to the anvil private key if not deploying or in anvil mode.
    ///
    /// ENV VAR: `PRIVATE_KEY`
    #[clap(
        long,
        default_value_if(
            "anvil",
            "true",
            "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80"
        )
    )]
    private_key: Option<String>,

    /// The etherscan API key to use.
    ///
    /// This will otherwise be loaded from the env, ignored if deploying to anvil.
    ///
    /// ENV VAR: `ETHERSCAN_API_KEY`
    #[clap(long)]
    etherscan_api_key: Option<String>,

    /// The etherscan URL to use.
    ///
    /// This will otherwise be loaded from the env, ignored if deploying to anvil.
    ///
    /// ENV VAR: `ETHERSCAN_URL`
    #[clap(long)]
    etherscan_url: Option<String>,

    /// An optional (hex-encoded) PCR0 to check against when verifying attestations.
    /// This ensures the correct program is being run on the enclave.
    ///
    /// In debug mode, PCR0s are not included in the attestations.
    #[clap(long)]
    pcr0: Option<String>,

    /// If we should attempt to register the signers with the contracts,
    /// if this flag is not set, we will verify the attestations and print the addresses.
    ///
    /// Default is true if deploying.
    #[clap(long, default_value_if("deploy", "true", "true"))]
    register_signers: bool,

    /// The address of the verifier gateway.
    ///
    /// ENV VAR: `SP1_VERIFIER_GATEWAY`
    #[clap(long)]
    verifier_gateway: Option<Address>,
}

#[derive(Debug, Deserialize)]
struct Deployment {
    #[serde(rename = "SP1TeeVerifier")]
    sp1_tee_verifier: Address,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    ///////////////////////////////
    // Global args.
    ///////////////////////////////

    let mut args = Args::parse();

    let pk = unwrap_or_env(&args.private_key, "PRIVATE_KEY");

    let signer = pk
        .parse::<PrivateKeySigner>()
        .expect("Invalid private key provided");
    let wallet = EthereumWallet::new(signer);

    let provider = ProviderBuilder::new()
        .wallet(wallet)
        .connect_http(args.rpc_url.parse().expect("Failed to parse RPC url"));

    // This can only be reached iff `deploy` is true & the pk was taken from the env.
    if args.private_key.is_none() {
        args.private_key = Some(pk.clone());
    }

    ///////////////////////////////
    // Deploy the contracts.
    ///////////////////////////////

    // Deploy the contracts if the flag is set.
    // Otherwise, we will only add the PCRs to the existing contracts.
    if args.deploy {
        println!("Deploying contracts..");

        let mut command = Command::new("forge");
        command.current_dir("contracts");

        // If the RPC url is anvil, we need to use the anvil deploy args
        if !args.rpc_url.starts_with("https://") {
            anvil_deploy_args(&mut command, &args, &provider);
        } else {
            deploy_args(&mut command, &args, &provider);
        }

        // Run the command, piping the output to the parent process.
        let output = command
            .stderr(Stdio::inherit())
            .stdout(Stdio::inherit())
            .output()
            .expect("Failed to run forge script");

        // If panic, early exit with the output.
        if !output.status.success() {
            panic!("Failed to deploy contracts");
        }
    }

    let chain_id = provider
        .get_chain_id()
        .await
        .expect("Failed to get chain id");

    // Load the deployment from the path.
    let deployment_path = contracts_path(chain_id);
    let deployment: Deployment = serde_json::from_reader(
        std::fs::File::open(deployment_path).expect("Failed to open deployment.json"),
    )
    .expect("Failed to parse deployment.json");

    println!("Deployed Verifier: {:?}", deployment.sp1_tee_verifier);

    ///////////////////////////////
    // Add the signers
    ///////////////////////////////

    let attestations = sp1_tee_host::attestations::get_raw_attestations()
        .await
        .expect("Failed to get attestations");

    let verifier = TEEVerifier::new(deployment.sp1_tee_verifier, provider);

    // For each attestation, verify the attestation and add the signer, optionally checking the PCR0.
    for RawAttestation {
        address,
        attestation,
    } in attestations
    {
        // Verify the attestation.
        let doc = match sp1_tee_host::attestations::verify_attestation(&attestation) {
            Ok(doc) => doc,
            Err(e) => {
                eprintln!(
                    "Failed to verify attestation for address: {:?}, error: {:?}",
                    address, e
                );
                eprintln!("Its possible this can happen if an enclave goes down, and the expiry period has not been reached yet.");
                continue;
            }
        };

        // PCR0 is optional, as in debug mode PCR0 is empty in the attestation.
        if let Some(ref pcr0) = args.pcr0 {
            let pcr0_bytes = hex::decode(pcr0).expect("Failed to decode pcr0");
            let pcr0_bytes = pcr0_bytes.as_slice();

            if doc.pcrs[&0] != pcr0_bytes {
                panic!(
                    "PCR0 mismatch for address: {}, expected: {:?}, got: {:?}",
                    address, pcr0_bytes, doc.pcrs[&0]
                );
            }
        }

        // Derive the address from the public key.
        let pubkey_bytes = doc
            .public_key
            .expect("Public key is not set in attestation");

        let derived_address = ethereum_address_from_sec1_bytes(&pubkey_bytes)
            .expect("Failed to derive address from public key");

        if derived_address != address {
            panic!(
                "Address mismatch expected: {:?}, got: {:?}",
                address, derived_address
            );
        }

        // Check if the signer is already registered.
        if verifier
            .isSigner(address)
            .call()
            .await
            .expect("Failed to check if signer is registered")
        {
            // This signer is already registered, so continue.
            continue;
        }

        if args.register_signers {
            verifier
                .addSigner(address)
                .send()
                .await
                .expect("Failed send tx to add signer")
                .watch()
                .await
                .expect("Failed to get confirmation of adding signer");

            println!("Added signer: {:?}", address);
        } else {
            println!("Found valid signer: {:?}", address);
        }
    }

    if args.deploy {
        println!("Setup complete, ownership is set to the sender.");
    } else {
        println!("Setup complete.");
    }
}

fn unwrap_or_env(value: &Option<String>, env_var: &str) -> String {
    match value {
        Some(value) => value.clone(),
        None => std::env::var(env_var).unwrap_or_else(|_| {
            panic!(
                "{} env var is not set, and was not provided in the Args.",
                env_var
            )
        }),
    }
}

fn deploy_args<P: WalletProvider>(cmd: &mut Command, args: &Args, provider: &P) {
    // let etherscan_url = unwrap_or_env(&args.etherscan_url, "ETHERSCAN_URL");
    let etherscan_api_key = unwrap_or_env(&args.etherscan_api_key, "ETHERSCAN_API_KEY");
    let verifier_gateway = unwrap_or_env(
        &args.verifier_gateway.as_ref().map(|a| a.to_string()),
        "SP1_VERIFIER_GATEWAY",
    );

    println!(
        "Deploying contracts with verifier gateway: {}",
        verifier_gateway
    );

    // NOTE: Private key is overriden on the `Args` type, so we don't check the env here.
    cmd.env("SP1_VERIFIER_GATEWAY", &verifier_gateway).args([
        "script",
        "script/Deploy.s.sol",
        "--rpc-url",
        &args.rpc_url,
        "--verify",
        "--etherscan-api-key",
        &etherscan_api_key,
        "--broadcast",
        "--sender",
        &provider.default_signer_address().to_string(),
        "--private-key",
        args.private_key.as_ref().expect("Private key is not set"),
    ]);
}

fn anvil_deploy_args<P: WalletProvider>(cmd: &mut Command, args: &Args, provider: &P) {
    cmd.args([
        "script",
        "script/Deploy.s.sol",
        "--rpc-url",
        &args.rpc_url,
        "--broadcast",
        "--sender",
        &provider.default_signer_address().to_string(),
        "--private-key",
        args.private_key.as_ref().expect("Private key is not set"),
    ]);
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
