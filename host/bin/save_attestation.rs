#[tokio::main]
async fn main() {
    sp1_tee_host::save_attestation(
        sp1_tee_host::SaveAttestationArgs {
            bucket: "sp1-tee-attestations-testing".to_string(),
            ..Default::default()
        }
    ).await.unwrap();
}