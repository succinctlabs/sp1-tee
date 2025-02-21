use k256::ecdsa::SigningKey;

pub struct Server {
    signing_key: SigningKey,
    enc_key_arn: String
}