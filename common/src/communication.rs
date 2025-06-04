use serde::{de::DeserializeOwned, Serialize};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio_vsock::{VsockAddr, VsockStream as VsockStreamRaw};

pub struct VsockStream<In, Out> {
    stream: VsockStreamRaw,
    _marker: std::marker::PhantomData<(In, Out)>,
}

impl<In, Out> VsockStream<In, Out> {
    pub fn new(stream: VsockStreamRaw) -> Self {
        Self {
            stream,
            _marker: std::marker::PhantomData,
        }
    }

    pub async fn connect(cid: u32, port: u32) -> Result<Self, CommunicationError> {
        let addr = VsockAddr::new(cid, port);
        let stream = VsockStreamRaw::connect(addr).await?;

        Ok(Self {
            stream,
            _marker: std::marker::PhantomData,
        })
    }
}

/// Async methods.
impl<In, Out> VsockStream<In, Out>
where
    In: DeserializeOwned,
    Out: Serialize,
{
    pub async fn send(&mut self, message: Out) -> Result<(), CommunicationError> {
        let message_bytes = bincode::serialize(&message)?;

        if message_bytes.len() > u32::MAX as usize {
            return Err(CommunicationError::MessageTooLarge);
        }

        let message_len = (message_bytes.len() as u32).to_be_bytes();

        self.stream.write_all(&message_len).await?;
        self.stream.write_all(&message_bytes).await?;

        Ok(())
    }

    pub async fn recv(&mut self) -> Result<In, CommunicationError> {
        // Read a u32 from the stream so we can allocate the correct amount of memory for the message bytes.
        // Interprets this u32 as a be_bytes of the message length.
        let mut message_len_buf = [0; 4];
        self.stream.read_exact(&mut message_len_buf).await?;

        // Convert the message length to a u32.
        let message_len = u32::from_be_bytes(message_len_buf);

        // Allocate a buffer to store the message bytes.
        let mut message_buf = vec![0; message_len as usize];

        // Read the message bytes from the stream.
        self.stream.read_exact(&mut message_buf).await?;

        // Deserialize the message bytes into the desired type.
        Ok(bincode::deserialize(&message_buf)?)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum CommunicationError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Bincode error: {0}")]
    Bincode(#[from] bincode::Error),

    #[error("Enclave message too large")]
    MessageTooLarge,
}
