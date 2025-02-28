use sp1_tee_common::{CommunicationError, EnclaveRequest, EnclaveResponse, VsockStream};

/// A wrapper around [`VsockStream`] that allows for sending messages to the enclave.
///
/// This stream is bi-directional, and it will automatically close the connection when the stream is dropped.
pub struct HostStream {
    stream: Option<VsockStream<EnclaveResponse, EnclaveRequest>>,
}

impl HostStream {
    /// Connects to the enclave on the given CID and port.
    pub async fn new(cid: u32, port: u16) -> Result<Self, CommunicationError> {
        let stream = VsockStream::connect(cid, port as u32).await?;

        Ok(Self { stream: Some(stream) })
    }

    /// Sends a request to the enclave.
    pub async fn send(&mut self, request: EnclaveRequest) -> Result<(), CommunicationError> {
        self.stream.as_mut().expect("Stream should be initialized, this is a bug").send(request).await
    }

    /// Receives a response from the enclave.
    pub async fn recv(&mut self) -> Result<EnclaveResponse, CommunicationError> {
        self.stream.as_mut().expect("Stream should be initialized, this is a bug").recv().await
    }
}

impl Drop for HostStream {
    fn drop(&mut self) {
        let mut stream = self.stream.take().expect("Stream should be initialized, this is a bug");

        tokio::task::spawn(async move {
            if let Err(e) = stream.send(EnclaveRequest::CloseSession).await {
                tracing::error!("Failed to send close session request: {}", e);
            }
        });
    }
}