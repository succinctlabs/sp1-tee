use alloy::primitives::Address;
use clap::{Parser, Subcommand};
use sp1_tee_host::attestations::AttestationVerificationError;

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
        #[clap(long)]
        pcr0: String,
        /// The SP1 circuit version to validate the signer against.
        #[clap(long)]
        version: u32,
    },
    /// Validate all signers listed on a TEE verifier contract.
    Contract {
        /// The address of the contract to validate the signers of.
        contract: Address,
        /// The RPC URL to use to validate the signers.
        #[clap(long)]
        rpc_url: String,
        /// The SP1 circuit version to validate the signers against.
        #[clap(long)]
        version: u32,
        /// The PCR0 value to validate the signers against.
        #[clap(long)]
        pcr0: String,
    }
}

#[tokio::main]
async fn main() {
    let args = Args::parse();

    match args.command {
        Command::Signer { signer, version, pcr0 } => {
            sp1_tee_host::attestations::verify_attestation_for_signer(signer, version, &pcr0)
                .await
                .unwrap();

            println!("Validated signer: {:?}", signer);
        }
        Command::Contract {
            contract,
            pcr0,
            version,
            rpc_url,
        } => {
            let provider =
                alloy::providers::ProviderBuilder::new().on_http(rpc_url.parse().unwrap());

            let instance = sp1_tee_host::contract::TEEVerifier::new(contract, provider);

            let signers = instance.getSigners().call().await.unwrap()._0;

            for signer in signers {
                println!("-----------------------------------");

                match sp1_tee_host::attestations::verify_attestation_for_signer(signer, version, &pcr0).await {
                    Ok(_) => {
                        println!("Validated signer: {:?}", signer);
                    }
                    // It is expected that some signers will not be for the given version.
                    Err(AttestationVerificationError::VersionMismatch(_, _)) => {
                        println!("Signer: {:?}, not for version {}, skipping...", signer, version);
                    }
                    Err(e) => {
                        panic!("Failed to validate signer {}: {:?}", signer, e);
                    }
                }
            }
        }
    }
}
