use vsock::VsockStream as VsockStreamRaw;
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

    pub fn connect(cid: u32, port: u32) -> Result<Self, CommunicationError> {
        let stream = VsockStreamRaw::connect_with_cid_port(cid, port)?;

        Ok(Self { stream, _marker: std::marker::PhantomData })
    }
}

impl<In, Out> EnclaveStream<In, Out> 
    where 
        In: DeserializeOwned,
        Out: Serialize,
{
    /// Blocking read of a message from the stream.
    pub fn recv(&mut self) -> Result<In, CommunicationError> {
        // Read a u32 from the stream so we can allocate the correct amount of memory for the message bytes.
        // Interprets this u32 as a be_bytes of the message length.
        let mut message_len_buf = [0; 4];
        self.stream.read_exact(&mut message_len_buf)?;

        let message_len = u32::from_be_bytes(message_len_buf);

        let mut message_buf = vec![0; message_len as usize];

        self.stream.read_exact(&mut message_buf)?;

        Ok(bincode::deserialize(&message_buf)?)
    }

    /// Blocking write of a message to the stream.
    pub fn send(&mut self, message: Out) -> Result<(), CommunicationError> {
        let message_bytes = bincode::serialize(&message)?;

        if message_bytes.len() > u32::MAX as usize {
            return Err(CommunicationError::MessageTooLarge);
        }

        let message_len = (message_bytes.len() as u32).to_be_bytes();

        self.stream.write_all(&message_len)?;
        self.stream.write_all(&message_bytes)?;

        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum CommunicationError {
    Io(#[from] std::io::Error),
    Bincode(#[from] bincode::Error),
    MessageTooLarge,
}

impl std::fmt::Display for CommunicationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}