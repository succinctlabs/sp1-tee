use std::sync::Arc;
use rand::Rng;

use sp1_sdk::{Prover, ProverClient, TEEProof};

#[tokio::main]
async fn main() {
    const RSP_ELF: &[u8] = include_bytes!("../../fixtures/rsp.elf");
    const RSP_INPUT: &[u8] = include_bytes!("../../fixtures/rsp-input.bin");
    
    let mut stdin = sp1_sdk::SP1Stdin::new();
    stdin.write_slice(RSP_INPUT);
    let stdin = Arc::new(stdin);

    // Initialize the RNG.
    let mut rng = rand::thread_rng();

    let concurrent_requests_max: u32 = 50;

    let network_pk = std::env::var("NETWORK_PK").unwrap();
    let prover = ProverClient::builder().network().private_key(&network_pk).build();
    let prover = Arc::new(prover);

    let (pk, _) = prover.setup(RSP_ELF);
    let pk = Arc::new(pk);

    // The number of minutes to sleep between requests.
    let mut sleep: u32 = rng.gen_range(1..=60);

    loop {
        let concurrent_requests = rng.gen_range(1..=concurrent_requests_max);

        println!("Starting {} requests", concurrent_requests);

        let requests = (0..concurrent_requests).map(|i| {
            let pk = pk.clone();
            let prover = prover.clone();
            let stdin = stdin.clone();

            async move {
                if let Err(e) = prover.prove(&pk, &stdin).tee_proof(TEEProof::NitroIntegrity).await {
                    println!("Error getting proof for request {}: {}", i, e);
                }
            }
        }).collect::<Vec<_>>();

        let _ = futures::future::join_all(requests).await;

        println!("Completed {} requests", concurrent_requests);

        tokio::time::sleep(tokio::time::Duration::from_secs(sleep as u64)).await;

        sleep = rng.gen_range(1..=60);
    }
}
