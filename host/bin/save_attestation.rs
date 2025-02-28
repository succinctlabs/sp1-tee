#[tokio::main]
async fn main() {
    sp1_tee_host::save_attestation(Default::default()).await.unwrap();
}