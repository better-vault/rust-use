use rust_use_agent::ipc::Endpoint;
use std::{
    fs::File,
    io::{self, Read, Seek, Write},
};

/// Keep cursor reads, app requests, and cursor updates ordered across CLI processes.
pub(crate) struct Cursor(File);

impl Cursor {
    pub(crate) fn open(endpoint: &Endpoint) -> io::Result<Self> {
        let path = endpoint.revision_path();
        #[cfg(windows)]
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let mut options = File::options();
        #[cfg(windows)]
        std::os::windows::fs::OpenOptionsExt::share_mode(&mut options, 0);
        let file = options
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(path)?;
        #[cfg(unix)]
        rustix::fs::flock(&file, rustix::fs::FlockOperation::LockExclusive)?;
        Ok(Self(file))
    }

    pub(crate) fn revision(&mut self) -> io::Result<u64> {
        let mut saved = String::new();
        self.0.read_to_string(&mut saved)?;
        Ok(saved.trim().parse().unwrap_or(0))
    }

    pub(crate) fn observe(&mut self, reply: &str) -> io::Result<()> {
        if let Some(rev) = reply
            .lines()
            .find_map(|line| line.strip_prefix("rev ")?.parse::<u64>().ok())
        {
            self.0.rewind()?;
            self.0.set_len(0)?;
            write!(self.0, "{rev}")?;
        }
        Ok(())
    }
}
