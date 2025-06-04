use alloy::primitives::Address;
use sp1_sdk::network::proto::network::{
    prover_network_client::ProverNetworkClient, GetTeeWhitelistStatusRequest,
};
use std::time::Duration;
use tonic::transport::{Channel, ClientTlsConfig, Endpoint, Error};

pub struct AuthClient {
    url: String,
}

#[derive(Debug, thiserror::Error)]
pub enum AuthClientError {
    #[error("Failed to connect to the prover network: {0}")]
    FailedToConnectToProverNetwork(#[from] tonic::transport::Error),

    #[error("Failed to get tee whitelist status: {0}")]
    FailedToGetTeeWhitelistStatus(#[from] tonic::Status),
}

/// Configures the endpoint for the gRPC client.
///
/// Sets reasonable settings to handle timeouts and keep-alive.
fn configure_endpoint(addr: &str) -> Result<Endpoint, Error> {
    let mut endpoint = Endpoint::new(addr.to_string())?
        .timeout(Duration::from_secs(60))
        .connect_timeout(Duration::from_secs(15))
        .keep_alive_while_idle(true)
        .http2_keep_alive_interval(Duration::from_secs(15))
        .keep_alive_timeout(Duration::from_secs(15))
        .tcp_keepalive(Some(Duration::from_secs(60)))
        .tcp_nodelay(true);

    // Configure TLS if using HTTPS.
    if addr.starts_with("https://") {
        let tls_config = ClientTlsConfig::new().with_enabled_roots();
        endpoint = endpoint.tls_config(tls_config)?;
    }

    Ok(endpoint)
}

impl AuthClient {
    pub fn new(addr: &str) -> Self {
        Self {
            url: addr.to_string(),
        }
    }

    async fn get_prover_client(&self) -> Result<ProverNetworkClient<Channel>, AuthClientError> {
        let endpoint = configure_endpoint(&self.url)?;
        let channel = endpoint.connect().await?;

        let client = ProverNetworkClient::new(channel);
        Ok(client)
    }

    pub async fn is_whitelisted(&self, address: Address) -> Result<bool, AuthClientError> {
        let request = GetTeeWhitelistStatusRequest {
            address: address.to_vec(),
        };

        let mut client = self.get_prover_client().await?;
        let response = client.get_tee_whitelist_status(request).await?;

        Ok(response.into_inner().is_whitelisted)
    }
}
