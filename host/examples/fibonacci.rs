use sp1_tee_host::api::TEERequest;
use sp1_tee_host::client::Client;
use sp1_sdk::SP1Stdin;
use clap::Parser;

#[derive(Debug, Parser)]
struct Args {
    /// The address to connect to.
    #[clap(short, long, default_value = "http://localhost:3000")]
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
        id: [1; 32],
        program: program.to_vec(),
        stdin: stdin,
    };

    let client = Client::new(&args.address);

    let response = client.execute(request).await;

    println!("Response: {:#?}", response);
}