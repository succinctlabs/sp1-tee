use sp1_tee_host::api::{TEERequest, TEEResponse};
use sp1_sdk::SP1Stdin;
use clap::Parser;

use eventsource_stream::Eventsource;
use futures::stream::{StreamExt, Stream};

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
        id: [1; 32],
        program: program.to_vec(),
        stdin: stdin,
    };

    let client = reqwest::Client::new();
    let response: Vec<TEEResponse> = client.post(args.address)
        .json(&request)
        .send()
        .await
        .expect("Failed to send request")
        .bytes_stream()
        .eventsource()
        .map(|event| {
            match event {
                Ok(event) => serde_json::from_str(&event.data).expect("Failed to parse response"),
                Err(e) => {
                    panic!("Failed to parse response: {}", e);
                }
            }
        })
        .take(1)
        .collect::<Vec<_>>()
        .await;

    assert_eq!(response.len(), 1);

    println!("Response: {:#?}", response);
}