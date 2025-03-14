use rand::Rng;
use std::sync::Arc;

use sp1_sdk::{Prover, ProverClient, TEEProof};

#[tokio::main]
async fn main() {
    sp1_tee_host::init_tracing();

    const RSP_ELF: &[u8] = include_bytes!("../../fixtures/rsp.elf");
    const RSP_INPUT: &[u8] = include_bytes!("../../fixtures/rsp-input.bin");

    let mut stdin = sp1_sdk::SP1Stdin::new();
    stdin.write_slice(RSP_INPUT);
    let stdin = Arc::new(stdin);

    // Initialize the RNG.
    let mut rng = rand::thread_rng();

    let concurrent_requests_max: u32 = 25;

    let network_pk = std::env::var("NETWORK_PK").unwrap();
    let prover = ProverClient::builder()
        .network()
        .private_key(&network_pk)
        .build();
    let prover = Arc::new(prover);

    let (pk, _) = prover.setup(RSP_ELF);
    let pk = Arc::new(pk);
    tracing::info!("Setup complete");

    // The number of minutes to sleep between requests.
    let mut sleep: u32 = rng.gen_range(1..=20);

    loop {
        let concurrent_requests = rng.gen_range(1..=concurrent_requests_max);

        println!("Starting {} requests", concurrent_requests);

        let requests = (0..concurrent_requests)
            .map(|i| {
                let pk = pk.clone();
                let prover = prover.clone();
                let stdin = stdin.clone();

                async move {
                    if let Err(e) = prover
                        .prove(&pk, &stdin)
                        .compressed()
                        .cycle_limit(900_000_000)
                        .gas_limit(1000000000)
                        .tee_proof(TEEProof::NitroIntegrity)
                        .await
                    {
                        tracing::error!(
                            alert = true,
                            "Enclave stress test error \n Error getting proof for request {}: {}",
                            i,
                            e
                        );
                    }
                }
            })
            .collect::<Vec<_>>();

        let _ = futures::future::join_all(requests).await;

        tracing::info!("Completed {} requests", concurrent_requests);
        tracing::info!("Sleeping for {} minutes", sleep);

        tokio::time::sleep(tokio::time::Duration::from_secs(sleep as u64 * 60)).await;

        sleep = rng.gen_range(1..=60);
    }
}
