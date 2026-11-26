use super::{Endpoint, IpcError, frame, list_endpoints};
use interprocess::local_socket::{Stream, prelude::*};

/// Persistent synchronous connection with its own agent revision cursor.
pub struct Client {
    stream: Stream,
}

impl Client {
    /// Connect to an explicitly named app.
    ///
    /// # Errors
    /// Returns the socket connection error if the app is unreachable.
    pub fn connect(endpoint: &Endpoint) -> Result<Self, IpcError> {
        Ok(Self {
            stream: Stream::connect(endpoint.address()?)?,
        })
    }
    /// Connect to the only discoverable app.
    ///
    /// # Errors
    /// Returns a discovery error for zero or multiple candidates, or a connection error.
    pub fn connect_default() -> Result<Self, IpcError> {
        match list_endpoints().as_slice() {
            [] => Err(IpcError::NoServer),
            [only] => Self::connect(only),
            many => Err(IpcError::Ambiguous(
                many.iter().map(|e| e.name().to_owned()).collect(),
            )),
        }
    }
    /// Send one command and read its complete text reply.
    ///
    /// # Errors
    /// Returns an error on invalid framing or I/O failure.
    pub fn request(&mut self, payload: &str) -> Result<String, IpcError> {
        frame::write(&mut self.stream, payload)?;
        frame::read(&mut self.stream)
    }
}
