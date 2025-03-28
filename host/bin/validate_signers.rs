use alloy::primitives::Address;
use clap::{Parser, Subcommand};
use sp1_tee_host::attestations::RawAttestation;

#[derive(Parser)]
struct Args {
    /// The command to run.
    #[clap(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Validate a single signer.
    Signer {
        /// The signer to validate.
        signer: Address,
        /// The PCR0 value to validate the signer against.
        pcr0: String,
    },
    /// Validate all signers listed on a TEE verifier contract.
    Contract {
        /// The address of the contract to validate the signers of.
        contract: Address,
        /// The RPC URL to use to validate the signers.
        rpc_url: String,
        /// The PCR0 value to validate the signers against.
        pcr0: String,
    },
    /// Validate all signers that are attested in the s3 bucket.
    All {
        /// The PCR0 value to validate the signers against.
        pcr0: String,
    },
}

#[tokio::main]
async fn main() {
    let args = Args::parse();

    match args.command {
        Command::Signer { signer, pcr0 } => {
            sp1_tee_host::attestations::verify_attestation_for_signer(signer, &pcr0)
                .await
                .unwrap();

            println!("Validated signer: {:?}", signer);
        }
        Command::Contract {
            contract,
            pcr0,
            rpc_url,
        } => {
            let provider =
                alloy::providers::ProviderBuilder::new().on_http(rpc_url.parse().unwrap());

            let instance = sp1_tee_host::contract::TEEVerifier::new(contract, provider);

            let signers = instance.getSigners().call().await.unwrap()._0;

            for signer in signers {
                println!("-----------------------------------");

                sp1_tee_host::attestations::verify_attestation_for_signer(signer, &pcr0)
                    .await
                    .unwrap();

                println!("Validated signer: {:?}", signer);
            }
        }
        Command::All { pcr0 } => {
            let attestations = sp1_tee_host::attestations::get_raw_attestations()
                .await
                .expect("Failed to get attestations");

            // For each attestation, verify the attestation and add the signer, optionally checking the PCR0.
            for RawAttestation {
                address,
                attestation,
            } in attestations
            {
                println!("-----------------------------------");

                // Verify the attestation.
                let doc = match sp1_tee_host::attestations::verify_attestation(&attestation) {
                    Ok(doc) => doc,
                    Err(e) => {
                        eprintln!(
                            "Failed to verify attestation for address: {:?}, error: {:?}",
                            address, e
                        );
                        eprintln!("Its possible this can happen if an enclave goes down, and the expiry period has not been reached yet.");
                        eprintln!("");
                        continue;
                    }
                };

                // Verify the PCR0 value.
                let doc_pcr0 = hex::encode(doc.pcrs[&0].as_ref());
                if doc_pcr0 != pcr0.replace("0x", "") {
                    eprintln!(
                        "PCR0 mismatch for address: {}, expected: {:?}, got: {:?}",
                        address, pcr0, doc_pcr0
                    );
                    continue;
                }

                // Derive the address from the public key.
                let pubkey_bytes = doc
                    .public_key
                    .expect("Public key is not set in attestation");

                let derived_address = sp1_tee_host::ethereum_address_from_sec1_bytes(&pubkey_bytes)
                    .expect("Failed to derive address from public key");

                if derived_address != address {
                    panic!(
                        "Address mismatch expected: {:?}, got: {:?}",
                        address, derived_address
                    );
                }

                println!("Validated signer: {:?}", address);
            }
        }
    }
}
