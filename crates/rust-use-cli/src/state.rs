use rust_use_agent::ipc::Endpoint;
use std::{
    fs::{self, File},
    io,
    path::PathBuf,
};

/// Keep cursor reads, app requests, and cursor updates ordered across CLI processes.
pub(crate) struct Cursor {
    path: PathBuf,
    _lock: File,
}

pub(crate) struct SavedRevision {
    pub(crate) session: Option<u64>,
    pub(crate) revision: u64,
}

impl SavedRevision {
    fn parse(saved: &str) -> Option<Self> {
        let saved = saved.trim();
        let (session, revision) = match saved.split_once(':') {
            Some((session, revision)) => (Some(parse_session(session)?), revision),
            None => (None, saved),
        };
        Some(Self {
            session,
            revision: revision.parse().ok()?,
        })
    }
}

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
        // Lock a separate file so the lock stays valid when the cursor is replaced.
        let lock = options
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(path.with_extension("rev.lock"))?;
        #[cfg(unix)]
        rustix::fs::flock(&lock, rustix::fs::FlockOperation::LockExclusive)?;
        Ok(Self { path, _lock: lock })
    }

    pub(crate) fn read(&self) -> io::Result<Option<SavedRevision>> {
        match fs::read_to_string(&self.path) {
            Ok(saved) => Ok(SavedRevision::parse(&saved)),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e),
        }
    }

    pub(crate) fn observe(&self, session: u64, reply: &str) -> io::Result<()> {
        if let Some(rev) = reply
            .lines()
            .find_map(|line| line.strip_prefix("rev ")?.parse::<u64>().ok())
        {
            let temp = self.path.with_extension("rev.tmp");
            fs::write(&temp, format!("{session:016x}:{rev}"))?;
            fs::rename(temp, &self.path)?;
        }
        Ok(())
    }
}

fn parse_session(session: &str) -> Option<u64> {
    if session.len() != 16 || !session.bytes().all(|digit| digit.is_ascii_hexdigit()) {
        return None;
    }
    u64::from_str_radix(session, 16).ok().filter(|&id| id != 0)
}

pub(crate) fn parse_hello(reply: &str) -> io::Result<(u64, u64)> {
    let fields: Vec<_> = reply.split_ascii_whitespace().collect();
    if let ["rust-use", _, "session", session, "rev", revision] = fields.as_slice()
        && reply.lines().count() == 1
        && let (Some(session), Ok(revision)) = (parse_session(session), revision.parse())
    {
        return Ok((session, revision));
    }
    Err(io::Error::new(
        io::ErrorKind::InvalidData,
        "invalid hello reply from app",
    ))
}

#[cfg(test)]
mod tests {
    use super::parse_hello;
    use rust_use_agent::UiAgentExt;
    use rust_use_core::Ui;

    #[test]
    fn hello_parses() -> std::io::Result<()> {
        let ui = Ui::new();
        let reply = ui.agent().handle("hello");
        assert_eq!(
            reply,
            format!(
                "rust-use {} session {:016x} rev 0",
                env!("CARGO_PKG_VERSION"),
                ui.session()
            )
        );
        assert_eq!(parse_hello(&reply)?, (ui.session(), ui.revision()));
        assert!(parse_hello("err unknown command `hello`").is_err());
        Ok(())
    }
}
