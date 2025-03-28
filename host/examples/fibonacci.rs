use alloy::network::EthereumWallet;
use alloy::primitives::Address;
use alloy::providers::Provider;
use alloy::providers::ProviderBuilder;
use alloy::signers::local::PrivateKeySigner;
use clap::Parser;
use sp1_sdk::network::tee::TEEProof;
use sp1_sdk::HashableKey;
use sp1_sdk::Prover;
use sp1_sdk::SP1Stdin;

use sp1_tee_host::contract::TEEVerifier as SP1Gateway;

#[derive(Debug, Parser)]
struct Args {
    /// The address to connect to.
    #[clap(short, long, default_value = "https://tee.production.succinct.xyz")]
    address: String,

    /// The number of fibonacci numbers to compute.
    #[clap(short, long, default_value = "10")]
    count: u32,

    /// The TEE verifier address on Anvil.
    #[clap(short, long)]
    verifier: Option<Address>,
}

#[tokio::main]
async fn main() {
    sp1_tee_host::init_tracing();

    let args = Args::parse();
    let program = include_bytes!("../../fixtures/fibonacci.elf");

    let mut stdin = SP1Stdin::new();
    stdin.write(&args.count);

    let signers = sp1_sdk::network::tee::get_tee_signers().await.unwrap();
    println!("Signers: {:?}", signers);

    let network_pk = std::env::var("NETWORK_PK").unwrap();
    let prover = sp1_sdk::ProverClient::builder()
        .network()
        .tee_signers(&signers)
        .private_key(&network_pk)
        .build();

    let (pk, vk) = prover.setup(program);
    let proof = prover
        .prove(&pk, &stdin)
        .plonk()
        .tee_proof(TEEProof::NitroIntegrity)
        .run()
        .unwrap();

    // Verify the proof with the rust verifier
    prover.verify(&proof, &vk).unwrap();
    println!("Proof verified with rust verifier");

    if let Some(verifier) = args.verifier {
        let provider = anvil_provider();

        let _ = provider.get_chain_id().await.expect("Failed to fetch chain id on default anvil ports, make sure anvil is running");

        let verifier = SP1Gateway::new(verifier, provider);

        let hash = verifier
            .verifyProof(
                vk.bytes32_raw().into(),
                proof.public_values.to_vec().into(),
                proof.bytes().into(),
            )
            .send()
            .await
            .unwrap()
            .watch()
            .await
            .unwrap();

        println!("Proof verified, tx hash: {:?}", hash);
    } else {
        println!("Proof bytes: {:?}", hex::encode(proof.bytes()));
        println!("VK: {:?}", vk.bytes32());
        println!(
            "public values: {:?}",
            hex::encode(proof.public_values.as_slice())
        );
    }
}

fn anvil_provider() -> impl Provider {
    let anvil_pk = "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80"
            .parse::<PrivateKeySigner>()
            .unwrap();

    let wallet = EthereumWallet::new(anvil_pk);
    let provider = ProviderBuilder::new()
        .wallet(wallet)
        .on_http("http://127.0.0.1:8545".parse().unwrap());

    provider
}

//
// Anvil Commands
// cargo run --bin sp1-tee-setup -- --deploy --anvil
//
// cargo run --example fibonacci --features client -- --verifier 0xcf7ed3acca5a467e9e704c703e8d87f634fb0fc9
//
