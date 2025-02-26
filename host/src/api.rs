use sp1_sdk::SP1Stdin;
use serde::{Deserialize, Serialize};

use k256::ecdsa::Signature;

#[derive(Debug, Serialize, Deserialize)]
pub struct TEERequest {
    pub id: [u8; 32],
    pub program: Vec<u8>,
    pub stdin: SP1Stdin,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TEEResponse {
    pub vkey: [u8; 32],
    pub public_values: Vec<u8>,
    pub signature: Signature,
    pub recovery_id: u8,
}