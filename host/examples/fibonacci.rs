use sp1_tee_host::api::{TEERequest, TEEResponse};
use sp1_sdk::SP1Stdin;
use clap::Parser;

#[derive(Debug, Parser)]
struct Args {
    /// The address to connect to.
    #[clap(short, long, default_value = "http://localhost:3000/execute")]
    address: String,

    /// The number of fibonacci numbers to compute.
    #[clap(short, long, default_value = "10")]
    count: u32,
}

#[tokio::main]
async fn main() { 
    let args = Args::parse();
    let program = include_bytes!("../../fixtures/fibonacci.elf");

    let mut stdin = SP1Stdin::new();
    stdin.write(&args.count);

    let request = TEERequest {
        program: program.to_vec(),
        stdin: stdin,
    };

    let client = reqwest::Client::new();
    let response: TEEResponse = client.post(args.address)
        .json(&request)
        .send()
        .await
        .expect("Failed to send request")
        .json()
        .await
        .expect("Failed to parse response");

    println!("Response: {:#?}", response);
}