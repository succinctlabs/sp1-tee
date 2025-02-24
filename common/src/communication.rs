use tokio_vsock::{VsockStream as VsockStreamRaw, VsockAddr};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use std::io::{Read, Write};
use serde::{Serialize, de::DeserializeOwned};

pub struct EnclaveStream<In, Out> {
    stream: VsockStreamRaw,
    _marker: std::marker::PhantomData<(In, Out)>,
}

impl<In, Out> EnclaveStream<In, Out> {
    pub fn new(stream: VsockStreamRaw) -> Self {
        Self { stream, _marker: std::marker::PhantomData }
    }

    pub async fn connect(cid: u32, port: u32) -> Result<Self, CommunicationError> {
        let addr = VsockAddr::new(cid, port);
        let stream = VsockStreamRaw::connect(addr).await?;

        Ok(Self { stream, _marker: std::marker::PhantomData })
    }
}

/// Blocking methods.
impl<In, Out> EnclaveStream<In, Out> 
    where 
        In: DeserializeOwned,
        Out: Serialize,
{
    /// Blocking read of a message from the stream.
    pub fn blocking_recv(&mut self) -> Result<In, CommunicationError> {
        // Read a u32 from the stream so we can allocate the correct amount of memory for the message bytes.
        // Interprets this u32 as a be_bytes of the message length.
        let mut message_len_buf = [0; 4];
        self.blocking_read(&mut message_len_buf)?;

        // Convert the message length to a u32.
        let message_len = u32::from_be_bytes(message_len_buf);

        // Allocate a buffer to store the message bytes.
        let mut message_buf = vec![0; message_len as usize];

        // Read the message bytes from the stream.
        self.blocking_read(&mut message_buf)?;

        // Deserialize the message bytes into the desired type.
        Ok(bincode::deserialize(&message_buf)?)
    }

    /// Blocking write of a message to the stream.
    pub fn blocking_send(&mut self, message: Out) -> Result<(), CommunicationError> {
        // Serialize the message into bytes.
        let message_bytes = bincode::serialize(&message)?;

        // Check if the message is too large.
        if message_bytes.len() > u32::MAX as usize {
            return Err(CommunicationError::MessageTooLarge);
        }

        // Convert the message length to a u32 and store it in a buffer.
        let message_len = (message_bytes.len() as u32).to_be_bytes();

        // Write the message length and message bytes to the stream.
        self.blocking_write_all(&message_len)?;
        self.blocking_write_all(&message_bytes)?;

        Ok(())
    }
    
    /// Blocking read into a buffer from the stream.
    /// 
    /// NOTE: Interally, [`VsockStreamRaw`] is set to be non-blocking, 
    /// hence it will throw an error if the buffer is not immediately readable.
    #[inline]
    fn blocking_read(&mut self, buf: &mut [u8]) -> Result<(), CommunicationError> {
        loop {
            match <VsockStreamRaw as Read>::read_exact(&mut self.stream, buf) {
                Ok(_) => return Ok(()),
                Err(e) => {
                    if e.kind() == std::io::ErrorKind::WouldBlock {
                        continue;
                    }
                }
            }
        }
    }
    
    /// Blocking write of a buffer to the stream.
    /// 
    /// NOTE: Interally, [`VsockStreamRaw`] is set to be non-blocking, 
    /// hence it will throw an error if the buffer is not immediately writable.
    #[inline]
    fn blocking_write_all(&mut self, buf: &[u8]) -> Result<(), CommunicationError> {
        loop {
            match <VsockStreamRaw as Write>::write_all(&mut self.stream, buf) {
                Ok(_) => return Ok(()),
                Err(e) => {
                    if e.kind() == std::io::ErrorKind::WouldBlock {
                        continue;
                    }
                }
            }
        }
    }
}

/// Async methods.
impl<In, Out> EnclaveStream<In, Out>
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

        self.async_write_all(&message_len).await?;
        self.async_write_all(&message_bytes).await?;

        Ok(())
    }

    pub async fn recv(&mut self) -> Result<In, CommunicationError> {
        // Read a u32 from the stream so we can allocate the correct amount of memory for the message bytes.
        // Interprets this u32 as a be_bytes of the message length.
        let mut message_len_buf = [0; 4];
        self.async_read(&mut message_len_buf).await?;

        // Convert the message length to a u32.
        let message_len = u32::from_be_bytes(message_len_buf);

        // Allocate a buffer to store the message bytes.
        let mut message_buf = vec![0; message_len as usize];

        // Read the message bytes from the stream.
        self.async_read(&mut message_buf).await?;

        // Deserialize the message bytes into the desired type. 
        Ok(bincode::deserialize(&message_buf)?)
    }

    /// Convenience method for reading from the stream.
    #[inline]
    async fn async_read(&mut self, buf: &mut [u8]) -> Result<(), CommunicationError> {
        <VsockStreamRaw as AsyncReadExt>::read_exact(&mut self.stream, buf).await?;

        Ok(())
    }

    /// Convenience method for writing to the stream.
    #[inline]
    async fn async_write_all(&mut self, buf: &[u8]) -> Result<(), CommunicationError> {
        <VsockStreamRaw as AsyncWriteExt>::write_all(&mut self.stream, buf).await?;

        Ok(())
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