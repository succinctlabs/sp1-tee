use sp1_tee_common::{EnclaveRequest, EnclaveResponse, VsockStream, CommunicationError};

mod attestations;
pub use attestations::{SaveAttestationArgs, SaveAttestationError, save_attestation};

/// A wrapper around [`VsockStream`] that allows for sending messages to the enclave.
/// 
/// This stream is bi-directional, and it will automatically close the connection when the stream is dropped.
pub struct HostStream {
    stream: VsockStream<EnclaveResponse, EnclaveRequest>,
}

impl HostStream {
    pub async fn new(cid: u32, port: u32) -> Result<Self, CommunicationError> {
        let stream = VsockStream::connect(cid, port).await?;

        Ok(Self { stream })
    }

    pub async fn send(&mut self, request: EnclaveRequest) -> Result<(), CommunicationError> {
        self.stream.send(request).await
    }

    pub async fn recv(&mut self) -> Result<EnclaveResponse, CommunicationError> {
        self.stream.recv().await
    }
}

impl Drop for HostStream {
    fn drop(&mut self) {
        // Make sure the enclave drops the connection when the host drops the stream.
        //
        // Ignore any errors as they stream may already be closed.
        let _ = self.stream.blocking_send(EnclaveRequest::CloseSession);
    }
}
