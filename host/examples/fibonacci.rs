use alloy::primitives::Address;
use alloy::providers::Provider;
use sp1_tee_host::api::TEERequest;
use sp1_tee_host::Client;
use sp1_sdk::SP1Stdin;
use sp1_sdk::network::tee::TEEProof;
use clap::Parser;

#[derive(Debug, Parser)]
struct Args {
    /// The address to connect to.
    #[clap(short, long, default_value = "https://tee.production.succinct.xyz")]
    address: String,

    /// The number of fibonacci numbers to compute.
    #[clap(short, long, default_value = "10")]
    count: u32,

    /// The private key to use for the prover.
    #[clap(short, long)]
    pk: String,

    /// The contract address to submit the proof to.
    #[clap(short, long)]
    contract: Address,
}

#[tokio::main]
async fn main() { 
    let args = Args::parse();
    let program = include_bytes!("../../fixtures/fibonacci.elf");

    let mut stdin = SP1Stdin::new();
    stdin.write(&args.count);

    let prover = sp1_sdk::ProverClient::builder().network().private_key(&args.pk).build();
    let (pk, vk) = prover.setup(program);
    let proof = prover.prove(&pk, &stdin)
        .with_tee_integrity_proof(TEEProof::NitroIntegrity)
        .run()
        .unwrap();

    println!("Proof bytes: {:?}", proof.bytes());
}