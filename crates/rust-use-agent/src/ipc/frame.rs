use super::IpcError;
use std::io::{self, Read, Write};

pub(super) const MAX: usize = 16 << 20;

pub(super) fn read(stream: &mut impl Read) -> Result<String, IpcError> {
    let mut header = [0; 4];
    stream.read_exact(&mut header)?;
    let len = u32::from_le_bytes(header) as usize;
    if len > MAX {
        return Err(IpcError::FrameTooLarge(len));
    }
    let mut bytes = vec![0; len];
    stream.read_exact(&mut bytes)?;
    String::from_utf8(bytes).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e).into())
}

pub(super) fn write(stream: &mut impl Write, text: &str) -> Result<(), IpcError> {
    if text.len() > MAX {
        return Err(IpcError::FrameTooLarge(text.len()));
    }
    let len = u32::try_from(text.len()).map_err(|_| IpcError::FrameTooLarge(text.len()))?;
    stream.write_all(&len.to_le_bytes())?;
    stream.write_all(text.as_bytes())?;
    Ok(())
}
